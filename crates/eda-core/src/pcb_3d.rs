//! Read-only physical geometry for the PCB viewer; never rewrites native models.
use crate::pcb::*;
use eda_geom::Point;
use geo::{BooleanOps, Coord, LineString, Polygon, unary_union};
use uuid::Uuid;

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
            i.component == Some(c.index) && i.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        }) {
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
            && !item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
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
    polygons: Vec<(Bounds, Polygon<f64>)>,
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
                && !item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
                && contour.len() >= 3
            {
                polygons.push((Bounds::points(contour), polygon(contour)));
            }
        }
        Self { polygons }
    }
    pub fn surface(&self, outer: &[Point], holes: &[Vec<Point>]) -> Vec<StackSurface> {
        let bounds = Bounds::points(outer);
        let cuts: Vec<_> = self
            .polygons
            .iter()
            .filter(|(b, _)| {
                !bounds.empty
                    && b.min.x <= bounds.max.x
                    && b.max.x >= bounds.min.x
                    && b.min.y <= bounds.max.y
                    && b.max.y >= bounds.min.y
            })
            .map(|(_, p)| p)
            .collect();
        if cuts.is_empty() {
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
        let result = p.difference(&unary_union(cuts));
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
