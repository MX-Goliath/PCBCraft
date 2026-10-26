//! Shared hole-size constraints for batch/online DRC and interactive via placement.
use super::*;

pub fn hole_percent_limits(r: &Fields) -> Result<(f64, f64), String> {
    let read = |key| {
        text(r, key)
            .trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{}: invalid {key}", text(r, "NAME")))
    };
    let (min, max) = (read("MINPERCENT")?, read("MAXPERCENT")?);
    if !min.is_finite() || !max.is_finite() || min < 0.0 || min > max {
        return Err(format!(
            "{}: require 0 <= minimum <= maximum (finite percentages)",
            text(r, "NAME")
        ));
    }
    Ok((min, max))
}

/// Percent uses the smaller nominal land dimension, independent of rotation.
/// For varying pad stacks, evaluate each physical copper land independently.
/// These non-circular/stack cases await native-reference verification; see RULES.md.
pub fn hole_size_violation(
    board: &Board,
    item: &Item,
    r: &Fields,
) -> Result<Option<String>, String> {
    let hole = item.drill_size();
    if hole <= 0 {
        return Ok(None);
    }
    if text(r, "ABSOLUTEVALUES") != "FALSE" {
        let (min, max) = (dimension(r, "MINLIMIT")?, dimension(r, "MAXLIMIT")?);
        return Ok((hole < min || hole > max).then(|| {
            format!(
                "{}: hole-size {:.4} mm outside {:.4}..{:.4} mm",
                text(r, "NAME"),
                hole as f64 / 1e6,
                min as f64 / 1e6,
                max as f64 / 1e6
            )
        }));
    }
    let (min, max) = hole_percent_limits(r)?;
    let mut found = false;
    for layer in board.layers.iter().filter(|l| l.copper) {
        if !crate::pcb_polygon::on_layer(board, item, layer.id) {
            continue;
        }
        let Some(land) = item.pad_geometry(layer.id) else {
            continue;
        };
        let size = land.size.x.min(land.size.y);
        if size <= 0 {
            return Err(format!(
                "{}: percent hole size requires a copper land",
                text(r, "NAME")
            ));
        }
        found = true;
        let percent = hole as f64 * 100.0 / size as f64;
        if percent + 1e-9 < min || percent > max + 1e-9 {
            return Ok(Some(format!(
                "{}: hole {:.4}% outside {:.4}..{:.4}% of pad/via size on {}",
                text(r, "NAME"),
                percent,
                min,
                max,
                layer.name
            )));
        }
    }
    if !found {
        return Err(format!(
            "{}: no physical copper land for percent hole size",
            text(r, "NAME")
        ));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn board() -> Board {
        let mut b = Board::default();
        b.items.push(Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::default(),
                diameter: 1_000_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        let mut r = default_rule("HoleSize");
        r.insert("ABSOLUTEVALUES".into(), "FALSE".into());
        b.rules.push(r);
        b
    }

    #[test]
    fn percent_drc_and_router_use_priority_scope_and_inclusive_limits() {
        let mut b = board();
        b.rules[0].insert("MINPERCENT".into(), "30".into());
        b.rules[0].insert("MAXPERCENT".into(), "30".into());
        assert!(route_check(&b, &b.items[0]).is_ok());
        assert!(check(&b).violations.is_empty());
        b.rules[0].insert("MINPERCENT".into(), "31".into());
        b.rules[0].insert("MAXPERCENT".into(), "80".into());
        assert!(
            route_check(&b, &b.items[0])
                .unwrap_err()
                .contains("30.0000%")
        );
        let report = check(&b);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].items, vec![b.items[0].id]);
        assert_eq!(report.violations[0].kind, "HoleSize");
        let mut lower = default_rule("HoleSize");
        lower.insert("PRIORITY".into(), "2".into());
        b.rules.push(lower);
        assert!(route_check(&b, &b.items[0]).is_err());
        b.rules[0].insert("SCOPE1EXPRESSION".into(), "IsPad".into());
        assert!(route_check(&b, &b.items[0]).is_ok());
        assert!(check(&b).violations.is_empty());
    }

    #[test]
    fn percent_uses_nominal_land_not_rotated_bounds_and_checks_stack() {
        let mut b = board();
        let mut land = b.items[0].pad_geometry(BOTTOM).unwrap();
        land.size = Point {
            x: 350_000,
            y: 350_000,
        };
        b.items[0].set_stack_mode(StackMode::TopMiddleBottom, &Board::default());
        b.items[0].set_pad_geometry(BOTTOM, land);
        assert!(
            hole_size_violation(&b, &b.items[0], &b.rules[0])
                .unwrap()
                .unwrap()
                .contains("Bottom")
        );
        b.items[0] = Item::new(
            MULTI_LAYER,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(1, 2),
                rotation: 45.0,
                shape: PadShape::Rectangle,
                hole: 300_000,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        b.rules[0].insert("MINPERCENT".into(), "30".into());
        b.rules[0].insert("MAXPERCENT".into(), "30".into());
        assert!(
            hole_size_violation(&b, &b.items[0], &b.rules[0])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn percent_edit_preserves_inactive_absolute_values_and_rejects_invalid_input() {
        let b = board();
        let mut r = b.rules[0].clone();
        r.insert("MINPERCENT".into(), "25.5".into());
        let edited = b.replace_design_rule(0, r.clone()).unwrap();
        assert_eq!(edited.rules[0]["MINLIMIT"], b.rules[0]["MINLIMIT"]);
        for (min, max) in [
            ("NaN", "80"),
            ("20", "inf"),
            ("-1", "80"),
            ("80", "20"),
            ("", "80"),
        ] {
            r.insert("MINPERCENT".into(), min.into());
            r.insert("MAXPERCENT".into(), max.into());
            assert!(b.replace_design_rule(0, r.clone()).is_err());
        }
    }
}
