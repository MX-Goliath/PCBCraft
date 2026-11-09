//! Single-segment sliding along its normal, preserving adjacent straight angles.
use super::*;

#[derive(Clone)]
pub(super) struct Slide {
    item: Item,
    joins: [Option<(Point, Point)>; 2], // original junction, fixed neighbour end
}
impl Slide {
    pub fn new(board: &Board, id: Uuid) -> Self {
        let item = board.item(id).unwrap().clone();
        let Shape::Track { start, end, .. } = item.shape else {
            unreachable!()
        };
        let mut attached: [Vec<&Item>; 2] = Default::default();
        let mut interior = false;
        for other in &board.items {
            if other.id == id
                || other.net != item.net
                || other.keepout
                || other.polygon.is_some()
                || !on_layer(board, other, item.layer)
            {
                continue;
            }
            let points = contacts(&item, other);
            interior |= points.iter().any(|p| *p != start && *p != end);
            // Do not reshape a segment around contact topology this graph cannot
            // resolve (for example an arc crossing the middle of a straight).
            interior |= !matches!(
                other.shape,
                Shape::Track { .. } | Shape::Pad { .. } | Shape::Via { .. }
            ) && item.bounds().intersects(other.bounds());
            for (n, at) in [start, end].into_iter().enumerate() {
                if points.contains(&at) {
                    attached[n].push(other);
                }
            }
        }
        let mut joins = [None; 2];
        if !interior {
            for (n, at) in [start, end].into_iter().enumerate() {
                if let [other] = attached[n].as_slice()
                    && other.component.is_none()
                    && let Shape::Track {
                        start: a, end: b, ..
                    } = other.shape
                {
                    if at == a {
                        joins[n] = Some((a, b));
                    }
                    if at == b {
                        joins[n] = Some((b, a));
                    }
                }
            }
        }
        Self { item, joins }
    }
    pub fn at(&self, delta: Point) -> Result<Item, String> {
        let Shape::Track { start, end, width } = self.item.shape else {
            unreachable!()
        };
        let dx = (end.x - start.x) as f64;
        let dy = (end.y - start.y) as f64;
        let length = dx * dx + dy * dy;
        if length == 0. {
            return Ok(self.item.clone());
        }
        let along = (delta.x as f64 * dx + delta.y as f64 * dy) / length;
        let mut normal = Point {
            x: (delta.x as f64 - along * dx).round() as i64,
            y: (delta.y as f64 - along * dy).round() as i64,
        };
        // Preserve exact integer 45-degree geometry after pointer quantization.
        if dx.abs() == dy.abs() {
            let nx = -dy.signum() as i64;
            let ny = dx.signum() as i64;
            let amount = ((delta.x * nx + delta.y * ny) as f64 / 2.).round() as i64;
            normal = Point {
                x: nx * amount,
                y: ny * amount,
            };
        }
        if normal == Point::default() {
            return Ok(self.item.clone());
        }
        let mut moved = self.item.clone();
        crate::pcb::transform_item(&mut moved, Point::default(), normal, 0., false)?;
        let Shape::Track {
            start: a, end: b, ..
        } = moved.shape
        else {
            unreachable!()
        };
        let mut ends = [a, b];
        for (n, join) in self.joins.iter().enumerate() {
            let Some((old_at, fixed)) = join else {
                continue;
            };
            let vx = (old_at.x - fixed.x) as f64;
            let vy = (old_at.y - fixed.y) as f64;
            let cross = dx * vy - dy * vx;
            if cross.abs() < 1. {
                continue;
            } // collinear neighbours need a new bend
            let t = ((fixed.x - a.x) as f64 * vy - (fixed.y - a.y) as f64 * vx) / cross;
            let intersection = Point {
                x: (a.x as f64 + dx * t).round() as i64,
                y: (a.y as f64 + dy * t).round() as i64,
            };
            if (intersection.x - fixed.x) as f64 * vx + (intersection.y - fixed.y) as f64 * vy < 0.
            {
                return Err(CONNECTION_BLOCKED.into());
            }
            ends[n] = intersection;
        }
        // Stop before reversing the dragged segment or passing a fixed vertex.
        // Prepared previews retain/refine the last valid location on this error.
        if (ends[1].x - ends[0].x) as f64 * dx + (ends[1].y - ends[0].y) as f64 * dy <= 0. {
            return Err(CONNECTION_BLOCKED.into());
        }
        moved.shape = Shape::Track {
            start: ends[0],
            end: ends[1],
            width,
        };
        moved.validate_geometry()?;
        Ok(moved)
    }
}
