//! Schematic area directives. Geometry and electrical scope use document units.
use crate::{
    Point,
    schematic::{CompileMask, SchPrimitiveKind as K, Schematic},
};
use std::collections::BTreeSet;
use uuid::Uuid;

pub fn edges(points: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
}
pub fn contains(points: &[Point], p: Point, boundary: bool) -> bool {
    let mut inside = false;
    for (a, b) in edges(points) {
        if p.on_segment(a, b) {
            return boundary;
        }
        if (a.y > p.y) != (b.y > p.y) {
            let cross = (b.x as i128 - a.x as i128) * (p.y as i128 - a.y as i128)
                - (p.x as i128 - a.x as i128) * (b.y as i128 - a.y as i128);
            if (cross > 0) == (b.y > a.y) {
                inside = !inside;
            }
        }
    }
    inside
}
/// Check every interval split by the polygon boundary, including concave notches.
fn covers_segment(points: &[Point], a: Point, b: Point) -> bool {
    if !contains(points, a, true) || !contains(points, b, true) {
        return false;
    }
    if a == b {
        return true;
    }
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let mut cuts = vec![0.0, 1.0];
    for (c, d) in edges(points) {
        let (ex, ey) = ((d.x - c.x) as f64, (d.y - c.y) as f64);
        let cross = dx * ey - dy * ex;
        if cross != 0.0 {
            let (cx, cy) = ((c.x - a.x) as f64, (c.y - a.y) as f64);
            let t = (cx * ey - cy * ex) / cross;
            let u = (cx * dy - cy * dx) / cross;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                cuts.push(t);
            }
        } else {
            for p in [c, d] {
                if p.on_segment(a, b) {
                    cuts.push(
                        ((p.x - a.x) as f64 * dx + (p.y - a.y) as f64 * dy) / (dx * dx + dy * dy),
                    );
                }
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.windows(2).all(|v| {
        let t = (v[0] + v[1]) * 0.5;
        contains(
            points,
            Point {
                x: (a.x as f64 + t * dx).round() as i64,
                y: (a.y as f64 + t * dy).round() as i64,
            },
            true,
        )
    })
}
pub fn valid_polygon(points: &[Point]) -> bool {
    if !(3..=10000).contains(&points.len()) || !points.iter().all(|p| p.valid()) {
        return false;
    }
    let area: i128 = edges(points)
        .map(|(a, b)| a.x as i128 * b.y as i128 - b.x as i128 * a.y as i128)
        .sum();
    if area == 0 {
        return false;
    }
    let cross = |a: Point, b: Point, c: Point| {
        (b.x as i128 - a.x as i128) * (c.y as i128 - a.y as i128)
            - (b.y as i128 - a.y as i128) * (c.x as i128 - a.x as i128)
    };
    for (i, (a, b)) in edges(points).enumerate() {
        if a == b {
            return false;
        }
        for (j, (c, d)) in edges(points).enumerate().skip(i + 1) {
            if j == i + 1 || (i == 0 && j == points.len() - 1) {
                continue;
            }
            if a.on_segment(c, d)
                || b.on_segment(c, d)
                || c.on_segment(a, b)
                || d.on_segment(a, b)
                || (cross(a, b, c).signum() != cross(a, b, d).signum()
                    && cross(c, d, a).signum() != cross(c, d, b).signum())
            {
                return false;
            }
        }
    }
    true
}
pub fn rectangle(a: Point, b: Point) -> Vec<Point> {
    vec![a, Point { x: a.x, y: b.y }, b, Point { x: b.x, y: a.y }]
}
/// Area transformations affect only the directive, never the circuitry it covers.
pub fn area_fragment(fragment: &crate::schematic_edit::Fragment) -> bool {
    fragment.components.is_empty()
        && !fragment.primitives.is_empty()
        && fragment
            .primitives
            .iter()
            .all(|p| matches!(p.kind, K::Blanket { .. } | K::CompileMask { .. }))
}
/// Reflect X coordinates first, then rotate and translate in document coordinates.
/// Keeping this order explicit lets the editor compose X/Y and rotation shortcuts.
pub fn transform_areas(
    fragment: &crate::schematic_edit::Fragment,
    pivot: Point,
    turns: u32,
    reflected: bool,
    delta: Point,
) -> Result<crate::schematic_edit::Fragment, String> {
    if !area_fragment(fragment) {
        return Err("Select only Blanket or Compile Mask directives to reflect them".into());
    }
    let mut result = fragment.clone();
    if reflected {
        let reflect = |p: &mut Point| -> Result<(), String> {
            p.x = i64::try_from(2 * pivot.x as i128 - p.x as i128)
                .map_err(|_| "Coordinate overflow")?;
            if !p.valid() {
                return Err("Object exceeds the document coordinate range".into());
            }
            Ok(())
        };
        for p in &mut result.primitives {
            match &mut p.kind {
                K::Blanket { points, .. } => {
                    for p in points {
                        reflect(p)?;
                    }
                }
                K::CompileMask { a, b, .. } => {
                    reflect(a)?;
                    reflect(b)?;
                }
                _ => unreachable!(),
            }
        }
    }
    result.transformed(pivot, turns, delta)
}
/// The old JSON field remains readable. Typed primitives are authoritative.
pub fn masks(s: &Schematic) -> Vec<CompileMask> {
    let typed: Vec<_> = s
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::CompileMask { a, b, collapsed } = p.kind {
                Some(CompileMask { a, b, collapsed })
            } else {
                None
            }
        })
        .collect();
    if typed.is_empty() {
        s.compile_masks.clone()
    } else {
        typed
    }
}
pub fn refresh_masks(s: &mut Schematic) {
    s.compile_masks = s
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::CompileMask { a, b, collapsed } = p.kind {
                Some(CompileMask { a, b, collapsed })
            } else {
                None
            }
        })
        .collect();
}
#[derive(Default, Debug)]
pub struct Scope {
    pub components: BTreeSet<Uuid>,
    pub primitives: BTreeSet<Uuid>,
}
pub fn handles(kind: &K) -> Vec<Point> {
    let points = match kind {
        K::Blanket {
            points,
            collapsed: false,
        } => points.clone(),
        K::CompileMask {
            a,
            b,
            collapsed: false,
        } => rectangle(*a, *b),
        _ => return vec![],
    };
    points
        .iter()
        .copied()
        .chain(edges(&points).map(|(a, b)| Point {
            x: (a.x + b.x) / 2,
            y: (a.y + b.y) / 2,
        }))
        .collect()
}
pub fn resize(kind: &mut K, handle: usize, delta: Point) -> Result<(), String> {
    let mut points = match kind {
        K::Blanket {
            points,
            collapsed: false,
        } => points.clone(),
        K::CompileMask {
            a,
            b,
            collapsed: false,
        } => rectangle(*a, *b),
        _ => return Err("Collapsed area directives can only be moved".into()),
    };
    let n = points.len();
    if handle >= 2 * n {
        return Err("Invalid area handle".into());
    }
    let move_point = |p: &mut Point| {
        p.x += delta.x;
        p.y += delta.y;
    };
    if matches!(kind, K::CompileMask { .. })
        || (points.len() == 4 && edges(&points).all(|(a, b)| a.x == b.x || a.y == b.y))
    {
        // Rectangle corners resize both axes; segment centres move one edge.
        let original = points.clone();
        if handle < n {
            let corner = original[handle];
            for p in &mut points {
                if p.x == corner.x {
                    p.x += delta.x;
                }
                if p.y == corner.y {
                    p.y += delta.y;
                }
            }
        } else {
            let i = handle - n;
            let a = original[i];
            let b = original[(i + 1) % n];
            for p in &mut points {
                if a.x == b.x && p.x == a.x {
                    p.x += delta.x;
                }
                if a.y == b.y && p.y == a.y {
                    p.y += delta.y;
                }
            }
        }
    } else if handle < n {
        move_point(&mut points[handle]);
    } else {
        let i = handle - n;
        move_point(&mut points[i]);
        move_point(&mut points[(i + 1) % n]);
    }
    if !valid_polygon(&points) {
        return Err("Blanket outline must be nonzero and cannot intersect itself".into());
    }
    match kind {
        K::Blanket { points: p, .. } => *p = points,
        K::CompileMask { a, b, .. } => {
            *a = points[0];
            *b = points[2];
        }
        _ => unreachable!(),
    }
    Ok(())
}
/// Components must fit entirely. Nets are selected by a hotspot or wire segment
/// with a vertex inside; an interior-only crossing does not select a net.
pub fn blanket_scope(s: &Schematic, points: &[Point]) -> Scope {
    let excluded = crate::schematic_compile::exclusions(s);
    let mut result = Scope::default();
    let mut outside = std::collections::HashSet::new();
    for primitive in &s.primitives {
        if let Some(id) = primitive.component
            && !matches!(primitive.kind, K::Text { .. })
        {
            let geometry = crate::schematic_compile::geometry(primitive);
            if !geometry.iter().all(|p| contains(points, *p, true))
                || !edges(&geometry).all(|(a, b)| covers_segment(points, a, b))
            {
                outside.insert(id);
            }
        }
    }
    for c in &s.components {
        if !excluded.components.contains(&c.id)
            && !outside.contains(&c.id)
            && contains(points, c.position, true)
        {
            result.components.insert(c.id);
        }
    }
    for p in &s.primitives {
        if excluded.primitives.contains(&p.id)
            || p.component
                .is_some_and(|c| excluded.components.contains(&c))
        {
            continue;
        }
        let inside = if let K::Polyline {
            points: path,
            electrical,
            ..
        } = &p.kind
        {
            matches!(electrical.as_str(), "Wire" | "Bus" | "Harness")
                && path.iter().any(|p| contains(points, *p, true))
        } else {
            crate::schematic_edit::electrical_endpoints(p).any(|p| contains(points, p, true))
        };
        if inside {
            result.primitives.insert(p.id);
        }
    }
    result
}

#[derive(Default, Debug)]
pub struct CompiledDirectives {
    pub classes: std::collections::BTreeMap<(u32, String), BTreeSet<String>>,
    pub component_parameters:
        std::collections::BTreeMap<(Uuid, Uuid), std::collections::BTreeMap<String, String>>,
    pub differential_nets: BTreeSet<String>,
    pub no_erc_nets: BTreeSet<String>,
    pub rules: Vec<ScopedRule>,
}
#[derive(Debug)]
pub struct ScopedRule {
    pub id: Uuid,
    pub fields: std::collections::BTreeMap<String, String>,
    pub nets: BTreeSet<String>,
    pub class: Option<String>,
}
/// Parameter Sets apply only through an actual attachment, never by mere area
/// overlap. Disabled Blankets contribute no scope; active overlapping ones union.
pub fn compile(
    sheets: &[&crate::Document],
    nets: &crate::design_sync::Netlist,
    precision: i64,
) -> CompiledDirectives {
    let mut out = CompiledDirectives::default();
    for doc in sheets {
        let Some(s) = &doc.schematic else { continue };
        let excluded = crate::schematic_compile::exclusions(s);
        let blankets: Vec<_> = s
            .primitives
            .iter()
            .filter(|p| !excluded.primitives.contains(&p.id))
            .filter_map(|p| {
                if let K::Blanket {
                    points,
                    collapsed: false,
                } = &p.kind
                {
                    Some((points, blanket_scope(s, points)))
                } else {
                    None
                }
            })
            .collect();
        for p in &s.primitives {
            if excluded.primitives.contains(&p.id) {
                continue;
            }
            let (position, parameters) = match &p.kind {
                K::ParameterSet {
                    position,
                    parameters,
                    ..
                } => (*position, Some(parameters)),
                K::NoErc { position } => (*position, None),
                _ => continue,
            };
            let mut scope = Scope::default();
            for (points, covered) in &blankets {
                if edges(points).any(|(a, b)| near_segment(position, a, b, precision)) {
                    scope.components.extend(&covered.components);
                    scope.primitives.extend(&covered.primitives);
                }
            }
            // Also support an ordinary Parameter Set attached directly to a net.
            for primitive in &s.primitives {
                let attached = match &primitive.kind {
                    K::Polyline {
                        points, electrical, ..
                    } if electrical == "Wire" => points
                        .windows(2)
                        .any(|e| near_segment(position, e[0], e[1], precision)),
                    _ => crate::schematic_edit::electrical_endpoints(primitive)
                        .any(|at| near_segment(position, at, at, precision)),
                };
                if attached && !excluded.primitives.contains(&primitive.id) {
                    scope.primitives.insert(primitive.id);
                }
            }
            let names: BTreeSet<_> = scope
                .primitives
                .iter()
                .filter_map(|id| nets.primitive_nets.get(&(doc.id, *id)).cloned())
                .collect();
            let Some(parameters) = parameters else {
                out.no_erc_nets.extend(names);
                continue;
            };
            if let K::ParameterSet { rules, .. } = &p.kind {
                let mut class = parameters
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("ClassName"))
                    .map(|(_, v)| v.clone());
                if class.is_none() {
                    for (points, _) in &blankets {
                        if !edges(points).any(|(a, b)| near_segment(position, a, b, precision)) {
                            continue;
                        }
                        class = s
                            .primitives
                            .iter()
                            .filter(|other| !excluded.primitives.contains(&other.id))
                            .find_map(|other| {
                                if let K::ParameterSet {
                                    position,
                                    parameters,
                                    ..
                                } = &other.kind
                                    && edges(points)
                                        .any(|(a, b)| near_segment(*position, a, b, precision))
                                {
                                    parameters
                                        .iter()
                                        .find(|(k, v)| {
                                            k.eq_ignore_ascii_case("ClassName")
                                                && !v.trim().is_empty()
                                        })
                                        .map(|(_, v)| v.clone())
                                } else {
                                    None
                                }
                            });
                        if class.is_some() {
                            break;
                        }
                    }
                }
                for (index, rule) in rules.iter().enumerate() {
                    out.rules.push(ScopedRule {
                        id: Uuid::new_v5(&p.id, format!("Rule{index}").as_bytes()),
                        fields: rule.clone(),
                        nets: names.clone(),
                        class: class.clone(),
                    });
                }
            }

            for (key, value) in parameters {
                if value.trim().is_empty() {
                    continue;
                }
                if key.eq_ignore_ascii_case("ClassName") {
                    out.classes
                        .entry((0, value.clone()))
                        .or_default()
                        .extend(names.iter().cloned());
                } else if key.eq_ignore_ascii_case("CompClassName") {
                    out.classes.entry((1, value.clone())).or_default().extend(
                        s.components
                            .iter()
                            .filter(|c| scope.components.contains(&c.id))
                            .map(|c| crate::design_sync::logical_designator(c).to_owned()),
                    );
                } else if key.eq_ignore_ascii_case("DifferentialPair") {
                    if matches!(value.to_ascii_lowercase().as_str(), "true" | "t" | "1") {
                        out.differential_nets.extend(names.iter().cloned());
                    }
                } else {
                    for id in &scope.components {
                        out.component_parameters
                            .entry((doc.id, *id))
                            .or_default()
                            .insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }
    out
}
fn near_segment(p: Point, a: Point, b: Point, tolerance: i64) -> bool {
    if tolerance <= 0 {
        return p.on_segment(a, b);
    }
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let t = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        (((p.x - a.x) as f64 * dx + (p.y - a.y) as f64 * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0)
    };
    ((p.x - a.x) as f64 - t * dx).hypot((p.y - a.y) as f64 - t * dy) <= tolerance as f64
}

/// Canonical defaults shared by placement, previews and native round trips.
pub fn primitive(kind: K) -> crate::schematic::SchPrimitive {
    let mut p = crate::schematic_edit::primitive(kind, None);
    p.presentation.allow_sync = true;
    match &p.kind {
        K::Blanket { .. } => {
            p.color = 255;
            p.fill = Some(14_476_031);
            p.width_nm = 508_000;
            p.presentation.record_type = 225;
        }
        K::CompileMask { .. } => {
            p.color = 128;
            p.fill = Some(0xffffff);
            p.width_nm = 101_600;
            p.presentation.record_type = 211;
        }
        K::NoErc { .. } => {
            p.color = 128;
            p.width_nm = 101_600;
            p.presentation.record_type = 22;
        }
        K::ParameterSet { name, .. } => {
            p.color = 255;
            p.presentation.record_type = 43;
            p.presentation.name = name.clone();
        }
        _ => {}
    }
    p
}
#[cfg(test)]
#[path = "schematic_directives_tests.rs"]
mod tests;
