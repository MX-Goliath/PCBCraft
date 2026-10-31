//! Interactive routing session with bounded obstacle avoidance. No shove or full DRC.
//! A session owns a draft; cancel never mutates the original document. Completing
//! the session is one History transaction, including splits and layer-change vias.
use crate::{pcb::*, pcb_polygon::on_layer};
use eda_geom::Point;
use std::collections::BTreeSet;
use uuid::Uuid;
#[path = "pcb_route_obstacles.rs"]
mod obstacles;
pub use obstacles::Conflict;
#[path = "pcb_route_drag.rs"]
mod drag;
pub use drag::{DragOptions, drag_track_endpoint, drag_tracks, track_selection};
#[path = "pcb_route_preview.rs"]
mod preview;
pub use preview::RoutePreview;

/// Resolve an unconditional preferred routing dimension. Unknown higher-priority
/// scopes stop resolution; they must not silently fall back to a generic rule.
pub fn preferred_dimension(board: &Board, kind: &str, field: &str) -> Option<i64> {
    let mut rules: Vec<_> = board
        .rules
        .iter()
        .filter(|r| {
            r.get("RULEKIND")
                .is_some_and(|k| k.eq_ignore_ascii_case(kind))
                && !r
                    .get("ENABLED")
                    .is_some_and(|v| v.eq_ignore_ascii_case("FALSE") || v == "0")
        })
        .collect();
    rules.sort_by_key(|r| {
        r.get("PRIORITY")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    let rule = rules.first()?;
    if !rule
        .get("SCOPE1EXPRESSION")
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("All"))
    {
        return None;
    }
    dimension_nm(rule.get(field)?).ok().filter(|v| *v > 0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Corner {
    #[default]
    FortyFive,
    Orthogonal,
    AnyAngle,
}
#[derive(Clone)]
pub struct RouteSession {
    pub revision: u64,
    draft: Board,
    pub anchor: Point,
    pub layer: u16,
    pub net: Option<u32>,
    pub width: i64,
    pub corner: Corner,
    pub diagonal_first: bool,
    pub conflict: Conflict,
    pub clearance: i64,
    source_len: usize,
    contacts: BTreeSet<Uuid>,
    obstacles: obstacles::Obstacles,
    checkpoints: Vec<Checkpoint>,
    preview: preview::Cache,
}
#[derive(Clone)]
struct Checkpoint {
    items_len: usize,
    restored: Vec<(usize, Item)>,
    anchor: Point,
    layer: u16,
    pours_stale: bool,
    contacts: BTreeSet<Uuid>,
}
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub item: Uuid,
    pub point: Point,
    pub net: Option<u32>,
}
fn project(p: Point, a: Point, b: Point) -> Point {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let d = dx * dx + dy * dy;
    let t = if d == 0.0 {
        0.0
    } else {
        (((p.x - a.x) as f64 * dx + (p.y - a.y) as f64 * dy) / d).clamp(0.0, 1.0)
    };
    Point {
        x: (a.x as f64 + t * dx).round() as i64,
        y: (a.y as f64 + t * dy).round() as i64,
    }
}
pub fn target(board: &Board, p: Point, layer: u16, tolerance: i64) -> Option<Target> {
    board
        .items
        .iter()
        .filter(|i| {
            on_layer(board, i, layer) && !i.keepout && !i.is_drill_only() && i.polygon.is_none()
        })
        .filter_map(|i| {
            let (at, priority) = match i.shape {
                Shape::Pad { position, .. } | Shape::Via { position, .. } => (position, 0),
                Shape::Track { start, end, .. } => (project(p, start, end), 1),
                _ => return None,
            };
            let distance = hit_distance(i, p);
            (distance <= tolerance as f64).then_some((
                Target {
                    item: i.id,
                    point: at,
                    net: i.net,
                },
                priority,
                distance,
            ))
        })
        .min_by(|a, b| {
            // A direct copper hit wins over a nearby pad within the pick radius.
            (a.2 > 2.0)
                .cmp(&(b.2 > 2.0))
                .then(a.1.cmp(&b.1))
                .then_with(|| a.2.total_cmp(&b.2))
        })
        .map(|x| x.0)
}
fn split_at(board: &mut Board, p: Point, layer: u16, net: Option<u32>) -> Result<(), String> {
    let mut additions = Vec::new();
    for i in &mut board.items {
        if i.net != net || i.layer != layer || i.polygon.is_some() || i.component.is_some() {
            continue;
        }
        if let Shape::Track { start, end, width } = i.shape {
            // Native quantization can differ by a nanometre, but a large geometric
            // picking tolerance must never silently move an existing endpoint.
            if p != start && p != end && segment_distance(p, start, end) <= 2.0 {
                if i.locked {
                    return Err("Cannot split a locked track".into());
                }
                let mut other = i.clone();
                other.id = Uuid::new_v4();
                other.source = None;
                other.shape = Shape::Track {
                    start: p,
                    end,
                    width,
                };
                i.shape = Shape::Track {
                    start,
                    end: p,
                    width,
                };
                additions.push(other);
            }
        }
    }
    board.items.extend(additions);
    Ok(())
}
impl RouteSession {
    pub fn begin(
        board: &Board,
        revision: u64,
        p: Point,
        layer: u16,
        net: Option<u32>,
        width: i64,
        tolerance: i64,
    ) -> Result<Self, String> {
        if !copper_layer(layer) {
            return Err("Route on a copper layer".into());
        }
        if width <= 0 {
            return Err("Track width must be positive".into());
        }
        let hit = target(board, p, layer, tolerance);
        let anchor = hit.map_or(p, |t| t.point);
        let net = hit.map_or(net, |t| t.net);
        let probe = crate::pcb_rules::probe(layer, net, anchor, width);
        let width =
            crate::pcb_rules::preferred(board, &probe, "Width", "PREFEREDWIDTH")?.unwrap_or(width);
        crate::pcb_rules::route_check(board, &crate::pcb_rules::probe(layer, net, anchor, width))?;
        let mut draft = board.clone();
        split_at(&mut draft, anchor, layer, net)?;
        draft.validate()?;
        // No Net is not a common electrical net. Only the copper touched at
        // startup and the session's own additions may merge with this route.
        let contacts = board
            .items
            .iter()
            .filter(|i| {
                net.is_none()
                    && i.net.is_none()
                    && !i.keepout
                    && !i.is_drill_only()
                    && i.polygon.is_none()
                    && on_layer(board, i, layer)
                    && matches!(
                        i.shape,
                        Shape::Pad { .. } | Shape::Via { .. } | Shape::Track { .. }
                    )
                    && hit_distance(i, anchor) <= 2.0
            })
            .map(|i| i.id)
            .collect();
        let obstacles =
            obstacles::Obstacles::for_contacts(&draft, layer, &probe, &contacts, board.items.len());
        obstacles.error()?;
        let preview = preview::Cache::new(board, &draft);
        Ok(Self {
            revision,
            draft,
            anchor,
            layer,
            net,
            width,
            corner: Corner::FortyFive,
            diagonal_first: true,
            conflict: Conflict::Walkaround,
            clearance: 200_000,
            source_len: board.items.len(),
            contacts,
            obstacles,
            checkpoints: vec![],
            preview,
        })
    }
    fn checkpoint(&self, next: &Board) -> Checkpoint {
        let restored = self
            .draft
            .items
            .iter()
            .zip(&next.items)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(n, (a, _))| (n, a.clone()))
            .collect();
        Checkpoint {
            items_len: self.draft.items.len(),
            restored,
            anchor: self.anchor,
            layer: self.layer,
            pours_stale: self.draft.pours_stale,
            contacts: self.contacts.clone(),
        }
    }
    pub fn has_segments(&self) -> bool {
        !self.checkpoints.is_empty()
    }
    pub fn draft(&self) -> &Board {
        &self.draft
    }
    pub fn points(&self, end: Point) -> Vec<Point> {
        if !(1..=100_000_000_000).contains(&self.width)
            || !(0..=100_000_000_000).contains(&self.clearance)
        {
            return vec![self.anchor];
        }
        self.obstacles.resolve(self, end)
    }
    fn endpoint_contact(&self, end: Point) -> Option<Uuid> {
        // Connected dragging uses a separately prepared environment and must
        // never acquire a new no-net contact merely by dropping onto copper.
        (self.net.is_none() && self.source_len != usize::MAX)
            .then(|| target(&self.draft, end, self.layer, 0))
            .flatten()
            .filter(|t| t.net.is_none() && t.point == end)
            .map(|t| t.item)
    }
    fn free_points(&self, start: Point, end: Point) -> Vec<Point> {
        match self.corner {
            Corner::FortyFive => route_points(start, end, self.diagonal_first),
            Corner::Orthogonal => {
                let c = if self.diagonal_first {
                    Point {
                        x: start.x,
                        y: end.y,
                    }
                } else {
                    Point {
                        x: end.x,
                        y: start.y,
                    }
                };
                let mut v = vec![start];
                if c != start && c != end {
                    v.push(c);
                }
                if end != start {
                    v.push(end);
                }
                v
            }
            Corner::AnyAngle => {
                if end == start {
                    vec![start]
                } else {
                    vec![start, end]
                }
            }
        }
    }
    fn append_to(&self, mut board: Board, end: Point) -> Result<Board, String> {
        if !(1..=100_000_000_000).contains(&self.width)
            || !(0..=100_000_000_000).contains(&self.clearance)
        {
            return Err("Width must be positive and clearance non-negative".into());
        }
        crate::pcb_rules::route_check(
            &board,
            &crate::pcb_rules::probe(self.layer, self.net, self.anchor, self.width),
        )?;
        self.obstacles.error()?;
        let original_len = board.items.len();
        for pair in self.points(end).windows(2) {
            if pair[0] != pair[1] {
                board.items.push(Item::new(
                    self.layer,
                    self.net,
                    Shape::Track {
                        start: pair[0],
                        end: pair[1],
                        width: self.width,
                    },
                ));
            }
        }
        board.pours_stale |= board.items.len() != original_len && !board.polygon_pours.is_empty();
        board.validate()?;
        Ok(board)
    }
    pub fn preview(&self, end: Point) -> Result<Board, String> {
        self.append_to(self.draft.clone(), end)
    }
    /// Place one route leg. The caller supplies a snap target derived from the
    /// original board, so the route cannot finish against its own preview.
    pub fn place(&mut self, end: Point, hit: Option<Target>) -> Result<(), String> {
        let end = hit.map_or(end, |t| t.point);
        if let Some(hit) = hit
            && hit.net != self.net
        {
            return Err("The routing target belongs to a different net".into());
        }
        if end == self.anchor {
            return Ok(());
        }
        let end = *self.points(end).last().unwrap_or(&self.anchor);
        if end == self.anchor {
            return Ok(());
        }
        let mut next = self.append_to(self.draft.clone(), end)?;
        split_at(&mut next, end, self.layer, self.net)?;
        next.validate()?;
        let checkpoint = self.checkpoint(&next);
        if let Some(id) = self.endpoint_contact(end) {
            self.contacts.insert(id);
        }
        self.preview.invalidate(&checkpoint);
        self.checkpoints.push(checkpoint);
        self.draft = next;
        self.anchor = end;
        Ok(())
    }
    /// Append the active leg and a via as a single backtrack checkpoint.
    pub fn via(
        &mut self,
        end: Point,
        to_layer: u16,
        diameter: i64,
        hole: i64,
    ) -> Result<(), String> {
        if !(1..=100_000_000_000).contains(&self.width)
            || !(0..=100_000_000_000).contains(&self.clearance)
        {
            return Err("Invalid routing width or clearance".into());
        }
        if !copper_layer(to_layer) || to_layer == self.layer {
            return Err("Choose a different copper layer".into());
        }
        if !self
            .draft
            .layers
            .iter()
            .any(|l| l.id == to_layer && l.copper)
        {
            return Err("Destination layer is not in the layer stack".into());
        }
        if hole <= 0 || diameter <= hole || diameter > 100_000_000_000 {
            return Err("Via diameter must be greater than its positive drill size".into());
        }
        let via_probe = Item::new(
            MULTI_LAYER,
            self.net,
            Shape::Via {
                position: end,
                diameter,
                hole,
                from_layer: self.layer,
                to_layer,
            },
        );
        crate::pcb_rules::route_check(&self.draft, &via_probe)?;
        crate::pcb_rules::route_check(
            &self.draft,
            &crate::pcb_rules::probe(to_layer, self.net, end, self.width),
        )?;
        if self.points(end).last() != Some(&end) {
            return Err("Cannot place a via beyond an obstacle".into());
        }
        let mut contacts = self.contacts.clone();
        contacts.extend(self.endpoint_contact(end));
        if self.conflict != Conflict::Ignore {
            let mut layers: Vec<_> = self.draft.layers.iter().filter(|l| l.copper).collect();
            layers.sort_by_key(|l| l.order);
            let from = layers
                .iter()
                .position(|l| l.id == self.layer)
                .ok_or("Missing source layer")?;
            let to = layers
                .iter()
                .position(|l| l.id == to_layer)
                .ok_or("Missing destination layer")?;
            for layer in &layers[from.min(to)..=from.max(to)] {
                let obstacles = obstacles::Obstacles::for_contacts(
                    &self.draft,
                    layer.id,
                    &via_probe,
                    &contacts,
                    self.source_len,
                );
                if !obstacles.clear(end, end, diameter / 2, self.clearance) {
                    return Err("Via violates copper clearance on its layer span".into());
                }
            }
        }
        let mut next = self.append_to(self.draft.clone(), end)?;
        let has_via = next.items.iter().any(|i| {
            i.net == self.net
                && i.anchor() == end
                && (matches!(i.shape, Shape::Via { .. })
                    || matches!(i.shape, Shape::Pad { hole, plated: true, .. } if hole > 0))
                && on_layer(&next, i, self.layer)
                && on_layer(&next, i, to_layer)
        });
        if !has_via {
            next.items.push(Item::new(
                MULTI_LAYER,
                self.net,
                Shape::Via {
                    position: end,
                    diameter,
                    hole,
                    from_layer: self.layer,
                    to_layer,
                },
            ));
        }
        split_at(&mut next, end, self.layer, self.net)?;
        split_at(&mut next, end, to_layer, self.net)?;
        next.pours_stale |= !has_via && !next.polygon_pours.is_empty();
        next.validate()?;
        let checkpoint = self.checkpoint(&next);
        self.preview.invalidate(&checkpoint);
        self.checkpoints.push(checkpoint);
        self.contacts = contacts;
        self.draft = next;
        self.anchor = end;
        self.layer = to_layer;
        self.rebuild_obstacles();
        Ok(())
    }
    pub fn backtrack(&mut self) -> bool {
        if let Some(c) = self.checkpoints.pop() {
            self.preview.invalidate(&c);
            self.draft.items.truncate(c.items_len);
            for (n, item) in c.restored {
                self.draft.items[n] = item;
            }
            self.draft.pours_stale = c.pours_stale;
            self.contacts = c.contacts;
            self.anchor = c.anchor;
            self.layer = c.layer;
            self.rebuild_obstacles();
            true
        } else {
            false
        }
    }
    fn rebuild_obstacles(&mut self) {
        self.obstacles = obstacles::Obstacles::for_contacts(
            &self.draft,
            self.layer,
            &crate::pcb_rules::probe(self.layer, self.net, self.anchor, self.width),
            &self.contacts,
            self.source_len,
        );
    }
    pub fn finish(&self, current_revision: u64) -> Result<Board, String> {
        if self.revision != current_revision {
            return Err("Document changed during routing; cancel and restart".into());
        }
        self.draft.validate()?;
        Ok(self.draft.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn no_net_pad(position: Point) -> Item {
        Item::new(
            TOP,
            None,
            Shape::Pad {
                position,
                size: Point::mm(2, 2),
                shape: PadShape::Round,
                rotation: 0.0,
                hole: 0,
                plated: true,
                name: String::new(),
                bottom_size: None,
                middle_size: None,
            },
        )
    }
    fn inherited_pour_board() -> Board {
        let mut board = Board {
            nets: vec![
                Net {
                    index: 0,
                    name: "GND".into(),
                    properties: Fields::new(),
                },
                Net {
                    index: 1,
                    name: "SIGNAL".into(),
                    properties: Fields::new(),
                },
            ],
            ..Default::default()
        };
        let contour = vec![
            Point::mm(-5, -5),
            Point::mm(20, -5),
            Point::mm(20, 5),
            Point::mm(-5, 5),
        ];
        board.polygon_pours.push(crate::pcb_polygon::Polygon {
            index: 0,
            net: Some(0),
            contour: crate::pcb_polygon::straight_contour(&contour),
            ..Default::default()
        });
        let mut fragment = Item::new(
            TOP,
            None,
            Shape::Region {
                contour,
                holes: vec![],
                cutout: false,
            },
        );
        fragment.polygon = Some(0);
        board.items.push(fragment);
        let mut pad = no_net_pad(Point::mm(0, 0));
        pad.net = Some(0);
        board.items.push(pad);
        board
    }
    #[test]
    fn routing_and_vias_leave_a_pad_inside_its_inherited_net_pour_without_changing_native_net_fields()
     {
        let board = inherited_pour_board();
        let original = board.clone();
        let mut route =
            RouteSession::begin(&board, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        route.conflict = Conflict::Stop;
        assert_eq!(route.points(Point::mm(3, 0)).last(), Some(&Point::mm(3, 0)));
        route.place(Point::mm(2, 0), None).unwrap();
        route
            .via(Point::mm(3, 0), BOTTOM, 800_000, 400_000)
            .unwrap();
        assert_eq!(route.layer, BOTTOM);
        assert!(route.backtrack());
        assert_eq!(route.points(Point::mm(5, 0)).last(), Some(&Point::mm(5, 0)));
        assert_eq!(route.draft.items[0].net, None);
        assert_eq!(board, original);
    }
    #[test]
    fn managed_foreign_pours_allow_routing_but_free_copper_orphans_and_keepouts_block() {
        let mut board = inherited_pour_board();
        let contour = vec![
            Point::mm(5, -5),
            Point::mm(6, -5),
            Point::mm(6, 5),
            Point::mm(5, 5),
        ];
        board.polygon_pours.push(crate::pcb_polygon::Polygon {
            index: 1,
            net: Some(1),
            contour: crate::pcb_polygon::straight_contour(&contour),
            ..Default::default()
        });
        let mut fragment = Item::new(
            TOP,
            None,
            Shape::Region {
                contour,
                holes: vec![],
                cutout: false,
            },
        );
        fragment.polygon = Some(1);
        board.items.push(fragment);
        let route = |board: &Board| {
            let mut route =
                RouteSession::begin(board, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
            route.conflict = Conflict::Stop;
            route
        };
        assert_eq!(
            route(&board).points(Point::mm(10, 0)).last(),
            Some(&Point::mm(10, 0))
        );
        board.polygon_pours[1].shelved = true;
        assert_eq!(
            route(&board).points(Point::mm(10, 0)).last(),
            Some(&Point::mm(10, 0))
        );
        board.items[0].net = Some(1);
        assert_eq!(
            route(&board).points(Point::mm(10, 0)).last(),
            Some(&Point::mm(10, 0))
        );
        board.items[0].polygon = None;
        assert_eq!(
            route(&board).points(Point::mm(10, 0)),
            vec![Point::mm(0, 0)]
        );
        board.items[0].polygon = Some(99);
        assert_eq!(
            route(&board).points(Point::mm(10, 0)),
            vec![Point::mm(0, 0)]
        );
        board.items[0].polygon = Some(0);
        board.items[0].net = None;
        board.items[0].keepout = true;
        assert_eq!(
            route(&board).points(Point::mm(10, 0)),
            vec![Point::mm(0, 0)]
        );
        board.items[0].keepout = false;
        board.items[0]
            .properties
            .insert("ISBOARDCUTOUT".into(), "TRUE".into());
        assert_eq!(
            route(&board).points(Point::mm(10, 0)),
            vec![Point::mm(0, 0)]
        );
    }
    #[test]
    fn route_across_foreign_pour_repours_rule_clearance_and_undo_restores_original_copper() {
        use geo::{Area, BooleanOps, Buffer};
        let mut board = inherited_pour_board();
        board.items[1].net = Some(1);
        let mut rule = crate::pcb_rules::default_rule("Clearance");
        rule.insert("SCOPE1EXPRESSION".into(), "IsPolygon".into());
        rule.insert("GAP".into(), "0.6mm".into());
        board.rules.push(rule);
        let board = crate::pcb_polygon::repour(&board).unwrap();
        let original = board.clone();
        for conflict in [Conflict::Stop, Conflict::Walkaround] {
            let mut route =
                RouteSession::begin(&board, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
            route.conflict = conflict;
            let endpoint = Point::mm(5, 0);
            assert_eq!(route.points(endpoint).last(), Some(&endpoint));
            assert!(route.preview(endpoint).unwrap().pours_stale);
            route.place(Point::mm(3, 0), None).unwrap();
            route.via(endpoint, BOTTOM, 800_000, 400_000).unwrap();
            assert!(route.backtrack());
            assert_eq!(route.points(endpoint).last(), Some(&endpoint));
            route.via(endpoint, BOTTOM, 800_000, 400_000).unwrap();
            assert_eq!(board, original);
            let mut doc = crate::Document::new(crate::DocumentKind::Board, "Foreign pour");
            doc.pcb = Some(board.clone());
            let before = doc.clone();
            let mut history = crate::History::default();
            history
                .commit_pcb(&mut doc, route.finish(0).unwrap())
                .unwrap();
            let prepared = crate::pcb_polygon::prepare_repour(
                doc.id,
                history.revision,
                doc.pcb.as_ref().unwrap(),
            )
            .unwrap();
            history.apply_pcb_pour(&mut doc, prepared).unwrap();
            let poured = doc.pcb.as_ref().unwrap();
            assert!(!poured.pours_stale);
            let fragments: Vec<_> = poured
                .items
                .iter()
                .filter(|i| i.polygon == Some(0))
                .collect();
            assert!(!fragments.is_empty());
            for added in poured.items.iter().filter(|i| {
                i.net == Some(1) && matches!(i.shape, Shape::Track { .. } | Shape::Via { .. })
            }) {
                let clearance = crate::pcb_polygon::copper(added, TOP).buffer(0.599);
                for fragment in &fragments {
                    assert!(
                        crate::pcb_polygon::copper(fragment, TOP)
                            .intersection(&clearance)
                            .unsigned_area()
                            < 1e-8,
                        "foreign pour must clear the new route and via by the scoped 0.6mm rule"
                    );
                }
            }
            let after = doc.clone();
            history.undo(&mut doc);
            assert_eq!(doc, before);
            assert!(!history.can_undo());
            history.redo(&mut doc);
            assert_eq!(doc, after);
        }
    }
    #[test]
    fn inherited_polygon_net_drives_clearance_net_scope_and_membership_queries() {
        let mut board = inherited_pour_board();
        board.classes.push(Fields::from([
            ("KIND".into(), "0".into()),
            ("NAME".into(), "Return".into()),
            ("M0".into(), "GND".into()),
        ]));
        let mut rule = crate::pcb_rules::default_rule("Clearance");
        rule.insert("SCOPE1EXPRESSION".into(), "InNet('SIGNAL')".into());
        rule.insert("SCOPE2EXPRESSION".into(), "InNetClass('Return')".into());
        rule.insert("GAP".into(), "0.4mm".into());
        board.rules.push(rule);
        let fragment = &board.items[0];
        assert_eq!(board.item_net(fragment), Some(0));
        assert_eq!(
            crate::pcb_rules::query(&board, fragment, "InNet('GND') AND InNetClass('Return')"),
            Ok(true)
        );
        let foreign = crate::pcb_rules::probe(TOP, Some(1), Point::mm(0, 0), 250_000);
        assert_eq!(
            crate::pcb_rules::clearance(&board, &foreign, fragment),
            Ok(Some(400_000))
        );
        let own = crate::pcb_rules::probe(TOP, Some(0), Point::mm(0, 0), 250_000);
        assert_eq!(
            crate::pcb_rules::clearance(&board, &own, fragment),
            Ok(None)
        );
        board.rules[0].insert("NETSCOPE".into(), "SameNet".into());
        board.rules[0].insert("SCOPE1EXPRESSION".into(), "All".into());
        board.rules[0].insert("SCOPE2EXPRESSION".into(), "All".into());
        assert_eq!(
            crate::pcb_rules::clearance(&board, &own, fragment),
            Ok(Some(400_000))
        );
        assert_eq!(
            crate::pcb_rules::clearance(&board, &foreign, fragment),
            Ok(None)
        );
    }
    #[test]
    fn no_net_route_leaves_its_pad_connects_a_target_and_keeps_other_copper_blocking() {
        let board = Board {
            items: vec![
                no_net_pad(Point::mm(0, 0)),
                no_net_pad(Point::mm(10, 0)),
                no_net_pad(Point::mm(5, 0)),
            ],
            ..Default::default()
        };
        let original = board.clone();
        let mut route =
            RouteSession::begin(&board, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        route.conflict = Conflict::Stop;
        assert_eq!(route.points(Point::mm(2, 0)).last(), Some(&Point::mm(2, 0)));
        assert!(route.points(Point::mm(10, 0)).last().unwrap().x < 4_000_000);
        route.place(Point::mm(2, 3), None).unwrap();
        route.place(Point::mm(10, 3), None).unwrap();
        let target = target(&board, Point::mm(10, 0), TOP, 0).unwrap();
        assert_eq!(route.points(target.point).last(), Some(&target.point));
        let preview = route.preview(target.point).unwrap();
        route.place(target.point, Some(target)).unwrap();
        assert_eq!(route.anchor, target.point);
        assert_eq!(route.draft.items.len(), preview.items.len());
        assert_eq!(board, original);
        assert!(route.backtrack());
        assert!(!route.contacts.contains(&target.item));
        let mut model = crate::Document::new(crate::DocumentKind::Board, "No Net routing");
        model.pcb = Some(board);
        let original = model.clone();
        let mut history = crate::History::default();
        history
            .commit_pcb(&mut model, route.finish(0).unwrap())
            .unwrap();
        history.undo(&mut model);
        assert_eq!(model, original);
    }
    #[test]
    fn no_net_track_split_and_layer_change_do_not_block_the_session_itself() {
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(10, 0),
                width: 250_000,
            },
        ));
        let mut route =
            RouteSession::begin(&board, 0, Point::mm(5, 0), TOP, None, 250_000, 0).unwrap();
        route.conflict = Conflict::Stop;
        route.place(Point::mm(5, 3), None).unwrap();
        assert_eq!(route.anchor, Point::mm(5, 3));
        route.via(route.anchor, BOTTOM, 800_000, 400_000).unwrap();
        assert_eq!(route.points(Point::mm(8, 3)).last(), Some(&Point::mm(8, 3)));
        route.place(Point::mm(8, 3), None).unwrap();
        assert!(route.backtrack());
        assert!(route.backtrack());
        assert_eq!(route.layer, TOP);
        assert_eq!(route.points(Point::mm(5, 5)).last(), Some(&Point::mm(5, 5)));
        assert_eq!(board.items.len(), 1);
    }
    #[test]
    fn a_direct_track_hit_wins_over_an_adjacent_pad_and_drill_only_pads_are_not_targets() {
        let mut board = Board::default();
        board.items.push(no_net_pad(Point::mm(0, 0)));
        let track = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(2, -3),
                end: Point::mm(2, 3),
                width: 250_000,
            },
        );
        let id = track.id;
        board.items.push(track);
        assert_eq!(
            target(&board, Point::mm(2, 0), TOP, 2_000_000)
                .unwrap()
                .item,
            id
        );
        let mut hole = no_net_pad(Point::mm(20, 0));
        hole.layer = MULTI_LAYER;
        if let Shape::Pad {
            size, hole, plated, ..
        } = &mut hole.shape
        {
            *size = Point::default();
            *hole = 1_000_000;
            *plated = false;
        }
        board.items.push(hole);
        assert!(target(&board, Point::mm(20, 0), TOP, 0).is_none());
    }
    #[test]
    fn layer_change_on_a_plated_through_pad_reuses_its_existing_connection() {
        let mut board = Board::default();
        let mut pad = no_net_pad(Point::mm(0, 0));
        pad.layer = MULTI_LAYER;
        if let Shape::Pad { hole, .. } = &mut pad.shape {
            *hole = 800_000;
        }
        board.items.push(pad);
        let mut route =
            RouteSession::begin(&board, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        route.via(route.anchor, BOTTOM, 600_000, 300_000).unwrap();
        assert_eq!(route.draft.items, board.items);
        assert_eq!(route.points(Point::mm(3, 0)).last(), Some(&Point::mm(3, 0)));
    }
    #[test]
    fn spatial_board_edge_rule_is_found_away_from_origin_and_zero_permits_crossing() {
        let mut board = Board {
            outline: crate::pcb_polygon::straight_contour(&[
                Point::mm(0, 0),
                Point::mm(10, 0),
                Point::mm(10, 10),
                Point::mm(0, 10),
            ]),
            ..Default::default()
        };
        let mut room = Fields::from([
            ("RULEKIND".into(), "RoomDefinition".into()),
            ("NAME".into(), "East".into()),
        ]);
        for (n, (x, y)) in [(5, -5), (15, -5), (15, 15), (5, 15)]
            .into_iter()
            .enumerate()
        {
            room.insert(format!("VX{n}"), format!("{x}mm"));
            room.insert(format!("VY{n}"), format!("{y}mm"));
        }
        let mut edge = crate::pcb_rules::default_rule("BoardOutlineClearance");
        edge.insert("SCOPE1EXPRESSION".into(), "WithinRoom('East')".into());
        edge.insert("GAP".into(), "1mm".into());
        board.rules = vec![room, edge];
        let route = |board: &Board, start| {
            let mut route = RouteSession::begin(board, 0, start, TOP, None, 200_000, 0).unwrap();
            route.corner = Corner::AnyAngle;
            route.conflict = Conflict::Stop;
            route
        };
        let r = route(&board, Point::mm(8, 5));
        assert!(r.points(Point::mm(12, 5)).last().unwrap().x < 9_000_000);
        let r = route(&board, Point::mm(2, 5));
        assert_eq!(r.points(Point::mm(-2, 5)).last(), Some(&Point::mm(-2, 5)));
        board.rules[1].insert("GAP".into(), "0mm".into());
        let r = route(&board, Point::mm(8, 5));
        assert_eq!(r.points(Point::mm(12, 5)).last(), Some(&Point::mm(12, 5)));
    }
    #[test]
    fn room_clearance_follows_the_proposed_segment_after_placing_a_leg() {
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point { x: 0, y: 500_000 },
                end: Point {
                    x: 3_000_000,
                    y: 500_000,
                },
                width: 200_000,
            },
        ));
        let mut room = Fields::from([
            ("RULEKIND".into(), "RoomDefinition".into()),
            ("NAME".into(), "DUT".into()),
        ]);
        for (n, (x, y)) in [(0, 1), (10, 1), (10, 10), (0, 10)].into_iter().enumerate() {
            room.insert(format!("VX{n}"), format!("{x}mm"));
            room.insert(format!("VY{n}"), format!("{y}mm"));
        }
        let mut scoped = crate::pcb_rules::default_rule("Clearance");
        scoped.insert("SCOPE1EXPRESSION".into(), "WithinRoom('DUT')".into());
        scoped.insert("GAP".into(), "0.1mm".into());
        scoped.insert(
            "OBJECTCLEARANCES".into(),
            "ClearanceObj_Track-ClearanceObj_Track:393701".into(),
        );
        let mut fallback = crate::pcb_rules::default_rule("Clearance");
        fallback.insert("PRIORITY".into(), "2".into());
        fallback.insert("GAP".into(), "0.1mm".into());
        board.rules = vec![room, scoped, fallback];
        let mut r =
            RouteSession::begin(&board, 0, Point::mm(8, -1), TOP, None, 200_000, 0).unwrap();
        r.corner = Corner::AnyAngle;
        r.conflict = Conflict::Stop;
        // The original clearance probe at (0,0) lies outside DUT. A subsequent
        // segment inside DUT must nevertheless use its larger scoped gap.
        r.place(Point::mm(8, 2), None).unwrap();
        let target = Point {
            x: 1_000_000,
            y: 1_200_000,
        };
        assert_ne!(r.points(target).last(), Some(&target));
        assert!(r.points(target).last().unwrap().x > target.x);
        r.place(Point::mm(8, -1), None).unwrap();
        assert_eq!(r.points(Point::mm(1, -1)).last(), Some(&Point::mm(1, -1)));
        assert_eq!(board.items.len(), 1);
    }
    #[test]
    fn empty_preview_keeps_pours_current_and_invalid_settings_do_not_overflow() {
        let mut b = Board::default();
        b.polygon_pours.push(crate::pcb_polygon::Polygon {
            contour: crate::pcb_polygon::straight_contour(&[
                Point::mm(-1, -1),
                Point::mm(10, -1),
                Point::mm(10, 10),
            ]),
            ..Default::default()
        });
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        assert!(!r.preview(r.anchor).unwrap().pours_stale);
        assert!(r.preview(Point::mm(1, 0)).unwrap().pours_stale);
        r.clearance = i64::MAX;
        assert_eq!(r.points(Point::mm(1, 0)), vec![r.anchor]);
        assert!(r.preview(Point::mm(1, 0)).is_err());
        assert!(r.via(Point::mm(1, 0), BOTTOM, i64::MAX, 1).is_err());
        assert!(r.via(r.anchor, BOTTOM, 600_000, 300_000).is_err());
        assert_eq!(r.draft, b);
    }
    fn obstacle_board() -> Board {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(5, -1),
                end: Point::mm(5, 1),
                width: 500_000,
            },
        ));
        b
    }
    #[test]
    fn preferred_width_does_not_bypass_unknown_priority_scope() {
        let mut b = Board::default();
        b.rules.push(
            [
                ("RULEKIND", "Width"),
                ("SCOPE1EXPRESSION", "All"),
                ("PRIORITY", "2"),
                ("PREFEREDWIDTH", "10mil"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
        );
        assert_eq!(
            preferred_dimension(&b, "Width", "PREFEREDWIDTH"),
            Some(254_000)
        );
        let mut scoped = b.rules[0].clone();
        scoped.insert("SCOPE1EXPRESSION".into(), "InNet('ANT')".into());
        scoped.insert("PRIORITY".into(), "1".into());
        b.rules.push(scoped);
        assert_eq!(preferred_dimension(&b, "Width", "PREFEREDWIDTH"), None);
    }
    #[test]
    fn polygon_cutout_is_passable_but_board_cutout_and_keepout_block() {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Region {
                contour: vec![
                    Point::mm(4, -1),
                    Point::mm(6, -1),
                    Point::mm(6, 1),
                    Point::mm(4, 1),
                ],
                holes: vec![],
                cutout: true,
            },
        ));
        let end = Point::mm(10, 0);
        let route = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 100_000, 0).unwrap();
        assert_eq!(route.points(end).len(), 2);
        b.items[0]
            .properties
            .insert("ISBOARDCUTOUT".into(), "TRUE".into());
        let mut route = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 100_000, 0).unwrap();
        route.conflict = Conflict::Stop;
        assert!(route.points(end).last().unwrap().x < 4_000_000);
        b.items[0].properties.clear();
        b.items[0].keepout = true;
        let mut route = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 100_000, 0).unwrap();
        route.conflict = Conflict::Stop;
        assert!(route.points(end).last().unwrap().x < 4_000_000);
    }
    #[test]
    fn stop_preview_and_placement_obey_clearance_and_backtrack() {
        let b = obstacle_board();
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 500_000, 0).unwrap();
        r.conflict = Conflict::Stop;
        let end = Point::mm(10, 0);
        let points = r.points(end);
        let actual = *points.last().unwrap();
        assert!((4_299_990..=4_300_000).contains(&actual.x));
        assert_eq!(actual.y, 0);
        let preview = r.preview(end).unwrap();
        r.place(end, None).unwrap();
        assert_eq!(r.anchor, actual);
        assert_eq!(preview.items.len(), r.draft.items.len());
        assert!(r.backtrack());
        assert_eq!(r.draft, b);
    }
    #[test]
    fn walkaround_reaches_target_with_clear_octilinear_segments() {
        let b = obstacle_board();
        let r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 500_000, 0).unwrap();
        let end = Point::mm(10, 0);
        let points = r.points(end);
        assert_eq!(points.last(), Some(&end));
        assert!(points.len() > 2);
        for edge in points.windows(2) {
            assert!(r.obstacles.clear(edge[0], edge[1], 450_000, 0));
            let dx = (edge[0].x - edge[1].x).abs();
            let dy = (edge[0].y - edge[1].y).abs();
            assert!(dx == 0 || dy == 0 || dx == dy);
        }
    }
    #[test]
    fn ignore_is_explicit_and_other_layers_do_not_block() {
        let mut b = obstacle_board();
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 500_000, 0).unwrap();
        r.conflict = Conflict::Ignore;
        assert_eq!(
            r.points(Point::mm(10, 0)),
            vec![Point::mm(0, 0), Point::mm(10, 0)]
        );
        b.items[0].layer = BOTTOM;
        let r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 500_000, 0).unwrap();
        assert_eq!(r.points(Point::mm(10, 0)).len(), 2);
    }
    #[test]
    fn via_checks_destination_clearance_atomically() {
        let mut b = obstacle_board();
        b.items[0].layer = BOTTOM;
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 500_000, 0).unwrap();
        let before = r.draft.clone();
        assert!(r.via(Point::mm(5, 0), BOTTOM, 800_000, 400_000).is_err());
        assert_eq!(r.draft, before);
        assert_eq!(r.layer, TOP);
        assert!(!r.has_segments());
    }
    #[test]
    fn preview_never_crosses_a_thin_obstacle_between_endpoints() {
        let mut b = obstacle_board();
        b.items[0].shape = Shape::Track {
            start: Point::mm(5, -50),
            end: Point::mm(5, 50),
            width: 1,
        };
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 100_000, 0).unwrap();
        r.conflict = Conflict::Stop;
        r.clearance = 0;
        assert!(r.points(Point::mm(10, 0)).last().unwrap().x < 5_000_000);
    }
    #[test]
    fn session_is_isolated_and_backtracks_via_with_its_leg() {
        let b = Board::default();
        let mut r = RouteSession::begin(&b, 7, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        r.place(Point::mm(3, 1), None).unwrap();
        let n = r.draft.items.len();
        r.via(Point::mm(4, 2), BOTTOM, 600_000, 300_000).unwrap();
        assert_eq!(r.layer, BOTTOM);
        assert!(r.draft.items.len() > n);
        assert!(r.backtrack());
        assert_eq!(r.layer, TOP);
        assert_eq!(r.draft.items.len(), n);
        assert!(b.items.is_empty());
        assert!(r.finish(8).is_err());
        assert!(r.finish(7).is_ok());
    }
    #[test]
    fn finishing_at_track_splits_it_without_disconnection() {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(10, 0),
                width: 250_000,
            },
        ));
        let h = target(&b, Point::mm(5, 0), TOP, 0).unwrap();
        let mut r = RouteSession::begin(&b, 0, Point::mm(5, 5), TOP, None, 250_000, 0).unwrap();
        r.place(h.point, Some(h)).unwrap();
        assert_eq!(r.draft.items.len(), 3);
        assert_eq!(b.items.len(), 1);
    }
    #[test]
    fn wrong_net_target_is_atomic() {
        let b = Board::default();
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        assert!(
            r.place(
                Point::mm(1, 1),
                Some(Target {
                    item: Uuid::nil(),
                    point: Point::mm(1, 1),
                    net: Some(1)
                })
            )
            .is_err()
        );
        assert!(r.draft.items.is_empty());
    }
    #[test]
    fn fortyfive_routes_cover_every_quadrant() {
        let b = Board::default();
        let r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        for x in -8..=8 {
            for y in -8..=8 {
                for pair in r.points(Point::mm(x, y)).windows(2) {
                    let dx = (pair[1].x - pair[0].x).abs();
                    let dy = (pair[1].y - pair[0].y).abs();
                    assert!(dx == 0 || dy == 0 || dx == dy);
                }
            }
        }
    }
}
