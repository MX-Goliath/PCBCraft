//! Membership queries used by imported clearance scopes.
use super::text;
use crate::pcb::*;
use geo::{BooleanOps, Intersects, Relate, Validation};

fn quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start().strip_prefix('\'')?;
    let mut chars = s.char_indices().peekable();
    let mut value = String::new();
    while let Some((index, c)) = chars.next() {
        if c != '\'' {
            value.push(c);
        } else if chars.peek().is_some_and(|(_, c)| *c == '\'') {
            chars.next();
            value.push('\'');
        } else {
            return Some((value, &s[index + 1..]));
        }
    }
    None
}

pub(super) fn component_parameter_comparison(board: &Board, item: &Item, s: &str) -> Option<bool> {
    let (function, args) = s.split_once('(')?;
    if !function.trim().eq_ignore_ascii_case("CompParameterValue") {
        return None;
    }
    let (name, tail) = quoted(args)?;
    let tail = tail.trim_start().strip_prefix(')')?.trim_start();
    let (op, tail) = if let Some(tail) = tail.strip_prefix("<>") {
        ("<>", tail)
    } else if let Some(tail) = tail.strip_prefix('=') {
        ("=", tail)
    } else if tail
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("Like"))
        && tail[4..].starts_with(char::is_whitespace)
    {
        ("LIKE", &tail[4..])
    } else {
        return None;
    };
    let (pattern, tail) = quoted(tail)?;
    if !tail.trim().is_empty() {
        return None;
    }
    let Some(component) = item
        .component
        .and_then(|n| board.components.iter().find(|c| c.index == n))
    else {
        return Some(false);
    };
    // Legacy JSON without interpreted parameters cannot safely evaluate a rule.
    let parameters = component.parameters.as_ref()?;
    let Some((_, value)) = parameters
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(&name))
    else {
        return Some(false);
    };
    Some(match op {
        "=" => value.eq_ignore_ascii_case(&pattern),
        "<>" => !value.eq_ignore_ascii_case(&pattern),
        _ => like(value, &pattern),
    })
}

pub(super) fn field_comparison(board: &Board, item: &Item, s: &str) -> Option<bool> {
    let end = s.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    let (field, tail) = s.split_at(end);
    let tail = tail.trim_start();
    let (op, tail) = if let Some(tail) = tail.strip_prefix("<>") {
        ("<>", tail)
    } else if let Some(tail) = tail.strip_prefix('=') {
        ("=", tail)
    } else if tail
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("Like"))
        && tail[4..].starts_with(char::is_whitespace)
    {
        ("LIKE", &tail[4..])
    } else {
        return None;
    };
    let (value, rest) = quoted(tail)?;
    if !rest.trim().is_empty() {
        return None;
    }
    let actual = match field.trim().to_ascii_uppercase().as_str() {
        "OBJECTKIND" => item.kind(),
        "NAME" => match &item.shape {
            Shape::Pad { name, .. } => name,
            _ => return Some(false),
        },
        "LAYER" => {
            let matched = layer_matches(board, item.layer, &value);
            return (op != "LIKE").then_some(if op == "<>" { !matched } else { matched });
        }
        "DRILLTYPE" => match &item.shape {
            Shape::Pad { .. } => item
                .properties
                .get("DRILLTYPE")
                .map_or("Drilled", String::as_str),
            _ => return Some(op == "<>"),
        },
        _ => return None,
    };
    Some(match op {
        "LIKE" => like(actual, &value),
        "<>" => !actual.eq_ignore_ascii_case(&value),
        _ => actual.eq_ignore_ascii_case(&value),
    })
}

/// Find a comparison outside parentheses and quoted strings. Operators in names,
/// escaped quotes and nested function arguments must never split the query.
pub(super) fn boolean_comparison(s: &str) -> Option<(&str, bool)> {
    let mut level = 0i32;
    let mut quoted = false;
    for (n, c) in s.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        }
        if quoted {
            continue;
        }
        match c {
            '(' => level += 1,
            ')' => level -= 1,
            '=' | '<' if level == 0 => {
                let (tail, negate) = if c == '=' {
                    (&s[n + 1..], false)
                } else {
                    (s[n..].strip_prefix("<>")?, true)
                };
                let tail = tail.trim();
                let value = tail
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''))
                    .unwrap_or(tail);
                let value = if value.eq_ignore_ascii_case("True") {
                    true
                } else if value.eq_ignore_ascii_case("False") {
                    false
                } else {
                    return None;
                };
                return Some((s[..n].trim(), value != negate));
            }
            _ => {}
        }
        if level < 0 {
            return None;
        }
    }
    None
}

pub(super) fn differential_pair(board: &Board, item: &Item, pattern: &str) -> bool {
    board
        .differential_pairs
        .iter()
        .any(|pair| like(text(pair, "NAME"), pattern) && pair_contains(board, item, pair))
}

pub(super) fn pair_contains(board: &Board, item: &Item, pair: &Fields) -> bool {
    let Some(net) = board.item_net(item) else {
        return false;
    };
    let name = board.net_name(Some(net));
    ["POSITIVENETNAME", "NEGATIVENETNAME"]
        .iter()
        .any(|key| !text(pair, key).is_empty() && text(pair, key).eq_ignore_ascii_case(name))
}

pub(super) fn like(value: &str, pattern: &str) -> bool {
    let value: Vec<_> = value.chars().collect();
    let pattern: Vec<_> = pattern.chars().collect();
    let (mut v, mut p) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while v < value.len() {
        if p < pattern.len()
            && pattern[p] != '*'
            && (pattern[p] == '?' || pattern[p].eq_ignore_ascii_case(&value[v]))
        {
            v += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            retry = v;
            p += 1;
        } else if let Some(index) = star {
            retry += 1;
            v = retry;
            p = index + 1;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

pub(super) fn layer_matches(board: &Board, layer: u16, name: &str) -> bool {
    if board
        .layers
        .iter()
        .any(|l| l.id == layer && l.name.eq_ignore_ascii_case(name))
    {
        return true;
    }
    let compact = |s: &str| {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    };
    compact(&layer_name(layer)).eq_ignore_ascii_case(&compact(name))
        || match name.to_ascii_uppercase().as_str() {
            "TOP" => layer == TOP,
            "BOTTOM" => layer == BOTTOM,
            _ => false,
        }
}

pub(super) fn in_layer_class(board: &Board, layer: u16, name: &str) -> Option<bool> {
    let mut result = false;
    for class in board
        .classes
        .iter()
        .filter(|c| text(c, "KIND") == "4" && text(c, "NAME").eq_ignore_ascii_case(name))
    {
        if text(class, "SUPERCLASS").eq_ignore_ascii_case("TRUE") {
            result |= match text(class, "NAME").to_ascii_uppercase().as_str() {
                "ALL LAYERS" => board.layers.iter().any(|l| l.id == layer),
                "ELECTRICAL LAYERS" => copper_layer(layer),
                "SIGNAL LAYERS" => (1..=32).contains(&layer),
                "COMPONENT LAYERS" => matches!(layer, TOP | BOTTOM),
                _ => return None,
            };
        } else {
            result |= class
                .iter()
                .filter(|(key, _)| {
                    key.strip_prefix("MEMBER")
                        .or_else(|| key.strip_prefix('M'))
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
                .any(|(_, member)| layer_matches(board, layer, member));
        }
    }
    // A missing named class has no members, as opposed to an unknown predicate.
    Some(result)
}

fn room_points(room: &Fields) -> Option<Vec<eda_geom::Point>> {
    let mut points = Vec::new();
    for n in 0..room.len() {
        let Some(x) = room.get(&format!("VX{n}")) else {
            break;
        };
        let dim = |key: &str| dimension_nm(room.get(key)?).ok();
        let position = eda_geom::Point {
            x: dimension_nm(x).ok()?,
            y: dim(&format!("VY{n}"))?,
        };
        match room
            .get(&format!("KIND{n}"))
            .map(String::as_str)
            .unwrap_or("0")
        {
            "0" => {}
            // Curved native rooms need exact containment support before use as scopes.
            _ => return None,
        }
        if position.x.unsigned_abs() > 100_000_000_000
            || position.y.unsigned_abs() > 100_000_000_000
        {
            return None;
        }
        points.push(position);
    }
    let vertices = room
        .keys()
        .filter(|k| {
            k.strip_prefix("VX")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .count();
    (points.len() >= 3 && points.len() == vertices).then_some(points)
}

pub(super) fn within_room(board: &Board, item: &Item, name: &str) -> Option<bool> {
    let mut result = false;
    for room in board.rules.iter().filter(|r| {
        text(r, "RULEKIND") == "RoomDefinition" && text(r, "NAME").eq_ignore_ascii_case(name)
    }) {
        result |= Room::parse(room)?.contains(item);
    }
    Some(result)
}

/// Parsed, normalized rooms belong to one immutable board snapshot. Unsupported
/// rooms stay errors; construction is lazy so unused rules cannot reject a route.
pub(super) struct Rooms<'a> {
    board: &'a Board,
    parsed: std::cell::RefCell<std::collections::BTreeMap<String, Option<Vec<Room>>>>,
}
struct Room {
    bounds: Bounds,
    area: geo::MultiPolygon<f64>,
}
impl<'a> Rooms<'a> {
    pub(super) fn new(board: &'a Board) -> Self {
        Self {
            board,
            parsed: Default::default(),
        }
    }
    pub(super) fn contains(&self, item: &Item, name: &str) -> Option<bool> {
        let mut parsed = self.parsed.borrow_mut();
        let rooms = parsed.entry(name.to_ascii_uppercase()).or_insert_with(|| {
            self.board
                .rules
                .iter()
                .filter(|r| {
                    text(r, "RULEKIND") == "RoomDefinition"
                        && text(r, "NAME").eq_ignore_ascii_case(name)
                })
                .map(Room::parse)
                .collect()
        });
        let mut result = false;
        for room in rooms.as_ref()? {
            result |= room.contains(item);
        }
        Some(result)
    }
}
impl Room {
    fn parse(room: &Fields) -> Option<Self> {
        let points = room_points(room)?;
        let bounds = Bounds::points(&points);
        let area = geo::Polygon::new(
            points
                .iter()
                .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
                .collect::<Vec<_>>()
                .into(),
            vec![],
        );
        // Native rooms can contain a retraced closing edge. Normalize the ring
        // with even/odd fill before containment, preserving concave outlines.
        let area = if area.is_valid() {
            geo::MultiPolygon(vec![area])
        } else {
            area.union(&geo::MultiPolygon(vec![]))
        };
        if area.0.is_empty() || !area.is_valid() {
            return None;
        }
        Some(Self { bounds, area })
    }
    fn contains(&self, item: &Item) -> bool {
        if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
            let p = item.anchor();
            self.area
                .intersects(&geo::Point::new(p.x as f64 / 1e6, p.y as f64 / 1e6))
        } else {
            let b = item.bounds();
            b.min.x >= self.bounds.min.x
                && b.min.y >= self.bounds.min.y
                && b.max.x <= self.bounds.max.x
                && b.max.y <= self.bounds.max.y
                && self
                    .area
                    .relate(&crate::pcb_polygon::copper(item, item.layer))
                    .is_covers()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_rules::query;
    use eda_geom::Point;

    #[test]
    fn component_parameters_scope_child_copper_with_escaped_strings_and_wildcards() {
        let mut board = Board::default();
        board.components.push(Component {
            id: uuid::Uuid::new_v4(),
            index: 7,
            designator: "BM1".into(),
            comment: String::new(),
            footprint: "SIP10".into(),
            position: Point::default(),
            rotation: 0.,
            layer: TOP,
            locked: false,
            properties: Fields::new(),
            footprint_parameters: None,
            parameters: Some(Fields::from([
                ("ID".into(), "216".into()),
                ("Vendor's name".into(), "A'B OR (C)".into()),
                ("Empty".into(), "".into()),
            ])),
        });
        let mut child = track(Point::mm(1, 1), Point::mm(2, 1));
        child.component = Some(7);
        for expression in [
            "CompParameterValue('id') Like '216'",
            "CompParameterValue('ID') Like '?1*'",
            "CompParameterValue('ID') = '216'",
            "CompParameterValue('ID') <> '217'",
            "CompParameterValue('Vendor''s name') Like 'A''B OR (C)'",
            "CompParameterValue('Empty') Like ''",
            "InPadClass('TP_Pads') Or CompParameterValue('ID') Like '216'",
            "Not (CompParameterValue('ID') Like '217')",
            "HasFootprint('SIP?0')",
        ] {
            assert_eq!(query(&board, &child, expression), Ok(true), "{expression}");
        }
        assert_eq!(
            query(&board, &child, "CompParameterValue('missing') Like '*'"),
            Ok(false)
        );
        assert_eq!(
            query(&board, &child, "CompParameterValue('ID') Like '21'"),
            Ok(false)
        );
        let mut scoped = crate::pcb_rules::default_rule("Clearance");
        scoped.insert(
            "SCOPE1EXPRESSION".into(),
            "CompParameterValue('ID') Like '216'".into(),
        );
        scoped.insert("NETSCOPE".into(), "AnyNet".into());
        scoped.insert("GAP".into(), "1mm".into());
        let mut fallback = crate::pcb_rules::default_rule("Clearance");
        fallback.insert("PRIORITY".into(), "2".into());
        fallback.insert("NETSCOPE".into(), "AnyNet".into());
        board.rules = vec![scoped, fallback];
        let foreign = track(Point::mm(3, 1), Point::mm(4, 1));
        assert_eq!(
            crate::pcb_rules::clearance(&board, &child, &foreign),
            Ok(Some(1_000_000))
        );
        board.components[0]
            .parameters
            .as_mut()
            .unwrap()
            .insert("ID".into(), "217".into());
        assert_eq!(
            crate::pcb_rules::clearance(&board, &child, &foreign),
            Ok(Some(200_000))
        );
        let expression = "CompParameterValue('ID') Like '216'";
        board.components[0].parameters = None;
        assert!(query(&board, &child, expression).is_err());
        assert!(crate::pcb_rules::clearance(&board, &child, &foreign).is_err());
        child.component = None;
        assert_eq!(query(&board, &child, expression), Ok(false));
        for expression in [
            "CompParameterValue('ID') Like",
            "CompParameterValue('ID') Like '216' junk",
            "CompParameterValue('ID', 'other') Like '216'",
            "CompParameterValue('ID') LikeX '216'",
            "All OR CompParameterValue('ID') > '216'",
        ] {
            assert!(query(&board, &child, expression).is_err(), "{expression}");
        }
    }

    #[test]
    fn has_pad_uses_full_name_and_never_matches_other_copper() {
        let board = Board::default();
        let pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(1, 1),
                rotation: 0.,
                shape: PadShape::Round,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        assert_eq!(query(&board, &pad, "HasPad('Free-*')"), Ok(true));
        assert_eq!(query(&board, &pad, "HasPad('BM1-*')"), Ok(false));
        assert_eq!(query(&board, &pad, "ObjectKind = 'Pad'"), Ok(true));
        assert_eq!(query(&board, &pad, "ObjectKind = 'Component'"), Ok(false));
        assert_eq!(
            query(
                &board,
                &track(Point::mm(0, 0), Point::mm(1, 0)),
                "HasPad('*')"
            ),
            Ok(false)
        );
        for (value, pattern, expected) in [
            ("", "*", true),
            ("abcd", "a*d", true),
            ("abcd", "a*c?", true),
            ("ab", "a*b?", false),
            ("контакт", "???????", true),
            ("ab", "*a*b*", true),
            ("ab", "*a*c", false),
            ("*ab", "*b", true),
            ("**", "*", true),
        ] {
            assert_eq!(like(value, pattern), expected);
        }
    }

    fn track(a: Point, b: Point) -> Item {
        Item::new(
            TOP,
            None,
            Shape::Track {
                start: a,
                end: b,
                width: 200_000,
            },
        )
    }
    fn room() -> Fields {
        let mut room = Fields::from([
            ("NAME".into(), "DUT".into()),
            ("RULEKIND".into(), "RoomDefinition".into()),
        ]);
        // Concave L: its bounding box also includes an area outside the room.
        for (n, (x, y)) in [(0, 0), (10, 0), (10, 4), (4, 4), (4, 10), (0, 10)]
            .into_iter()
            .enumerate()
        {
            room.insert(format!("VX{n}"), format!("{x}mm"));
            room.insert(format!("VY{n}"), format!("{y}mm"));
        }
        room
    }
    #[test]
    fn prepared_room_scopes_reuse_geometry_without_reusing_item_membership() {
        let mut board = Board {
            rules: vec![room()],
            ..Default::default()
        };
        let inside = track(Point::mm(1, 1), Point::mm(3, 1));
        let outside = track(Point::mm(5, 5), Point::mm(6, 6));
        let straddling = track(Point::mm(1, 1), Point::mm(5, 5));
        for curved in [false, true] {
            if curved {
                board.rules[0].insert("KIND0".into(), "1".into());
            }
            let rooms = Rooms::new(&board);
            for expression in [
                "WithinRoom('DUT')",
                "WithinRoom('dut') OR IsVia",
                "NOT WithinRoom('DUT')",
                "WithinRoom('missing', 'DUT') = True",
                "WithinRoom('missing') AND UnknownPredicate",
            ] {
                for item in [&outside, &inside, &straddling, &inside, &outside] {
                    assert_eq!(
                        crate::pcb_rules::scope_prepared(&board, item, expression, 0, Some(&rooms)),
                        crate::pcb_rules::scope(&board, item, expression, 0),
                        "{expression}"
                    );
                }
            }
            assert_eq!(
                rooms.contains(&inside, "DUT"),
                if curved { None } else { Some(true) }
            );
            assert_eq!(
                rooms.contains(&outside, "DUT"),
                if curved { None } else { Some(false) }
            );
        }
    }
    #[test]
    fn layer_classes_use_native_members_kinds_builtins_and_layer_aliases() {
        let mut b = Board::default();
        b.layers[0].name = "Custom top".into();
        b.classes = vec![Fields::from([
            ("NAME".into(), "L3_L4".into()),
            ("KIND".into(), "4".into()),
            ("M0".into(), "TOP".into()),
            ("MEMBER1".into(), "Bottom Layer".into()),
        ])];
        let i = track(Point::mm(1, 1), Point::mm(2, 1));
        assert_eq!(
            query(&b, &i, "InLayerClass('l3_l4') AND OnLayer('TOP')"),
            Ok(true)
        );
        assert_eq!(query(&b, &i, "OnLayer('Custom top')"), Ok(true));
        assert_eq!(query(&b, &i, "OnLayer('Top Layer')"), Ok(true));
        assert_eq!(query(&b, &i, "InLayerClass('missing')"), Ok(false));
        b.classes[0].insert("KIND".into(), "0".into());
        assert_eq!(query(&b, &i, "InLayerClass('L3_L4')"), Ok(false));
        for (name, layer, expected) in [
            ("Electrical Layers", 39, true),
            ("Signal Layers", 39, false),
            ("Component Layers", 2, false),
            ("Component Layers", BOTTOM, true),
            ("All Layers", TOP_OVERLAY, true),
        ] {
            b.classes = vec![Fields::from([
                ("NAME".into(), name.into()),
                ("KIND".into(), "4".into()),
                ("SUPERCLASS".into(), "TRUE".into()),
            ])];
            let mut i = i.clone();
            i.layer = layer;
            assert_eq!(
                query(&b, &i, &format!("InLayerClass('{name}')")),
                Ok(expected)
            );
        }
    }
    #[test]
    fn within_room_uses_full_shape_for_tracks_but_centres_for_pads_and_vias() {
        let b = Board {
            rules: vec![room()],
            ..Default::default()
        };
        for (a, b_, expected) in [
            (Point::mm(1, 1), Point::mm(9, 1), true),
            (Point::mm(1, 1), Point::mm(5, 5), false),
            (Point::mm(1, 1), Point::mm(10, 1), false),
            (Point::mm(5, 5), Point::mm(6, 6), false),
        ] {
            assert_eq!(query(&b, &track(a, b_), "WithinRoom('dut')"), Ok(expected));
        }
        let via = Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::mm(10, 1),
                diameter: 2_000_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        );
        assert_eq!(query(&b, &via, "WithinRoom('missing', 'DUT')"), Ok(true));
        let pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::mm(10, 1),
                size: Point::mm(2, 2),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        assert_eq!(query(&b, &pad, "WithinRoom('DUT')"), Ok(true));
    }
    #[test]
    fn malformed_or_curved_rooms_and_unknown_predicates_remain_errors() {
        let mut b = Board {
            rules: vec![room()],
            ..Default::default()
        };
        let i = track(Point::mm(1, 1), Point::mm(2, 1));
        b.rules[0].insert("KIND0".into(), "1".into());
        assert!(query(&b, &i, "WithinRoom('DUT')").is_err());
        b.rules[0] = room();
        b.rules[0].remove("VY1");
        assert!(query(&b, &i, "WithinRoom('DUT')").is_err());
        assert!(query(&b, &i, "InLayerClass('missing') AND UnknownPredicate").is_err());
    }
    #[test]
    fn native_room_with_retraced_closing_edge_keeps_its_filled_area() {
        let mut room = Fields::from([
            ("RULEKIND".into(), "RoomDefinition".into()),
            ("NAME".into(), "COUPLE".into()),
        ]);
        for (n, (x, y)) in [(0, 0), (4, 0), (4, 4), (0, 4), (0, 0), (1, 0), (0, 0)]
            .into_iter()
            .enumerate()
        {
            room.insert(format!("VX{n}"), format!("{x}mm"));
            room.insert(format!("VY{n}"), format!("{y}mm"));
        }
        let b = Board {
            rules: vec![room],
            ..Default::default()
        };
        assert_eq!(
            query(
                &b,
                &track(Point::mm(1, 1), Point::mm(2, 1)),
                "WithinRoom('COUPLE')"
            ),
            Ok(true)
        );
        assert_eq!(
            query(
                &b,
                &track(Point::mm(5, 5), Point::mm(6, 5)),
                "WithinRoom('COUPLE')"
            ),
            Ok(false)
        );
    }
    #[test]
    fn polygon_aliases_match_generated_signal_copper_only() {
        let b = Board::default();
        let mut i = track(Point::mm(1, 1), Point::mm(2, 1));
        for expression in ["InPoly", "InPolygon"] {
            i.polygon = None;
            i.layer = TOP;
            assert_eq!(query(&b, &i, expression), Ok(false));
            i.polygon = Some(0);
            assert_eq!(query(&b, &i, expression), Ok(true));
            i.layer = TOP_OVERLAY;
            assert_eq!(query(&b, &i, expression), Ok(false));
        }
    }
}
