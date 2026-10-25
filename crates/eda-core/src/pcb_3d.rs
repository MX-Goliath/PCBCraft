//! Read-only physical geometry for the PCB viewer; never rewrites native models.
use crate::pcb::*;
use eda_geom::Point;
use geo::{BooleanOps, Coord, LineString, Polygon, unary_union};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelTriangle {
    pub vertices: [[f64; 3]; 3],
    pub color: [u8; 4],
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Model {
    pub name: String,
    pub triangles: Vec<ModelTriangle>,
}
impl Model {
    pub fn valid(&self) -> bool {
        !self.triangles.is_empty()
            && self.triangles.len() <= 1_000_000
            && self.triangles.iter().all(|t| {
                t.vertices
                    .iter()
                    .flatten()
                    .all(|x| x.is_finite() && x.abs() <= 100_000.0)
            })
    }
}

/// Native MODEL.2D is an absolute board origin. MODEL.3D Euler rotations
/// precede the mounting-side reflection; component rotation is already baked
/// into the body record and must not be applied a second time.
#[derive(Clone, Copy, Debug)]
pub struct ModelTransform {
    rotations: [(f64, f64); 3],
    offset: [f64; 3],
    bottom: bool,
    thickness: f64,
}
impl ModelTransform {
    /// Parse placement once per body, not once per triangle vertex.
    pub fn new(board: &Board, item: &Item) -> Option<Self> {
        let f = &item.properties;
        let mut rotations = [(0., 1.); 3];
        for (rotation, key) in
            rotations
                .iter_mut()
                .zip(["MODEL.3D.ROTX", "MODEL.3D.ROTY", "MODEL.3D.ROTZ"])
        {
            let angle = f
                .get(key)
                .map_or(Some(0.), |v| v.trim().parse::<f64>().ok())?;
            if !angle.is_finite() {
                return None;
            }
            *rotation = angle.to_radians().sin_cos();
        }
        let mut offset = [0.; 3];
        for (value, key) in offset
            .iter_mut()
            .zip(["MODEL.2D.X", "MODEL.2D.Y", "MODEL.3D.DZ"])
        {
            *value = dimension_nm(f.get(key).map_or("0", String::as_str)).ok()? as f64 / 1e6;
        }
        let bottom = item
            .component
            .and_then(|index| board.components.iter().find(|c| c.index == index))
            .is_some_and(|c| c.layer == BOTTOM);
        Some(Self {
            rotations,
            offset,
            bottom,
            thickness: board.stack_thickness_nm() as f64 / 1e6,
        })
    }
    pub fn point(&self, mut p: [f64; 3]) -> [f64; 3] {
        for ((s, c), (a, b)) in self.rotations.into_iter().zip([(1, 2), (2, 0), (0, 1)]) {
            let (x, y) = (p[a], p[b]);
            p[a] = x * c - y * s;
            p[b] = x * s + y * c;
        }
        if self.bottom {
            p[1] = -p[1];
        }
        for (p, offset) in p.iter_mut().zip(self.offset) {
            *p += offset;
        }
        if self.bottom {
            p[2] = -self.thickness - p[2];
        }
        p
    }
}
pub fn model_point(board: &Board, item: &Item, p: [f64; 3]) -> Option<[f64; 3]> {
    Some(ModelTransform::new(board, item)?.point(p))
}

pub(crate) fn transform_model(
    item: &mut Item,
    pivot: Point,
    delta: Point,
    angle: f64,
    flip: bool,
    bottom: bool,
) -> Result<(), String> {
    if !item.properties.contains_key("MODELID") {
        return Ok(());
    }
    let f = &mut item.properties;
    let p = Point {
        x: dimension_nm(f.get("MODEL.2D.X").map_or("0", String::as_str))?,
        y: dimension_nm(f.get("MODEL.2D.Y").map_or("0", String::as_str))?,
    };
    let p = if flip {
        Point {
            x: 2 * pivot.x - p.x,
            y: p.y,
        }
    } else {
        transform_point(p, pivot, delta, angle)?
    };
    let rotation = f
        .get("MODEL.3D.ROTZ")
        .map_or(Ok(0.), |v| v.parse::<f64>())
        .map_err(|_| "Invalid 3D model rotation")?;
    let rotation = if flip {
        rotation + 180.
    } else {
        rotation + if bottom { -angle } else { angle }
    }
    .rem_euclid(360.);
    f.insert("MODEL.2D.X".into(), format!("{}mm", p.x as f64 / 1e6));
    f.insert("MODEL.2D.Y".into(), format!("{}mm", p.y as f64 / 1e6));
    f.insert("MODEL.3D.ROTZ".into(), rotation.to_string());
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Body {
    pub component: Uuid,
    pub outline: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
    pub bottom_nm: i64,
    pub top_nm: i64,
    pub simplified: bool,
}
fn rotate(p: Point, origin: Point, angle: f64) -> Point {
    transform_point(p, origin, Point::default(), angle).unwrap_or(p)
}
fn dimension(fields: &Fields, key: &str) -> Option<i64> {
    fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| dimension_nm(v).ok())
        .filter(|v| *v >= 0 && *v < 1_000_000_000)
}
/// Native body outlines are already in board coordinates. Missing bodies use the
/// rotated local land envelope, explicitly labelled as simplified in the viewer.
pub fn bodies(board: &Board) -> Vec<Body> {
    let mut result = vec![];
    for c in &board.components {
        let mut found = false;
        for item in board.items.iter().filter(|i| {
            i.component == Some(c.index)
                && i.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
                && !i
                    .properties
                    .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
        }) {
            if item
                .properties
                .get("MODELID")
                .is_some_and(|id| board.models.contains_key(id))
            {
                found = true;
                continue;
            }
            if let Shape::Region { contour, holes, .. } = &item.shape
                && contour.len() >= 3
            {
                let stand = dimension(&item.properties, "STANDOFFHEIGHT").unwrap_or(0);
                let height = dimension(&item.properties, "OVERALLHEIGHT")
                    .filter(|h| *h > stand)
                    .or_else(|| dimension(&c.properties, "HEIGHT").filter(|h| *h > stand))
                    .unwrap_or(1_000_000)
                    .max(stand + 10_000);
                let (bottom_nm, top_nm) = if c.layer == BOTTOM {
                    (
                        -board.stack_thickness_nm() - height,
                        -board.stack_thickness_nm() - stand,
                    )
                } else {
                    (stand, height)
                };
                result.push(Body {
                    component: c.id,
                    outline: contour.clone(),
                    holes: holes.clone(),
                    bottom_nm,
                    top_nm,
                    simplified: true,
                });
                found = true;
            }
        }
        if found {
            continue;
        }
        if board.source.is_some() {
            continue;
        }
        let mut bounds = Bounds::empty();
        for item in board
            .items
            .iter()
            .filter(|i| i.component == Some(c.index) && matches!(i.shape, Shape::Pad { .. }))
        {
            for p in item.pad_outline(c.layer) {
                bounds.include(rotate(p, c.position, -c.rotation));
            }
        }
        if bounds.empty {
            bounds = Bounds::points(&rectangle(c.position, Point::mm(2, 2), 0.0));
        }
        let size = Point {
            x: (bounds.max.x - bounds.min.x).max(200_000),
            y: (bounds.max.y - bounds.min.y).max(200_000),
        };
        // Inset the envelope slightly so pads remain visible around the placeholder.
        let size = Point {
            x: size.x * 3 / 4,
            y: size.y * 3 / 4,
        };
        let outline = rectangle(bounds.centre(), size, 0.0)
            .into_iter()
            .map(|p| rotate(p, c.position, c.rotation))
            .collect();
        let height = dimension(&c.properties, "HEIGHT")
            .filter(|h| *h > 0)
            .unwrap_or(1_000_000);
        let (bottom_nm, top_nm) = if c.layer == BOTTOM {
            (
                -board.stack_thickness_nm() - height,
                -board.stack_thickness_nm(),
            )
        } else {
            (0, height)
        };
        result.push(Body {
            component: c.id,
            outline,
            holes: vec![],
            bottom_nm,
            top_nm,
            simplified: true,
        });
    }
    result
}
fn polygon(points: &[Point]) -> Polygon<f64> {
    let mut p: Vec<_> = points
        .iter()
        .map(|p| Coord {
            x: p.x as f64 / 1e6,
            y: p.y as f64 / 1e6,
        })
        .collect();
    if let Some(first) = p.first().copied() {
        p.push(first);
    }
    Polygon::new(LineString::new(p), vec![])
}
fn physical_cutout(item: &Item) -> bool {
    item.properties
        .get("ISBOARDCUTOUT")
        .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
        && !item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        && !item
            .properties
            .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
}
/// Actual routed board cutouts and through drills, unioned before subtraction so
/// overlapping holes and edge slots do not create invalid triangulation rings.
pub fn substrate(board: &Board) -> Vec<StackSurface> {
    let outline = board.outline_points(256);
    if outline.len() < 3 {
        return vec![];
    }
    let mut cuts = vec![];
    for item in &board.items {
        let through = match item.shape {
            Shape::Pad { hole, .. } => hole > 0,
            Shape::Via {
                from_layer,
                to_layer,
                ..
            } => {
                (from_layer == TOP && to_layer == BOTTOM)
                    || (from_layer == BOTTOM && to_layer == TOP)
            }
            _ => false,
        };
        if through {
            let drill = item.drill_outline();
            if drill.len() >= 3 {
                cuts.push(polygon(&drill));
            }
        }
        if let Shape::Region {
            contour,
            cutout: true,
            ..
        } = &item.shape
            && physical_cutout(item)
            && contour.len() >= 3
        {
            cuts.push(polygon(contour));
        }
    }
    let outer = polygon(&outline);
    let result = if cuts.is_empty() {
        geo::MultiPolygon(vec![outer])
    } else {
        outer.difference(&unary_union(cuts.iter()))
    };
    let ring = |line: &LineString<f64>| {
        line.0
            .iter()
            .take(line.0.len().saturating_sub(1))
            .map(|p| Point {
                x: (p.x * 1e6).round() as i64,
                y: (p.y * 1e6).round() as i64,
            })
            .collect()
    };
    result
        .0
        .iter()
        .map(|p| StackSurface {
            outline: ring(p.exterior()),
            holes: p.interiors().iter().map(ring).collect(),
        })
        .collect()
}

/// Physical through cuts applied to surface copper as well as the substrate.
/// Bounds reject unrelated drills before any polygon boolean operation.
pub struct Cuts {
    index: rstar::RTree<CutEntry>,
    convex: Vec<Point>,
    polygons: Vec<(Bounds, Polygon<f64>)>,
    board: Polygon<f64>,
}
struct CutEntry {
    bounds: rstar::AABB<[f64; 2]>,
    index: usize,
}
impl rstar::RTreeObject for CutEntry {
    type Envelope = rstar::AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}
fn envelope(b: Bounds) -> rstar::AABB<[f64; 2]> {
    rstar::AABB::from_corners(
        [b.min.x as f64, b.min.y as f64],
        [b.max.x as f64, b.max.y as f64],
    )
}
fn cross(a: Point, b: Point, p: Point) -> i128 {
    (b.x as i128 - a.x as i128) * (p.y as i128 - a.y as i128)
        - (b.y as i128 - a.y as i128) * (p.x as i128 - a.x as i128)
}
fn convex_outline(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return vec![];
    }
    let mut direction = 0;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        for p in points {
            let side = cross(*a, *b, *p).signum();
            if side != 0 {
                if direction != 0 && direction != side {
                    return vec![];
                }
                direction = side;
            }
        }
    }
    if direction == 0 {
        return vec![];
    }
    let mut result = points.to_vec();
    if direction < 0 {
        result.reverse();
    }
    result
}
impl Cuts {
    pub fn new(board: &Board) -> Self {
        let mut polygons = vec![];
        for item in &board.items {
            let through = match item.shape {
                Shape::Pad { hole, .. } => hole > 0,
                Shape::Via {
                    from_layer,
                    to_layer,
                    ..
                } => {
                    (from_layer == TOP && to_layer == BOTTOM)
                        || (from_layer == BOTTOM && to_layer == TOP)
                }
                _ => false,
            };
            if through {
                let points = item.drill_outline();
                if points.len() >= 3 {
                    polygons.push((Bounds::points(&points), polygon(&points)));
                }
            }
            if let Shape::Region {
                contour,
                cutout: true,
                ..
            } = &item.shape
                && physical_cutout(item)
                && contour.len() >= 3
            {
                polygons.push((Bounds::points(contour), polygon(contour)));
            }
        }
        let outline = board.outline_points(256);
        let index = rstar::RTree::bulk_load(
            polygons
                .iter()
                .enumerate()
                .map(|(i, (b, _))| CutEntry {
                    bounds: envelope(*b),
                    index: i,
                })
                .collect(),
        );
        Self {
            index,
            convex: convex_outline(&outline),
            polygons,
            board: polygon(&outline),
        }
    }
    fn near(&self, bounds: Bounds) -> Vec<usize> {
        if bounds.empty {
            return vec![];
        }
        let mut indices: Vec<_> = self
            .index
            .locate_in_envelope_intersecting(&envelope(bounds))
            .map(|p| p.index)
            .collect();
        // Preserve boolean input order for reproducible triangulation.
        indices.sort_unstable();
        indices
    }
    pub fn same_geometry(&self, other: &Self) -> bool {
        self.board == other.board && self.polygons == other.polygons
    }
    /// Only cuts touching these item bounds can change its clipped artwork.
    pub fn same_in(&self, other: &Self, bounds: Bounds) -> bool {
        self.board == other.board
            && self
                .near(bounds)
                .iter()
                .map(|i| &self.polygons[*i])
                .eq(other.near(bounds).iter().map(|i| &other.polygons[*i]))
    }
    pub fn surface(&self, outer: &[Point], holes: &[Vec<Point>]) -> Vec<StackSurface> {
        let bounds = Bounds::points(outer);
        let cuts: Vec<_> = self
            .near(bounds)
            .into_iter()
            .map(|i| &self.polygons[i].1)
            .collect();
        // A convex board contains the entire polygon when it contains every
        // vertex. Concave outlines still use the exact polygon boolean path.
        let inside = !self.convex.is_empty()
            && self
                .convex
                .iter()
                .zip(self.convex.iter().cycle().skip(1))
                .take(self.convex.len())
                .all(|(a, b)| outer.iter().all(|p| cross(*a, *b, *p) >= 0));
        if inside && cuts.is_empty() {
            return vec![StackSurface {
                outline: outer.to_vec(),
                holes: holes.to_vec(),
            }];
        }
        let p = polygon(outer);
        let p = Polygon::new(
            p.exterior().clone(),
            holes
                .iter()
                .map(|h| polygon(h).exterior().clone())
                .collect(),
        );
        let clipped = if inside {
            geo::MultiPolygon(vec![p])
        } else {
            p.intersection(&self.board)
        };
        let result = if cuts.is_empty() {
            clipped
        } else {
            clipped.difference(&unary_union(cuts))
        };
        let ring = |line: &LineString<f64>| {
            line.0
                .iter()
                .take(line.0.len().saturating_sub(1))
                .map(|p| Point {
                    x: (p.x * 1e6).round() as i64,
                    y: (p.y * 1e6).round() as i64,
                })
                .collect()
        };
        result
            .0
            .iter()
            .map(|p| StackSurface {
                outline: ring(p.exterior()),
                holes: p.interiors().iter().map(ring).collect(),
            })
            .collect()
    }
}

/// Solder resist is a physical film above copper, with the union of all pad,
/// via and explicitly drawn mask openings removed. It is not the board core.
pub fn solder_mask(board: &Board, bottom: bool) -> Vec<StackSurface> {
    solder_mask_on_substrate(board, bottom, &substrate(board))
}

/// Reuse the already cut substrate for both mask films in a scene rebuild.
pub fn solder_mask_on_substrate(
    board: &Board,
    bottom: bool,
    substrate: &[StackSurface],
) -> Vec<StackSurface> {
    let mut apertures = Vec::new();
    for item in &board.items {
        if item.keepout
            || item
                .properties
                .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
        {
            continue;
        }
        if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
            if let Ok(points) = item.mask_outline(board, MaskKind::Solder, bottom)
                && points.len() >= 3
            {
                apertures.push(polygon(&points));
            }
        } else if item.layer == if bottom { 38 } else { 37 } {
            for surface in planar_outline(item) {
                let p = polygon(&surface.outline);
                apertures.push(Polygon::new(
                    p.exterior().clone(),
                    surface
                        .holes
                        .iter()
                        .map(|h| polygon(h).exterior().clone())
                        .collect(),
                ));
            }
        }
    }
    let cuts = unary_union(apertures.iter());
    let mut result = Vec::new();
    for s in substrate {
        let p = polygon(&s.outline);
        let p = Polygon::new(
            p.exterior().clone(),
            s.holes
                .iter()
                .map(|h| polygon(h).exterior().clone())
                .collect(),
        );
        for p in p.difference(&cuts) {
            let ring = |line: &LineString<f64>| {
                line.0
                    .iter()
                    .take(line.0.len().saturating_sub(1))
                    .map(|p| Point {
                        x: (p.x * 1e6).round() as i64,
                        y: (p.y * 1e6).round() as i64,
                    })
                    .collect()
            };
            result.push(StackSurface {
                outline: ring(p.exterior()),
                holes: p.interiors().iter().map(ring).collect(),
            });
        }
    }
    result
}

/// Document-space planar artwork, shared by physical mask and copper geometry.
pub fn planar_outline(item: &Item) -> Vec<StackSurface> {
    let outline = match &item.shape {
        Shape::Track { start, end, width } => crate::pcb_polygon::pad_outline_radius(
            midpoint(*start, *end),
            Point {
                x: (((end.x - start.x) as f64).hypot((end.y - start.y) as f64) as i64) + width,
                y: *width,
            },
            ((end.y - start.y) as f64)
                .atan2((end.x - start.x) as f64)
                .to_degrees(),
            PadShape::Round,
            100,
        ),
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            let half = (*width / 2).max(1);
            let outer = arc_points(*centre, radius + half, *start_angle, *end_angle, 128);
            let mut inner = if *radius > half {
                arc_points(*centre, radius - half, *start_angle, *end_angle, 128)
            } else {
                vec![*centre]
            };
            inner.reverse();
            if arc_sweep(*start_angle, *end_angle) >= 360. - 1e-8 {
                return vec![StackSurface {
                    outline: outer,
                    holes: if inner.len() >= 3 {
                        vec![inner]
                    } else {
                        vec![]
                    },
                }];
            }
            let axis = arc_points(*centre, *radius, *start_angle, *end_angle, 128);
            let end = *start_angle + arc_sweep(*start_angle, *end_angle);
            let mut outline = outer;
            outline.extend(arc_points(*axis.last().unwrap(), half, end, end + 180., 32));
            outline.extend(inner);
            outline.extend(arc_points(
                axis[0],
                half,
                *start_angle + 180.,
                *start_angle + 360.,
                32,
            ));
            outline.dedup();
            return vec![StackSurface {
                outline,
                holes: vec![],
            }];
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => rectangle(
            midpoint(*start, *end),
            Point {
                x: (end.x - start.x).abs(),
                y: (end.y - start.y).abs(),
            },
            *rotation,
        ),
        Shape::Region {
            contour,
            holes,
            cutout: false,
        } => {
            return vec![StackSurface {
                outline: contour.clone(),
                holes: holes.clone(),
            }];
        }
        _ => return vec![],
    };
    vec![StackSurface {
        outline,
        holes: vec![],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, DocumentKind};
    fn board() -> Board {
        Board::from_legacy(&Document::new(DocumentKind::Board, "3D"))
    }
    fn component() -> Component {
        Component {
            id: Uuid::new_v4(),
            index: 0,
            designator: "U1".into(),
            comment: String::new(),
            footprint: String::new(),
            position: Point::mm(10, 10),
            rotation: 90.,
            layer: TOP,
            locked: false,
            properties: Fields::from([("HEIGHT".into(), "2mm".into())]),
        }
    }
    fn contains(surfaces: &[StackSurface], p: Point) -> bool {
        surfaces.iter().any(|s| {
            point_in_polygon(p, &s.outline) && !s.holes.iter().any(|h| point_in_polygon(p, h))
        })
    }
    #[test]
    fn indexed_cuts_match_full_boolean_for_convex_concave_and_edge_crossings() {
        use geo::Area;
        for concave in [false, true] {
            let mut b = board();
            if concave {
                b.outline = [
                    Point::mm(0, 0),
                    Point::mm(30, 0),
                    Point::mm(30, 10),
                    Point::mm(10, 10),
                    Point::mm(10, 30),
                    Point::mm(0, 30),
                ]
                .into_iter()
                .map(|position| ContourVertex {
                    position,
                    arc: None,
                })
                .collect();
            }
            for x in [5, 6, 15, 25] {
                b.items.push(Item::new(
                    MULTI_LAYER,
                    None,
                    Shape::Via {
                        position: Point::mm(x, 8),
                        diameter: 3_000_000,
                        hole: 2_000_000,
                        from_layer: TOP,
                        to_layer: BOTTOM,
                    },
                ));
            }
            let cuts = Cuts::new(&b);
            for x in -2..35 {
                let outer = rectangle(Point::mm(x, 10), Point::mm(5, 6), 17.);
                let hole = rectangle(Point::mm(x, 10), Point::mm(1, 1), 0.);
                let p = Polygon::new(
                    polygon(&outer).exterior().clone(),
                    vec![polygon(&hole).exterior().clone()],
                );
                let expected = p
                    .intersection(&cuts.board)
                    .difference(&unary_union(cuts.polygons.iter().map(|(_, p)| p)));
                let actual = geo::MultiPolygon(
                    cuts.surface(&outer, &[hole])
                        .iter()
                        .map(|s| {
                            Polygon::new(
                                polygon(&s.outline).exterior().clone(),
                                s.holes
                                    .iter()
                                    .map(|h| polygon(h).exterior().clone())
                                    .collect(),
                            )
                        })
                        .collect(),
                );
                assert!(
                    actual.xor(&expected).unsigned_area() < 0.0001,
                    "concave={concave} x={x}"
                );
            }
        }
    }
    #[test]
    fn prepared_model_transform_preserves_euler_order_bottom_offsets_and_invalid_data() {
        let mut b = board();
        b.components.push(component());
        let mut item = Item::new(
            57,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point::mm(1, 0),
                width: 100_000,
            },
        );
        item.component = Some(0);
        item.properties = Fields::from([
            ("MODEL.3D.ROTX".into(), "90".into()),
            ("MODEL.3D.ROTY".into(), "90".into()),
            ("MODEL.3D.ROTZ".into(), "90".into()),
            ("MODEL.2D.X".into(), "10mm".into()),
            ("MODEL.2D.Y".into(), "20mm".into()),
            ("MODEL.3D.DZ".into(), "2mm".into()),
        ]);
        let top = ModelTransform::new(&b, &item).unwrap().point([1., 2., 3.]);
        for (a, e) in top.into_iter().zip([13., 22., 1.]) {
            assert!((a - e).abs() < 1e-10);
        }
        b.components[0].layer = BOTTOM;
        let bottom = ModelTransform::new(&b, &item).unwrap().point([1., 2., 3.]);
        for (a, e) in bottom.into_iter().zip([13., 18., -2.6]) {
            assert!((a - e).abs() < 1e-10);
        }
        item.properties.insert("MODEL.3D.ROTX".into(), "NaN".into());
        assert!(ModelTransform::new(&b, &item).is_none());
    }
    #[test]
    fn curved_artwork_is_a_bounded_band_with_round_caps_and_a_true_ring_counter() {
        let mut arc = Item::new(
            TOP,
            None,
            Shape::Arc {
                centre: Point::mm(10, 10),
                radius: 5_000_000,
                start_angle: 0.,
                end_angle: 90.,
                width: 2_000_000,
            },
        );
        let partial = planar_outline(&arc);
        assert_eq!(partial.len(), 1);
        assert!(partial[0].outline.len() < 200);
        assert!(contains(
            &partial,
            Point {
                x: 15_500_000,
                y: 9_700_000
            }
        ));
        assert!(!contains(&partial, Point::mm(10, 10)));
        if let Shape::Arc { end_angle, .. } = &mut arc.shape {
            *end_angle = 360.;
        }
        let circle = planar_outline(&arc);
        assert_eq!(circle[0].holes.len(), 1);
        assert!(!contains(&circle, Point::mm(10, 10)));
        assert!(contains(&circle, Point::mm(15, 10)));
    }
    #[test]
    fn copper_polygon_cutouts_never_drill_the_board_or_other_copper() {
        let mut b = board();
        let mut cut = Item::new(
            TOP,
            None,
            Shape::Region {
                contour: rectangle(Point::mm(10, 10), Point::mm(4, 4), 0.),
                holes: vec![],
                cutout: true,
            },
        );
        cut.properties.insert("KIND".into(), "1".into());
        b.items.push(cut);
        let p = Point::mm(10, 10);
        assert!(contains(&substrate(&b), p));
        let copper = rectangle(p, Point::mm(8, 8), 0.);
        assert!(contains(&Cuts::new(&b).surface(&copper, &[]), p));
        b.items[0]
            .properties
            .insert("ISBOARDCUTOUT".into(), "TRUE".into());
        assert!(!contains(&substrate(&b), p));
        assert!(!contains(&Cuts::new(&b).surface(&copper, &[]), p));
    }
    #[test]
    fn mask_unions_explicit_openings_tenting_and_does_not_change_source() {
        let mut b = board();
        let mut via = Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::mm(10, 10),
                diameter: 2_000_000,
                hole: 500_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        );
        let mut settings = MaskSettings::for_item(&via);
        settings.solder[0].tented = true;
        via.masks = Some(Box::new(settings));
        b.items.push(via);
        b.items.push(Item::new(
            37,
            None,
            Shape::Fill {
                start: Point::mm(14, 14),
                end: Point::mm(18, 18),
                rotation: 0.,
            },
        ));
        let before = b.clone();
        assert!(contains(
            &solder_mask(&b, false),
            Point {
                x: 10_700_000,
                y: 10_000_000
            }
        ));
        assert!(!contains(
            &solder_mask(&b, true),
            Point {
                x: 10_700_000,
                y: 10_000_000
            }
        ));
        assert!(!contains(&solder_mask(&b, false), Point::mm(16, 16)));
        assert!(contains(&solder_mask(&b, true), Point::mm(16, 16)));
        assert_eq!(b, before);
    }
    #[test]
    fn native_mesh_follows_rotation_translation_flip_and_undo_with_shared_geometry() {
        let mut b = board();
        b.components.push(component());
        let mut item = Item::new(
            57,
            None,
            Shape::Region {
                contour: rectangle(Point::mm(10, 10), Point::mm(2, 2), 0.),
                holes: vec![],
                cutout: false,
            },
        );
        item.component = Some(0);
        item.properties = Fields::from([
            ("MODELID".into(), "model".into()),
            ("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into()),
            ("MODEL.2D.X".into(), "10mm".into()),
            ("MODEL.2D.Y".into(), "10mm".into()),
            ("MODEL.3D.ROTZ".into(), "30".into()),
        ]);
        b.models.insert(
            "model".into(),
            std::sync::Arc::new(Model {
                name: "native".into(),
                triangles: vec![ModelTriangle {
                    vertices: [[0., 0., 0.], [1., 0., 0.], [0., 1., 2.]],
                    color: [255; 4],
                }],
            }),
        );
        b.items.push(item);
        let p = [0.3, 0.8, 1.];
        let original = model_point(&b, &b.items[0], p).unwrap();
        let moved = b
            .edited(Edit::Transform {
                selection: vec![b.components[0].id],
                pivot: Point::mm(10, 10),
                translation: Point::mm(2, 3),
                angle: 90.,
            })
            .unwrap();
        let q = model_point(&moved, &moved.items[0], p).unwrap();
        assert!((q[0] - (12. - (original[1] - 10.))).abs() < 1e-6);
        assert!((q[1] - (13. + (original[0] - 10.))).abs() < 1e-6);
        assert!(std::sync::Arc::ptr_eq(
            &b.models["model"],
            &moved.models["model"]
        ));
        let flipped = crate::pcb_tools::flip_component(&b, b.components[0].id).unwrap();
        let q = model_point(&flipped, &flipped.items[0], p).unwrap();
        assert!((q[0] - (20. - original[0])).abs() < 1e-6);
        assert!((q[1] - original[1]).abs() < 1e-6);
        assert!((q[2] + b.stack_thickness_nm() as f64 / 1e6 + original[2]).abs() < 1e-6);
        let moved_bottom = flipped
            .edited(Edit::Transform {
                selection: vec![flipped.components[0].id],
                pivot: Point::mm(10, 10),
                translation: Point::mm(2, 3),
                angle: 90.,
            })
            .unwrap();
        let r = model_point(&moved_bottom, &moved_bottom.items[0], p).unwrap();
        assert!((r[0] - (12. - (q[1] - 10.))).abs() < 1e-6);
        assert!((r[1] - (13. + (q[0] - 10.))).abs() < 1e-6);
        let mut doc = Document::new(DocumentKind::Board, "mesh");
        doc.pcb = Some(b.clone());
        let before = doc.clone();
        let mut history = crate::history::History::default();
        history.commit_pcb(&mut doc, moved).unwrap();
        history.undo(&mut doc);
        assert_eq!(doc, before);
        assert!(bodies(&b).is_empty());
    }
    #[test]
    fn native_body_height_standoff_bottom_and_rotation_do_not_edit_source() {
        let mut b = board();
        b.components.push(component());
        let outline = rectangle(Point::mm(10, 10), Point::mm(4, 2), 90.);
        let mut body = Item::new(
            57,
            None,
            Shape::Region {
                contour: outline.clone(),
                holes: vec![],
                cutout: false,
            },
        );
        body.component = Some(0);
        body.properties = Fields::from([
            ("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into()),
            ("OVERALLHEIGHT".into(), "3mm".into()),
            ("STANDOFFHEIGHT".into(), "0.5mm".into()),
        ]);
        b.items.push(body);
        let before = b.clone();
        let bodies = bodies(&b);
        assert_eq!(b, before);
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0].outline, outline);
        assert_eq!(
            (bodies[0].bottom_nm, bodies[0].top_nm),
            (500_000, 3_000_000)
        );
        b.components[0].layer = BOTTOM;
        let bottom = super::bodies(&b);
        assert_eq!(
            (bottom[0].bottom_nm, bottom[0].top_nm),
            (-4_600_000, -2_100_000)
        );
    }
    #[test]
    fn fallback_preserves_component_identity_and_height() {
        let mut b = board();
        let c = component();
        let id = c.id;
        b.components.push(c);
        let bodies = bodies(&b);
        assert_eq!(bodies[0].component, id);
        assert!(bodies[0].simplified);
        assert_eq!(bodies[0].top_nm, 2_000_000);
        assert!(bodies[0].outline.iter().all(|p| p.x > 0 && p.y > 0));
    }
    #[test]
    fn substrate_unions_overlapping_through_drills_and_ignores_blind_drill() {
        let mut b = board();
        for x in [10, 11] {
            b.items.push(Item::new(
                MULTI_LAYER,
                None,
                Shape::Via {
                    position: Point::mm(x, 10),
                    diameter: 4_000_000,
                    hole: 3_000_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
            ));
        }
        b.items.push(Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::mm(20, 10),
                diameter: 4_000_000,
                hole: 3_000_000,
                from_layer: TOP,
                to_layer: 2,
            },
        ));
        let copper =
            Cuts::new(&b).surface(&rectangle(Point::mm(10, 10), Point::mm(20, 2), 0.), &[]);
        assert!(copper.iter().all(|p| {
            !point_in_polygon(Point::mm(10, 10), &p.outline)
                || p.holes
                    .iter()
                    .any(|h| point_in_polygon(Point::mm(10, 10), h))
        }));
        let surfaces = substrate(&b);
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].holes.len(), 1);
        assert!(point_in_polygon(Point::mm(10, 10), &surfaces[0].holes[0]));
        assert!(!point_in_polygon(Point::mm(20, 10), &surfaces[0].holes[0]));
    }
}
