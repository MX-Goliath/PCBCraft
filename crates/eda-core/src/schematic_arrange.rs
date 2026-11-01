//! Schematic selection and arrangement in document coordinates.
use crate::schematic_edit::{Fragment, Placement};
use crate::{
    Point,
    schematic::{SchPrimitive, SchPrimitiveKind as K, Schematic},
    schematic_edit::{self as edit, Edit, Selection},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    HorizontalCenters,
    DistributeHorizontal,
    Top,
    Bottom,
    VerticalCenters,
    DistributeVertical,
    Grid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Inside,
    Outside,
    Touching,
    Line,
    Lasso,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    fn points(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut points = points.into_iter();
        let p = points.next()?;
        let mut b = Self { min: p, max: p };
        for p in points {
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        }
        Some(b)
    }
    pub fn contains(self, p: Point) -> bool {
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }
    pub fn intersects(self, b: Self) -> bool {
        self.min.x <= b.max.x
            && self.max.x >= b.min.x
            && self.min.y <= b.max.y
            && self.max.y >= b.min.y
    }
}
pub fn primitive_bounds(s: &Schematic, p: &SchPrimitive) -> Option<Bounds> {
    let mut points = match &p.kind {
        K::Blanket { points, .. } => points.clone(),
        K::CompileMask { a, b, .. } => vec![*a, *b],
        K::ParameterSet { position, .. } => vec![*position],
        K::Polyline { points, .. } | K::Bezier { points } => points.clone(),
        K::Rectangle { a, b, .. }
        | K::SheetSymbol { a, b, .. }
        | K::HarnessConnector { a, b, .. }
        | K::Image { a, b, .. }
        | K::TextFrame { a, b, .. }
        | K::Unsupported { a, b, .. } => vec![*a, *b],
        K::Arc {
            centre,
            radii,
            start,
            end,
        } => {
            let sweep = (end - start).rem_euclid(360.);
            let sweep = if sweep < 1e-9 { 360. } else { sweep };
            let mut angles = vec![*start, *end];
            angles.extend(
                [0., 90., 180., 270.]
                    .into_iter()
                    .filter(|angle| (angle - start).rem_euclid(360.) <= sweep),
            );
            angles
                .into_iter()
                .map(|a| Point {
                    x: centre.x + (a.to_radians().cos() * radii.x as f64).round() as i64,
                    y: centre.y + (a.to_radians().sin() * radii.y as f64).round() as i64,
                })
                .collect()
        }
        K::Port {
            position,
            width_nm,
            height_nm,
            style,
            ..
        } => {
            if *style >= 4 {
                vec![
                    Point {
                        x: position.x - height_nm / 2,
                        y: position.y,
                    },
                    Point {
                        x: position.x + height_nm / 2,
                        y: position.y + width_nm,
                    },
                ]
            } else {
                vec![
                    Point {
                        x: position.x,
                        y: position.y - height_nm / 2,
                    },
                    Point {
                        x: position.x + width_nm,
                        y: position.y + height_nm / 2,
                    },
                ]
            }
        }
        K::Text {
            position,
            text,
            font,
            orientation,
            ..
        } => {
            let h = s.fonts.get(*font)?.height_nm;
            let w = h.saturating_mul(text.chars().count() as i64) * 3 / 5;
            let q = if orientation % 2 == 0 {
                Point {
                    x: position.x + w,
                    y: position.y + h,
                }
            } else {
                Point {
                    x: position.x - h,
                    y: position.y + w,
                }
            };
            vec![*position, q]
        }
        _ => edit::electrical_endpoints(p).collect(),
    };
    if points.is_empty() {
        points.extend(edit::anchor(s, p.id));
    }
    Bounds::points(points)
}
pub fn object_bounds(s: &Schematic, id: Uuid) -> Option<Bounds> {
    let f = edit::extract(s, id).ok()?;
    Bounds::points(
        f.primitives
            .iter()
            .filter(|p| !p.presentation.hidden)
            .filter_map(|p| primitive_bounds(s, p))
            .flat_map(|b| [b.min, b.max])
            .chain(f.components.iter().map(|c| c.position)),
    )
}
pub fn roots(s: &Schematic, ids: &[Uuid]) -> Vec<Uuid> {
    let mut out = vec![];
    for &id in ids {
        if out.contains(&id) {
            continue;
        }
        let mut p = s.primitive(id);
        let mut owned = false;
        let mut depth = 0;
        while let Some(v) = p {
            depth += 1;
            if depth > 128 {
                owned = true;
                break;
            }
            if v.component.is_some_and(|c| ids.contains(&c))
                || v.presentation
                    .parent
                    .is_some_and(|parent| ids.contains(&parent))
            {
                owned = true;
                break;
            }
            p = v.presentation.parent.and_then(|id| s.primitive(id));
        }
        if !owned && edit::anchor(s, id).is_some() {
            out.push(id);
        }
    }
    out
}
pub fn align(s: &Schematic, ids: &[Uuid], mode: Align) -> Result<Edit, String> {
    let ids = roots(s, ids);
    if ids.is_empty() || mode != Align::Grid && ids.len() < 2 {
        return Err("Select at least two objects to align".into());
    }
    let mut objects: Vec<_> = ids
        .into_iter()
        .map(|id| {
            object_bounds(s, id)
                .map(|b| (id, b))
                .ok_or("Object has no geometry")
        })
        .collect::<Result<_, _>>()?;
    let total = Bounds::points(objects.iter().flat_map(|(_, b)| [b.min, b.max])).unwrap();
    let horizontal = matches!(
        mode,
        Align::Left | Align::Right | Align::HorizontalCenters | Align::DistributeHorizontal
    );
    let distribution = matches!(
        mode,
        Align::DistributeHorizontal | Align::DistributeVertical
    );
    if distribution {
        objects.sort_by_key(|(_, b)| {
            if horizontal {
                b.min.x + b.max.x
            } else {
                b.min.y + b.max.y
            }
        });
    }
    let centre = |b: Bounds| {
        if horizontal {
            (b.min.x + b.max.x) / 2
        } else {
            (b.min.y + b.max.y) / 2
        }
    };
    let first = centre(objects[0].1);
    let last = centre(objects[objects.len() - 1].1);
    let count = objects.len();
    let mut edits = vec![];
    for (n, (id, b)) in objects.into_iter().enumerate() {
        let anchor = edit::anchor(s, id).unwrap();
        let mut delta = Point::default();
        match mode {
            Align::Left => delta.x = total.min.x - b.min.x,
            Align::Right => delta.x = total.max.x - b.max.x,
            Align::Top => delta.y = total.max.y - b.max.y,
            Align::Bottom => delta.y = total.min.y - b.min.y,
            Align::HorizontalCenters => {
                delta.x = (total.min.x + total.max.x - b.min.x - b.max.x) / 2
            }
            Align::VerticalCenters => delta.y = (total.min.y + total.max.y - b.min.y - b.max.y) / 2,
            Align::DistributeHorizontal | Align::DistributeVertical => {
                let shift = first
                    + ((last - first) as i128 * n as i128 / (count - 1) as i128) as i64
                    - centre(b);
                if horizontal {
                    delta.x = shift;
                } else {
                    delta.y = shift;
                }
            }
            Align::Grid => {
                let snap =
                    |v: i64| ((v as f64 / s.sheet.snap_nm as f64).round() as i64) * s.sheet.snap_nm;
                delta = Point {
                    x: snap(anchor.x) - anchor.x,
                    y: snap(anchor.y) - anchor.y,
                };
            }
        }
        if delta != Point::default() {
            edits.push(Edit::TransformSelection {
                selection: Selection {
                    ids: vec![id],
                    segments: vec![],
                },
                pivot: anchor,
                quarter_turns: 0,
                delta,
                connected: false,
            });
        }
    }
    Ok(Edit::Batch(edits))
}
fn inside_polygon(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    let mut a = *polygon.last().unwrap_or(&point);
    for &b in polygon {
        if point.on_segment(a, b) {
            return true;
        }
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x as f64 + (point.y - a.y) as f64 * (b.x - a.x) as f64 / (b.y - a.y) as f64;
            if (point.x as f64) < x {
                inside = !inside;
            }
        }
        a = b;
    }
    inside
}
fn cross(a: Point, b: Point, c: Point) -> i128 {
    (b.x - a.x) as i128 * (c.y - a.y) as i128 - (b.y - a.y) as i128 * (c.x - a.x) as i128
}
fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (ab, ac, cd, ce) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    if ab == 0 && c.on_segment(a, b)
        || ac == 0 && d.on_segment(a, b)
        || cd == 0 && a.on_segment(c, d)
        || ce == 0 && b.on_segment(c, d)
    {
        return true;
    }
    ab.signum() != ac.signum() && cd.signum() != ce.signum()
}
pub fn select_region(s: &Schematic, path: &[Point], mode: Region) -> Vec<Uuid> {
    let Some(area) = Bounds::points(path.iter().copied()) else {
        return vec![];
    };
    // Build group bounds once; extracting every component separately is quadratic
    // on a large sheet with many pins.
    let mut boxes = std::collections::HashMap::<Uuid, Bounds>::new();
    let merge = |map: &mut std::collections::HashMap<Uuid, Bounds>, id, b: Bounds| {
        map.entry(id)
            .and_modify(|old| {
                old.min.x = old.min.x.min(b.min.x);
                old.min.y = old.min.y.min(b.min.y);
                old.max.x = old.max.x.max(b.max.x);
                old.max.y = old.max.y.max(b.max.y);
            })
            .or_insert(b);
    };
    for c in &s.components {
        merge(
            &mut boxes,
            c.id,
            Bounds {
                min: c.position,
                max: c.position,
            },
        );
    }
    for p in s.primitives.iter().filter(|p| !p.presentation.hidden) {
        if let Some(b) = primitive_bounds(s, p) {
            merge(&mut boxes, p.id, b);
            if let Some(owner) = p.component.or(p.presentation.parent) {
                merge(&mut boxes, owner, b);
            }
        }
    }
    let ids: Vec<_> = s
        .components
        .iter()
        .map(|c| c.id)
        .chain(
            s.primitives
                .iter()
                .filter(|p| {
                    p.component.is_none()
                        && p.presentation.parent.is_none()
                        && !p.presentation.hidden
                })
                .map(|p| p.id),
        )
        .collect();
    ids.into_iter()
        .filter(|id| {
            let Some(&b) = boxes.get(id) else {
                return false;
            };
            let corners = [
                b.min,
                Point {
                    x: b.max.x,
                    y: b.min.y,
                },
                b.max,
                Point {
                    x: b.min.x,
                    y: b.max.y,
                },
            ];
            if let Some(p) = s.primitive(*id)
                && let K::Polyline { points, closed, .. } = &p.kind
            {
                if points.is_empty() {
                    return false;
                }
                let edges: Vec<_> = points
                    .windows(2)
                    .map(|e| (e[0], e[1]))
                    .chain((*closed).then(|| (*points.last().unwrap_or(&b.min), points[0])))
                    .collect();
                return match mode {
                    Region::Inside => points.iter().all(|p| area.contains(*p)),
                    Region::Outside => !points.iter().all(|p| area.contains(*p)),
                    Region::Lasso => points.iter().all(|p| inside_polygon(*p, path)),
                    Region::Touching => {
                        edges.iter().any(|(a, b)| {
                            area.contains(*a)
                                || area.contains(*b)
                                || (0..4).any(|n| {
                                    let box_corners = [
                                        area.min,
                                        Point {
                                            x: area.max.x,
                                            y: area.min.y,
                                        },
                                        area.max,
                                        Point {
                                            x: area.min.x,
                                            y: area.max.y,
                                        },
                                    ];
                                    intersects(*a, *b, box_corners[n], box_corners[(n + 1) % 4])
                                })
                        }) || *closed && p.fill.is_some() && inside_polygon(area.min, points)
                    }
                    Region::Line => path.windows(2).any(|line| {
                        edges
                            .iter()
                            .any(|(a, b)| intersects(*a, *b, line[0], line[1]))
                            || *closed && p.fill.is_some() && inside_polygon(line[0], points)
                    }),
                };
            }
            match mode {
                Region::Inside => corners.iter().all(|p| area.contains(*p)),
                Region::Outside => !corners.iter().all(|p| area.contains(*p)),
                Region::Lasso => corners.iter().all(|p| inside_polygon(*p, path)),
                Region::Touching => area.intersects(b),
                Region::Line => path.windows(2).any(|line| {
                    b.contains(line[0])
                        || b.contains(line[1])
                        || (0..4)
                            .any(|n| intersects(line[0], line[1], corners[n], corners[(n + 1) % 4]))
                }),
            }
        })
        .collect()
}
pub(crate) fn reorder(
    s: &mut Schematic,
    ids: &[Uuid],
    target: Option<Uuid>,
    front: bool,
) -> Result<(), String> {
    let ids = roots(s, ids);
    let mut members = std::collections::HashSet::new();
    for id in ids {
        members.extend(edit::extract(s, id)?.primitives.into_iter().map(|p| p.id));
    }
    if target.is_some_and(|id| members.contains(&id)) {
        return Err("Choose a different stack target".into());
    }
    let target_members = target
        .map(|id| {
            edit::extract(s, id).map(|f| {
                f.primitives
                    .into_iter()
                    .map(|p| p.id)
                    .collect::<std::collections::HashSet<_>>()
            })
        })
        .transpose()?;
    let mut selected = vec![];
    s.primitives.retain(|p| {
        if members.contains(&p.id) {
            selected.push(p.clone());
            false
        } else {
            true
        }
    });
    let at = if let Some(target_members) = target_members {
        let mut positions = s
            .primitives
            .iter()
            .enumerate()
            .filter_map(|(n, p)| target_members.contains(&p.id).then_some(n));
        if front {
            positions
                .next_back()
                .ok_or("Stack target no longer exists")?
                + 1
        } else {
            positions.next().ok_or("Stack target no longer exists")?
        }
    } else if front {
        s.primitives.len()
    } else {
        0
    };
    s.primitives.splice(at..at, selected);
    Ok(())
}

pub fn shape(
    s: &Schematic,
    kind: Placement,
    points: &[Point],
    value: &str,
) -> Result<Fragment, String> {
    if points.len() < 2 {
        return Err("Choose the second point".into());
    }
    let start = points[0];
    let end = *points.last().unwrap();
    let a = Point {
        x: start.x.min(end.x),
        y: start.y.max(end.y),
    };
    let b = Point {
        x: start.x.max(end.x),
        y: start.y.min(end.y),
    };
    let font = s.sheet.system_font;
    let primitive = match kind {
        Placement::CompileMask => K::CompileMask{a:Point{x:a.x,y:b.y},b:Point{x:b.x,y:a.y},collapsed:false},
        Placement::Blanket => K::Blanket{points:vec![a,Point{x:b.x,y:a.y},b,Point{x:a.x,y:b.y}],collapsed:false},
        Placement::Rectangle | Placement::RoundRectangle => K::Rectangle {
            a,
            b,
            radius: if kind == Placement::RoundRectangle {
                Point {
                    x: (b.x - a.x).min(a.y - b.y) / 5,
                    y: (b.x - a.x).min(a.y - b.y) / 5,
                }
            } else {
                Point::default()
            },
        },
        Placement::SheetSymbol => K::SheetSymbol {
            a,
            b,
            designator: (1..).map(|n|format!("Sheet{n}")).find(|name|!s.primitives.iter().any(|p|matches!(&p.kind,K::SheetSymbol{designator,..} if designator.eq_ignore_ascii_case(name)))).unwrap(),
            filename: value.into(),
        },
        Placement::TextFrame | Placement::Note => K::TextFrame {
            a,
            b,
            text: if value.is_empty() {
                "Text".into()
            } else {
                value.into()
            },
            font,
            alignment: 0,
            word_wrap: true,
            show_border: true,
        },
        Placement::Arc | Placement::EllipticalArc | Placement::FullCircle | Placement::Ellipse => {
            let circle = matches!(kind, Placement::FullCircle | Placement::Arc);
            let (centre, radii) = if circle {
                let dx = (points[1].x - start.x) as f64;
                let dy = (points[1].y - start.y) as f64;
                let radius = dx.hypot(dy).round() as i64;
                (
                    start,
                    Point {
                        x: radius,
                        y: radius,
                    },
                )
            } else {
                (start,Point{x:(points[1].x-start.x).abs(),y:points.get(2).map_or((points[1].x-start.x).abs(),|p|(p.y-start.y).abs())})
            };
            let angle = |p: Point| {
                ((p.y - centre.y) as f64)
                    .atan2((p.x - centre.x) as f64)
                    .to_degrees()
                    .rem_euclid(360.0)
            };
            let (first, last) = if matches!(kind, Placement::FullCircle | Placement::Ellipse) {
                (0.0, 360.0)
            } else if kind==Placement::EllipticalArc {
                (0.0,90.0)
            } else {
                (
                    points.get(2).map_or(0.0,|p|angle(*p)),
                    if points.len() > 3 {
                        angle(points[3])
                    } else {
                        points.get(2).map_or(90.0,|p|angle(*p)+90.0)
                    },
                )
            };
            K::Arc {
                centre,
                radii,
                start: first,
                end: last,
            }
        }
        Placement::Polygon => K::Polyline {
            points: points.to_vec(),
            closed: true,
            electrical: "Graphic".into(),
        },
        Placement::Bezier => {
            if points.len() < 4 {
                return Err("Choose four Bezier control points".into());
            }
            K::Bezier {
                points: points[..(1+(points.len()-1)/3*3)].to_vec(),
            }
        }
        _ => return Err("Unsupported shape tool".into()),
    };
    let mut p = edit::primitive(primitive, None);
    p.presentation.record_type = match kind {
        Placement::Note => 209,
        Placement::TextFrame => 28,
        _ => 0,
    };
    p.color = if kind == Placement::Blanket { 255 } else { 128 };
    if kind == Placement::Blanket {
        p.line_style = 1;
        p.fill = Some(0xffffff);
    }
    if kind == Placement::CompileMask {
        p.fill = Some(0xfafafa);
    }
    p.presentation.allow_sync = true;
    p.width_nm = 101_600;
    if matches!(
        kind,
        Placement::SheetSymbol
            | Placement::Rectangle
            | Placement::RoundRectangle
            | Placement::Polygon
            | Placement::Note
    ) {
        p.fill = Some(if kind == Placement::SheetSymbol {
            0xb0ffff
        } else {
            0x80ffff
        });
    }
    let mut f = Fragment {
        primitives: vec![p],
        ..Default::default()
    };
    if kind == Placement::SheetSymbol {
        let id = f.primitives[0].id;
        let K::SheetSymbol { designator, .. } = &f.primitives[0].kind else {
            unreachable!()
        };
        let designator = designator.clone();
        for (record, text, position) in [
            (32, designator, a),
            (33, value.to_owned(), Point { x: a.x, y: b.y }),
        ] {
            let mut p = edit::primitive(
                K::Text {
                    position,
                    text: text.clone(),
                    font,
                    orientation: 0,
                    justification: 0,
                },
                None,
            );
            p.presentation.parent = Some(id);
            p.presentation.record_type = record;
            p.presentation.allow_sync = true;
            f.primitives.push(p);
        }
    }
    Ok(f)
}

/// A device reference owns entries derived from the referenced sheet's ports.
pub fn device_sheet(s: &Schematic, child: &Schematic, name: &str) -> Result<Fragment, String> {
    let ports: Vec<_> = child
        .primitives
        .iter()
        .filter(|p| p.component.is_none() && matches!(p.kind, K::Port { .. }))
        .collect();
    let height = ((ports.len() as i64 + 1) * 2_540_000).max(12_700_000);
    let mut f = shape(
        s,
        Placement::SheetSymbol,
        &[
            Point { x: 0, y: 0 },
            Point {
                x: 38_100_000,
                y: -height,
            },
        ],
        name,
    )?;
    let id = f.primitives[0].id;
    f.primitives[0].presentation.symbol_type = "Device Sheet".into();
    f.primitives[0].fill = Some(10_942_152);
    let mut designator = format!("U_{name}");
    if s.primitives.iter().any(
        |p| matches!(&p.kind,K::SheetSymbol{designator:d,..}if d.eq_ignore_ascii_case(&designator)),
    ) {
        designator=(1..).map(|n|format!("U_{name}_{n}")).find(|d|!s.primitives.iter().any(|p|matches!(&p.kind,K::SheetSymbol{designator,..}if designator.eq_ignore_ascii_case(d)))).unwrap();
    }
    if let K::SheetSymbol { designator: d, .. } = &mut f.primitives[0].kind {
        *d = designator.clone();
    }
    for p in f.primitives.iter_mut().skip(1) {
        if let K::Text { position, text, .. } = &mut p.kind {
            if p.presentation.record_type == 32 {
                *text = designator.clone();
                *position = Point { x: 0, y: 2_540_000 };
            } else {
                *position = Point::default();
            }
        }
    }
    for (n, port) in ports.into_iter().enumerate() {
        let K::Port { name, io_type, .. } = &port.kind else {
            unreachable!()
        };
        let side = u32::from(*io_type == 1);
        let mut entry = edit::primitive(
            K::SheetEntry {
                position: Point {
                    x: if side == 0 { 0 } else { 38_100_000 },
                    y: -(n as i64 + 1) * 2_540_000,
                },
                side,
                io_type: *io_type,
                name: name.clone(),
                font: s.sheet.system_font,
                text_color: 0x800000,
                arrow_kind: "Block & Triangle".into(),
            },
            None,
        );
        entry.presentation.record_type = 16;
        entry.presentation.parent = Some(id);
        entry.presentation.name = name.clone();
        entry.presentation.harness_type = port.presentation.harness_type.clone();
        entry.presentation.allow_sync = true;
        entry.color = 0x800000;
        entry.fill = Some(8_454_143);
        entry.width_nm = 101_600;
        f.primitives.push(entry);
    }
    Ok(f)
}

pub fn flip_symbols(s: &Schematic, ids: &[Uuid], vertical: bool) -> Result<Edit, String> {
    let mut edits = vec![];
    for symbol in s
        .primitives
        .iter()
        .filter(|p| ids.contains(&p.id) && matches!(p.kind, K::SheetSymbol { .. }))
    {
        let K::SheetSymbol { a, b, .. } = symbol.kind else {
            unreachable!()
        };
        for child in s
            .primitives
            .iter()
            .filter(|p| p.presentation.parent == Some(symbol.id))
        {
            let mut p = child.clone();
            let mirror = |at: &mut Point| {
                if vertical {
                    at.y = a.y + b.y - at.y;
                } else {
                    at.x = a.x + b.x - at.x;
                }
            };
            match &mut p.kind {
                K::SheetEntry { position, side, .. } => {
                    mirror(position);
                    if vertical && *side >= 2 {
                        *side = 5 - *side;
                    } else if !vertical && *side <= 1 {
                        *side = 1 - *side;
                    }
                }
                K::Text { position, .. } => mirror(position),
                _ => continue,
            }
            edits.push(Edit::PrimitiveProperties {
                id: p.id,
                value: Box::new(p),
            });
        }
    }
    Ok(Edit::Batch(edits))
}
pub fn entry_actions(s: &Schematic, ids: &[Uuid], op: u8) -> Result<Edit, String> {
    let mut entries: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| {
            matches!(p.kind, K::SheetEntry { .. })
                && (ids.contains(&p.id)
                    || op == 0 && p.presentation.parent.is_some_and(|id| ids.contains(&id)))
        })
        .cloned()
        .collect();
    let old = entries.clone();
    for p in &mut entries {
        let K::SheetEntry {
            position,
            side,
            io_type,
            ..
        } = &mut p.kind
        else {
            unreachable!()
        };
        match op {
            0 | 2 => *io_type = (*io_type + 1) % 4,
            1 => {
                let mut same: Vec<_> = old
                    .iter()
                    .filter(|e| {
                        e.presentation.parent == p.presentation.parent
                            && matches!(e.kind,K::SheetEntry{side:s,..}if s==*side)
                    })
                    .collect();
                same.sort_by_key(|p| {
                    if let K::SheetEntry { position, side, .. } = p.kind {
                        if side <= 1 { -position.y } else { position.x }
                    } else {
                        0
                    }
                });
                let index = same.iter().position(|e| e.id == p.id).unwrap();
                if let K::SheetEntry { position: at, .. } = same[same.len() - 1 - index].kind {
                    *position = at;
                }
            }
            3 => {
                let parent = s
                    .primitive(p.presentation.parent.ok_or("Missing Sheet Symbol")?)
                    .ok_or("Missing Sheet Symbol")?;
                if let K::SheetSymbol { a, b, .. } = parent.kind {
                    if *side <= 1 {
                        *side = 1 - *side;
                        position.x = if *side == 0 { a.x } else { b.x };
                    } else {
                        *side = 5 - *side;
                        position.y = if *side == 2 { a.y } else { b.y };
                    }
                }
            }
            _ => return Err("Unknown sheet entry operation".into()),
        }
    }
    Ok(Edit::Batch(
        entries
            .into_iter()
            .map(|p| Edit::PrimitiveProperties {
                id: p.id,
                value: Box::new(p),
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, DocumentKind, History};
    #[test]
    fn align_distribute_reorder_preserve_owners_and_undo() {
        let mut d = Document::new(DocumentKind::Schematic, "Arrange");
        let s = d.schematic.as_mut().unwrap();
        let mut ids = vec![];
        for (x, y) in [(0, 0), (20, 10), (50, 30)] {
            let p = edit::primitive(
                K::Rectangle {
                    a: Point::mm(x, y),
                    b: Point::mm(x + 5, y + 5),
                    radius: Point::default(),
                },
                None,
            );
            ids.push(p.id);
            s.primitives.push(p);
        }
        let before = d.clone();
        let command = align(d.schematic.as_ref().unwrap(), &ids, Align::Top).unwrap();
        let mut h = History::default();
        h.schematic_edit(
            &mut d,
            edit::Request {
                document: before.id,
                expected_revision: 0,
                edit: command,
            },
        )
        .unwrap();
        for id in &ids {
            assert_eq!(
                object_bounds(d.schematic.as_ref().unwrap(), *id)
                    .unwrap()
                    .max
                    .y,
                35_000_000
            );
        }
        h.undo(&mut d);
        assert_eq!(d, before);
        let command = align(
            d.schematic.as_ref().unwrap(),
            &ids,
            Align::DistributeHorizontal,
        )
        .unwrap();
        d = edit::preview(&d, &command).unwrap();
        assert_eq!(
            object_bounds(d.schematic.as_ref().unwrap(), ids[1])
                .unwrap()
                .min
                .x,
            25_000_000
        );
        let original = d.clone();
        d = edit::preview(
            &d,
            &Edit::Reorder {
                ids: vec![ids[0]],
                target: Some(ids[1]),
                front: true,
            },
        )
        .unwrap();
        assert_eq!(d.schematic.as_ref().unwrap().primitives[1].id, ids[0]);
        assert_eq!(
            d.schematic.as_ref().unwrap().primitives[1].kind,
            original.schematic.as_ref().unwrap().primitives[0].kind
        );
    }
    #[test]
    fn rectangle_line_lasso_and_outside_selection_have_distinct_meanings() {
        let mut s = Schematic::blank();
        for x in [0, 10, 20] {
            s.primitives.push(edit::primitive(
                K::Rectangle {
                    a: Point::mm(x, 0),
                    b: Point::mm(x + 5, 5),
                    radius: Point::default(),
                },
                None,
            ));
        }
        let area = [Point::mm(-1, -1), Point::mm(12, 6)];
        assert_eq!(select_region(&s, &area, Region::Inside).len(), 1);
        assert_eq!(select_region(&s, &area, Region::Touching).len(), 2);
        assert_eq!(select_region(&s, &area, Region::Outside).len(), 2);
        assert_eq!(
            select_region(&s, &[Point::mm(-1, 2), Point::mm(30, 2)], Region::Line).len(),
            3
        );
        assert_eq!(
            select_region(
                &s,
                &[
                    Point::mm(-1, -1),
                    Point::mm(6, -1),
                    Point::mm(6, 6),
                    Point::mm(-1, 6)
                ],
                Region::Lasso
            )
            .len(),
            1
        );
    }

    #[test]
    fn sheet_entry_operations_and_relative_stack_preserve_whole_owned_groups() {
        let mut d = Document::new(DocumentKind::Schematic, "Owned");
        let fragment = shape(
            d.schematic.as_ref().unwrap(),
            Placement::SheetSymbol,
            &[Point::mm(10, 30), Point::mm(30, 10)],
            "Child.SchDoc",
        )
        .unwrap();
        let symbol = fragment.primitives[0].id;
        d = edit::preview(&d, &Edit::Insert(fragment)).unwrap();
        let mut entries = vec![];
        for y in [25, 15, 20] {
            let mut p = edit::primitive(
                K::SheetEntry {
                    position: Point::mm(10, y),
                    side: 0,
                    io_type: 1,
                    name: format!("E{y}"),
                    font: 0,
                    text_color: 128,
                    arrow_kind: "Block & Triangle".into(),
                },
                None,
            );
            p.presentation.parent = Some(symbol);
            entries.push(p.id);
            d.schematic.as_mut().unwrap().primitives.push(p);
        }
        let before = d.clone();
        let command = entry_actions(d.schematic.as_ref().unwrap(), &entries, 1).unwrap();
        d = edit::preview(&d, &command).unwrap();
        assert!(
            matches!(d.schematic.as_ref().unwrap().primitive(entries[0]).unwrap().kind,K::SheetEntry{position,..} if position==Point::mm(10,15))
        );
        let command = flip_symbols(d.schematic.as_ref().unwrap(), &[symbol], false).unwrap();
        d = edit::preview(&d, &command).unwrap();
        for id in &entries {
            let p = d.schematic.as_ref().unwrap().primitive(*id).unwrap();
            assert_eq!(p.presentation.parent, Some(symbol));
            assert!(matches!(p.kind,K::SheetEntry{position,side:1,..} if position.x==30_000_000));
        }
        let command = entry_actions(d.schematic.as_ref().unwrap(), &[symbol], 0).unwrap();
        d = edit::preview(&d, &command).unwrap();
        assert!(entries.iter().all(|id| matches!(
            d.schematic.as_ref().unwrap().primitive(*id).unwrap().kind,
            K::SheetEntry { io_type: 2, .. }
        )));
        let p = edit::primitive(
            K::NoErc {
                position: Point::mm(40, 40),
            },
            None,
        );
        let moved = p.id;
        d.schematic.as_mut().unwrap().primitives.insert(0, p);
        d = edit::preview(
            &d,
            &Edit::Reorder {
                ids: vec![moved],
                target: Some(symbol),
                front: true,
            },
        )
        .unwrap();
        assert_eq!(
            d.schematic.as_ref().unwrap().primitives.last().unwrap().id,
            moved
        );
        let group = edit::extract(d.schematic.as_ref().unwrap(), symbol).unwrap();
        assert_eq!(group.primitives.len(), 6);
        assert_eq!(
            before.schematic.as_ref().unwrap().primitives.len() + 1,
            d.schematic.as_ref().unwrap().primitives.len()
        );
    }
}
