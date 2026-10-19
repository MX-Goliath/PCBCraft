//! Exact document coordinates. Pixels and camera coordinates belong to the renderer.
use serde::{Deserialize, Serialize};

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
}
