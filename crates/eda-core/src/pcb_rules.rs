//! Priority-ordered PCB constraints and explicit unsupported-rule diagnostics.
use crate::pcb::*;
use eda_geom::Point;
use uuid::Uuid;
#[path = "pcb_drc.rs"]
mod drc;
pub use drc::{CheckOptions, Report, Violation, check, check_with_options};
#[path = "pcb_rules_query.rs"]
mod membership;
#[path = "pcb_rules_polygon.rs"]
mod polygon;
pub use polygon::{
    PolygonConnection, advanced_connection, connection_constraints, polygon_connection,
    set_advanced_connection,
};
pub fn text<'a>(r: &'a Fields, key: &str) -> &'a str {
    r.get(key).map_or("", String::as_str)
}
pub fn enabled(r: &Fields) -> bool {
    !matches!(
        text(r, "ENABLED").to_ascii_uppercase().as_str(),
        "FALSE" | "0"
    )
}
pub fn dimension(r: &Fields, key: &str) -> Result<i64, String> {
    dimension_nm(text(r, key)).map_err(|e| format!("{}: {key}: {e}", text(r, "NAME")))
}
pub const KINDS: &[(&str, &str, &str)] = &[
    ("Electrical", "Clearance", "Clearance"),
    ("Electrical", "Short-Circuit", "ShortCircuit"),
    ("Electrical", "Un-Routed Net", "UnRoutedNet"),
    ("Routing", "Width", "Width"),
    ("Routing", "Routing Layers", "RoutingLayers"),
    ("Routing", "Routing Via Style", "RoutingVias"),
    ("Manufacturing", "Hole Size", "HoleSize"),
    (
        "Manufacturing",
        "Hole To Hole Clearance",
        "HoleToHoleClearance",
    ),
    (
        "Manufacturing",
        "Minimum Annular Ring",
        "MinimumAnnularRing",
    ),
    (
        "Manufacturing",
        "Board Outline Clearance",
        "BoardOutlineClearance",
    ),
    ("Electrical", "Modified Polygon", "UnpouredPolygon"),
    ("Mask", "Solder Mask Expansion", "SolderMaskExpansion"),
    ("Mask", "Paste Mask Expansion", "PasteMaskExpansion"),
    ("Plane", "Polygon Connect Style", "PolygonConnect"),
    (
        "Manufacturing",
        "Minimum Solder Mask Sliver",
        "MinimumSolderMaskSliver",
    ),
    (
        "Manufacturing",
        "Silk To Solder Mask Clearance",
        "SilkToSolderMaskClearance",
    ),
];
pub fn supported(kind: &str) -> bool {
    KINDS.iter().any(|(_, _, k)| *k == kind)
}
pub fn fields(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "MinimumSolderMaskSliver" => &[("MINSOLDERMASKWIDTH", "Minimum Solder Mask Sliver")],
        "SilkToSolderMaskClearance" => &[(
            "MINSILKSCREENTOMASKGAP",
            "Silkscreen To Object Minimum Clearance",
        )],
        "MinimumAnnularRing" => &[("MINIMUMRING", "Minimum Annular Ring")],
        "SolderMaskExpansion" | "PasteMaskExpansion" => &[("EXPANSION", "Expansion")],
        "Clearance" | "HoleToHoleClearance" | "BoardOutlineClearance" => {
            &[("GAP", "Minimum Clearance")]
        }
        "Width" => &[
            ("MINLIMIT", "Min Width"),
            ("PREFEREDWIDTH", "Preferred Width"),
            ("MAXLIMIT", "Max Width"),
        ],
        "RoutingVias" => &[
            ("MINWIDTH", "Min Diameter"),
            ("WIDTH", "Preferred Diameter"),
            ("MAXWIDTH", "Max Diameter"),
            ("MINHOLEWIDTH", "Min Hole"),
            ("HOLEWIDTH", "Preferred Hole"),
            ("MAXHOLEWIDTH", "Max Hole"),
        ],
        "HoleSize" => &[("MINLIMIT", "Min Hole Size"), ("MAXLIMIT", "Max Hole Size")],
        _ => &[],
    }
}
pub fn query(board: &Board, item: &Item, expression: &str) -> Result<bool, String> {
    scope(board, item, expression, 0).ok_or_else(|| format!("Unsupported query: {expression}"))
}
/// Lowest numeric priority wins, ties retain the file order. Pair queries are symmetric.
pub fn resolve<'a>(
    board: &'a Board,
    kind: &str,
    a: &Item,
    b: Option<&Item>,
) -> Result<Option<(usize, &'a Fields)>, String> {
    let mut rules: Vec<_> = board
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| enabled(r) && text(r, "RULEKIND") == kind)
        .collect();
    rules.sort_by_key(|(i, r)| (text(r, "PRIORITY").parse::<u32>().unwrap_or(u32::MAX), *i));
    for (index, r) in rules {
        let first = if text(r, "SCOPE1EXPRESSION").is_empty() {
            "All"
        } else {
            text(r, "SCOPE1EXPRESSION")
        };
        let second = if text(r, "SCOPE2EXPRESSION").is_empty() {
            "All"
        } else {
            text(r, "SCOPE2EXPRESSION")
        };
        let matches = if let Some(b) = b {
            match text(r, "NETSCOPE") {
                "DifferentNets" if a.net.is_some() && a.net == b.net => continue,
                "SameNet" if a.net.is_none() || a.net != b.net => continue,
                _ => {}
            }
            (query(board, a, first)? && query(board, b, second)?)
                || (query(board, b, first)? && query(board, a, second)?)
        } else {
            query(board, a, first)?
        };
        if matches {
            return Ok(Some((index, r)));
        }
    }
    Ok(None)
}
pub fn object_kind(i: &Item) -> &str {
    if i.polygon.is_some() {
        return "Poly";
    }
    match i.shape {
        Shape::Pad { hole: 0, .. } => "SMDPad",
        Shape::Pad { .. } => "THPad",
        Shape::Via { .. } => "Via",
        Shape::Track { .. } => "Track",
        Shape::Arc { .. } => "Arc",
        Shape::Fill { .. } => "Fill",
        Shape::Region { .. } => "Region",
        Shape::Text { .. } => "Text",
    }
}
pub fn gap(r: &Fields, a: &Item, b: &Item) -> Result<i64, String> {
    gap_objects(r, object_kind(a), object_kind(b))
}
pub fn gap_objects(r: &Fields, a: &str, b: &str) -> Result<i64, String> {
    let key = format!("ClearanceObj_{}-ClearanceObj_{}", a, b);
    let reverse = format!("ClearanceObj_{}-ClearanceObj_{}", b, a);
    for entry in text(r, "OBJECTCLEARANCES").split(';') {
        if let Some((k, v)) = entry.split_once(':')
            && (k == key || k == reverse)
        {
            // Native integer coordinates are 1/10000 mil.
            let n = v.parse::<i64>().map_err(|_| "Invalid clearance matrix")?;
            if !(0..=39_370_078_740).contains(&n) {
                return Err("Clearance matrix outside range".into());
            }
            return Ok((n as f64 * 2.54).round() as i64);
        }
    }
    dimension(r, "GAP")
}
pub fn clearance(board: &Board, a: &Item, b: &Item) -> Result<Option<i64>, String> {
    let Some((_, r)) = resolve(board, "Clearance", a, Some(b))? else {
        return Ok(None);
    };
    if text(r, "IGNOREPADTOPADCLEARANCEINFOOTPRINT") == "TRUE"
        && a.component.is_some()
        && a.component == b.component
        && matches!(a.shape, Shape::Pad { .. })
        && matches!(b.shape, Shape::Pad { .. })
    {
        return Ok(Some(0));
    }
    gap(r, a, b).map(Some)
}
/// Cutouts use the stricter electrical Region clearance and board-edge constraint.
pub fn cutout_clearance(board: &Board, item: &Item, cutout: &Item) -> Result<Option<i64>, String> {
    Ok(cutout_rule(board, item, cutout)?.map(|(_, _, gap)| gap))
}
pub(crate) fn cutout_rule<'a>(
    board: &'a Board,
    item: &Item,
    cutout: &Item,
) -> Result<Option<(usize, &'a Fields, i64)>, String> {
    let electrical = resolve(board, "Clearance", item, Some(cutout))?
        .map(|(n, r)| gap(r, item, cutout).map(|g| (n, r, g)))
        .transpose()?;
    let edge = resolve(board, "BoardOutlineClearance", item, None)?
        .map(|(n, r)| gap_objects(r, object_kind(item), "CutoutEdge").map(|g| (n, r, g)))
        .transpose()?;
    Ok(match (electrical, edge) {
        (Some(a), Some(b)) => Some(if a.2 > b.2 { a } else { b }),
        (a, b) => a.or(b),
    })
}

pub fn probe(layer: u16, net: Option<u32>, position: Point, width: i64) -> Item {
    Item::new(
        layer,
        net,
        Shape::Track {
            start: position,
            end: position,
            width,
        },
    )
}
pub fn preferred(board: &Board, item: &Item, kind: &str, key: &str) -> Result<Option<i64>, String> {
    resolve(board, kind, item, None)?
        .map(|(_, r)| {
            let layer_key = format!("{}_PREFWIDTH", width_layer_prefix(item.layer));
            dimension(
                r,
                if kind == "Width" && key == "PREFEREDWIDTH" && r.contains_key(&layer_key) {
                    &layer_key
                } else {
                    key
                },
            )
        })
        .transpose()
}
fn width_layer_prefix(layer: u16) -> String {
    match layer {
        TOP => "TOPLAYER".into(),
        BOTTOM => "BOTTOMLAYER".into(),
        n => format!("MIDLAYER{}", n.saturating_sub(1)),
    }
}
pub fn width_limits(r: &Fields, layer: u16) -> Result<(i64, i64), String> {
    let prefix = width_layer_prefix(layer);
    let min = format!("{prefix}_MINWIDTH");
    let max = format!("{prefix}_MAXWIDTH");
    Ok((
        dimension(
            r,
            if r.contains_key(&min) {
                &min
            } else {
                "MINLIMIT"
            },
        )?,
        dimension(
            r,
            if r.contains_key(&max) {
                &max
            } else {
                "MAXLIMIT"
            },
        )?,
    ))
}
pub fn route_check(board: &Board, item: &Item) -> Result<(), String> {
    for kind in ["Width", "RoutingLayers", "RoutingVias"] {
        if let Some((_, message)) = route_violation(board, item, kind)? {
            return Err(message);
        }
    }
    if let Shape::Via { diameter, hole, .. } = item.shape {
        if let Some((_, r)) = resolve(board, "HoleSize", item, None)? {
            if text(r, "ABSOLUTEVALUES") == "FALSE" {
                return Err(format!(
                    "{}: relative hole-size constraints are not implemented",
                    text(r, "NAME")
                ));
            }
            if hole < dimension(r, "MINLIMIT")? || hole > dimension(r, "MAXLIMIT")? {
                return Err(format!(
                    "{}: via drill is outside hole-size limits",
                    text(r, "NAME")
                ));
            }
        }
        if item.pad_stack.is_none()
            && let Some((_, r)) = resolve(board, "MinimumAnnularRing", item, None)?
            && diameter - hole + 4 < 2 * dimension(r, "MINIMUMRING")?
        {
            return Err(format!(
                "{}: via annular ring is below minimum",
                text(r, "NAME")
            ));
        }
    }
    Ok(())
}
/// Report each violated routing rule independently; do not infer its kind from text.
fn route_violation(
    board: &Board,
    item: &Item,
    kind: &str,
) -> Result<Option<(usize, String)>, String> {
    if item.keepout
        || item.polygon.is_some()
        || (!copper_layer(item.layer) && item.layer != MULTI_LAYER)
    {
        return Ok(None);
    }
    let applicable = match item.shape {
        Shape::Track { .. } | Shape::Arc { .. } => matches!(kind, "Width" | "RoutingLayers"),
        Shape::Via { .. } => kind == "RoutingVias",
        _ => false,
    };
    if !applicable {
        return Ok(None);
    }
    let Some((n, r)) = resolve(board, kind, item, None)? else {
        return Ok(None);
    };
    let message = match (&item.shape, kind) {
        (Shape::Track { width, .. } | Shape::Arc { width, .. }, "Width") => {
            let (min, max) = width_limits(r, item.layer)?;
            (*width + 2 < min || *width > max + 2).then(|| {
                format!(
                    "{}: width must be {:.4}..{:.4} mm",
                    text(r, "NAME"),
                    min as f64 / 1e6,
                    max as f64 / 1e6
                )
            })
        }
        (_, "RoutingLayers") => {
            let key = format!("{}_V5", layer_name(item.layer).to_ascii_uppercase());
            (text(r, &key) == "FALSE").then(|| {
                format!(
                    "{}: routing on {} is disabled",
                    text(r, "NAME"),
                    layer_name(item.layer)
                )
            })
        }
        (Shape::Via { diameter, hole, .. }, "RoutingVias") => {
            let mut invalid = false;
            for (v, min, max) in [
                (diameter, "MINWIDTH", "MAXWIDTH"),
                (hole, "MINHOLEWIDTH", "MAXHOLEWIDTH"),
            ] {
                invalid |= *v < dimension(r, min)? || *v > dimension(r, max)?;
            }
            invalid.then(|| {
                format!(
                    "{}: via dimensions are outside constraints",
                    text(r, "NAME")
                )
            })
        }
        _ => None,
    };
    Ok(message.map(|m| (n, m)))
}

pub fn validate_rule(board: &Board, r: &Fields) -> Result<(), String> {
    if text(r, "RULEKIND") == "PolygonConnect" {
        polygon::validate_connection(r)?;
    }
    if text(r, "NAME").trim().is_empty() {
        return Err("Rule name is empty".into());
    }
    if text(r, "PRIORITY")
        .parse::<u32>()
        .ok()
        .is_none_or(|n| n == 0)
    {
        return Err("Priority must be a positive integer".into());
    }
    let item = probe(TOP, None, Point::default(), 200_000);
    for key in ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"] {
        if !text(r, key).is_empty() {
            query(board, &item, text(r, key))?;
        }
    }
    for (key, _) in fields(text(r, "RULEKIND")) {
        let n = dimension(r, key)?;
        if !((if *key == "EXPANSION" {
            -100_000_000_000
        } else {
            0
        })..=100_000_000_000)
            .contains(&n)
        {
            return Err(format!("{key} is outside range"));
        }
    }
    for entry in text(r, "OBJECTCLEARANCES")
        .split(';')
        .filter(|s| !s.is_empty())
    {
        let (key, value) = entry
            .split_once(':')
            .ok_or("Invalid clearance matrix entry")?;
        if !key.starts_with("ClearanceObj_")
            || !key.contains("-ClearanceObj_")
            || value
                .parse::<i64>()
                .ok()
                .is_none_or(|n| !(0..=39_370_078_740).contains(&n))
        {
            return Err("Invalid clearance matrix entry".into());
        }
    }
    if text(r, "RULEKIND") == "Width" {
        for layer in board.layers.iter().filter(|l| l.copper) {
            let (min, max) = width_limits(r, layer.id)?;
            if min <= 0 || min > max {
                return Err(format!("Invalid width range on {}", layer.name));
            }
            let key = format!("{}_PREFWIDTH", width_layer_prefix(layer.id));
            if r.contains_key(&key) {
                let preferred = dimension(r, &key)?;
                if !(min..=max).contains(&preferred) {
                    return Err(format!("Invalid preferred width on {}", layer.name));
                }
            }
        }
    }
    if matches!(
        text(r, "RULEKIND"),
        "SolderMaskExpansion" | "PasteMaskExpansion"
    ) {
        let p = if text(r, "PERCENTAGE").is_empty() {
            0.0
        } else {
            text(r, "PERCENTAGE")
                .parse::<f64>()
                .map_err(|_| "Invalid mask percentage")?
        };
        if !p.is_finite() || p.abs() > 1000.0 {
            return Err("Invalid mask percentage".into());
        }
    }
    for (min, preferred, max) in match text(r, "RULEKIND") {
        "Width" => vec![("MINLIMIT", "PREFEREDWIDTH", "MAXLIMIT")],
        "RoutingVias" => vec![
            ("MINWIDTH", "WIDTH", "MAXWIDTH"),
            ("MINHOLEWIDTH", "HOLEWIDTH", "MAXHOLEWIDTH"),
        ],
        "HoleSize" => vec![("MINLIMIT", "MINLIMIT", "MAXLIMIT")],
        _ => vec![],
    } {
        if dimension(r, min)? <= 0
            || dimension(r, min)? > dimension(r, preferred)?
            || dimension(r, preferred)? > dimension(r, max)?
        {
            return Err("Require 0 < minimum <= preferred <= maximum".into());
        }
    }
    Ok(())
}
pub fn default_rule(kind: &str) -> Fields {
    let mut r = Fields::from([
        ("RULEKIND".into(), kind.into()),
        ("NAME".into(), kind.into()),
        ("ENABLED".into(), "TRUE".into()),
        ("PRIORITY".into(), "1".into()),
        ("SCOPE1EXPRESSION".into(), "All".into()),
        ("SCOPE2EXPRESSION".into(), "All".into()),
        ("UNIQUEID".into(), Uuid::new_v4().to_string()),
        (
            "NETSCOPE".into(),
            if kind == "Clearance" {
                "DifferentNets"
            } else {
                "AnyNet"
            }
            .into(),
        ),
    ]);
    for (key, _) in fields(kind) {
        r.insert(
            (*key).into(),
            match *key {
                "MAXLIMIT" | "MAXWIDTH" => "3mm",
                "MAXHOLEWIDTH" => "1mm",
                "MINHOLEWIDTH" | "HOLEWIDTH" => "0.3mm",
                "MINWIDTH" | "WIDTH" => "0.6mm",
                _ => "0.2mm",
            }
            .into(),
        );
    }
    if kind == "ShortCircuit" {
        r.insert("ALLOWED".into(), "FALSE".into());
    }
    if kind == "PolygonConnect" {
        for (key, value) in [
            ("CONNECTSTYLE", "Relief"),
            ("AIRGAPWIDTH", "0.254mm"),
            ("RELIEFCONDUCTORWIDTH", "0.254mm"),
            ("RELIEFENTRIES", "4"),
            ("POLYGONRELIEFANGLE", "90 Angle"),
        ] {
            r.insert(key.into(), value.into());
        }
    }
    r
}
// Unknown predicates deliberately propagate; a higher-priority unknown rule
// must not silently be replaced with a lower-priority All rule.
pub(crate) fn scope(board: &Board, item: &Item, s: &str, depth: usize) -> Option<bool> {
    if depth > 32 || s.len() > 8192 {
        return None;
    }
    let s = s.trim();
    if s.starts_with('(') && s.ends_with(')') {
        let mut level = 0;
        let mut quoted = false;
        let mut closes = false;
        for (n, c) in s.char_indices() {
            if c == '\'' {
                quoted = !quoted;
            }
            if !quoted {
                if c == '(' {
                    level += 1;
                }
                if c == ')' {
                    level -= 1;
                    if level == 0 && n != s.len() - 1 {
                        closes = true;
                        break;
                    }
                }
            }
        }
        if !closes {
            return scope(board, item, &s[1..s.len() - 1], depth + 1);
        }
    }
    let upper = s.to_ascii_uppercase();
    for op in ["OR", "AND"] {
        let mut level = 0;
        let mut quoted = false;
        for (n, c) in s.char_indices() {
            if c == '\'' {
                quoted = !quoted;
            }
            if quoted {
                continue;
            }
            if c == '(' {
                level += 1;
            } else if c == ')' {
                level -= 1;
            }
            let boundary = |c: char| !c.is_alphanumeric() && c != '_';
            if level == 0
                && upper[n..].starts_with(op)
                && s[..n].chars().next_back().is_none_or(boundary)
                && s[n + op.len()..].chars().next().is_none_or(boundary)
            {
                let a = scope(board, item, &s[..n], depth + 1);
                let b = scope(board, item, &s[n + op.len()..], depth + 1);
                return a
                    .zip(b)
                    .map(|(a, b)| if op == "AND" { a && b } else { a || b });
            }
        }
    }
    if upper.starts_with("NOT ") {
        return scope(board, item, &s[4..], depth + 1).map(|v| !v);
    }
    if upper.starts_with("NOT(") {
        return scope(board, item, &s[3..], depth + 1).map(|v| !v);
    }
    if upper.starts_with("COMPPARAMETERVALUE(") {
        return membership::component_parameter_comparison(board, item, s);
    }
    if upper.starts_with("OBJECTKIND") || upper.starts_with("NAME") {
        return membership::field_comparison(item, s);
    }
    match upper.as_str() {
        "ALL" | "TRUE" => return Some(true),
        "FALSE" => return Some(false),
        "ISPAD" => return Some(matches!(item.shape, Shape::Pad { .. })),
        "ISVIA" => return Some(matches!(item.shape, Shape::Via { .. })),
        "ISTRACK" => return Some(matches!(item.shape, Shape::Track { .. })),
        "ISARC" => return Some(matches!(item.shape, Shape::Arc { .. })),
        "ONCOPPER" => return Some(copper_layer(item.layer) || item.layer == MULTI_LAYER),
        "ONTOP" => return Some(item.layer == TOP),
        "ONBOTTOM" => return Some(item.layer == BOTTOM),
        "ISFILL" => return Some(matches!(item.shape, Shape::Fill { .. })),
        "ISREGION" => return Some(matches!(item.shape, Shape::Region { .. })),
        "ISPOLYGON" => return Some(item.polygon.is_some()),
        "INPOLY" | "INPOLYGON" => {
            return Some(item.polygon.is_some() && (1..=32).contains(&item.layer));
        }
        "ISTHRUPAD" => return Some(matches!(item.shape,Shape::Pad {hole,..} if hole>0)),
        "ISSMDPAD" => return Some(matches!(item.shape, Shape::Pad { hole: 0, .. })),
        _ => {}
    }
    for name in [
        "INNET",
        "INCOMPONENT",
        "HASFOOTPRINT",
        "HASPAD",
        "ONLAYER",
        "INPADCLASS",
        "INNETCLASS",
        "INCOMPONENTCLASS",
        "INLAYERCLASS",
        "WITHINROOM",
        "INNAMEDPOLYGON",
        "ISNAMEDPOLYGON",
    ] {
        if upper.starts_with(&format!("{name}(")) && upper.ends_with(')') {
            let raw = s[name.len() + 1..s.len() - 1].trim();
            let mut values = Vec::new();
            let mut chars = raw.chars().peekable();
            loop {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                if chars.next() != Some('\'') {
                    return None;
                }
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some('\'') if chars.peek() == Some(&'\'') => {
                            chars.next();
                            value.push('\'');
                        }
                        Some('\'') => break,
                        Some(c) => value.push(c),
                        None => return None,
                    }
                }
                values.push(value);
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                match chars.next() {
                    None => break,
                    Some(',') => {}
                    _ => return None,
                }
            }
            if matches!(name, "INLAYERCLASS" | "WITHINROOM") {
                let mut result = false;
                for value in &values {
                    result |= if name == "INLAYERCLASS" {
                        membership::in_layer_class(board, item.layer, value)?
                    } else {
                        membership::within_room(board, item, value)?
                    };
                }
                return Some(result);
            }
            let component = item
                .component
                .and_then(|n| board.components.iter().find(|c| c.index == n));
            return Some(values.iter().any(|value| {
                match name {
                    "INNET" => board.net_name(item.net).eq_ignore_ascii_case(value),
                    "INCOMPONENT" => {
                        component.is_some_and(|c| c.designator.eq_ignore_ascii_case(value))
                    }
                    "HASFOOTPRINT" => {
                        component.is_some_and(|c| membership::like(&c.footprint, value))
                    }
                    "HASPAD" => {
                        if let Shape::Pad { name, .. } = &item.shape {
                            let parent = component.map_or("Free", |c| c.designator.as_str());
                            membership::like(&format!("{parent}-{name}"), value)
                        } else {
                            false
                        }
                    }
                    "INNAMEDPOLYGON" | "ISNAMEDPOLYGON" => item.polygon.is_some_and(|index| {
                        board
                            .polygon_pours
                            .iter()
                            .any(|p| p.index == index && p.name.eq_ignore_ascii_case(value))
                    }),
                    "ONLAYER" => membership::layer_matches(board, item.layer, value),
                    _ => board
                        .classes
                        .iter()
                        .filter(|c| text(c, "NAME").eq_ignore_ascii_case(value))
                        .filter(|c| {
                            let expected = match name {
                                "INNETCLASS" => "0",
                                "INCOMPONENTCLASS" => "1",
                                _ => "3",
                            };
                            text(c, "KIND").is_empty() || text(c, "KIND") == expected
                        })
                        .any(|class| {
                            if text(class, "SUPERCLASS") == "TRUE" {
                                return match name {
                                    "INNETCLASS" => item.net.is_some(),
                                    "INCOMPONENTCLASS" => component.is_some(),
                                    _ => matches!(item.shape, Shape::Pad { .. }),
                                };
                            }
                            class
                                .iter()
                                .filter(|(k, _)| {
                                    k.strip_prefix("MEMBER")
                                        .or_else(|| k.strip_prefix('M'))
                                        .is_some_and(|suffix| {
                                            !suffix.is_empty()
                                                && suffix.bytes().all(|b| b.is_ascii_digit())
                                        })
                                })
                                .any(|(_, member)| match name {
                                    "INNETCLASS" => {
                                        board.net_name(item.net).eq_ignore_ascii_case(member)
                                    }
                                    "INCOMPONENTCLASS" => component
                                        .is_some_and(|c| c.designator.eq_ignore_ascii_case(member)),
                                    _ => {
                                        if let Shape::Pad { name, .. } = &item.shape {
                                            component.is_some_and(|c| {
                                                format!("{}-{name}", c.designator)
                                                    .eq_ignore_ascii_case(member)
                                            })
                                        } else {
                                            false
                                        }
                                    }
                                })
                        }),
                }
            }));
        }
    }
    None
}

impl Board {
    pub fn with_rules(&self, rules: Vec<Fields>) -> Result<Self, String> {
        for r in &rules {
            if self.rules.contains(r) {
                continue;
            }
            if !supported(text(r, "RULEKIND")) {
                if !self.rules.contains(r) {
                    return Err("Editing this rule type is not implemented".into());
                }
                continue;
            }
            validate_rule(self, r)?;
        }
        let mut names = std::collections::HashSet::new();
        for r in &rules {
            if !names.insert((text(r, "RULEKIND"), text(r, "NAME").to_ascii_lowercase())) {
                return Err("Duplicate rule name within type".into());
            }
        }
        let mut next = self.clone();
        let pour_rules = |rs: &[Fields]| {
            rs.iter()
                .filter(|r| {
                    matches!(
                        text(r, "RULEKIND"),
                        "Clearance" | "BoardOutlineClearance" | "PolygonConnect"
                    )
                })
                .map(|r| {
                    let mut constraints = r.clone();
                    for metadata in ["NAME", "COMMENT", "UNIQUEID"] {
                        constraints.remove(metadata);
                    }
                    constraints
                })
                .collect::<Vec<_>>()
        };
        if pour_rules(&self.rules) != pour_rules(&rules) && !self.polygon_pours.is_empty() {
            next.pours_stale = true;
        }
        next.rules = rules;
        next.validate()?;
        Ok(next)
    }
    pub fn replace_design_rule(&self, index: usize, rule: Fields) -> Result<Self, String> {
        let old = self.rules.get(index).ok_or("Rule no longer exists")?;
        if matches!(
            text(old, "RULEKIND"),
            "SolderMaskExpansion" | "PasteMaskExpansion"
        ) {
            return self.replace_mask_rule(index, rule);
        }
        if !supported(text(old, "RULEKIND")) || text(old, "RULEKIND") != text(&rule, "RULEKIND") {
            return Err("Unsupported native rule edit".into());
        }
        for key in old.keys().chain(rule.keys()) {
            if old.get(key) == rule.get(key) {
                continue;
            }
            let allowed = matches!(
                key.as_str(),
                "NAME"
                    | "COMMENT"
                    | "PRIORITY"
                    | "ENABLED"
                    | "SCOPE1EXPRESSION"
                    | "SCOPE2EXPRESSION"
                    | "ALLOWED"
                    | "NETSCOPE"
                    | "IGNOREPADTOPADCLEARANCEINFOOTPRINT"
                    | "OBJECTCLEARANCES"
                    | "ALLOWSHELVED"
                    | "ALLOWUNPOURED"
                    | "CLEARANCETOEXPOSEDCOPPER"
            ) || fields(text(old, "RULEKIND")).iter().any(|(k, _)| k == key)
                || (text(old, "RULEKIND") == "RoutingLayers" && key.ends_with("_V5"))
                || (text(old, "RULEKIND") == "PolygonConnect" && polygon::connection_field(key))
                || (text(old, "RULEKIND") == "Width"
                    && (key.ends_with("_MINWIDTH")
                        || key.ends_with("_MAXWIDTH")
                        || key.ends_with("_PREFWIDTH")));
            if !allowed {
                return Err(format!("Unsupported native rule field: {key}"));
            }
        }
        validate_rule(self, &rule)?;
        let mut rules = self.rules.clone();
        rules[index] = rule;
        self.with_rules(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_class_members_use_numeric_m_fields_and_respect_class_kind() {
        let mut b = board();
        let i = track(0, 0, 200_000);
        b.classes = vec![Fields::from([
            ("NAME".into(), "A'B".into()),
            ("KIND".into(), "0".into()),
            ("M0".into(), "A".into()),
            ("MODEL".into(), "B".into()),
        ])];
        assert_eq!(query(&b, &i, "InNetClass('A''B')"), Ok(true));
        assert_eq!(query(&b, &track(1, 0, 1), "InNetClass('A''B')"), Ok(false));
        b.classes[0].insert("KIND".into(), "1".into());
        assert_eq!(query(&b, &i, "InNetClass('A''B')"), Ok(false));
        b.classes[0].remove("KIND");
        b.classes[0].remove("M0");
        b.classes[0].insert("MEMBER0".into(), "A".into());
        assert_eq!(query(&b, &i, "InNetClass('A''B')"), Ok(true));
        b.classes[0].clear();
        b.classes[0].extend(Fields::from([
            ("NAME".into(), "All Nets".into()),
            ("KIND".into(), "0".into()),
            ("SUPERCLASS".into(), "TRUE".into()),
        ]));
        assert_eq!(query(&b, &i, "InNetClass('All Nets')"), Ok(true));
        assert_eq!(
            query(
                &b,
                &probe(TOP, None, Point::default(), 1),
                "InNetClass('All Nets')"
            ),
            Ok(false)
        );
    }
    #[test]
    fn preferred_route_width_uses_layer_override_and_validates_its_range() {
        let mut b = board();
        b.rules = vec![default_rule("Width")];
        b.rules[0].insert("TOPLAYER_PREFWIDTH".into(), "0.4mm".into());
        assert_eq!(
            preferred(&b, &track(0, 0, 1), "Width", "PREFEREDWIDTH"),
            Ok(Some(400_000))
        );
        let bottom = probe(BOTTOM, Some(0), Point::default(), 1);
        assert_eq!(
            preferred(&b, &bottom, "Width", "PREFEREDWIDTH"),
            dimension(&b.rules[0], "PREFEREDWIDTH").map(Some)
        );
        b.layers = vec![PcbLayer {
            id: TOP,
            name: "Top Layer".into(),
            copper: true,
            order: 0,
        }];
        let mut next = b.rules[0].clone();
        next.insert("TOPLAYER_PREFWIDTH".into(), "50mm".into());
        assert!(b.replace_design_rule(0, next).is_err());
    }
    #[test]
    fn exported_report_escapes_document_and_rule_data_and_preserves_incomplete_status() {
        let mut b = Board {
            rules: vec![default_rule("Width")],
            ..Default::default()
        };
        b.rules[0].insert("NAME".into(), "<script>alert(1)</script>".into());
        b.items.push(probe(TOP, None, Point::mm(1, 2), 100_000));
        let mut report = check(&b);
        report.truncated = true;
        report.diagnostics.insert("Unknown <condition>".into());
        let html = report.html(&b, "A&B.PcbDoc");
        assert!(html.contains("A&amp;B.PcbDoc"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Report incomplete"));
        assert!(html.contains("Unknown &lt;condition&gt;"));
        assert!(html.contains("1.000000"));
    }
    #[test]
    fn unsupported_query_branches_cannot_hide_behind_constants() {
        let b = board();
        let i = probe(TOP, None, Point::default(), 1);
        for expression in [
            "All OR UnknownPredicate",
            "False AND UnknownPredicate",
            "IsVia AND (UnknownPredicate)",
        ] {
            assert!(query(&b, &i, expression).is_err());
        }
        let mut b = Board {
            rules: vec![default_rule("PolygonConnect")],
            polygon_pours: vec![crate::pcb_polygon::Polygon::default()],
            ..Default::default()
        };
        // Rule labels do not change the geometry; constraints do.
        b.polygon_pours[0].contour = crate::pcb_polygon::straight_contour(&[
            Point::mm(0, 0),
            Point::mm(5, 0),
            Point::mm(5, 5),
            Point::mm(0, 5),
        ]);
        let mut rules = b.rules.clone();
        rules[0].insert("COMMENT".into(), "Updated note".into());
        assert!(!b.with_rules(rules.clone()).unwrap().pours_stale);
        rules[0].insert("AIRGAPWIDTH".into(), "0.5mm".into());
        assert!(b.with_rules(rules).unwrap().pours_stale);
    }
    fn board() -> Board {
        Board {
            nets: vec![
                Net {
                    index: 0,
                    name: "A".into(),
                    properties: Fields::new(),
                },
                Net {
                    index: 1,
                    name: "B".into(),
                    properties: Fields::new(),
                },
            ],
            ..Board::default()
        }
    }
    fn track(net: u32, y: i64, width: i64) -> Item {
        Item::new(
            TOP,
            Some(net),
            Shape::Track {
                start: Point { x: 0, y },
                end: Point { x: 2_000_000, y },
                width,
            },
        )
    }
    #[test]
    fn outline_rule_stops_routes_and_checks_copper_edges() {
        let mut b = board();
        b.outline = [
            Point::mm(0, 0),
            Point::mm(10, 0),
            Point::mm(10, 10),
            Point::mm(0, 10),
        ]
        .into_iter()
        .map(|position| ContourVertex {
            position,
            arc: None,
        })
        .collect();
        b.rules = vec![default_rule("BoardOutlineClearance")];
        let mut route =
            crate::pcb_route::RouteSession::begin(&b, 0, Point::mm(5, 5), TOP, Some(0), 200_000, 0)
                .unwrap();
        route.conflict = crate::pcb_route::Conflict::Stop;
        let points = route.points(Point::mm(11, 5));
        assert!(points.last().unwrap().x <= 9_700_000 && points.last().unwrap().x > 9_690_000);
        b.items = vec![Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(5, 5),
                end: Point::mm(10, 5),
                width: 200_000,
            },
        )];
        assert_eq!(check(&b).violations[0].kind, "BoardOutlineClearance");
    }
    #[test]
    fn board_cutout_rules_apply_in_routing_drc_and_pour() {
        let mut b = board();
        b.outline = crate::pcb_polygon::straight_contour(&[
            Point::mm(0, 0),
            Point::mm(20, 0),
            Point::mm(20, 20),
            Point::mm(0, 20),
        ]);
        let mut cutout = Item::new(
            KEEP_OUT,
            None,
            Shape::Region {
                contour: vec![
                    Point::mm(8, 8),
                    Point::mm(12, 8),
                    Point::mm(12, 12),
                    Point::mm(8, 12),
                ],
                holes: vec![],
                cutout: true,
            },
        );
        cutout
            .properties
            .insert("ISBOARDCUTOUT".into(), "TRUE".into());
        b.items.push(cutout);
        let mut r = default_rule("BoardOutlineClearance");
        r.insert("GAP".into(), "1mm".into());
        b.rules = vec![r];
        let mut route = crate::pcb_route::RouteSession::begin(
            &b,
            0,
            Point::mm(5, 10),
            TOP,
            Some(0),
            200_000,
            0,
        )
        .unwrap();
        route.conflict = crate::pcb_route::Conflict::Stop;
        let points = route.points(Point::mm(10, 10));
        assert!(points.last().unwrap().x <= 6_900_000 && points.last().unwrap().x > 6_890_000);
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(7, 9),
                end: Point::mm(7, 11),
                width: 200_000,
            },
        ));
        assert!(
            check(&b)
                .violations
                .iter()
                .any(|v| v.message.contains("cutout"))
        );
        b.items.pop();
        b.polygon_pours.push(crate::pcb_polygon::Polygon {
            contour: b.outline.clone(),
            net: Some(0),
            ..Default::default()
        });
        let poured = crate::pcb_polygon::repour(&b).unwrap();
        assert!(
            poured
                .items
                .iter()
                .filter(|i| i.polygon.is_some())
                .all(|i| hit_distance(
                    i,
                    Point {
                        x: 7_500_000,
                        y: 10_000_000
                    }
                ) > 400_000.0)
        );
        b.rules[0].insert("GAP".into(), "0mm".into());
        b.items = vec![Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(19, 4),
                end: Point::mm(21, 4),
                width: 200_000,
            },
        )];
        assert!(check(&b).violations.is_empty());
        let route = crate::pcb_route::RouteSession::begin(
            &b,
            0,
            Point::mm(19, 5),
            TOP,
            Some(0),
            200_000,
            0,
        )
        .unwrap();
        assert_eq!(
            *route.points(Point::mm(21, 5)).last().unwrap(),
            Point::mm(21, 5)
        );
    }
    #[test]
    fn drc_retains_multiple_rule_failures_and_router_rejects_manufacturing_limits() {
        let mut b = board();
        b.rules = vec![default_rule("Width"), default_rule("RoutingLayers")];
        b.rules[1].insert("TOP LAYER_V5".into(), "FALSE".into());
        b.items.push(track(0, 0, 100_000));
        let report = check(&b);
        assert!(report.violations.iter().any(|v| v.kind == "Width"));
        assert!(report.violations.iter().any(|v| v.kind == "RoutingLayers"));
        b.rules = vec![default_rule("HoleSize"), default_rule("MinimumAnnularRing")];
        let via = Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Via {
                position: Point::default(),
                diameter: 600_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        );
        assert!(route_check(&b, &via).unwrap_err().contains("annular ring"));
        b.rules[1].insert("MINIMUMRING".into(), "0.1mm".into());
        assert!(route_check(&b, &via).is_ok());
        b.rules[0].insert("MINLIMIT".into(), "0.4mm".into());
        assert!(route_check(&b, &via).unwrap_err().contains("hole-size"));
    }
    #[test]
    fn query_adjacent_boolean_tokens_and_polygon_names() {
        let b = board();
        let i = track(0, 0, 200_000);
        assert_eq!(
            query(&b, &i, "(InNet('B'))or(InNet('A'))and NOT(IsVia)"),
            Ok(true)
        );
        assert_eq!(query(&b, &i, "InNamedPolygon('none','other')"), Ok(false));
        assert!(query(&b, &i, "IsTrack ORder IsVia").is_err());
    }
    #[test]
    fn drc_options_limit_results_without_changing_board_or_rule_indices() {
        let mut b = board();
        b.rules = vec![default_rule("Width"), default_rule("ShortCircuit")];
        b.items = vec![track(0, 0, 100_000), track(1, 0, 100_000)];
        let before = b.clone();
        let report = check_with_options(
            &b,
            &CheckOptions {
                excluded: std::collections::BTreeSet::from(["Width".into()]),
                limit: 1,
            },
        );
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].rule, 1);
        assert!(!report.truncated);
        let report = check_with_options(
            &b,
            &CheckOptions {
                limit: 1,
                ..Default::default()
            },
        );
        assert_eq!(report.violations.len(), 1);
        assert!(report.truncated);
        assert_eq!(before, b);
    }
    #[test]
    fn annular_ring_handles_offcentre_holes_and_nonplated_pads() {
        let mut b = board();
        b.rules = vec![default_rule("MinimumAnnularRing")];
        b.items = vec![Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Pad {
                position: Point::mm(1, 1),
                size: Point::mm(1, 1),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 800_000,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        )];
        assert!(
            check(&b)
                .violations
                .iter()
                .any(|v| v.kind == "MinimumAnnularRing")
        );
        if let Shape::Pad { plated, .. } = &mut b.items[0].shape {
            *plated = false;
        }
        assert!(check(&b).violations.is_empty());
    }
    #[test]
    fn pair_priority_reverse_scope_and_disabled_rules() {
        let mut b = board();
        let a = track(0, 0, 200_000);
        let c = track(1, 500_000, 200_000);
        let mut generic = default_rule("Clearance");
        generic.insert("PRIORITY".into(), "2".into());
        let mut specific = default_rule("Clearance");
        specific.insert("SCOPE1EXPRESSION".into(), "InNet('B')".into());
        specific.insert("SCOPE2EXPRESSION".into(), "InNet('A')".into());
        specific.insert("GAP".into(), "0.5mm".into());
        b.rules = vec![generic, specific];
        assert_eq!(clearance(&b, &a, &c).unwrap(), Some(500_000));
        b.rules[1].insert("ENABLED".into(), "FALSE".into());
        assert_eq!(clearance(&b, &a, &c).unwrap(), Some(200_000));
        b.rules[1].insert("ENABLED".into(), "TRUE".into());
        b.rules[1].insert("SCOPE1EXPRESSION".into(), "UnknownPredicate".into());
        assert!(clearance(&b, &a, &c).is_err());
    }
    #[test]
    fn query_boolean_precedence_multi_args_and_nested_parentheses() {
        let b = board();
        let i = track(0, 0, 200_000);
        for s in [
            "((InNet('B', 'A'))) AND NOT IsVia",
            "IsVia OR IsTrack AND OnLayer('Top Layer')",
            "OnCopper AND InNet('A')",
        ] {
            assert_eq!(query(&b, &i, s), Ok(true));
        }
        assert_eq!(query(&b, &i, "InNet('B') AND IsTrack"), Ok(false));
        assert!(query(&b, &i, "Unsupported('A')").is_err());
    }
    #[test]
    fn matrix_units_and_actual_distance_are_used() {
        let mut b = board();
        let mut r = default_rule("Clearance");
        r.insert(
            "OBJECTCLEARANCES".into(),
            "ClearanceObj_Track-ClearanceObj_Track:196850".into(),
        );
        b.rules = vec![r];
        b.items = vec![track(0, 0, 200_000), track(1, 600_000, 200_000)];
        let report = check(&b);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(gap(&b.rules[0], &b.items[0], &b.items[1]).unwrap(), 499_999);
        assert!(
            report.violations[0]
                .message
                .contains("0.400000 mm < 0.499999 mm")
        );
        b.items[1].layer = BOTTOM;
        assert!(check(&b).violations.is_empty());
    }
    #[test]
    fn allowed_short_circuit_and_width_limits_respect_rules() {
        let mut b = board();
        b.items = vec![track(0, 0, 100_000), track(1, 0, 200_000)];
        b.rules = vec![default_rule("ShortCircuit"), default_rule("Width")];
        let r = check(&b);
        assert!(r.violations.iter().any(|v| v.kind == "ShortCircuit"));
        assert!(r.violations.iter().any(|v| v.kind == "Width"));
        b.rules[0].insert("ALLOWED".into(), "TRUE".into());
        assert!(
            !check(&b)
                .violations
                .iter()
                .any(|v| v.kind == "ShortCircuit")
        );
    }
    #[test]
    fn invalid_rule_edit_and_history_are_atomic() {
        let mut b = board();
        b.rules = vec![default_rule("Width")];
        let old = b.clone();
        let mut rules = b.rules.clone();
        rules[0].insert("MINLIMIT".into(), "5mm".into());
        assert!(b.with_rules(rules).is_err());
        assert_eq!(b, old);
        let mut doc = crate::Document::new(crate::DocumentKind::Board, "Rules");
        doc.pcb = Some(b.clone());
        let before = doc.clone();
        let mut history = crate::History::default();
        b.rules[0].insert("PREFEREDWIDTH".into(), "0.5mm".into());
        history.commit_pcb(&mut doc, b).unwrap();
        let encoded = serde_json::to_string(&doc).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::Document>(&encoded).unwrap(),
            doc
        );
        history.undo(&mut doc);
        assert_eq!(doc, before);
    }
    #[test]
    fn scoped_rules_drive_route_width_and_pair_obstacles() {
        let mut b = board();
        let mut width = default_rule("Width");
        width.insert("PREFEREDWIDTH".into(), "0.4mm".into());
        b.rules = vec![width, default_rule("Clearance")];
        b.rules[1].insert("GAP".into(), "0.5mm".into());
        b.items = vec![Item::new(
            TOP,
            Some(1),
            Shape::Track {
                start: Point {
                    x: 1_000_000,
                    y: -2_000_000,
                },
                end: Point {
                    x: 1_000_000,
                    y: 2_000_000,
                },
                width: 200_000,
            },
        )];
        let mut route = crate::pcb_route::RouteSession::begin(
            &b,
            0,
            Point::default(),
            TOP,
            Some(0),
            100_000,
            0,
        )
        .unwrap();
        assert_eq!(route.width, 400_000);
        route.conflict = crate::pcb_route::Conflict::Stop;
        let points = route.points(Point { x: 2_000_000, y: 0 });
        assert!(points.last().unwrap().x <= 200_000);
        route.width = 50_000;
        assert!(route.preview(Point { x: 200_000, y: 0 }).is_err());
        assert_eq!(b.items.len(), 1);
    }
}
