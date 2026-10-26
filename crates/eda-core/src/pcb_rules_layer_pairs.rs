//! Layer-pair checks use fabrication definitions, independently of via diameter rules.
use super::*;
use std::collections::BTreeSet;

fn enforced(r: &Fields) -> Result<bool, String> {
    match text(r, "ENFORCE").to_ascii_uppercase().as_str() {
        "" | "TRUE" | "1" => Ok(true),
        "FALSE" | "0" => Ok(false),
        _ => Err(format!("{}: invalid ENFORCE flag", text(r, "NAME"))),
    }
}

fn copper_id(board: &Board, value: &str) -> Result<u16, String> {
    let token = value.trim().to_ascii_uppercase().replace(' ', "");
    let named = board
        .layers
        .iter()
        .find(|l| l.copper && l.name.eq_ignore_ascii_case(value.trim()));
    let id = match token.as_str() {
        "TOP" | "TOPLAYER" => Some(TOP),
        "BOTTOM" | "BOTTOMLAYER" => Some(BOTTOM),
        _ => named
            .map(|l| l.id)
            .or_else(|| {
                token
                    .strip_prefix("MID-LAYER")
                    .or_else(|| token.strip_prefix("MIDLAYER"))
                    .or_else(|| token.strip_prefix("MID"))
                    .and_then(|n| n.parse::<u16>().ok())
                    .filter(|n| (1..=30).contains(n))
                    .map(|n| n + 1)
            })
            .or_else(|| {
                token
                    .strip_prefix("INTERNALPLANE")
                    .and_then(|n| n.parse::<u16>().ok())
                    .filter(|n| (1..=16).contains(n))
                    .map(|n| n + 38)
            })
            .or_else(|| {
                token
                    .strip_prefix("LAYER")
                    .unwrap_or(&token)
                    .parse::<u32>()
                    .ok()
                    .and_then(native_layer)
            }),
    };
    id.filter(|id| board.layers.iter().any(|l| l.copper && l.id == *id))
        .ok_or_else(|| format!("Unknown copper layer in drill-pair definition: {value}"))
}

fn span(copper: &[u16], a: u16, b: u16) -> Result<(u16, u16), String> {
    let index = |id| {
        copper
            .iter()
            .position(|l| *l == id)
            .ok_or_else(|| format!("Missing copper layer {id} in layer pair"))
    };
    let (a_index, b_index) = (index(a)?, index(b)?);
    if a_index == b_index {
        return Err("A drill pair must span two different copper layers".into());
    }
    Ok(if a_index < b_index { (a, b) } else { (b, a) })
}

/// An edited typed stack is authoritative. Imported legacy drill pairs remain
/// read-only board properties; an absent table has Altium's default thru span.
fn allowed_pairs(board: &Board, copper: &[u16]) -> Result<BTreeSet<(u16, u16)>, String> {
    let default = || span(copper, TOP, BOTTOM).map(|pair| BTreeSet::from([pair]));
    if let Some(stack) = &board.layer_stack {
        stack.validate_for_board(board)?;
        if stack.vias.is_empty() {
            return default();
        }
        return stack
            .vias
            .iter()
            .map(|v| span(copper, v.start, v.end))
            .collect();
    }
    let rows: BTreeSet<_> = board
        .properties
        .keys()
        .filter_map(|key| {
            let key = key.strip_prefix("LAYERPAIR")?;
            let end = key.find(|c: char| !c.is_ascii_digit())?;
            key[..end].parse::<usize>().ok()
        })
        .collect();
    if rows.is_empty() {
        return default();
    }
    let substacks: BTreeSet<_> = board
        .properties
        .iter()
        .filter(|(k, _)| k.starts_with("LAYERPAIR") && k.contains("SUBSTACK_"))
        .map(|(_, value)| value)
        .collect();
    if substacks.len() > 1 {
        return Err("Layer Pairs for multiple native substacks are not implemented".into());
    }
    rows.into_iter()
        .map(|n| {
            let read = |suffix| {
                let key = format!("LAYERPAIR{n}{suffix}");
                let value = board
                    .properties
                    .get(&key)
                    .ok_or_else(|| format!("Incomplete drill pair: missing {key}"))?;
                copper_id(board, value)
            };
            span(copper, read("LOW")?, read("HIGH")?)
        })
        .collect()
}

pub fn layer_pairs_violation(
    board: &Board,
    item: &Item,
    r: &Fields,
) -> Result<Option<String>, String> {
    if item.drill_size() <= 0 || !enforced(r)? {
        return Ok(None);
    }
    let (from, to) = match item.shape {
        Shape::Via {
            from_layer,
            to_layer,
            ..
        } => (from_layer, to_layer),
        Shape::Pad { .. } => (TOP, BOTTOM),
        _ => return Ok(None),
    };
    let mut layers: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
    layers.sort_by_key(|l| l.order);
    let copper: Vec<_> = layers.iter().map(|l| l.id).collect();
    let actual = span(&copper, from, to)?;
    let allowed = allowed_pairs(board, &copper)?;
    Ok((!allowed.contains(&actual)).then(|| {
        format!(
            "{}: layer pair {} to {} is not defined in the layer stack",
            text(r, "NAME"),
            board.layers.iter().find(|l| l.id == actual.0).unwrap().name,
            board.layers.iter().find(|l| l.id == actual.1).unwrap().name
        )
    }))
}

pub(super) fn validate(r: &Fields) -> Result<(), String> {
    enforced(r).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        layer_stack::{Stack, ViaKind, ViaType},
        pcb_route::RouteSession,
    };
    fn board() -> Board {
        let original = Board::default();
        let mut stack = Stack::from_board(&original);
        stack.add_copper(1).unwrap();
        stack.add_copper(1).unwrap();
        let mut board = stack.apply(&original).unwrap();
        board.rules.push(default_rule("LayerPairs"));
        board
    }
    fn via(from: u16, to: u16) -> Item {
        Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::mm(1, 1),
                diameter: 900_000,
                hole: 400_000,
                from_layer: from,
                to_layer: to,
            },
        )
    }
    fn definition(start: u16, end: u16, kind: ViaKind) -> ViaType {
        ViaType {
            id: Uuid::new_v4(),
            name: format!("{start}-{end}"),
            kind,
            start,
            end,
            diameter: 600_000,
            hole: 300_000,
        }
    }
    #[test]
    fn typed_via_spans_follow_physical_order_and_do_not_replace_size_rules() {
        let mut b = board();
        let stack = b.layer_stack.as_mut().unwrap();
        let first_inner = stack.copper()[1].copper_id.unwrap();
        stack.vias = vec![
            definition(TOP, BOTTOM, ViaKind::Through),
            definition(TOP, first_inner, ViaKind::Micro),
        ];
        // Different drill/diameter from the definition is permitted by Layer Pairs.
        assert!(route_check(&b, &via(TOP, first_inner)).is_ok());
        assert!(route_check(&b, &via(first_inner, TOP)).is_ok());
        assert!(route_check(&b, &via(BOTTOM, TOP)).is_ok());
        let denied = via(first_inner, BOTTOM);
        assert!(route_check(&b, &denied).unwrap_err().contains("layer pair"));
        b.items.push(denied.clone());
        let report = check(&b);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].kind, "LayerPairs");
        assert_eq!(report.violations[0].items, vec![denied.id]);
        assert_eq!(report.violations[0].layer, MULTI_LAYER);
        let options = CheckOptions {
            excluded: BTreeSet::from(["LayerPairs".into()]),
            ..Default::default()
        };
        assert!(check_with_options(&b, &options).violations.is_empty());
    }
    #[test]
    fn legacy_pairs_are_read_only_and_edited_stack_is_authoritative() {
        let mut b = board();
        let first_inner = b.layer_stack.as_ref().unwrap().copper()[1]
            .copper_id
            .unwrap();
        let saved_stack = b.layer_stack.take().unwrap();
        b.properties
            .insert("LAYERPAIR0LOW".into(), format!("MID{}", first_inner - 1));
        b.properties.insert("LAYERPAIR0HIGH".into(), "TOP".into());
        b.properties
            .insert("LAYERPAIR0SUBSTACK_0".into(), "original-stack".into());
        let original = b.clone();
        assert!(route_check(&b, &via(TOP, first_inner)).is_ok());
        assert!(route_check(&b, &via(TOP, BOTTOM)).is_err());
        assert_eq!(b, original);
        let pad = Item::new(
            MULTI_LAYER,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(1, 1),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 300_000,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        assert!(
            layer_pairs_violation(&b, &pad, &b.rules[0])
                .unwrap()
                .is_some()
        );
        b.layer_stack = Some(saved_stack);
        // The empty typed table presents the default Thru span in the manager.
        assert!(route_check(&b, &via(TOP, BOTTOM)).is_ok());
        assert!(route_check(&b, &via(TOP, first_inner)).is_err());
    }
    #[test]
    fn unknown_incomplete_and_multi_stack_pairs_cannot_silently_pass() {
        let mut b = board();
        b.layer_stack = None;
        b.properties
            .insert("LAYERPAIR0DRILLGUIDE".into(), "FALSE".into());
        assert!(
            route_check(&b, &via(TOP, BOTTOM))
                .unwrap_err()
                .contains("Incomplete")
        );
        b.properties.insert("LAYERPAIR0LOW".into(), "TOP".into());
        assert!(
            route_check(&b, &via(TOP, BOTTOM))
                .unwrap_err()
                .contains("Incomplete")
        );
        b.properties
            .insert("LAYERPAIR0HIGH".into(), "MissingLayer".into());
        assert!(
            route_check(&b, &via(TOP, BOTTOM))
                .unwrap_err()
                .contains("Unknown copper")
        );
        b.properties
            .insert("LAYERPAIR0HIGH".into(), "BOTTOM".into());
        b.properties
            .insert("LAYERPAIR0SUBSTACK_0".into(), "a".into());
        b.properties
            .insert("LAYERPAIR0SUBSTACK_1".into(), "b".into());
        assert!(
            check(&{
                let mut n = b.clone();
                n.items.push(via(TOP, BOTTOM));
                n
            })
            .diagnostics
            .iter()
            .any(|s| s.contains("multiple native substacks"))
        );
        b.rules[0].insert("ENFORCE".into(), "FALSE".into());
        assert!(route_check(&b, &via(TOP, BOTTOM)).is_ok());
    }
    #[test]
    fn denied_transition_is_atomic_and_enforcement_can_be_disabled() {
        let b = board();
        let inner = b.layer_stack.as_ref().unwrap().copper()[1]
            .copper_id
            .unwrap();
        let mut route =
            RouteSession::begin(&b, 0, Point::default(), TOP, None, 200_000, 0).unwrap();
        assert!(route.via(Point::mm(2, 0), inner, 900_000, 400_000).is_err());
        assert_eq!(route.draft(), &b);
        assert_eq!(route.anchor, Point::default());
        assert_eq!(route.layer, TOP);
        assert!(!route.has_segments());
        assert!(!route.backtrack());
        let mut enabled = b.clone();
        enabled.rules[0].insert("ENFORCE".into(), "FALSE".into());
        let mut route =
            RouteSession::begin(&enabled, 0, Point::default(), TOP, None, 200_000, 0).unwrap();
        route.via(Point::mm(2, 0), inner, 900_000, 400_000).unwrap();
        assert_eq!(route.layer, inner);
        assert!(route.backtrack());
        assert_eq!(route.draft(), &enabled);
    }
    #[test]
    fn scope_priority_disabled_and_validation_apply_to_layer_pairs() {
        let mut b = board();
        b.rules[0].insert("SCOPE1EXPRESSION".into(), "IsPad".into());
        assert!(route_check(&b, &via(TOP, 2)).is_ok());
        b.rules[0].insert("SCOPE1EXPRESSION".into(), "IsVia".into());
        assert!(route_check(&b, &via(TOP, 2)).is_err());
        let mut fallback = default_rule("LayerPairs");
        fallback.insert("NAME".into(), "Lower priority".into());
        fallback.insert("PRIORITY".into(), "2".into());
        fallback.insert("ENFORCE".into(), "FALSE".into());
        b.rules.push(fallback);
        assert!(route_check(&b, &via(TOP, 2)).is_err());
        b.rules[0].insert("ENABLED".into(), "FALSE".into());
        assert!(route_check(&b, &via(TOP, 2)).is_ok());
        let mut changed = b.rules[0].clone();
        changed.insert("ENFORCE".into(), "invalid".into());
        assert!(b.replace_design_rule(0, changed).is_err());
    }
}
