//! Batched polygon booleans; no GUI or document mutation.
use super::*;
use geo::orient::Direction;
use geo::{BoundingRect, Orient, unary_union};
use rstar::{AABB, RTree, RTreeObject};
use std::collections::HashMap;

struct Obstacle {
    index: usize,
    bounds: Bounds,
}
impl RTreeObject for Obstacle {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.bounds.min.x as f64, self.bounds.min.y as f64],
            [self.bounds.max.x as f64, self.bounds.max.y as f64],
        )
    }
}
/// Include every clearance matrix entry before pruning. Uninterpretable rules
/// use the full coordinate range, so pruning cannot conceal an unsupported rule.
fn clearance_bound(board: &Board) -> i64 {
    let mut gap = 0;
    for r in board.rules.iter().filter(|r| crate::pcb_rules::enabled(r)) {
        let kind = crate::pcb_rules::text(r, "RULEKIND");
        if kind == "PolygonConnect" {
            for prefix in ["", "THPAD.", "SMDPAD.", "VIA."] {
                if crate::pcb_rules::text(r, &format!("{prefix}CONNECTSTYLE")) == "Relief" {
                    let Ok(value) = crate::pcb_rules::dimension(r, &format!("{prefix}AIRGAPWIDTH"))
                    else {
                        return 200_000_000_000;
                    };
                    gap = gap.max(value);
                }
            }
            continue;
        }
        if !matches!(kind, "Clearance" | "BoardOutlineClearance") {
            continue;
        }
        let Ok(value) = crate::pcb_rules::dimension(r, "GAP") else {
            return 200_000_000_000;
        };
        gap = gap.max(value);
        for entry in crate::pcb_rules::text(r, "OBJECTCLEARANCES")
            .split(';')
            .filter(|s| !s.is_empty())
        {
            let Some((_, value)) = entry.split_once(':') else {
                return 200_000_000_000;
            };
            let Some(value) = value
                .parse::<i64>()
                .ok()
                .filter(|n| (0..=39_370_078_740).contains(n))
            else {
                return 200_000_000_000;
            };
            gap = gap.max((value as f64 * 2.54).round() as i64);
        }
    }
    gap
}

/// Rebuild all pours in dependency order, leaving the caller untouched on error.
pub fn repour(board: &Board) -> Result<Board, String> {
    repour_controlled(board, &PourControl::default())
}

pub(super) fn repour_controlled(board: &Board, control: &PourControl) -> Result<Board, String> {
    control.check()?;
    let mut result = board.clone();
    for p in &board.polygon_pours {
        p.validate()
            .map_err(|e| format!("Polygon {}: {e}", p.index))?;
        if !p.shelved && p.fill_mode == FillMode::Hatched {
            return Err(format!(
                "Polygon {}: hatched repour is not implemented; existing copper was preserved",
                p.index
            ));
        }
    }
    let managed: BTreeSet<_> = board.polygon_pours.iter().map(|p| p.index).collect();
    // Generated geometry is never treated as independent obstacles on the next repour.
    result
        .items
        .retain(|i| i.polygon.is_none_or(|id| !managed.contains(&id)));
    // A polygon cutout is a real native Region, not just private metadata or a
    // visual hole. It affects copper pours on its layer, as a standalone cutout
    // does in the PCB editor; the owner tag keeps group transforms predictable.
    for p in &board.polygon_pours {
        for (n, hole) in p.holes.iter().enumerate() {
            let mut item = Item::new(
                p.layer,
                None,
                Shape::Region {
                    contour: hole.clone(),
                    holes: vec![],
                    cutout: true,
                },
            );
            item.id = Uuid::new_v5(&p.id, format!("cutout:{n}").as_bytes());
            item.polygon = Some(p.index);
            item.properties
                .insert("PCBCRAFT_POLYGON_CUTOUT_OWNER".into(), p.index.to_string());
            result.items.push(item);
        }
    }
    let base = result.items.clone();
    let outline = board.outline_points(256);
    let board_clip = if outline.len() >= 3 {
        Some(mp(ring_polygon(&outline, &[])))
    } else {
        None
    };
    let mut ordered: Vec<_> = board.polygon_pours.iter().collect();
    ordered.sort_by_key(|p| (p.priority, p.index));
    let mut poured: Vec<(&Polygon, MultiPolygon<f64>)> = Vec::new();
    let mut geometry = HashMap::new();
    let mut buffered = HashMap::new();
    let item_bounds: Vec<_> = base.iter().map(Item::bounds).collect();
    let obstacles_index = RTree::bulk_load(
        item_bounds
            .iter()
            .enumerate()
            .map(|(index, &bounds)| Obstacle { index, bounds })
            .collect(),
    );
    let rule_gap = clearance_bound(board);
    control
        .total
        .store(ordered.len(), std::sync::atomic::Ordering::Relaxed);
    for (step, p) in ordered.into_iter().enumerate() {
        control.check()?;
        control
            .completed
            .store(step, std::sync::atomic::Ordering::Relaxed);
        if p.shelved || p.fill_mode == FillMode::None {
            continue;
        }
        let mut material = mp(ring_polygon(&p.points(), &p.holes));
        let mut probe = Item::new(
            p.layer,
            p.net,
            Shape::Region {
                contour: p.points(),
                holes: p.holes.clone(),
                cutout: false,
            },
        );
        probe.polygon = Some(p.index);
        let electrical = copper_layer(p.layer);
        if electrical && let Some(clip) = &board_clip {
            let gap = crate::pcb_rules::resolve(board, "BoardOutlineClearance", &probe, None)?
                .map(|(_, r)| crate::pcb_rules::gap_objects(r, "Poly", "OutlineEdge"))
                .transpose()?
                .unwrap_or(0);
            material = material.intersection(&clip.buffer(-(gap as f64) / 1e6));
        }
        let bounds = material.bounding_rect();
        let mut anchors = Vec::new();
        let mut obstacles = Vec::new();
        let expansion = rule_gap.max(p.clearance).max(p.thermal_gap);
        let mut candidates: Vec<_> = bounds
            .map(|r| {
                let envelope = AABB::from_corners(
                    [
                        (r.min().x * 1e6).floor() - expansion as f64,
                        (r.min().y * 1e6).floor() - expansion as f64,
                    ],
                    [
                        (r.max().x * 1e6).ceil() + expansion as f64,
                        (r.max().y * 1e6).ceil() + expansion as f64,
                    ],
                );
                obstacles_index
                    .locate_in_envelope_intersecting(&envelope)
                    .map(|o| o.index)
                    .collect()
            })
            .unwrap_or_default();
        candidates.sort_unstable();
        // One union and subtraction per polygon, rather than repeatedly copying
        // a growing contour after each track, pad, via and overlapping polygon.
        for index in candidates {
            let item = &base[index];
            control.check()?;
            if !(electrical
                || item.layer == p.layer
                    && matches!(item.shape, Shape::Region { cutout: true, .. }))
            {
                continue;
            }
            if item
                .properties
                .get("PCBCRAFT_LEGACY_REGION_DUPLICATE")
                .is_some_and(|s| s == "TRUE")
                || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
            {
                continue;
            }
            let board_cutout = matches!(&item.shape, Shape::Region { cutout: true, .. })
                && item
                    .properties
                    .get("ISBOARDCUTOUT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"));
            let keepout = item.keepout || item.layer == KEEP_OUT;
            if !board_cutout
                && !item.is_drill_only()
                && !on_layer(board, item, p.layer)
                && item.layer != KEEP_OUT
            {
                continue;
            }
            if keepout && !matches!(item.shape, Shape::Region { cutout: true, .. }) {
                let mask = item
                    .properties
                    .get("KEEPOUTRESTRICTIONS")
                    .map(|s| {
                        s.parse::<u8>()
                            .ok()
                            .filter(|n| *n <= 31)
                            .ok_or("Invalid keepout restriction mask")
                    })
                    .transpose()?
                    .unwrap_or(31);
                if mask & 4 == 0 {
                    continue;
                }
            }
            let is_cutout = matches!(item.shape, Shape::Region { cutout: true, .. });
            let same = p.net.is_some()
                && board.item_net(item) == p.net
                && !keepout
                && !item.is_drill_only()
                && !is_cutout;
            let settings = if same && matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                Some(
                    crate::pcb_rules::polygon_connection(board, item, &probe)?.unwrap_or(
                        crate::pcb_rules::PolygonConnection {
                            style: p.connection,
                            gap: p.thermal_gap,
                            width: p.spoke_width,
                            conductors: 4,
                            angle: p.spoke_angle,
                        },
                    ),
                )
            } else {
                None
            };
            let over = same
                && (p.same_net == SameNet::All
                    || (p.same_net == SameNet::PolygonsOnly && item.polygon.is_some()));
            let gap = if board_cutout {
                crate::pcb_rules::cutout_clearance(board, &probe, item)?.unwrap_or(0)
            } else if is_cutout {
                0
            } else if let Some(settings) = settings.filter(|s| s.style != Connection::None) {
                settings.gap
            } else if over {
                0
            } else {
                crate::pcb_rules::obstacle_clearance(board, &probe, item)?.unwrap_or(p.clearance)
            };
            let b = item_bounds[index];
            if bounds.is_none_or(|r| {
                b.max.x as f64 / 1e6 + gap as f64 / 1e6 < r.min().x
                    || b.min.x as f64 / 1e6 - gap as f64 / 1e6 > r.max().x
                    || b.max.y as f64 / 1e6 + gap as f64 / 1e6 < r.min().y
                    || b.min.y as f64 / 1e6 - gap as f64 / 1e6 > r.max().y
            }) {
                continue;
            }
            let geom = geometry
                .entry((p.layer, index))
                .or_insert_with(|| copper(item, p.layer));
            match settings.map(|s| s.style) {
                Some(Connection::Direct) => anchors.push(geom.clone()),
                Some(Connection::Thermal) => {
                    anchors.push(geom.clone());
                    obstacles.push(thermals(item, settings.unwrap(), geom));
                }
                _ if over && settings.is_none() => anchors.push(geom.clone()),
                _ => obstacles.push(if gap == 0 {
                    geom.clone()
                } else {
                    buffered
                        .entry((p.layer, index, gap))
                        .or_insert_with(|| geom.buffer(gap as f64 / 1e6))
                        .clone()
                }),
            }
        }
        for (other, geom) in poured
            .iter()
            .filter(|(other, _)| electrical && other.layer == p.layer)
        {
            control.check()?;
            if p.net.is_some() && other.net == p.net && p.same_net != SameNet::None {
                anchors.push(geom.clone());
            } else {
                let mut other_probe = Item::new(
                    other.layer,
                    other.net,
                    Shape::Region {
                        contour: other.points(),
                        holes: other.holes.clone(),
                        cutout: false,
                    },
                );
                other_probe.polygon = Some(other.index);
                let clearance = crate::pcb_rules::obstacle_clearance(board, &probe, &other_probe)?
                    .unwrap_or(p.clearance.max(other.clearance));
                obstacles.push(geom.buffer(clearance as f64 / 1e6));
            }
        }
        control.check()?;
        if !obstacles.is_empty() {
            // unary_union uses a winding fill rule. Normalize imported clockwise
            // and counterclockwise outlines, including all their interior holes.
            let obstacles: Vec<_> = obstacles
                .iter()
                .map(|g| g.orient(Direction::Default))
                .collect();
            material = material.difference(&unary_union(&obstacles));
        }
        control.check()?;
        if p.neck_width > 0 {
            let radius = p.neck_width as f64 / 2e6;
            material = material.intersection(&material.buffer(-radius).buffer(radius));
        }
        material.0.retain(|poly| {
            poly.unsigned_area() >= p.min_island_area_mm2
                && (!electrical || !p.remove_islands || anchors.iter().any(|a| poly.intersects(a)))
        });
        for (n, poly) in material.0.iter().enumerate() {
            let contour = from_line(poly.exterior());
            if contour.len() < 3 {
                continue;
            }
            let mut item = Item::new(
                p.layer,
                p.net,
                Shape::Region {
                    contour,
                    holes: poly
                        .interiors()
                        .iter()
                        .map(from_line)
                        .filter(|v| v.len() >= 3)
                        .collect(),
                    cutout: false,
                },
            );
            item.id = Uuid::new_v5(&p.id, format!("copper:{n}").as_bytes());
            item.polygon = Some(p.index);
            result.items.push(item);
            if result.items.len() > MAX_ITEMS {
                return Err("Pour produces too many primitives".into());
            }
        }
        poured.push((p, material));
    }
    control.check()?;
    control.completed.store(
        control.total.load(std::sync::atomic::Ordering::Relaxed),
        std::sync::atomic::Ordering::Relaxed,
    );
    result.pours_stale = false;
    result.validate()?;
    Ok(result)
}
