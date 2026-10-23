//! Manufacturing checks use the same apertures as mask display/export.
use super::*;
use geo::{BooleanOps, BoundingRect, MultiPolygon};

pub(super) fn check_masks(board: &Board, out: &mut Report) {
    let active = |kind| {
        board
            .rules
            .iter()
            .any(|r| enabled(r) && text(r, "RULEKIND") == kind)
    };
    let sliver = active("MinimumSolderMaskSliver");
    let silk = active("SilkToSolderMaskClearance");
    if !sliver && !silk {
        return;
    }
    let mut pairs = 0usize;
    for bottom in [false, true] {
        let layer = if bottom { BOTTOM } else { TOP };
        let mask_layer = if bottom { 38 } else { 37 };
        let silk_layer = if bottom { BOTTOM_OVERLAY } else { TOP_OVERLAY };
        let mut openings = Vec::new();
        for item in board.items.iter().filter(|i| !i.keepout) {
            let geometry = if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                match item.mask_outline(board, MaskKind::Solder, bottom) {
                    Ok(points) if points.len() >= 3 => MultiPolygon(vec![polygon(&points)]),
                    Ok(_) => continue,
                    Err(e) => {
                        out.diagnostics.insert(e);
                        continue;
                    }
                }
            } else if item.layer == mask_layer {
                if matches!(item.shape, Shape::Text { .. }) {
                    out.diagnostics
                        .insert("Mask text outlines are not checked".into());
                    continue;
                }
                pcb_polygon::copper(item, mask_layer)
            } else {
                continue;
            };
            if !geometry.0.is_empty() {
                openings.push((item, geometry));
            }
        }
        if sliver {
            let mut max_gap = 0.0_f64;
            for r in board
                .rules
                .iter()
                .filter(|r| enabled(r) && text(r, "RULEKIND") == "MinimumSolderMaskSliver")
            {
                match dimension(r, "MINSOLDERMASKWIDTH") {
                    Ok(n) => max_gap = max_gap.max(n as f64 / 1e6),
                    Err(e) => {
                        out.diagnostics.insert(e);
                    }
                }
            }
            // Joined apertures form one opening, including manual bridges between pads.
            // Checking individual pad rectangles would report fictitious strips here.
            // Only boolean-union intersecting aperture groups. Re-unioning every
            // disjoint pad into one global polygon makes online DRC quadratic in
            // all contour vertices, even though most apertures never meet.
            let mut groups: Vec<(MultiPolygon<f64>, Vec<&Item>, geo::Rect<f64>)> = Vec::new();
            for (item, shape) in &openings {
                let mut shape = shape.clone();
                let mut owners = vec![*item];
                let mut bounds = shape.bounding_rect().unwrap();
                let mut n = 0;
                while n < groups.len() {
                    if bounds.intersects(&groups[n].2) && shape.intersects(&groups[n].0) {
                        let (other, other_owners, _) = groups.swap_remove(n);
                        shape = shape.union(&other);
                        owners.extend(other_owners);
                        bounds = shape.bounding_rect().unwrap();
                        n = 0;
                    } else {
                        n += 1;
                    }
                }
                groups.push((shape, owners, bounds));
            }
            groups.sort_by(|a, b| a.2.min().x.total_cmp(&b.2.min().x));
            let boxes: Vec<_> = groups.iter().map(|(_, _, b)| *b).collect();
            let owners: Vec<_> = groups.iter().map(|(_, o, _)| o).collect();
            for (a, (shape, _, _)) in groups.iter().enumerate() {
                for (b, (other, _, _)) in groups.iter().enumerate().skip(a + 1) {
                    if boxes[b].min().x > boxes[a].max().x + max_gap {
                        break;
                    }
                    if boxes[b].max().y + max_gap < boxes[a].min().y
                        || boxes[b].min().y > boxes[a].max().y + max_gap
                    {
                        continue;
                    }
                    pairs += 1;
                    if pairs > 2_000_000 {
                        out.truncated = true;
                        out.diagnostics
                            .insert("Mask pair budget reached; DRC is incomplete".into());
                        return;
                    }
                    let distance = (Euclidean.distance(shape, other) * 1e6).round() as i64;
                    let mut reported = BTreeSet::new();
                    for i in owners[a] {
                        for j in owners[b] {
                            match resolve(board, "MinimumSolderMaskSliver", i, Some(j)) {
                                Ok(Some((n, r))) => match dimension(r, "MINSOLDERMASKWIDTH") {
                                    Ok(required)
                                        if distance + 2 < required && reported.insert(n) =>
                                    {
                                        out.add(
                                            n,
                                            r,
                                            format!(
                                                "Solder mask sliver {:.6} mm < {:.6} mm",
                                                distance as f64 / 1e6,
                                                required as f64 / 1e6
                                            ),
                                            &[i, j],
                                            mask_layer,
                                        )
                                    }
                                    Err(e) => {
                                        out.diagnostics.insert(e);
                                    }
                                    _ => {}
                                },
                                Err(e) => {
                                    out.diagnostics.insert(e);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if silk {
            for item in board
                .items
                .iter()
                .filter(|i| i.layer == silk_layer && !i.keepout)
            {
                if matches!(item.shape, Shape::Text { .. }) {
                    out.diagnostics
                        .insert("Silk text glyph outlines are not checked".into());
                    continue;
                }
                let geometry = pcb_polygon::copper(item, silk_layer);
                if geometry.0.is_empty() {
                    continue;
                }
                for (other, opening) in &openings {
                    pairs += 1;
                    if pairs > 2_000_000 {
                        out.truncated = true;
                        out.diagnostics
                            .insert("Silk/mask pair budget reached; DRC is incomplete".into());
                        return;
                    }
                    match resolve(board, "SilkToSolderMaskClearance", item, Some(other)) {
                        Ok(Some((n, r))) => match dimension(r, "MINSILKSCREENTOMASKGAP") {
                            Ok(required) => {
                                let target = if text(r, "CLEARANCETOEXPOSEDCOPPER") == "TRUE" {
                                    // The exposed-copper mode targets component pads.
                                    if !matches!(other.shape, Shape::Pad { .. }) {
                                        continue;
                                    }
                                    opening.intersection(&pcb_polygon::copper(other, layer))
                                } else {
                                    opening.clone()
                                };
                                if target.0.is_empty() {
                                    continue;
                                }
                                if let (Some(a), Some(b)) =
                                    (geometry.bounding_rect(), target.bounding_rect())
                                {
                                    let gap = required as f64 / 1e6;
                                    if a.min().x > b.max().x + gap
                                        || a.max().x + gap < b.min().x
                                        || a.min().y > b.max().y + gap
                                        || a.max().y + gap < b.min().y
                                    {
                                        continue;
                                    }
                                }
                                let distance =
                                    (Euclidean.distance(&geometry, &target) * 1e6).round() as i64;
                                if distance + 2 < required {
                                    out.add(
                                        n,
                                        r,
                                        format!(
                                            "Silk to solder mask clearance {:.6} mm < {:.6} mm",
                                            distance as f64 / 1e6,
                                            required as f64 / 1e6
                                        ),
                                        &[item, other],
                                        silk_layer,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn board() -> Board {
        let mut b = Board::default();
        for x in [0, 1_250_000] {
            b.items.push(Item::new(
                TOP,
                None,
                Shape::Pad {
                    position: Point { x, y: 0 },
                    size: Point::mm(1, 1),
                    rotation: 0.0,
                    shape: PadShape::Rectangle,
                    hole: 0,
                    plated: false,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            ));
        }
        b.rules = vec![
            default_rule("MinimumSolderMaskSliver"),
            default_rule("SilkToSolderMaskClearance"),
        ];
        b
    }
    #[test]
    fn sliver_uses_mask_expansion_and_unions_manual_apertures() {
        let mut b = board();
        let r = check(&b);
        assert_eq!(r.violations.len(), 1);
        assert!(r.violations[0].message.contains("0.050000"));
        assert_eq!(r.violations[0].layer, 37);
        b.items.push(Item::new(
            37,
            None,
            Shape::Fill {
                start: Point { x: 0, y: -500_000 },
                end: Point {
                    x: 1_250_000,
                    y: 500_000,
                },
                rotation: 0.0,
            },
        ));
        assert!(check(&b).violations.is_empty());
        b.items.pop();
        let mut settings = MaskSettings::for_item(&b.items[1]);
        settings.solder[0].tented = true;
        b.items[1].masks = Some(Box::new(settings));
        assert!(check(&b).violations.is_empty());
    }
    #[test]
    fn silk_mask_exposed_copper_scopes_and_opposite_side() {
        let mut b = board();
        b.items.truncate(1);
        b.rules[1].insert("MINSILKSCREENTOMASKGAP".into(), "0.02mm".into());
        b.rules[1].insert("SCOPE1EXPRESSION".into(), "IsPad".into());
        b.items.push(Item::new(
            TOP_OVERLAY,
            None,
            Shape::Track {
                start: Point {
                    x: 580_000,
                    y: -400_000,
                },
                end: Point {
                    x: 580_000,
                    y: 400_000,
                },
                width: 20_000,
            },
        ));
        assert_eq!(check(&b).violations.len(), 1);
        b.rules[1].insert("CLEARANCETOEXPOSEDCOPPER".into(), "TRUE".into());
        assert!(check(&b).violations.is_empty());
        b.rules[1].insert("CLEARANCETOEXPOSEDCOPPER".into(), "FALSE".into());
        b.items[1].layer = BOTTOM_OVERLAY;
        assert!(check(&b).violations.is_empty());
    }
}
