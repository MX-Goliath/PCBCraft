use super::*;
use crate::{pcb_connectivity, pcb_polygon};
use geo::{BoundingRect, Contains, Distance, Euclidean, Intersects, LinesIter, Relate};
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "pcb_drc_holes.rs"]
mod holes;
#[path = "pcb_drc_mask.rs"]
mod mask;
#[path = "pcb_drc_geometry.rs"]
mod prepared_geometry;
#[path = "pcb_drc_rules.rs"]
mod prepared_rules;
use prepared_rules::Rules;
#[cfg(test)]
#[path = "pcb_drc_tests.rs"]
mod tests;
#[derive(Clone, Debug)]
pub struct Violation {
    pub rule: usize,
    pub kind: String,
    pub message: String,
    pub items: Vec<Uuid>,
    pub position: Point,
    pub layer: u16,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub violations: Vec<Violation>,
    pub diagnostics: BTreeSet<String>,
    pub truncated: bool,
    limit: usize,
    cancel: Option<Arc<AtomicBool>>,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            violations: vec![],
            diagnostics: BTreeSet::new(),
            truncated: false,
            limit: 5000,
            cancel: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub excluded: BTreeSet<String>,
    pub limit: usize,
}
impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            excluded: BTreeSet::new(),
            limit: 5000,
        }
    }
}
pub fn check_with_options(board: &Board, options: &CheckOptions) -> Report {
    let mut filtered = board.clone();
    for r in &mut filtered.rules {
        if options.excluded.contains(text(r, "RULEKIND")) {
            r.insert("ENABLED".into(), "FALSE".into());
        }
    }
    check_inner(&filtered, options.limit.clamp(1, 50_000), None, &mut |_| {})
}
/// Own the worker snapshot, avoiding a second clone. Cancelled checks publish no report.
pub fn check_owned_cancellable(
    mut board: Board,
    options: &CheckOptions,
    cancel: Arc<AtomicBool>,
) -> Option<Report> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    for rule in &mut board.rules {
        if options.excluded.contains(text(rule, "RULEKIND")) {
            rule.insert("ENABLED".into(), "FALSE".into());
        }
    }
    let mut report = check_inner(
        &board,
        options.limit.clamp(1, 50_000),
        Some(cancel.clone()),
        &mut |_| {},
    );
    report.cancel = None;
    (!cancel.load(Ordering::Relaxed)).then_some(report)
}
/// Read-only profiling entry point; time is never part of rule evaluation.
pub fn check_profiled(
    board: &Board,
    options: &CheckOptions,
) -> (Report, Vec<(&'static str, std::time::Duration)>) {
    let trace = std::env::var_os("PCBCRAFT_DRC_TRACE").is_some();
    let mut started = std::time::Instant::now();
    let mut phases = Vec::new();
    let mut filtered = board.clone();
    for rule in &mut filtered.rules {
        if options.excluded.contains(text(rule, "RULEKIND")) {
            rule.insert("ENABLED".into(), "FALSE".into());
        }
    }
    phases.push(("snapshot", started.elapsed()));
    started = std::time::Instant::now();
    let report = check_inner(
        &filtered,
        options.limit.clamp(1, 50_000),
        None,
        &mut |name| {
            if trace {
                eprintln!(
                    "DRC {name}: {:.3} ms",
                    started.elapsed().as_secs_f64() * 1000.
                );
            }
            phases.push((name, started.elapsed()));
            started = std::time::Instant::now();
        },
    );
    (report, phases)
}
impl Report {
    fn stopped(&mut self) -> bool {
        // One excess finding establishes truncation; an exactly-full complete
        // report keeps its existing complete status.
        self.truncated
            || self
                .cancel
                .as_ref()
                .is_some_and(|c| c.load(Ordering::Relaxed))
    }
    /// Standalone, escaped report of this snapshot. No external assets or scripts.
    pub fn html(&self, board: &Board, document: &str) -> String {
        use std::fmt::Write;
        fn escaped(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;")
        }
        let mut html = format!(
            "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Design Rule Verification Report</title><style>body{{font:14px Arial,sans-serif;margin:32px;color:#20252b}}table{{border-collapse:collapse;width:100%;margin-bottom:24px}}th,td{{border:1px solid #bbc3cc;padding:7px;text-align:left}}th{{background:#e4eaf0}}.warning{{color:#962500}}h1{{font-size:24px}}</style><h1>Design Rule Verification Report</h1><p>Document: {}</p><p>{} violations; {} unchecked conditions.</p>",
            escaped(document),
            self.violations.len(),
            self.diagnostics.len()
        );
        if self.truncated {
            html.push_str("<p class=\"warning\">Report incomplete: the configured check/report limit was reached.</p>");
        }
        if !self.diagnostics.is_empty() {
            html.push_str("<h2>Unchecked conditions</h2><ul class=\"warning\">");
            for d in &self.diagnostics {
                let _ = write!(html, "<li>{}</li>", escaped(d));
            }
            html.push_str("</ul>");
        }
        html.push_str(
            "<h2>Summary</h2><table><tr><th>Rule</th><th>Type</th><th>Violations</th></tr>",
        );
        let mut counts = std::collections::BTreeMap::new();
        for v in &self.violations {
            *counts.entry(v.rule).or_insert(0usize) += 1;
        }
        for (n, count) in counts {
            if let Some(r) = board.rules.get(n) {
                let _ = write!(
                    html,
                    "<tr><td>{}</td><td>{}</td><td>{count}</td></tr>",
                    escaped(text(r, "NAME")),
                    escaped(text(r, "RULEKIND"))
                );
            }
        }
        html.push_str("</table><h2>Violations</h2><table><tr><th>Rule</th><th>Description</th><th>Layer</th><th>X (mm)</th><th>Y (mm)</th><th>Objects</th></tr>");
        for v in &self.violations {
            let name = board
                .rules
                .get(v.rule)
                .map_or(v.kind.as_str(), |r| text(r, "NAME"));
            let ids = v
                .items
                .iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.6}</td><td>{:.6}</td><td>{ids}</td></tr>",
                escaped(name),
                escaped(&v.message),
                escaped(&layer_name(v.layer)),
                v.position.x as f64 / 1e6,
                v.position.y as f64 / 1e6
            );
        }
        html.push_str("</table><p>Generated by PCBCraft. Only the implemented checks and geometry listed above were evaluated.</p></html>");
        html
    }
    fn add(&mut self, rule: usize, r: &Fields, message: String, items: &[&Item], layer: u16) {
        if self.violations.len() >= self.limit {
            self.truncated = true;
            return;
        }
        self.violations.push(Violation {
            rule,
            kind: text(r, "RULEKIND").into(),
            message,
            items: items.iter().map(|i| i.id).collect(),
            position: items.first().map_or(Point::default(), |i| i.anchor()),
            layer,
        });
    }
}
/// Geometry is evaluated per physical copper layer, independent of display visibility.
/// Polygon tessellation is shared with copper connectivity. Limits are explicit.
pub fn check(board: &Board) -> Report {
    check_inner(board, 5000, None, &mut |_| {})
}
// Native compatibility records and body projections are retained for lossless
// persistence/display, but do not describe additional manufactured copper.
fn checkable_item(item: &Item) -> bool {
    !item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        && text(&item.properties, "PCBCRAFT_LEGACY_REGION_DUPLICATE") != "TRUE"
}
// Object-specific keepouts are clearance obstacles, not electrical copper.
fn keepout_restricts(keepout: &Item, item: &Item) -> Result<bool, String> {
    let mask = match keepout.properties.get("KEEPOUTRESTRICTIONS") {
        Some(value) => value
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 31)
            .ok_or_else(|| {
                "Keepout restriction mask is invalid; clearance is not fully checked".to_owned()
            })?,
        None => 31, // Legacy unrestricted keepout.
    };
    let bit = match item.shape {
        Shape::Via { .. } => 1,
        Shape::Track { .. } | Shape::Arc { .. } => 2,
        Shape::Pad { hole: 0, .. } => 8,
        Shape::Pad { .. } => 16,
        _ => 4,
    };
    Ok(mask & bit != 0)
}
fn check_inner(
    board: &Board,
    limit: usize,
    cancel: Option<Arc<AtomicBool>>,
    phase: &mut dyn FnMut(&'static str),
) -> Report {
    let rules = Rules::with_cancel(board, cancel.clone());
    let mut out = Report {
        limit,
        cancel,
        ..Default::default()
    };
    for r in board.rules.iter().filter(|r| enabled(r)) {
        if text(r, "RULEKIND") == "UnRoutedNet"
            && text(r, "CHECKBADCONNECTIONS").eq_ignore_ascii_case("TRUE")
        {
            out.diagnostics.insert(format!(
                "{}: poor/incomplete contact verification (CHECKBADCONNECTIONS) is not checked",
                text(r, "NAME")
            ));
        }
        if text(r, "RULEKIND") == "PolygonConnect"
            && let Err(e) = super::polygon::validate_connection(r)
        {
            out.diagnostics.insert(e);
        }
        if !supported(text(r, "RULEKIND"))
            && !matches!(
                text(r, "RULEKIND"),
                "SolderMaskExpansion" | "PasteMaskExpansion"
            )
        {
            out.diagnostics.insert(format!(
                "{}: {} is not checked",
                text(r, "NAME"),
                text(r, "RULEKIND")
            ));
        }
    }
    for item in board.items.iter().filter(|i| checkable_item(i)) {
        if out.stopped() {
            phase("item_rules");
            return out;
        }
        if item.keepout {
            if rules.active("Clearance")
                && let Err(error) = keepout_restricts(item, item)
            {
                out.diagnostics.insert(error);
            }
            continue;
        }
        if matches!(item.shape, Shape::Text { .. }) {
            if rules.active("Clearance")
                && board.layers.iter().any(|l| l.copper && l.id == item.layer)
            {
                out.diagnostics
                    .insert("Clearance: copper text glyph geometry is not checked".into());
            }
            if board.layers.iter().any(|l| {
                (l.copper || matches!(l.id, TOP_OVERLAY | BOTTOM_OVERLAY)) && l.id == item.layer
            }) {
                match rules.resolve("BoardOutlineClearance", item, None) {
                    Ok(Some((_, rule))) => match gap_objects(rule, "Text", "OutlineEdge") {
                        Ok(gap) if gap > 0 => {
                            out.diagnostics.insert(format!(
                                "{}: text board-outline geometry is not checked",
                                text(rule, "NAME")
                            ));
                        }
                        Err(error) => {
                            out.diagnostics.insert(error);
                        }
                        _ => {}
                    },
                    Err(error) => {
                        out.diagnostics.insert(error);
                    }
                    _ => {}
                }
            }
        }
        if item.drill_size() > 0 {
            match rules.resolve("LayerPairs", item, None) {
                Ok(Some((n, r))) => match layer_pairs_violation(board, item, r) {
                    Ok(Some(message)) => out.add(n, r, message, &[item], MULTI_LAYER),
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
        for kind in ["Width", "RoutingLayers", "RoutingVias"] {
            if !route_rule_applicable(item, kind) {
                continue;
            }
            match rules
                .resolve(kind, item, None)
                .and_then(|rule| route_violation_resolved(item, kind, rule))
            {
                Ok(Some((n, message))) => out.add(n, &board.rules[n], message, &[item], item.layer),
                Err(e) => {
                    out.diagnostics.insert(e);
                }
                _ => {}
            }
        }
        if item.drill_size() > 0 {
            match rules.resolve("HoleSize", item, None) {
                Ok(Some((n, r))) => match hole_size_violation(board, item, r) {
                    Ok(Some(message)) => out.add(n, r, message, &[item], item.layer),
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
    phase("item_rules");
    let outline = polygon(&board.outline_points(256));
    let prepared_outline = geo::PreparedGeometry::from(&outline);
    let outline_edges = rstar::RTree::bulk_load(
        outline
            .exterior()
            .lines_iter()
            .map(rstar::primitives::CachedEnvelope::new)
            .collect(),
    );
    let mut annular = std::collections::HashMap::<usize, Vec<(geo::MultiPolygon<f64>, f64)>>::new();
    let cutouts: Vec<_> = board
        .items
        .iter()
        .filter(|i| {
            checkable_item(i)
                && matches!(i.shape, Shape::Region { cutout: true, .. })
                && text(&i.properties, "ISBOARDCUTOUT") == "TRUE"
        })
        .collect();
    let edge_checks = rules.active("MinimumAnnularRing")
        || rules.active("BoardOutlineClearance")
        || (!cutouts.is_empty() && rules.active("Clearance"));
    for layer in board
        .layers
        .iter()
        .filter(|l| edge_checks && (l.copper || matches!(l.id, TOP_OVERLAY | BOTTOM_OVERLAY)))
    {
        for item in board.items.iter().filter(|i| {
            checkable_item(i)
                && pcb_polygon::on_layer(board, i, layer.id)
                && !i.keepout
                && !matches!(
                    i.shape,
                    Shape::Text { .. } | Shape::Region { cutout: true, .. }
                )
        }) {
            if out.stopped() {
                phase("outline_annular");
                return out;
            }
            let copper = pcb_polygon::copper(item, layer.id);
            if copper.0.is_empty() {
                continue;
            }
            if item.drill_size() > 0 && !matches!(item.shape, Shape::Pad { plated: false, .. }) {
                match rules.resolve("MinimumAnnularRing", item, None) {
                    Ok(Some((n, r))) => match dimension(r, "MINIMUMRING") {
                        Ok(required) => {
                            let lands = annular.entry(item as *const Item as usize).or_default();
                            let actual = if let Some(actual) =
                                circular_annular_ring_nm(item, layer.id)
                            {
                                actual
                            } else if let Some((_, actual)) =
                                lands.iter().find(|(shape, _)| *shape == copper)
                            {
                                *actual
                            } else {
                                let drill = polygon(&item.drill_outline());
                                let actual = if copper.contains(&drill) {
                                    copper
                                        .0
                                        .iter()
                                        .map(|p| Euclidean.distance(p.exterior(), drill.exterior()))
                                        .fold(f64::INFINITY, f64::min)
                                        * 1e6
                                } else {
                                    0.0
                                };
                                lands.push((copper.clone(), actual));
                                actual
                            };
                            if actual + 2.0 < required as f64 {
                                out.add(
                                    n,
                                    r,
                                    format!(
                                        "Annular ring {:.4} mm < {:.4} mm",
                                        actual / 1e6,
                                        required as f64 / 1e6
                                    ),
                                    &[item],
                                    layer.id,
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
            if outline.exterior().0.len() >= 4 {
                match rules.resolve("BoardOutlineClearance", item, None) {
                    Ok(Some((n, r))) => match gap_objects(r, object_kind(item), "OutlineEdge") {
                        Ok(required) if required > 0 => {
                            // No outline edge in the expanded box proves that
                            // no contour crosses an edge or violates this gap.
                            // Classify each island by one point in that case.
                            let far = copper.bounding_rect().is_some_and(|bounds| {
                                let gap = required as f64 / 1e6;
                                let envelope = rstar::AABB::from_corners(
                                    geo::Point::new(bounds.min().x - gap, bounds.min().y - gap),
                                    geo::Point::new(bounds.max().x + gap, bounds.max().y + gap),
                                );
                                outline_edges
                                    .locate_in_envelope_intersecting(&envelope)
                                    .next()
                                    .is_none()
                            });
                            let outside = !copper.0.iter().all(|p| {
                                if far {
                                    p.exterior().0.first().is_some_and(|p| outline.contains(p))
                                } else {
                                    prepared_outline.relate(p).is_contains()
                                }
                            });
                            let distance = if outside {
                                0
                            } else if far {
                                required
                            } else {
                                (Euclidean.distance(&copper, outline.exterior()) * 1e6).round()
                                    as i64
                            };
                            if outside || distance + 2 < required {
                                out.add(
                                    n,
                                    r,
                                    if outside {
                                        "Copper crosses the board outline".into()
                                    } else {
                                        format!(
                                            "Board outline clearance {:.6} mm < {:.6} mm",
                                            distance as f64 / 1e6,
                                            required as f64 / 1e6
                                        )
                                    },
                                    &[item],
                                    layer.id,
                                );
                            }
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
            for cutout in &cutouts {
                if out.stopped() {
                    phase("outline_annular");
                    return out;
                }
                match cutout_rule(board, item, cutout) {
                    Ok(Some((n, r, required))) if required > 0 => {
                        let distance = (Euclidean
                            .distance(&copper, &pcb_polygon::copper(cutout, layer.id))
                            * 1e6)
                            .round() as i64;
                        if distance + 2 < required {
                            out.add(
                                n,
                                r,
                                format!(
                                    "Board cutout clearance {:.6} mm < {:.6} mm",
                                    distance as f64 / 1e6,
                                    required as f64 / 1e6
                                ),
                                &[item, cutout],
                                layer.id,
                            );
                        }
                    }
                    Err(e) => {
                        out.diagnostics.insert(e);
                    }
                    _ => {}
                }
            }
        }
    }
    phase("outline_annular");
    for p in &board.polygon_pours {
        if out.stopped() {
            phase("pour_status");
            return out;
        }
        let mut item = Item::new(
            p.layer,
            p.net,
            Shape::Region {
                contour: p.contour.iter().map(|v| v.position).collect(),
                holes: vec![],
                cutout: false,
            },
        );
        item.polygon = Some(p.index);
        item.id = p.id;
        match rules.resolve("UnpouredPolygon", &item, None) {
            Ok(Some((n, r))) => {
                if (p.shelved && text(r, "ALLOWSHELVED") != "TRUE")
                    || (!p.shelved && board.pours_stale && text(r, "ALLOWUNPOURED") != "TRUE")
                {
                    out.add(
                        n,
                        r,
                        format!(
                            "Polygon {} is {}",
                            p.name,
                            if p.shelved {
                                "shelved"
                            } else {
                                "not up to date"
                            }
                        ),
                        &[&item],
                        p.layer,
                    );
                }
            }
            Err(e) => {
                out.diagnostics.insert(e);
            }
            _ => {}
        }
    }
    phase("pour_status");
    let mut max_gap = 0i64;
    for r in board
        .rules
        .iter()
        .filter(|r| enabled(r) && text(r, "RULEKIND") == "Clearance")
    {
        max_gap = max_gap.max(dimension(r, "GAP").unwrap_or(0));
        for entry in text(r, "OBJECTCLEARANCES").split(';') {
            if let Some((_, v)) = entry.split_once(':') {
                max_gap = max_gap.max(v.parse::<f64>().map_or(0, |v| (v * 2.54).round() as i64));
            }
        }
    }
    let mut pairs = 0usize;
    'layers: for layer in board
        .layers
        .iter()
        .filter(|l| l.copper && (rules.active("Clearance") || rules.active("ShortCircuit")))
    {
        let mut items: Vec<_> = board
            .items
            .iter()
            .filter(|i| {
                checkable_item(i)
                    && !i.is_drill_only()
                    && !matches!(
                        i.shape,
                        Shape::Text { .. } | Shape::Region { cutout: true, .. }
                    )
                    && (pcb_polygon::on_layer(board, i, layer.id)
                        || (i.keepout && i.layer == KEEP_OUT))
                    && !(board.pours_stale && i.polygon.is_some())
            })
            .collect();
        items.sort_by_cached_key(|i| i.bounds().min.x);
        let gaps = rules.clearance_bounds(&items);
        let boxes: Vec<_> = items.iter().map(|i| i.bounds()).collect();
        let nets: Vec<_> = items.iter().map(|i| board.item_net(i)).collect();
        let geometry: Vec<std::cell::OnceCell<prepared_geometry::Geometry>> =
            items.iter().map(|_| std::cell::OnceCell::new()).collect();
        for (a, i) in items.iter().enumerate() {
            if out.stopped() {
                phase("copper_pairs");
                return out;
            }
            let bounds = boxes[a].expanded(gaps.as_ref().map_or(max_gap, |g| g[a]));
            for b in a + 1..items.len() {
                if out.stopped() {
                    phase("copper_pairs");
                    return out;
                }
                let j = items[b];
                let other = boxes[b];
                if other.min.x > bounds.max.x {
                    break;
                }
                if other.max.y < bounds.min.y || other.min.y > bounds.max.y {
                    continue;
                }
                if i.keepout || j.keepout {
                    if i.keepout && j.keepout {
                        continue;
                    }
                    let (keepout, copper) = if i.keepout { (*i, j) } else { (j, *i) };
                    match keepout_restricts(keepout, copper) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(error) => {
                            out.diagnostics.insert(error);
                            continue;
                        }
                    }
                }
                pairs += 1;
                if pairs > 2_000_000 {
                    out.truncated = true;
                    out.diagnostics
                        .insert("Pair budget reached; DRC is incomplete".into());
                    break 'layers;
                }
                let first = geometry[a].get_or_init(|| {
                    prepared_geometry::Geometry::new(pcb_polygon::copper(i, layer.id))
                });
                let second = geometry[b].get_or_init(|| {
                    prepared_geometry::Geometry::new(pcb_polygon::copper(j, layer.id))
                });
                if first.shape.0.is_empty() || second.shape.0.is_empty() {
                    continue;
                }
                let intersects = !i.keepout
                    && !j.keepout
                    && (nets[a].is_none() || nets[a] != nets[b])
                    && rules.active("ShortCircuit")
                    && first.intersects(second);
                if intersects {
                    match rules.resolve("ShortCircuit", i, Some(j)) {
                        Ok(Some((n, r)))
                            if text(r, "ALLOWED") != "TRUE" && !net_tie_pair(board, i, j) =>
                        {
                            out.add(
                                n,
                                r,
                                "Copper objects on different nets intersect".into(),
                                &[i, j],
                                layer.id,
                            )
                        }
                        Err(e) => {
                            out.diagnostics.insert(e);
                        }
                        _ => {}
                    }
                }
                if out.stopped() {
                    phase("copper_pairs");
                    return out;
                }
                match rules.resolve("Clearance", i, Some(j)) {
                    Ok(Some((n, r))) => match rules.clearance(n, r, i, j) {
                        Ok(required) if required > 2 => {
                            let distance = if intersects {
                                0
                            } else {
                                let Some(distance) =
                                    first.distance_within(second, required as f64 / 1e6)
                                else {
                                    continue;
                                };
                                (distance * 1e6).round() as i64
                            };
                            // AD 24.4.1 copper probes accept a 48.26 nm deficit
                            // but report 50.8 nm (19/20 native coordinate units).
                            // This measured 50 nm margin is electrical only:
                            // solder-mask slivers report even a 2.54 nm deficit.
                            if distance + 50 < required {
                                out.add(
                                    n,
                                    r,
                                    format!(
                                        "Clearance {:.6} mm < {:.6} mm",
                                        distance as f64 / 1e6,
                                        required as f64 / 1e6
                                    ),
                                    &[i, j],
                                    layer.id,
                                );
                            }
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
    phase("copper_pairs");
    if out.stopped() {
        return out;
    }
    holes::check_holes_with_rules(board, &rules, &mut out, &mut pairs);
    phase("holes");
    if out.stopped() {
        return out;
    }
    if board
        .rules
        .iter()
        .any(|r| enabled(r) && text(r, "RULEKIND") == "UnRoutedNet")
    {
        let Some(connections) = pcb_connectivity::connections_cancellable(board, &|| {
            out.cancel
                .as_ref()
                .is_some_and(|c| c.load(Ordering::Relaxed))
        }) else {
            return out;
        };
        for c in connections {
            if out.stopped() {
                phase("connectivity");
                return out;
            }
            let item = probe(TOP, Some(c.net), c.start, 1);
            match rules.resolve("UnRoutedNet", &item, None) {
                Ok(Some((n, r))) => {
                    let previous = out.violations.len();
                    out.add(
                        n,
                        r,
                        format!("Un-routed connection: {}", board.net_name(Some(c.net))),
                        &[],
                        MULTI_LAYER,
                    );
                    if out.violations.len() > previous
                        && let Some(v) = out.violations.last_mut()
                    {
                        v.position = c.start;
                    }
                }
                Err(e) => {
                    out.diagnostics.insert(e);
                }
                _ => {}
            }
        }
    }
    phase("connectivity");
    if out.stopped() {
        return out;
    }
    mask::check_masks_with_rules(board, &rules, &mut out);
    phase("masks");
    if board.pours_stale {
        out.diagnostics
            .insert("Polygon pours are stale; repour before relying on DRC".into());
    }
    out
}

fn polygon(points: &[Point]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        points
            .iter()
            .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
            .collect::<Vec<_>>()
            .into(),
        vec![],
    )
}
