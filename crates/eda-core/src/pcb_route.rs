//! Interactive routing session with bounded obstacle avoidance. No shove or full DRC.
//! A session owns a draft; cancel never mutates the original document. Completing
//! the session is one History transaction, including splits and layer-change vias.
use crate::{pcb::*, pcb_polygon::on_layer};
use eda_geom::Point;
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
        .filter(|i| on_layer(board, i, layer) && !i.keepout && i.polygon.is_none())
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
        .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)))
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
        let obstacles = obstacles::Obstacles::new(board, layer, net);
        obstacles.error()?;
        let mut draft = board.clone();
        split_at(&mut draft, anchor, layer, net)?;
        draft.validate()?;
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
                let obstacles = obstacles::Obstacles::for_item(&self.draft, layer.id, &via_probe);
                if !obstacles.clear(end, end, diameter / 2, self.clearance) {
                    return Err("Via violates copper clearance on its layer span".into());
                }
            }
        }
        let mut next = self.append_to(self.draft.clone(), end)?;
        let has_via = next.items.iter().any(|i| {
            i.net == self.net
                && i.anchor() == end
                && matches!(i.shape, Shape::Via { .. })
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
        next.pours_stale |= !next.polygon_pours.is_empty();
        next.validate()?;
        let checkpoint = self.checkpoint(&next);
        self.preview.invalidate(&checkpoint);
        self.checkpoints.push(checkpoint);
        self.draft = next;
        self.anchor = end;
        self.layer = to_layer;
        self.obstacles = obstacles::Obstacles::new(&self.draft, self.layer, self.net);
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
            self.anchor = c.anchor;
            self.layer = c.layer;
            self.obstacles = obstacles::Obstacles::new(&self.draft, self.layer, self.net);
            true
        } else {
            false
        }
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
        r.conflict = Conflict::Ignore; // This test exercises no-net legacy track splitting.
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
