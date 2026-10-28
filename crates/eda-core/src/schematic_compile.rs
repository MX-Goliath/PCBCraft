//! Compile masks exclude whole electrical objects, without changing the drawing.
use crate::{
    schematic::{SchPrimitive, SchPrimitiveKind as K, Schematic},
    schematic_edit,
};
use eda_geom::Point;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Default)]
pub struct Exclusions {
    pub components: HashSet<Uuid>,
    pub primitives: HashSet<Uuid>,
    pub records: HashSet<u32>,
}
fn geometry(p: &SchPrimitive) -> Vec<Point> {
    match &p.kind {
        K::Polyline { points, .. } | K::Bezier { points } => points.clone(),
        K::Rectangle { a, b, .. }
        | K::SheetSymbol { a, b, .. }
        | K::TextFrame { a, b, .. }
        | K::Image { a, b, .. }
        | K::Unsupported { a, b, .. } => vec![*a, *b],
        K::Arc { centre, radii, .. } => vec![
            Point {
                x: centre.x - radii.x,
                y: centre.y - radii.y,
            },
            Point {
                x: centre.x + radii.x,
                y: centre.y + radii.y,
            },
        ],
        K::Pin { position, .. } => std::iter::once(*position)
            .chain(schematic_edit::electrical_endpoints(p))
            .collect(),
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
        K::Text { position, .. }
        | K::PowerPort { position, .. }
        | K::SheetEntry { position, .. }
        | K::Junction { position, .. }
        | K::NoErc { position }
        | K::Ieee { position, .. } => vec![*position],
    }
}
pub fn exclusions(s: &Schematic) -> Exclusions {
    let mut out = Exclusions::default();
    if s.compile_masks.iter().all(|m| m.collapsed) {
        return out;
    }
    let mut component_points: HashMap<_, Vec<_>> = s
        .components
        .iter()
        .map(|c| (c.id, vec![c.position]))
        .collect();
    let mut free_points = vec![];
    for p in s.primitives.iter().filter(|p| !p.presentation.hidden) {
        if let Some(c) = p.component {
            if !matches!(p.kind, K::Text { .. })
                && let Some(points) = component_points.get_mut(&c)
            {
                points.extend(geometry(p));
            }
        } else {
            free_points.push((p, geometry(p)));
        }
    }
    for mask in s.compile_masks.iter().filter(|m| !m.collapsed) {
        let contains = |p: &Point| {
            p.x > mask.a.x.min(mask.b.x)
                && p.x < mask.a.x.max(mask.b.x)
                && p.y > mask.a.y.min(mask.b.y)
                && p.y < mask.a.y.max(mask.b.y)
        };
        for c in &s.components {
            if component_points[&c.id].iter().all(contains) {
                out.components.insert(c.id);
                out.records.insert(c.source_index);
            }
        }
        for (p, points) in &free_points {
            if !points.is_empty() && points.iter().all(contains) {
                out.primitives.insert(p.id);
                out.records.insert(p.source_index);
            }
        }
    }
    for p in s.primitives.iter().chain(&s.parameter_objects) {
        if p.component.is_some_and(|c| out.components.contains(&c)) {
            out.primitives.insert(p.id);
            out.records.insert(p.source_index);
        }
    }
    // Hidden pins and future record-only objects still inherit masked ownership.
    for r in s.records.iter() {
        let mut owner = r.index;
        for _ in 0..64 {
            if out.records.contains(&owner) {
                out.records.insert(r.index);
                break;
            }
            let Some(next) = s
                .records
                .get(owner as usize)
                .and_then(|r| r.fields.get("OWNERINDEX"))
                .and_then(|v| v.parse::<u32>().ok())
            else {
                break;
            };
            if next == owner || next == 0 {
                break;
            }
            owner = next;
        }
    }
    out
}
