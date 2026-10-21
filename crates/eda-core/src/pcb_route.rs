//! Interactive manual routing session. No shove, obstacle solver or DRC.
//! A session owns a draft; cancel never mutates the original document. Completing
//! the session is one History transaction, including splits and layer-change vias.
use crate::{pcb::*, pcb_polygon::on_layer};
use eda_geom::Point;
use uuid::Uuid;

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
    pub draft: Board,
    pub anchor: Point,
    pub layer: u16,
    pub net: Option<u32>,
    pub width: i64,
    pub corner: Corner,
    pub diagonal_first: bool,
    checkpoints: Vec<Checkpoint>,
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
        let mut draft = board.clone();
        split_at(&mut draft, anchor, layer, net)?;
        draft.validate()?;
        Ok(Self {
            revision,
            draft,
            anchor,
            layer,
            net,
            width,
            corner: Corner::FortyFive,
            diagonal_first: true,
            checkpoints: vec![],
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
    pub fn points(&self, end: Point) -> Vec<Point> {
        match self.corner {
            Corner::FortyFive => route_points(self.anchor, end, self.diagonal_first),
            Corner::Orthogonal => {
                let c = if self.diagonal_first {
                    Point {
                        x: self.anchor.x,
                        y: end.y,
                    }
                } else {
                    Point {
                        x: end.x,
                        y: self.anchor.y,
                    }
                };
                let mut v = vec![self.anchor];
                if c != self.anchor && c != end {
                    v.push(c);
                }
                if end != self.anchor {
                    v.push(end);
                }
                v
            }
            Corner::AnyAngle => {
                if end == self.anchor {
                    vec![self.anchor]
                } else {
                    vec![self.anchor, end]
                }
            }
        }
    }
    fn append_to(&self, mut board: Board, end: Point) -> Result<Board, String> {
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
        board.pours_stale |= !board.polygon_pours.is_empty();
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
        let mut next = self.append_to(self.draft.clone(), end)?;
        split_at(&mut next, end, self.layer, self.net)?;
        next.validate()?;
        self.checkpoints.push(self.checkpoint(&next));
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
        if hole <= 0 || diameter <= hole {
            return Err("Via diameter must be greater than its positive drill size".into());
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
        self.checkpoints.push(self.checkpoint(&next));
        self.draft = next;
        self.anchor = end;
        self.layer = to_layer;
        Ok(())
    }
    pub fn backtrack(&mut self) -> bool {
        if let Some(c) = self.checkpoints.pop() {
            self.draft.items.truncate(c.items_len);
            for (n, item) in c.restored {
                self.draft.items[n] = item;
            }
            self.draft.pours_stale = c.pours_stale;
            self.anchor = c.anchor;
            self.layer = c.layer;
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
