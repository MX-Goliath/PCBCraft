//! Exact document coordinates. Pixels and camera coordinates belong to the renderer.
use serde::{Deserialize, Serialize};

/// Flattened, ordered drawing faces in document coordinates (RGB, no GUI types).
#[derive(Clone, Debug)]
pub struct DrawingFace {
    pub points: [Point; 3],
    pub rgb: [u8; 3],
}

/// Nanometres. Bounded to +/- 100 metres, including large schematic sheets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub const fn mm(x: i64, y: i64) -> Self {
        Self {
            x: x * 1_000_000,
            y: y * 1_000_000,
        }
    }

    pub fn valid(self) -> bool {
        self.x.unsigned_abs() <= 100_000_000_000 && self.y.unsigned_abs() <= 100_000_000_000
    }

    /// Exact point-on-segment predicate, without screen-space tolerances.
    pub fn on_segment(self, a: Self, b: Self) -> bool {
        let cross = (self.x as i128 - a.x as i128) * (b.y as i128 - a.y as i128)
            - (self.y as i128 - a.y as i128) * (b.x as i128 - a.x as i128);
        cross == 0
            && self.x >= a.x.min(b.x)
            && self.x <= a.x.max(b.x)
            && self.y >= a.y.min(b.y)
            && self.y <= a.y.max(b.y)
    }
}

#[cfg(test)]
mod tests {
    use super::Point;
    #[test]
    fn segment_contains_interior_endpoints_but_not_extension_or_nearby_points() {
        let a = Point::mm(-90_000, -90_000);
        let b = Point::mm(90_000, 90_000);
        assert!(Point::default().on_segment(a, b));
        assert!(a.on_segment(a, b));
        assert!(b.on_segment(b, a));
        assert!(!Point::mm(90_001, 90_001).on_segment(a, b));
        assert!(!Point { x: 0, y: 1 }.on_segment(a, b));
        assert!(a.on_segment(a, a));
        assert!(!b.on_segment(a, a));
    }
}
