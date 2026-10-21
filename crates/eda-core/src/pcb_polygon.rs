//! Editable polygon definitions and deterministic, GUI-independent copper generation.
//! Coordinates stay in nm in the model; boolean operations run in mm in geo.
//! This is not DRC: only pour clearance/thermal geometry is calculated here.
use crate::pcb::*;
use eda_geom::Point;
use geo::{
    Area, BooleanOps, Buffer, Coord, Intersects, LineString, MultiPolygon, Polygon as GeoPolygon,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Connection {
    #[default]
    Direct,
    Thermal,
    None,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Polygon {
    pub id: Uuid,
    pub index: u32,
    pub name: String,
    pub layer: u16,
    pub net: Option<u32>,
    pub contour: Vec<ContourVertex>,
    pub holes: Vec<Vec<Point>>,
    pub clearance: i64,
    pub connection: Connection,
    pub thermal_gap: i64,
    pub spoke_width: i64,
    pub spoke_angle: f64,
    /// Smaller values are poured first and have higher precedence.
    pub priority: u32,
    pub remove_islands: bool,
    pub min_island_area_mm2: f64,
    pub shelved: bool,
    pub locked: bool,
    /// Original properties remain available to the native writer.
    pub properties: Fields,
}
impl Default for Polygon {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            index: 0,
            name: "Polygon".into(),
            layer: TOP,
            net: None,
            contour: vec![],
            holes: vec![],
            clearance: 200_000,
            connection: Connection::Direct,
            thermal_gap: 250_000,
            spoke_width: 300_000,
            spoke_angle: 45.0,
            priority: 0,
            remove_islands: false,
            min_island_area_mm2: 0.0,
            shelved: false,
            locked: false,
            properties: Fields::new(),
        }
    }
}
impl Polygon {
    pub fn points(&self) -> Vec<Point> {
        contour_points(&self.contour)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !copper_layer(self.layer) {
            return Err("A polygon pour must be on a copper layer".into());
        }
        if !(0..=100_000_000).contains(&self.clearance)
            || !(1..=100_000_000).contains(&self.thermal_gap)
            || !(1..=100_000_000).contains(&self.spoke_width)
            || !self.spoke_angle.is_finite()
            || !self.min_island_area_mm2.is_finite()
            || self.min_island_area_mm2 < 0.0
        {
            return Err("Invalid polygon clearance/thermal/area settings".into());
        }
        if self.contour.iter().any(|v| {
            !v.position.valid()
                || v.arc.as_ref().is_some_and(|a| {
                    !a.centre.valid()
                        || a.radius <= 0
                        || a.radius > 100_000_000_000
                        || !a.start_angle.is_finite()
                        || !a.end_angle.is_finite()
                })
        }) {
            return Err("Invalid polygon contour arc".into());
        }
        validate_ring(&self.points())?;
        let outer = ring_polygon(&self.points(), &[]);
        for hole in &self.holes {
            validate_ring(hole)?;
            // A cutout may touch the outer boundary; intersection handles that case.
            if !outer.intersects(&ring_polygon(hole, &[])) {
                return Err("Polygon cutout is outside its outline".into());
            }
        }
        Ok(())
    }
}
pub fn contour_points(contour: &[ContourVertex]) -> Vec<Point> {
    let mut points = Vec::new();
    for v in contour {
        if let Some(a) = &v.arc {
            points.extend(arc_points(
                a.centre,
                a.radius,
                a.start_angle,
                a.end_angle,
                256,
            ));
        } else {
            points.push(v.position);
        }
    }
    points.dedup();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    points
}
pub fn straight_contour(points: &[Point]) -> Vec<ContourVertex> {
    points
        .iter()
        .copied()
        .map(|position| ContourVertex {
            position,
            arc: None,
        })
        .collect()
}
fn cross(a: Point, b: Point, c: Point) -> i128 {
    (b.x as i128 - a.x as i128) * (c.y as i128 - a.y as i128)
        - (b.y as i128 - a.y as i128) * (c.x as i128 - a.x as i128)
}
fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let x = cross(a, b, c);
    let y = cross(a, b, d);
    let z = cross(c, d, a);
    let w = cross(c, d, b);
    (x.signum() != y.signum() && z.signum() != w.signum())
        || (x == 0 && c.on_segment(a, b))
        || (y == 0 && d.on_segment(a, b))
        || (z == 0 && a.on_segment(c, d))
        || (w == 0 && b.on_segment(c, d))
}
/// Reject malformed outlines instead of silently asking a boolean engine to repair them.
pub fn validate_ring(points: &[Point]) -> Result<(), String> {
    if points.len() < 3 || points.len() > 20_000 || points.iter().any(|p| !p.valid()) {
        return Err("An outline needs 3..20000 valid vertices".into());
    }
    let mut area = 0i128;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if a == b {
            return Err("Adjacent polygon vertices must be distinct".into());
        }
        area += a.x as i128 * b.y as i128 - a.y as i128 * b.x as i128;
    }
    if area == 0 {
        return Err("Polygon outline has zero area".into());
    }
    // Quadratic check is bounded for interactive edits; large imported rings rely on
    // geo's boolean engine and the nonzero-area check, rather than blocking the UI.
    if points.len() <= 2048 {
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                if j == i + 1 || (i == 0 && j == points.len() - 1) {
                    continue;
                }
                if segments_cross(
                    points[i],
                    points[(i + 1) % points.len()],
                    points[j],
                    points[(j + 1) % points.len()],
                ) {
                    return Err("Self-intersecting polygon outline".into());
                }
            }
        }
    }
    Ok(())
}
fn ring(points: &[Point]) -> LineString<f64> {
    let mut v: Vec<Coord<f64>> = points
        .iter()
        .map(|p| Coord {
            x: p.x as f64 / 1e6,
            y: p.y as f64 / 1e6,
        })
        .collect();
    if v.len() > 1 && v.first() != v.last() {
        v.push(v[0]);
    }
    LineString::new(v)
}
fn ring_polygon(points: &[Point], holes: &[Vec<Point>]) -> GeoPolygon<f64> {
    GeoPolygon::new(ring(points), holes.iter().map(|h| ring(h)).collect())
}
fn mp(poly: GeoPolygon<f64>) -> MultiPolygon<f64> {
    MultiPolygon(vec![poly])
}
fn empty() -> MultiPolygon<f64> {
    MultiPolygon(vec![])
}
fn from_line(line: &LineString<f64>) -> Vec<Point> {
    let mut points: Vec<_> = line
        .0
        .iter()
        .map(|c| Point {
            x: (c.x * 1e6).round() as i64,
            y: (c.y * 1e6).round() as i64,
        })
        .collect();
    points.dedup();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    points
}
/// Pad outline is shared by the renderer, pour engine and hit testing.
pub fn pad_outline(position: Point, size: Point, angle: f64, shape: PadShape) -> Vec<Point> {
    let w = size.x as f64;
    let h = size.y as f64;
    let mut p = Vec::new();
    match shape {
        PadShape::Rectangle => return rectangle(position, size, angle),
        PadShape::Octagonal => {
            let d = w.min(h) * 0.2928932188134524;
            p.extend([
                (-w / 2.0 + d, -h / 2.0),
                (w / 2.0 - d, -h / 2.0),
                (w / 2.0, -h / 2.0 + d),
                (w / 2.0, h / 2.0 - d),
                (w / 2.0 - d, h / 2.0),
                (-w / 2.0 + d, h / 2.0),
                (-w / 2.0, h / 2.0 - d),
                (-w / 2.0, -h / 2.0 + d),
            ]);
        }
        PadShape::Round | PadShape::RoundedRectangle => {
            let r = w.min(h) / if shape == PadShape::Round { 2.0 } else { 4.0 };
            for (cx, cy, a) in [
                (w / 2.0 - r, h / 2.0 - r, 0.0),
                (-w / 2.0 + r, h / 2.0 - r, 90.0),
                (-w / 2.0 + r, -h / 2.0 + r, 180.0),
                (w / 2.0 - r, -h / 2.0 + r, 270.0),
            ] {
                for j in 0..=16 {
                    let t = (a + j as f64 * 90.0 / 16.0).to_radians();
                    p.push((cx + r * t.cos(), cy + r * t.sin()));
                }
            }
        }
    }
    let (s, c) = angle.to_radians().sin_cos();
    let mut out: Vec<_> = p
        .into_iter()
        .map(|(x, y)| Point {
            x: position.x + (x * c - y * s).round() as i64,
            y: position.y + (x * s + y * c).round() as i64,
        })
        .collect();
    out.dedup();
    out
}
/// True if an item has copper on the target layer. Blind vias obey the physical stack.
pub fn on_layer(board: &Board, item: &Item, layer: u16) -> bool {
    if item.layer != MULTI_LAYER {
        return item.layer == layer;
    }
    match item.shape {
        Shape::Via {
            from_layer,
            to_layer,
            ..
        } => {
            let mut stack: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
            stack.sort_by_key(|l| l.order);
            let a = stack.iter().position(|l| l.id == from_layer);
            let b = stack.iter().position(|l| l.id == to_layer);
            let c = stack.iter().position(|l| l.id == layer);
            match (a, b, c) {
                (Some(a), Some(b), Some(c)) => (a.min(b)..=a.max(b)).contains(&c),
                _ => false,
            }
        }
        Shape::Pad { .. } => copper_layer(layer),
        _ => false,
    }
}
fn segment_copper(a: Point, b: Point, width: i64) -> MultiPolygon<f64> {
    if a == b {
        return circle(a, width / 2);
    }
    LineString::from(vec![
        (a.x as f64 / 1e6, a.y as f64 / 1e6),
        (b.x as f64 / 1e6, b.y as f64 / 1e6),
    ])
    .buffer(width as f64 / 2e6)
}
fn circle(c: Point, r: i64) -> MultiPolygon<f64> {
    geo::Point::new(c.x as f64 / 1e6, c.y as f64 / 1e6).buffer(r as f64 / 1e6)
}
fn copper(item: &Item, layer: u16) -> MultiPolygon<f64> {
    match &item.shape {
        Shape::Track { start, end, width } => segment_copper(*start, *end, *width),
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => LineString::new(
            arc_points(*centre, *radius, *start_angle, *end_angle, 256)
                .into_iter()
                .map(|p| Coord {
                    x: p.x as f64 / 1e6,
                    y: p.y as f64 / 1e6,
                })
                .collect(),
        )
        .buffer(*width as f64 / 2e6),
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            bottom_size,
            middle_size,
            ..
        } => {
            let sz = if item.layer == MULTI_LAYER && layer == BOTTOM {
                bottom_size.unwrap_or(*size)
            } else if item.layer == MULTI_LAYER && layer != TOP {
                middle_size.unwrap_or(*size)
            } else {
                *size
            };
            mp(ring_polygon(
                &pad_outline(*position, sz, *rotation, *shape),
                &[],
            ))
        }
        Shape::Via {
            position, diameter, ..
        } => circle(*position, *diameter / 2),
        Shape::Fill {
            start,
            end,
            rotation,
        } => mp(ring_polygon(
            &rectangle(
                midpoint(*start, *end),
                Point {
                    x: (end.x - start.x).abs(),
                    y: (end.y - start.y).abs(),
                },
                *rotation,
            ),
            &[],
        )),
        Shape::Region { contour, holes, .. } => mp(ring_polygon(contour, holes)),
        // Copper text is excluded conservatively by its bounds. Exact stroke fonts are not modelled.
        Shape::Text { .. } => {
            let b = item.bounds();
            mp(ring_polygon(
                &rectangle(
                    b.centre(),
                    Point {
                        x: b.max.x - b.min.x,
                        y: b.max.y - b.min.y,
                    },
                    0.0,
                ),
                &[],
            ))
        }
    }
}
fn thermals(item: &Item, p: &Polygon, material: &MultiPolygon<f64>) -> MultiPolygon<f64> {
    let c = item.anchor();
    let b = item.bounds();
    let length = (b.max.x - b.min.x).max(b.max.y - b.min.y) + 2 * p.thermal_gap + 2 * p.spoke_width;
    let mut spokes = empty();
    for angle in [p.spoke_angle, p.spoke_angle + 90.0] {
        spokes = spokes.union(&mp(ring_polygon(
            &rectangle(
                c,
                Point {
                    x: length,
                    y: p.spoke_width,
                },
                angle,
            ),
            &[],
        )));
    }
    material
        .buffer(p.thermal_gap as f64 / 1e6)
        .difference(&material.union(&spokes))
}
/// Rebuild all pours in dependency order, leaving the caller untouched on error.
pub fn repour(board: &Board) -> Result<Board, String> {
    let mut result = board.clone();
    for p in &board.polygon_pours {
        p.validate()?;
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
    for p in ordered {
        if p.shelved {
            continue;
        }
        let mut material = mp(ring_polygon(&p.points(), &p.holes));
        if let Some(clip) = &board_clip {
            material = material.intersection(clip);
        }
        let mut anchors = empty();
        for item in &base {
            let board_cutout = matches!(&item.shape, Shape::Region { cutout: true, .. })
                && item
                    .properties
                    .get("ISBOARDCUTOUT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"));
            if !board_cutout && !on_layer(board, item, p.layer) && item.layer != KEEP_OUT {
                continue;
            }
            let geom = copper(item, p.layer);
            let is_cutout = matches!(item.shape, Shape::Region { cutout: true, .. });
            if is_cutout {
                material = material.difference(&geom);
                continue;
            }
            let same = p.net.is_some() && item.net == p.net && !item.keepout;
            if same {
                anchors = anchors.union(&geom);
                if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                    match p.connection {
                        Connection::Direct => {}
                        Connection::Thermal => {
                            material = material.difference(&thermals(item, p, &geom))
                        }
                        Connection::None => {
                            material = material.difference(&geom.buffer(p.clearance as f64 / 1e6))
                        }
                    }
                }
            } else {
                material = material.difference(&geom.buffer(p.clearance as f64 / 1e6));
            }
        }
        for (other, geom) in &poured {
            if other.layer != p.layer {
                continue;
            }
            if p.net.is_some() && other.net == p.net {
                anchors = anchors.union(geom);
            } else {
                material = material
                    .difference(&geom.buffer(p.clearance.max(other.clearance) as f64 / 1e6));
            }
        }
        material.0.retain(|poly| {
            poly.unsigned_area() >= p.min_island_area_mm2
                && (!p.remove_islands || poly.intersects(&anchors))
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
    result.pours_stale = false;
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square(x: i64, y: i64, w: i64, h: i64) -> Vec<Point> {
        vec![
            Point::mm(x, y),
            Point::mm(x + w, y),
            Point::mm(x + w, y + h),
            Point::mm(x, y + h),
        ]
    }
    fn board() -> Board {
        let mut b = Board::default();
        b.nets.push(Net {
            index: 0,
            name: "GND".into(),
            properties: Fields::new(),
        });
        b.outline = straight_contour(&square(0, 0, 20, 20));
        b.polygon_pours.push(Polygon {
            net: Some(0),
            contour: straight_contour(&square(0, 0, 20, 20)),
            ..Default::default()
        });
        b
    }
    fn area(b: &Board) -> f64 {
        b.items
            .iter()
            .filter(|i| {
                i.polygon.is_some() && !matches!(i.shape, Shape::Region { cutout: true, .. })
            })
            .map(|i| copper(i, TOP).unsigned_area())
            .sum()
    }
    #[test]
    fn empty_board_pours_and_repour_is_deterministic() {
        let a = repour(&board()).unwrap();
        assert!((area(&a) - 400.0).abs() < 1e-5);
        assert_eq!(a, repour(&a).unwrap());
    }
    #[test]
    fn foreign_copper_is_cleared() {
        let mut b = board();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::mm(10, 10),
                size: Point::mm(2, 2),
                rotation: 0.0,
                shape: PadShape::Rectangle,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        ));
        let a = repour(&b).unwrap();
        assert!(area(&a) < 396.0);
    }
    #[test]
    fn holes_and_shelving_are_real_geometry() {
        let mut b = board();
        b.polygon_pours[0].holes.push(square(5, 5, 5, 5));
        assert!((area(&repour(&b).unwrap()) - 375.0).abs() < 1e-5);
        b.polygon_pours[0].shelved = true;
        assert_eq!(area(&repour(&b).unwrap()), 0.0);
    }
    #[test]
    fn remove_unconnected_islands() {
        let mut b = board();
        b.polygon_pours[0].remove_islands = true;
        assert_eq!(area(&repour(&b).unwrap()), 0.0);
    }
    #[test]
    fn self_intersection_is_rejected() {
        assert!(
            validate_ring(&[
                Point::mm(0, 0),
                Point::mm(10, 10),
                Point::mm(0, 10),
                Point::mm(10, 0)
            ])
            .is_err()
        );
    }
    #[test]
    fn same_net_direct_connection_does_not_clear_pad() {
        let mut b = board();
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Via {
                position: Point::mm(10, 10),
                diameter: 1_000_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        assert!((area(&repour(&b).unwrap()) - 400.0).abs() < 1e-5);
        b.polygon_pours[0].connection = Connection::Thermal;
        assert!(area(&repour(&b).unwrap()) < 400.0);
    }
    #[test]
    fn priority_prevents_foreign_net_overlap() {
        let mut b = board();
        b.nets.push(Net {
            index: 1,
            name: "VCC".into(),
            properties: Fields::new(),
        });
        let mut p = b.polygon_pours[0].clone();
        p.id = Uuid::new_v4();
        p.index = 1;
        p.priority = 1;
        p.net = Some(1);
        b.polygon_pours.push(p);
        let a = repour(&b).unwrap();
        assert!(!a.items.iter().any(|i| i.polygon == Some(1)));
    }
}
