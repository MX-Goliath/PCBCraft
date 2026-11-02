//! Drill contours are built once; distant pairs never reach polygon distance.
use super::*;
use geo::BoundingRect;

#[cfg(test)]
pub(super) fn check_holes(board: &Board, out: &mut Report, pairs: &mut usize) {
    check_holes_with_rules(board, &Rules::new(board), out, pairs);
}
pub(super) fn check_holes_with_rules(
    board: &Board,
    rules: &Rules<'_>,
    out: &mut Report,
    pairs: &mut usize,
) {
    let active_rules: Vec<_> = board
        .rules
        .iter()
        .filter(|r| enabled(r) && text(r, "RULEKIND") == "HoleToHoleClearance")
        .collect();
    if active_rules.is_empty() {
        return;
    }
    let mut max_gap = 0.0_f64;
    for r in active_rules {
        match dimension(r, "GAP") {
            Ok(gap) => max_gap = max_gap.max(gap as f64 / 1e6),
            Err(e) => {
                out.diagnostics.insert(e);
            }
        }
        if text(r, "ALLOWSTACKEDMICROVIAS") == "TRUE" {
            out.diagnostics
                .insert("Stacked microvia hole-clearance exemption is not implemented".into());
        }
    }
    let mut holes: Vec<_> = board
        .items
        .iter()
        .filter(|i| i.drill_size() > 0)
        .filter_map(|item| {
            let shape = polygon(&item.drill_outline());
            shape.bounding_rect().map(|bounds| (item, shape, bounds))
        })
        .collect();
    holes.sort_by(|a, b| a.2.min().x.total_cmp(&b.2.min().x));
    for (a, (i, shape, bounds)) in holes.iter().enumerate() {
        if out.stopped() {
            return;
        }
        for (j, other, other_bounds) in &holes[a + 1..] {
            if out.stopped() {
                return;
            }
            if other_bounds.min().x > bounds.max().x + max_gap {
                break;
            }
            if other_bounds.min().y > bounds.max().y + max_gap
                || other_bounds.max().y + max_gap < bounds.min().y
            {
                continue;
            }
            *pairs += 1;
            if *pairs > 2_000_000 {
                out.truncated = true;
                out.diagnostics
                    .insert("Pair budget reached; hole DRC is incomplete".into());
                return;
            }
            match rules.resolve("HoleToHoleClearance", i, Some(j)) {
                Ok(Some((n, r))) => match dimension(r, "GAP") {
                    Ok(required) => {
                        let distance = (Euclidean.distance(shape, other) * 1e6).round() as i64;
                        if distance + 2 < required {
                            out.add(
                                n,
                                r,
                                format!(
                                    "Hole clearance {:.6} mm < {:.6} mm",
                                    distance as f64 / 1e6,
                                    required as f64 / 1e6
                                ),
                                &[i, j],
                                MULTI_LAYER,
                            );
                        }
                    }
                    Err(e) => {
                        out.diagnostics.insert(e);
                    }
                },
                Err(e) => {
                    out.diagnostics.insert(e);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn via(x: i64, y: i64) -> Item {
        Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point { x, y },
                diameter: 600_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        )
    }
    fn keys(report: &Report) -> BTreeSet<(usize, Vec<Uuid>, String)> {
        report
            .violations
            .iter()
            .map(|v| {
                let mut items = v.items.clone();
                items.sort();
                (v.rule, items, v.message.clone())
            })
            .collect()
    }
    #[test]
    fn spatial_pruning_matches_all_pairs_with_rotated_slots_and_scoped_rules() {
        let mut b = Board {
            rules: vec![default_rule("HoleToHoleClearance")],
            ..Default::default()
        };
        b.rules[0].insert("GAP".into(), "0.15mm".into());
        // Deterministic irregular positions, including coincident and boundary holes.
        for n in 0..80 {
            b.items
                .push(via((n * 7919 % 23) * 100_000, (n * 1543 % 19) * 100_000));
        }
        let mut slot = Item::new(
            MULTI_LAYER,
            None,
            Shape::Pad {
                position: Point::mm(4, 0),
                size: Point::mm(1, 1),
                rotation: 37.0,
                shape: PadShape::Rectangle,
                hole: 300_000,
                plated: false,
                name: "slot".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        slot.set_stack_mode(StackMode::Simple, &b);
        let stack = slot.pad_stack.as_mut().unwrap();
        stack.drill_shape = DrillShape::Slot;
        stack.slot_length = 4_000_000;
        stack.drill_rotation = 13.0;
        b.items.push(slot);
        let mut scoped = b.rules[0].clone();
        scoped.insert("NAME".into(), "Pads".into());
        scoped.insert("SCOPE1EXPRESSION".into(), "IsPad".into());
        scoped.insert("GAP".into(), "0.8mm".into());
        b.rules[0].insert("PRIORITY".into(), "2".into());
        b.rules.push(scoped);
        let mut expected = Report::default();
        for (a, i) in b.items.iter().enumerate() {
            for j in &b.items[a + 1..] {
                let (n, r) = resolve(&b, "HoleToHoleClearance", i, Some(j))
                    .unwrap()
                    .unwrap();
                let required = dimension(r, "GAP").unwrap();
                let distance = (Euclidean
                    .distance(&polygon(&i.drill_outline()), &polygon(&j.drill_outline()))
                    * 1e6)
                    .round() as i64;
                if distance + 2 < required {
                    expected.add(
                        n,
                        r,
                        format!(
                            "Hole clearance {:.6} mm < {:.6} mm",
                            distance as f64 / 1e6,
                            required as f64 / 1e6
                        ),
                        &[i, j],
                        MULTI_LAYER,
                    );
                }
            }
        }
        let mut actual = Report::default();
        check_holes(&b, &mut actual, &mut 0);
        assert!(!expected.violations.is_empty());
        assert_eq!(keys(&actual), keys(&expected));
    }
    #[test]
    fn distant_drills_do_not_exhaust_pair_budget_and_disabled_check_does_no_work() {
        let mut b = Board::default();
        b.rules.push(default_rule("HoleToHoleClearance"));
        for n in 0..3000 {
            b.items.push(via(n * 2_000_000, 0));
        }
        let mut report = Report::default();
        let mut pairs = 0;
        check_holes(&b, &mut report, &mut pairs);
        assert_eq!(pairs, 0);
        assert!(!report.truncated);
        b.rules[0].insert("ENABLED".into(), "FALSE".into());
        b.items.push(via(0, 0));
        check_holes(&b, &mut report, &mut pairs);
        assert!(report.violations.is_empty());
    }
}
