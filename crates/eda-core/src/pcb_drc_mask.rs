//! Manufacturing checks use the same apertures as mask display/export.
use super::*;
use geo::{BooleanOps, BoundingRect, MultiPolygon};
use rstar::{AABB, Envelope, RTree, RTreeObject};
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    index: usize,
    bounds: geo::Rect<f64>,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        envelope(self.bounds, 0.)
    }
}
fn envelope(bounds: geo::Rect<f64>, gap: f64) -> AABB<[f64; 2]> {
    AABB::from_corners(
        [bounds.min().x - gap, bounds.min().y - gap],
        [bounds.max().x + gap, bounds.max().y + gap],
    )
}

pub(super) fn check_masks_with_rules(board: &Board, rules: &Rules<'_>, out: &mut Report) {
    let active = |kind| {
        board
            .rules
            .iter()
            .any(|r| enabled(r) && text(r, "RULEKIND") == kind)
    };
    let sliver = active("MinimumSolderMaskSliver");
    let silk = active("SilkToSolderMaskClearance");
    if !sliver && !silk {
        return;
    }
    let mut pairs = 0usize;
    for bottom in [false, true] {
        let layer = if bottom { BOTTOM } else { TOP };
        let mask_layer = if bottom { 38 } else { 37 };
        let silk_layer = if bottom { BOTTOM_OVERLAY } else { TOP_OVERLAY };
        let mut openings = Vec::new();
        for item in board.items.iter().filter(|i| !i.keepout) {
            if out.stopped() {
                return;
            }
            let geometry = if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                match item.mask_outline(board, MaskKind::Solder, bottom) {
                    Ok(points) if points.len() >= 3 => MultiPolygon(vec![polygon(&points)]),
                    Ok(_) => continue,
                    Err(e) => {
                        out.diagnostics.insert(e);
                        continue;
                    }
                }
            } else if item.layer == mask_layer {
                if matches!(item.shape, Shape::Text { .. }) {
                    out.diagnostics
                        .insert("Mask text outlines are not checked".into());
                    continue;
                }
                pcb_polygon::copper(item, mask_layer)
            } else {
                continue;
            };
            if !geometry.0.is_empty() {
                openings.push((item, geometry));
            }
        }
        if sliver {
            let mut max_gap = 0.0_f64;
            for r in board
                .rules
                .iter()
                .filter(|r| enabled(r) && text(r, "RULEKIND") == "MinimumSolderMaskSliver")
            {
                match dimension(r, "MINSOLDERMASKWIDTH") {
                    Ok(n) => max_gap = max_gap.max(n as f64 / 1e6),
                    Err(e) => {
                        out.diagnostics.insert(e);
                    }
                }
            }
            // Joined apertures form one opening, including manual bridges between pads.
            // Checking individual pad rectangles would report fictitious strips here.
            // Only boolean-union intersecting aperture groups. Re-unioning every
            // disjoint pad into one global polygon makes online DRC quadratic in
            // all contour vertices, even though most apertures never meet.
            let mut groups: Vec<(MultiPolygon<f64>, Vec<&Item>, geo::Rect<f64>)> = Vec::new();
            let mut group_index = RTree::<Entry>::new();
            for (item, shape) in &openings {
                if out.stopped() {
                    return;
                }
                let mut shape = shape.clone();
                let mut owners = vec![*item];
                let mut bounds = shape.bounding_rect().unwrap();
                loop {
                    if out.stopped() {
                        return;
                    }
                    let mut candidates: Vec<_> = group_index
                        .locate_in_envelope_intersecting(&envelope(bounds, 0.))
                        .map(|entry| entry.index)
                        .collect();
                    candidates.sort_unstable();
                    if let Some(n) = candidates
                        .into_iter()
                        .find(|&n| shape.intersects(&groups[n].0))
                    {
                        group_index.remove(&Entry {
                            index: n,
                            bounds: groups[n].2,
                        });
                        let (other, other_owners, _) = groups.swap_remove(n);
                        if n < groups.len() {
                            group_index.remove(&Entry {
                                index: groups.len(),
                                bounds: groups[n].2,
                            });
                            group_index.insert(Entry {
                                index: n,
                                bounds: groups[n].2,
                            });
                        }
                        shape = shape.union(&other);
                        owners.extend(other_owners);
                        bounds = shape.bounding_rect().unwrap();
                    } else {
                        break;
                    }
                }
                group_index.insert(Entry {
                    index: groups.len(),
                    bounds,
                });
                groups.push((shape, owners, bounds));
            }
            groups.sort_by(|a, b| a.2.min().x.total_cmp(&b.2.min().x));
            let boxes: Vec<_> = groups.iter().map(|(_, _, b)| *b).collect();
            let owners: Vec<_> = groups.iter().map(|(_, o, _)| o).collect();
            for (a, (shape, _, _)) in groups.iter().enumerate() {
                for (b, (other, _, _)) in groups.iter().enumerate().skip(a + 1) {
                    if out.stopped() {
                        return;
                    }
                    if boxes[b].min().x > boxes[a].max().x + max_gap {
                        break;
                    }
                    if boxes[b].max().y + max_gap < boxes[a].min().y
                        || boxes[b].min().y > boxes[a].max().y + max_gap
                    {
                        continue;
                    }
                    pairs += 1;
                    if pairs > 2_000_000 {
                        out.truncated = true;
                        out.diagnostics
                            .insert("Mask pair budget reached; DRC is incomplete".into());
                        return;
                    }
                    let distance = std::cell::OnceCell::new();
                    let mut reported = BTreeSet::new();
                    for i in owners[a] {
                        for j in owners[b] {
                            if out.stopped() {
                                return;
                            }
                            match rules.resolve("MinimumSolderMaskSliver", i, Some(j)) {
                                Ok(Some((n, r))) => match dimension(r, "MINSOLDERMASKWIDTH") {
                                    Ok(required)
                                        if required > 2
                                            && *distance.get_or_init(|| {
                                                (Euclidean.distance(shape, other) * 1e6).round()
                                                    as i64
                                            }) + 2
                                                < required
                                            && reported.insert(n) =>
                                    {
                                        out.add(
                                            n,
                                            r,
                                            format!(
                                                "Solder mask sliver {:.6} mm < {:.6} mm",
                                                *distance.get().unwrap() as f64 / 1e6,
                                                required as f64 / 1e6
                                            ),
                                            &[i, j],
                                            mask_layer,
                                        )
                                    }
                                    Err(e) => {
                                        out.diagnostics.insert(e);
                                    }
                                    _ => {}
                                },
                                Err(e) => {
                                    out.diagnostics.insert(e);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if silk {
            let boxes: Vec<_> = openings.iter().map(|(_, g)| g.bounding_rect()).collect();
            let index = RTree::bulk_load(
                boxes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, b)| b.map(|bounds| Entry { index, bounds }))
                    .collect(),
            );
            let exposed: Vec<std::cell::OnceCell<MultiPolygon<f64>>> = openings
                .iter()
                .map(|_| std::cell::OnceCell::new())
                .collect();
            let max_gap = board
                .rules
                .iter()
                .filter(|r| enabled(r) && text(r, "RULEKIND") == "SilkToSolderMaskClearance")
                .map(|r| dimension(r, "MINSILKSCREENTOMASKGAP"))
                .collect::<Result<Vec<_>, _>>();
            let scoped_items: Vec<_> = openings
                .iter()
                .map(|(i, _)| *i)
                .chain(
                    board
                        .items
                        .iter()
                        .filter(|i| i.layer == silk_layer && !i.keepout),
                )
                .collect();
            let prune = max_gap.is_ok()
                && boxes.iter().all(Option::is_some)
                && rules.known_scopes("SilkToSolderMaskClearance", &scoped_items);
            let max_gap = max_gap
                .unwrap_or_default()
                .into_iter()
                .max()
                .unwrap_or(0)
                .max(0) as f64
                / 1e6;
            for item in board
                .items
                .iter()
                .filter(|i| i.layer == silk_layer && !i.keepout)
            {
                if out.stopped() {
                    return;
                }
                if matches!(item.shape, Shape::Text { .. }) {
                    out.diagnostics
                        .insert("Silk text glyph outlines are not checked".into());
                    continue;
                }
                let geometry = pcb_polygon::copper(item, silk_layer);
                if geometry.0.is_empty() {
                    continue;
                }
                let bounds = geometry.bounding_rect();
                let mut candidates: Vec<_> = if prune && let Some(bounds) = bounds {
                    index
                        .locate_in_envelope_intersecting(&envelope(bounds, max_gap))
                        .map(|entry| entry.index)
                        .collect()
                } else {
                    (0..openings.len()).collect()
                };
                // Preserve source order for rule priority, pair budgets and report limits.
                candidates.sort_unstable();
                for b in candidates {
                    let (other, opening) = &openings[b];
                    if out.stopped() {
                        return;
                    }
                    pairs += 1;
                    if pairs > 2_000_000 {
                        out.truncated = true;
                        out.diagnostics
                            .insert("Silk/mask pair budget reached; DRC is incomplete".into());
                        return;
                    }
                    match rules.resolve("SilkToSolderMaskClearance", item, Some(other)) {
                        Ok(Some((n, r))) => match dimension(r, "MINSILKSCREENTOMASKGAP") {
                            Ok(required) => {
                                if required <= 2 {
                                    continue;
                                }
                                // An exposed pad is a subset of its opening. Reject distant
                                // pairs before boolean clipping and reuse that clip per pad.
                                if let (Some(a), Some(b)) = (bounds, boxes[b]) {
                                    let gap = required as f64 / 1e6;
                                    if !envelope(a, gap).intersects(&envelope(b, 0.)) {
                                        continue;
                                    }
                                }
                                let target = if text(r, "CLEARANCETOEXPOSEDCOPPER") == "TRUE" {
                                    // The exposed-copper mode targets component pads.
                                    if !matches!(other.shape, Shape::Pad { .. }) {
                                        continue;
                                    }
                                    exposed[b].get_or_init(|| {
                                        opening.intersection(&pcb_polygon::copper(other, layer))
                                    })
                                } else {
                                    opening
                                };
                                if target.0.is_empty() {
                                    continue;
                                }
                                if let (Some(a), Some(b)) = (bounds, target.bounding_rect()) {
                                    let gap = required as f64 / 1e6;
                                    if a.min().x > b.max().x + gap
                                        || a.max().x + gap < b.min().x
                                        || a.min().y > b.max().y + gap
                                        || a.max().y + gap < b.min().y
                                    {
                                        continue;
                                    }
                                }
                                let distance =
                                    (Euclidean.distance(&geometry, target) * 1e6).round() as i64;
                                if distance + 2 < required {
                                    out.add(
                                        n,
                                        r,
                                        format!(
                                            "Silk to solder mask clearance {:.6} mm < {:.6} mm",
                                            distance as f64 / 1e6,
                                            required as f64 / 1e6
                                        ),
                                        &[item, other],
                                        silk_layer,
                                    );
                                }
                            }
                            Err(e) => {
                                out.diagnostics.insert(e);
                            }
                        },
                        Err(e) => {
                            out.diagnostics.insert(e);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silk_spatial_candidates_match_exhaustive_apertures_and_exposed_copper() {
        let mut b = board();
        b.rules.remove(0);
        let pad = b.items[0].clone();
        b.items.clear();
        for n in 0..24 {
            let mut item = pad.clone();
            item.id = Uuid::new_v4();
            if let Shape::Pad {
                position, rotation, ..
            } = &mut item.shape
            {
                *position = Point {
                    x: (n * 7919 % 19) * 150_000,
                    y: (n * 1543 % 17) * 150_000,
                };
                *rotation = n as f64 * 13.;
            }
            b.items.push(item);
        }
        for n in 0..27 {
            b.items.push(Item::new(
                TOP_OVERLAY,
                None,
                Shape::Track {
                    start: Point {
                        x: (n * 199 % 17) * 200_000,
                        y: (n * 83 % 19) * 100_000,
                    },
                    end: Point {
                        x: (n * 199 % 17) * 200_000,
                        y: (n * 83 % 19) * 100_000 + 700_000,
                    },
                    width: 50_000,
                },
            ));
        }
        for exposed in [false, true] {
            b.rules[0].insert(
                "CLEARANCETOEXPOSEDCOPPER".into(),
                if exposed { "TRUE" } else { "FALSE" }.into(),
            );
            let mut expected = Report::default();
            for item in b.items.iter().filter(|i| i.layer == TOP_OVERLAY) {
                let geometry = pcb_polygon::copper(item, TOP_OVERLAY);
                for other in b
                    .items
                    .iter()
                    .filter(|i| matches!(i.shape, Shape::Pad { .. }))
                {
                    let opening = MultiPolygon(vec![polygon(
                        &other.mask_outline(&b, MaskKind::Solder, false).unwrap(),
                    )]);
                    let target = if exposed {
                        opening.intersection(&pcb_polygon::copper(other, TOP))
                    } else {
                        opening
                    };
                    let (n, r) = resolve(&b, "SilkToSolderMaskClearance", item, Some(other))
                        .unwrap()
                        .unwrap();
                    let gap = dimension(r, "MINSILKSCREENTOMASKGAP").unwrap();
                    let distance = (Euclidean.distance(&geometry, &target) * 1e6).round() as i64;
                    if distance + 2 < gap {
                        expected.add(
                            n,
                            r,
                            format!(
                                "Silk to solder mask clearance {:.6} mm < {:.6} mm",
                                distance as f64 / 1e6,
                                gap as f64 / 1e6
                            ),
                            &[item, other],
                            TOP_OVERLAY,
                        );
                    }
                }
            }
            let actual = check(&b);
            let keys = |r: &Report| {
                r.violations
                    .iter()
                    .map(|v| (v.rule, v.items.clone(), v.message.clone(), v.layer))
                    .collect::<Vec<_>>()
            };
            assert!(!expected.violations.is_empty());
            assert_eq!(keys(&actual), keys(&expected));
            assert_eq!(actual.diagnostics, expected.diagnostics);
            assert!(!actual.truncated);
        }
    }
    #[test]
    fn distant_silk_pairs_do_not_exhaust_budget_and_unknown_scopes_remain_visible() {
        let mut b = board();
        b.rules.remove(0);
        let pad = b.items[0].clone();
        b.items.clear();
        for n in 0..2000 {
            let mut item = pad.clone();
            item.id = Uuid::new_v4();
            if let Shape::Pad { position, .. } = &mut item.shape {
                position.x = n * 3_000_000;
            }
            b.items.push(item);
            b.items.push(Item::new(
                TOP_OVERLAY,
                None,
                Shape::Track {
                    start: Point {
                        x: n * 3_000_000,
                        y: 100_000_000,
                    },
                    end: Point {
                        x: n * 3_000_000,
                        y: 101_000_000,
                    },
                    width: 20_000,
                },
            ));
        }
        let last = Item::new(
            TOP_OVERLAY,
            None,
            Shape::Track {
                start: Point {
                    x: 1999 * 3_000_000,
                    y: -400_000,
                },
                end: Point {
                    x: 1999 * 3_000_000,
                    y: 400_000,
                },
                width: 20_000,
            },
        );
        let id = last.id;
        b.items.push(last);
        let report = check(&b);
        assert!(!report.truncated);
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0].items.contains(&id));
        b.items.truncate(2);
        b.rules[0].insert("SCOPE1EXPRESSION".into(), "UnknownQuery".into());
        let report = check(&b);
        assert!(report.violations.is_empty());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|s| s.contains("Unsupported query: UnknownQuery"))
        );
    }
    fn board() -> Board {
        let mut b = Board::default();
        for x in [0, 1_250_000] {
            b.items.push(Item::new(
                TOP,
                None,
                Shape::Pad {
                    position: Point { x, y: 0 },
                    size: Point::mm(1, 1),
                    rotation: 0.0,
                    shape: PadShape::Rectangle,
                    hole: 0,
                    plated: false,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            ));
        }
        b.rules = vec![
            default_rule("MinimumSolderMaskSliver"),
            default_rule("SilkToSolderMaskClearance"),
        ];
        b
    }
    #[test]
    fn sliver_uses_mask_expansion_and_unions_manual_apertures() {
        let mut b = board();
        let r = check(&b);
        assert_eq!(r.violations.len(), 1);
        assert!(r.violations[0].message.contains("0.050000"));
        assert_eq!(r.violations[0].layer, 37);
        b.items.push(Item::new(
            37,
            None,
            Shape::Fill {
                start: Point { x: 0, y: -500_000 },
                end: Point {
                    x: 1_250_000,
                    y: 500_000,
                },
                rotation: 0.0,
            },
        ));
        assert!(check(&b).violations.is_empty());
        b.items.pop();
        let mut settings = MaskSettings::for_item(&b.items[1]);
        settings.solder[0].tented = true;
        b.items[1].masks = Some(Box::new(settings));
        assert!(check(&b).violations.is_empty());
    }
    #[test]
    fn silk_mask_exposed_copper_scopes_and_opposite_side() {
        let mut b = board();
        b.items.truncate(1);
        b.rules[1].insert("MINSILKSCREENTOMASKGAP".into(), "0.02mm".into());
        b.rules[1].insert("SCOPE1EXPRESSION".into(), "IsPad".into());
        b.items.push(Item::new(
            TOP_OVERLAY,
            None,
            Shape::Track {
                start: Point {
                    x: 580_000,
                    y: -400_000,
                },
                end: Point {
                    x: 580_000,
                    y: 400_000,
                },
                width: 20_000,
            },
        ));
        assert_eq!(check(&b).violations.len(), 1);
        b.rules[1].insert("CLEARANCETOEXPOSEDCOPPER".into(), "TRUE".into());
        assert!(check(&b).violations.is_empty());
        b.rules[1].insert("CLEARANCETOEXPOSEDCOPPER".into(), "FALSE".into());
        b.items[1].layer = BOTTOM_OVERLAY;
        assert!(check(&b).violations.is_empty());
    }
}
