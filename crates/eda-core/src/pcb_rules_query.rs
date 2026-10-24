//! Membership queries used by imported clearance scopes.
use super::text;
use crate::pcb::*;
use geo::{BooleanOps, Intersects, Relate, Validation};

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
        if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
            let p = item.anchor();
            result |= area.intersects(&geo::Point::new(p.x as f64 / 1e6, p.y as f64 / 1e6));
        } else {
            let b = item.bounds();
            if b.min.x >= bounds.min.x
                && b.min.y >= bounds.min.y
                && b.max.x <= bounds.max.x
                && b.max.y <= bounds.max.y
            {
                result |= area
                    .relate(&crate::pcb_polygon::copper(item, item.layer))
                    .is_covers();
            }
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_rules::query;
    use eda_geom::Point;

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
