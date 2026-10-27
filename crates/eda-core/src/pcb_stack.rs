//! Layer-specific lands and drill geometry. No screen coordinates or GPU state.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackMode {
    #[default]
    Simple,
    TopMiddleBottom,
    Full,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillShape {
    #[default]
    Round,
    Square,
    Slot,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PadGeometry {
    pub size: Point,
    pub shape: PadShape,
    pub corner_percent: u8,
    pub offset: Point,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PadStack {
    pub mode: StackMode,
    pub layers: BTreeMap<u16, PadGeometry>,
    pub drill_shape: DrillShape,
    pub slot_length: i64,
    pub drill_rotation: f64,
}
impl Item {
    pub(super) fn same_copper_geometry(&self, other: &Item, board: &Board) -> bool {
        let same_kind = matches!(
            (&self.shape, &other.shape),
            (Shape::Pad { .. }, Shape::Pad { .. }) | (Shape::Via { .. }, Shape::Via { .. })
        );
        if !same_kind {
            return self.shape == other.shape && self.pad_stack == other.pad_stack;
        }
        let plating =
            |i: &Item| matches!(i.shape, Shape::Pad { plated: true, .. } | Shape::Via { .. });
        let span = |i: &Item| {
            if let Shape::Via {
                from_layer,
                to_layer,
                ..
            } = i.shape
            {
                Some((from_layer, to_layer))
            } else {
                None
            }
        };
        plating(self) == plating(other)
            && span(self) == span(other)
            && self.drill_outline() == other.drill_outline()
            && board
                .layers
                .iter()
                .filter(|l| l.copper)
                .all(|l| self.pad_outline(l.id) == other.pad_outline(l.id))
    }
    pub(super) fn reconcile_legacy_stack(&mut self, old: &Item) {
        if self.pad_stack.is_none() || self.pad_stack != old.pad_stack {
            return;
        }
        match (&old.shape, self.shape.clone()) {
            (
                Shape::Pad {
                    size: a,
                    shape: sa,
                    bottom_size: ba,
                    middle_size: ma,
                    ..
                },
                Shape::Pad {
                    size: b,
                    shape: sb,
                    bottom_size: bb,
                    middle_size: mb,
                    ..
                },
            ) => {
                for (layer, size, changed) in [
                    (TOP, b, *a != b || *sa != sb),
                    (2, mb.unwrap_or(b), *ma != mb),
                    (BOTTOM, bb.unwrap_or(b), *ba != bb),
                ] {
                    if changed {
                        let mut g = self.pad_geometry(layer).unwrap();
                        g.size = size;
                        if layer == TOP {
                            g.shape = sb;
                        }
                        self.set_pad_geometry(layer, g);
                    }
                }
            }
            (Shape::Via { diameter: a, .. }, Shape::Via { diameter: b, .. }) if *a != b => {
                let mut g = self.pad_geometry(TOP).unwrap();
                g.size = Point { x: b, y: b };
                self.set_pad_geometry(TOP, g);
            }
            _ => {}
        }
    }
    pub fn stack_mode(&self) -> StackMode {
        self.pad_stack
            .as_ref()
            .map(|s| s.mode)
            .unwrap_or_else(|| match self.shape {
                Shape::Pad {
                    bottom_size: Some(_),
                    ..
                }
                | Shape::Pad {
                    middle_size: Some(_),
                    ..
                } => StackMode::TopMiddleBottom,
                _ => StackMode::Simple,
            })
    }
    pub fn pad_geometry(&self, layer: u16) -> Option<PadGeometry> {
        if let Some(stack) = &self.pad_stack {
            let key = match stack.mode {
                StackMode::Simple => TOP,
                StackMode::TopMiddleBottom if layer != TOP && layer != BOTTOM => 2,
                _ => layer,
            };
            if let Some(g) = stack.layers.get(&key) {
                return Some(g.clone());
            }
        }
        let (size, shape) = match self.shape {
            Shape::Pad {
                size,
                shape,
                bottom_size,
                middle_size,
                ..
            } => (
                if self.layer != MULTI_LAYER || layer == TOP {
                    size
                } else if layer == BOTTOM {
                    bottom_size.unwrap_or(size)
                } else {
                    middle_size.unwrap_or(size)
                },
                shape,
            ),
            Shape::Via { diameter, .. } => (
                Point {
                    x: diameter,
                    y: diameter,
                },
                PadShape::Round,
            ),
            _ => return None,
        };
        Some(PadGeometry {
            size,
            shape,
            corner_percent: self.pad_corner_percent(layer),
            offset: Point::default(),
        })
    }
    pub fn set_stack_mode(&mut self, mode: StackMode, board: &Board) {
        let mut layers = BTreeMap::new();
        match mode {
            StackMode::Simple => {
                if let Some(g) = self.pad_geometry(TOP) {
                    layers.insert(TOP, g);
                }
            }
            StackMode::TopMiddleBottom => {
                for layer in [TOP, 2, BOTTOM] {
                    if let Some(g) = self.pad_geometry(layer) {
                        layers.insert(layer, g);
                    }
                }
            }
            StackMode::Full => {
                for layer in board.layers.iter().filter(|l| l.copper) {
                    if let Some(g) = self.pad_geometry(layer.id) {
                        layers.insert(layer.id, g);
                    }
                }
            }
        }
        let stack = self.pad_stack.get_or_insert_with(Default::default);
        stack.mode = mode;
        stack.layers = layers;
        self.sync_stack_legacy();
    }
    pub fn set_pad_geometry(&mut self, layer: u16, geometry: PadGeometry) {
        let mode = self.stack_mode();
        let key = match mode {
            StackMode::Simple => TOP,
            StackMode::TopMiddleBottom if layer != TOP && layer != BOTTOM => 2,
            _ => layer,
        };
        self.pad_stack
            .get_or_insert_with(|| {
                Box::new(PadStack {
                    mode,
                    ..Default::default()
                })
            })
            .layers
            .insert(key, geometry);
        self.sync_stack_legacy();
    }
    fn sync_stack_legacy(&mut self) {
        let Some(top) = self.pad_geometry(TOP) else {
            return;
        };
        let middle = self.pad_geometry(2).unwrap();
        let bottom = self.pad_geometry(BOTTOM).unwrap();
        let mode = self.stack_mode();
        match &mut self.shape {
            Shape::Pad {
                size,
                shape,
                bottom_size,
                middle_size,
                ..
            } => {
                *size = top.size;
                *shape = top.shape;
                *bottom_size = (mode != StackMode::Simple).then_some(bottom.size);
                *middle_size = (mode != StackMode::Simple).then_some(middle.size);
            }
            Shape::Via { diameter, .. } => *diameter = top.size.x,
            _ => {}
        }
    }
    pub fn drill_size(&self) -> i64 {
        match self.shape {
            Shape::Pad { hole, .. } | Shape::Via { hole, .. } => hole,
            _ => 0,
        }
    }
    pub fn pad_outline(&self, layer: u16) -> Vec<Point> {
        let Some(g) = self.pad_geometry(layer) else {
            return vec![];
        };
        if g.size.x <= 0 || g.size.y <= 0 {
            return vec![];
        }
        let rotation = match self.shape {
            Shape::Pad { rotation, .. } => rotation,
            _ => 0.0,
        };
        let local = crate::pcb_polygon::pad_outline_radius(
            g.offset,
            g.size,
            0.0,
            g.shape,
            g.corner_percent,
        );
        local
            .into_iter()
            .filter_map(|p| transform_point(p, Point::default(), self.anchor(), rotation).ok())
            .collect()
    }
    pub fn drill_outline(&self) -> Vec<Point> {
        let hole = self.drill_size();
        if hole <= 0 {
            return vec![];
        }
        let (shape, len, angle) = self
            .pad_stack
            .as_ref()
            .map(|s| (s.drill_shape, s.slot_length, s.drill_rotation))
            .unwrap_or((DrillShape::Round, 0, 0.0));
        let rotation = angle
            + match self.shape {
                Shape::Pad { rotation, .. } => rotation,
                _ => 0.0,
            };
        crate::pcb_polygon::pad_outline_radius(
            self.anchor(),
            Point {
                x: if shape == DrillShape::Slot {
                    len.max(hole)
                } else {
                    hole
                },
                y: hole,
            },
            rotation,
            if shape == DrillShape::Square {
                PadShape::Rectangle
            } else {
                PadShape::Round
            },
            100,
        )
    }
    pub(super) fn validate_stack(&self) -> Result<(), String> {
        let Some(s) = &self.pad_stack else {
            return Ok(());
        };
        if !matches!(self.shape, Shape::Pad { .. } | Shape::Via { .. })
            || s.layers.len() > 32
            || s.slot_length < 0
            || s.slot_length > MAX_COORD
            || !s.drill_rotation.is_finite()
        {
            return Err("Invalid padstack/drill geometry".into());
        }
        if matches!(self.shape, Shape::Via { .. }) && s.drill_shape != DrillShape::Round {
            return Err("Vias require a round drill".into());
        }
        for (layer, g) in &s.layers {
            let empty = g.size == Point::default()
                && matches!(self.shape,Shape::Pad{plated:false,hole,..} if hole>0);
            if !(1..=32).contains(layer)
                || !(empty || positive(g.size.x) && positive(g.size.y))
                || g.corner_percent > 100
                || !valid_point(g.offset)
                || (matches!(self.shape,Shape::Via{hole,..} if hole>=g.size.x || hole>=g.size.y)
                    || matches!(self.shape, Shape::Via { .. })
                        && (g.shape != PadShape::Round
                            || g.size.x != g.size.y
                            || g.offset != Point::default()))
            {
                return Err(format!(
                    "Invalid padstack land size, shape or offset: layer={layer}, geometry={g:?}, drill={}",
                    self.drill_size()
                ));
            }
        }
        if s.drill_shape == DrillShape::Slot && s.slot_length < self.drill_size() {
            return Err("Slot length must be at least its width".into());
        }
        Ok(())
    }
    pub fn hit_distance_on_layer(&self, p: Point, layer: u16) -> f64 {
        if matches!(self.shape, Shape::Pad { .. } | Shape::Via { .. }) {
            let outline = self.pad_outline(layer);
            if outline.is_empty() {
                return polygon_distance(p, &self.drill_outline());
            }
            return polygon_distance(p, &outline);
        }
        hit_distance(self, p)
    }
}

#[derive(Clone, Debug)]
pub struct StackSurface {
    pub outline: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
}
pub fn cutout_surface(outline: &[Point], hole: &[Point]) -> Vec<StackSurface> {
    use geo::{BooleanOps, Coord, LineString, Polygon};
    let ring = |points: &[Point]| {
        let mut p: Vec<_> = points
            .iter()
            .map(|p| Coord {
                x: p.x as f64 / 1e6,
                y: p.y as f64 / 1e6,
            })
            .collect();
        if let Some(first) = p.first().copied() {
            p.push(first);
        }
        LineString::new(p)
    };
    if outline.len() < 3 {
        return vec![];
    }
    let outer = Polygon::new(ring(outline), vec![]);
    let result = if hole.len() >= 3 {
        outer.difference(&Polygon::new(ring(hole), vec![]))
    } else {
        geo::MultiPolygon(vec![outer])
    };
    let points = |r: &LineString<f64>| {
        r.0.iter()
            .take(r.0.len().saturating_sub(1))
            .map(|p| Point {
                x: (p.x * 1e6).round() as i64,
                y: (p.y * 1e6).round() as i64,
            })
            .collect()
    };
    result
        .0
        .into_iter()
        .map(|p| StackSurface {
            outline: points(p.exterior()),
            holes: p.interiors().iter().map(points).collect(),
        })
        .collect()
}
impl Board {
    pub fn stack_thickness_nm(&self) -> i64 {
        if let Some(s) = &self.layer_stack {
            return s.thickness();
        }
        let sum: i64 = self
            .properties
            .iter()
            .filter(|(k, _)| k.starts_with("V9_STACK_LAYER") && k.ends_with("_DIELHEIGHT"))
            .filter_map(|(_, v)| super::mask::dimension(v).ok())
            .filter(|v| *v > 0)
            .sum();
        if sum > 0 { sum } else { 1_600_000 }
    }
}
