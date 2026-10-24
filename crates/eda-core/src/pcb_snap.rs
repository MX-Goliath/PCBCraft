//! Document-space snapping, independent of renderer and platform.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapLayerMode {
    All,
    #[default]
    Current,
    Off,
}
#[derive(Clone, Debug)]
pub struct SnapOptions {
    pub mode: SnapLayerMode,
    pub distance: i64,
    pub track_vertices: bool,
    pub arc_centres: bool,
    pub pad_centres: bool,
    pub pad_edges: bool,
    pub via_centres: bool,
    pub region_vertices: bool,
    pub board_vertices: bool,
    pub component_origins: bool,
}
impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            mode: SnapLayerMode::Current,
            distance: 250_000,
            track_vertices: true,
            arc_centres: true,
            pad_centres: true,
            pad_edges: true,
            via_centres: true,
            region_vertices: true,
            board_vertices: false,
            component_origins: true,
        }
    }
}
impl Board {
    pub fn snap_with_options(
        &self,
        raw: Point,
        grid: i64,
        layer: u16,
        visible: &BTreeSet<u16>,
        options: &SnapOptions,
    ) -> Point {
        self.snap_with_items(raw, grid, layer, visible, options, self.items.iter())
    }

    /// Same exact snapping rules with a conservative spatial candidate set.
    /// Callers must include every item within the snap distance, in document
    /// order, including all pad-stack lands and arc/component centres.
    pub fn snap_with_items<'a>(
        &self,
        raw: Point,
        grid: i64,
        layer: u16,
        visible: &BTreeSet<u16>,
        options: &SnapOptions,
        items: impl IntoIterator<Item = &'a Item>,
    ) -> Point {
        let mut result = Point {
            x: snap_coord(raw.x, grid),
            y: snap_coord(raw.y, grid),
        };
        if options.mode == SnapLayerMode::Off {
            return result;
        }
        let mut best = (options.distance.max(0) as f64).powi(2);
        let mut consider = |p: Point| {
            let d = distance_sq(raw, p);
            if d <= best {
                best = d;
                result = p;
            }
        };
        for item in items {
            if !visible.contains(&item.layer)
                || (options.mode == SnapLayerMode::Current
                    && !crate::pcb_polygon::on_layer(self, item, layer))
                || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
                || item
                    .properties
                    .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
            {
                continue;
            }
            match &item.shape {
                Shape::Track { start, end, .. } if options.track_vertices => {
                    consider(*start);
                    consider(*end);
                }
                Shape::Arc {
                    centre,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    if options.arc_centres {
                        consider(*centre);
                    }
                    if options.track_vertices {
                        let points = arc_points(*centre, *radius, *start_angle, *end_angle, 2);
                        consider(points[0]);
                        consider(*points.last().unwrap());
                    }
                }
                Shape::Pad { position, .. } => {
                    if options.pad_centres {
                        consider(*position);
                    }
                    if options.pad_edges {
                        for candidate in self.layers.iter().filter(|l| {
                            l.copper
                                && visible.contains(&l.id)
                                && (options.mode == SnapLayerMode::All || l.id == layer)
                                && crate::pcb_polygon::on_layer(self, item, l.id)
                        }) {
                            if let Some(p) =
                                nearest_pad_edge(item, candidate.id, raw, options.distance)
                            {
                                consider(p);
                            }
                        }
                    }
                }
                Shape::Via { position, .. } if options.via_centres => consider(*position),
                Shape::Fill {
                    start,
                    end,
                    rotation,
                } if options.region_vertices => {
                    let size = Point {
                        x: (end.x - start.x).abs(),
                        y: (end.y - start.y).abs(),
                    };
                    for p in rectangle(midpoint(*start, *end), size, *rotation) {
                        consider(p);
                    }
                }
                Shape::Region { contour, holes, .. } if options.region_vertices => {
                    for p in contour.iter().chain(holes.iter().flatten()) {
                        consider(*p);
                    }
                }
                _ => {}
            }
        }
        if options.region_vertices {
            for polygon in &self.polygon_pours {
                if !polygon.shelved
                    && visible.contains(&polygon.layer)
                    && (options.mode == SnapLayerMode::All || polygon.layer == layer)
                {
                    for v in &polygon.contour {
                        consider(v.position);
                    }
                }
            }
        }
        if options.board_vertices {
            for v in &self.outline {
                consider(v.position);
            }
        }
        if options.component_origins {
            for c in &self.components {
                if visible.contains(&c.layer)
                    && (options.mode == SnapLayerMode::All || c.layer == layer)
                {
                    consider(c.position);
                }
            }
        }
        result
    }
}

/// Project onto actual copper, not the bounding box or the tessellated display
/// mesh. Rounded corners remain stable at every zoom and snap distance.
fn nearest_pad_edge(item: &Item, layer: u16, raw: Point, distance: i64) -> Option<Point> {
    let Shape::Pad {
        position, rotation, ..
    } = item.shape
    else {
        return None;
    };
    let g = item.pad_geometry(layer)?;
    if g.size.x <= 0 || g.size.y <= 0 {
        return None;
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let dx = (raw.x - position.x) as f64;
    let dy = (raw.y - position.y) as f64;
    let x = dx * cos + dy * sin - g.offset.x as f64;
    let y = -dx * sin + dy * cos - g.offset.y as f64;
    let hx = g.size.x as f64 / 2.0;
    let hy = g.size.y as f64 / 2.0;
    // Reject distant pads before doing any contour work.
    if (x.abs() - hx).max(0.0).powi(2) + (y.abs() - hy).max(0.0).powi(2)
        > (distance.max(0) as f64).powi(2)
    {
        return None;
    }
    let (px, py) = if g.shape == PadShape::Octagonal {
        let d = g.size.x.min(g.size.y) as f64 * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
        let vertices = [
            (-hx + d, -hy),
            (hx - d, -hy),
            (hx, -hy + d),
            (hx, hy - d),
            (hx - d, hy),
            (-hx + d, hy),
            (-hx, hy - d),
            (-hx, -hy + d),
        ];
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
            let vx = b.0 - a.0;
            let vy = b.1 - a.1;
            let len = vx * vx + vy * vy;
            let t = if len > 0.0 {
                ((x - a.0) * vx + (y - a.1) * vy) / len
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let px = a.0 + t * vx;
            let py = a.1 + t * vy;
            let dist = (x - px).powi(2) + (y - py).powi(2);
            if dist < best.0 {
                best = (dist, px, py);
            }
        }
        (best.1, best.2)
    } else {
        let radius = hx.min(hy)
            * match g.shape {
                PadShape::Round => 1.0,
                PadShape::RoundedRectangle => g.corner_percent.min(100) as f64 / 100.0,
                _ => 0.0,
            };
        let cx = x.clamp(-hx + radius, hx - radius);
        let cy = y.clamp(-hy + radius, hy - radius);
        let length = (x - cx).hypot(y - cy);
        if length > 0.0 {
            (
                cx + radius * (x - cx) / length,
                cy + radius * (y - cy) / length,
            )
        } else if hx - x.abs() < hy - y.abs() {
            (if x < 0.0 { -hx } else { hx }, y)
        } else {
            (x, if y < 0.0 { -hy } else { hy })
        }
    };
    let px = px + g.offset.x as f64;
    let py = py + g.offset.y as f64;
    let point = Point {
        x: (position.x as f64 + px * cos - py * sin).round() as i64,
        y: (position.y as f64 + px * sin + py * cos).round() as i64,
    };
    valid_point(point).then_some(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_visibility_grid_and_object_switches_control_snapping() {
        let mut b = Board::default();
        let p = Point {
            x: 110_000,
            y: 130_000,
        };
        b.items.push(Item::new(
            BOTTOM,
            None,
            Shape::Via {
                position: p,
                diameter: 600_000,
                hole: 300_000,
                from_layer: BOTTOM,
                to_layer: BOTTOM,
            },
        ));
        let visible = BTreeSet::from([TOP, BOTTOM]);
        let raw = Point {
            x: 120_000,
            y: 120_000,
        };
        let mut o = SnapOptions::default();
        assert_eq!(
            b.snap_with_options(raw, 100_000, TOP, &visible, &o),
            Point {
                x: 100_000,
                y: 100_000
            }
        );
        o.mode = SnapLayerMode::All;
        assert_eq!(b.snap_with_options(raw, 100_000, TOP, &visible, &o), p);
        o.via_centres = false;
        assert_eq!(b.snap_with_options(raw, 0, TOP, &visible, &o), raw);
        o.via_centres = true;
        assert_eq!(
            b.snap_with_options(raw, 0, TOP, &BTreeSet::from([TOP]), &o),
            raw
        );
        o.mode = SnapLayerMode::Off;
        assert_eq!(b.snap_with_options(raw, 0, TOP, &visible, &o), raw);
        o.mode = SnapLayerMode::All;
        o.distance = 1;
        assert_eq!(b.snap_with_options(raw, 0, TOP, &visible, &o), raw);
    }

    #[test]
    fn arc_centres_and_endpoints_have_independent_switches() {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Arc {
                centre: Point::default(),
                radius: 1_000_000,
                start_angle: 0.0,
                end_angle: 90.0,
                width: 100_000,
            },
        ));
        let visible = BTreeSet::from([TOP]);
        let raw = Point {
            x: 10_000,
            y: 20_000,
        };
        let mut options = SnapOptions {
            track_vertices: false,
            ..Default::default()
        };
        assert_eq!(
            b.snap_with_options(raw, 0, TOP, &visible, &options),
            Point::default()
        );
        options.arc_centres = false;
        assert_eq!(b.snap_with_options(raw, 0, TOP, &visible, &options), raw);
        options.track_vertices = true;
        let near_end = Point {
            x: 1_010_000,
            y: 20_000,
        };
        assert_eq!(
            b.snap_with_options(near_end, 0, TOP, &visible, &options),
            Point::mm(1, 0)
        );
    }

    #[test]
    fn rotated_fill_vertices_and_board_outline_use_document_geometry() {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Fill {
                start: Point::mm(-2, -1),
                end: Point::mm(2, 1),
                rotation: 90.0,
            },
        ));
        let visible = BTreeSet::from([TOP]);
        let mut options = SnapOptions::default();
        let raw = Point {
            x: 1_010_000,
            y: 2_020_000,
        };
        assert_eq!(
            b.snap_with_options(raw, 0, TOP, &visible, &options),
            Point::mm(1, 2)
        );
        options.region_vertices = false;
        assert_eq!(b.snap_with_options(raw, 0, TOP, &visible, &options), raw);
        b.outline.push(ContourVertex {
            position: Point::mm(1, 2),
            arc: None,
        });
        options.board_vertices = true;
        assert_eq!(
            b.snap_with_options(raw, 0, TOP, &visible, &options),
            Point::mm(1, 2)
        );
    }
    fn land(shape: PadShape) -> Item {
        Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(2, 2),
                rotation: 0.0,
                shape,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        )
    }
    fn edge_options() -> SnapOptions {
        SnapOptions {
            pad_centres: false,
            ..Default::default()
        }
    }
    fn close(actual: Point, expected: Point) {
        assert!(
            (actual.x - expected.x).abs() <= 2 && (actual.y - expected.y).abs() <= 2,
            "{actual:?} != {expected:?}"
        );
    }
    #[test]
    fn pad_edges_project_to_finite_edges_and_respect_distance_grid_and_switches() {
        let mut board = Board::default();
        board.items.push(land(PadShape::Rectangle));
        let before = board.clone();
        let visible = BTreeSet::from([TOP]);
        let raw = Point {
            x: 1_090_000,
            y: 370_000,
        };
        let mut options = edge_options();
        assert_eq!(
            board.snap_with_options(raw, 500_000, TOP, &visible, &options),
            Point {
                x: 1_000_000,
                y: 370_000
            }
        );
        let corner = Point {
            x: 1_090_000,
            y: 1_100_000,
        };
        assert_eq!(
            board.snap_with_options(corner, 0, TOP, &visible, &options),
            Point::mm(1, 1)
        );
        options.distance = 89_999;
        assert_eq!(
            board.snap_with_options(raw, 0, TOP, &visible, &options),
            raw
        );
        options.distance = 250_000;
        options.pad_edges = false;
        assert_eq!(
            board.snap_with_options(raw, 0, TOP, &visible, &options),
            raw
        );
        options.pad_edges = true;
        options.mode = SnapLayerMode::Off;
        assert_eq!(
            board.snap_with_options(raw, 500_000, TOP, &visible, &options),
            Point {
                x: 1_000_000,
                y: 500_000
            }
        );
        options.mode = SnapLayerMode::Current;
        assert_eq!(
            board.snap_with_options(raw, 0, TOP, &BTreeSet::new(), &options),
            raw
        );
        assert_eq!(board, before);
    }
    #[test]
    fn curved_edges_are_analytic_instead_of_snapping_to_display_chords() {
        let mut board = Board::default();
        board.items.push(land(PadShape::Round));
        let visible = BTreeSet::from([TOP]);
        let options = edge_options();
        for n in 0..120 {
            let angle = (n as f64 * 3.0 + 1.3).to_radians();
            for radius in [900_000.0, 1_100_000.0] {
                let point = |r: f64| Point {
                    x: (angle.cos() * r).round() as i64,
                    y: (angle.sin() * r).round() as i64,
                };
                close(
                    board.snap_with_options(point(radius), 0, TOP, &visible, &options),
                    point(1_000_000.0),
                );
            }
        }
        let outside = Point {
            x: 900_000,
            y: 900_000,
        };
        assert_eq!(
            board.snap_with_options(outside, 0, TOP, &visible, &options),
            outside
        );
        // Round elongated lands are capsules: the straight section remains straight.
        if let Shape::Pad { size, .. } = &mut board.items[0].shape {
            *size = Point::mm(4, 2);
        }
        assert_eq!(
            board.snap_with_options(
                Point {
                    x: 500_000,
                    y: 1_100_000
                },
                0,
                TOP,
                &visible,
                &options
            ),
            Point {
                x: 500_000,
                y: 1_000_000
            }
        );
        let p = board.snap_with_options(
            Point {
                x: 1_800_000,
                y: 800_000,
            },
            0,
            TOP,
            &visible,
            &options,
        );
        close(
            p,
            Point {
                x: 1_707_107,
                y: 707_107,
            },
        );
    }
    #[test]
    fn rounded_and_octagonal_edges_do_not_use_bounding_rectangle_corners() {
        let mut board = Board::default();
        let mut pad = land(PadShape::RoundedRectangle);
        pad.properties
            .insert("PCBCRAFT_PAD_RADIUS_1".into(), "50".into());
        board.items.push(pad);
        let visible = BTreeSet::from([TOP]);
        let options = edge_options();
        let raw = Point {
            x: 900_000,
            y: 900_000,
        };
        close(
            board.snap_with_options(raw, 0, TOP, &visible, &options),
            Point {
                x: 853_553,
                y: 853_553,
            },
        );
        board.items[0] = land(PadShape::Octagonal);
        let raw = Point {
            x: 800_000,
            y: 800_000,
        };
        close(
            board.snap_with_options(raw, 0, TOP, &visible, &options),
            Point {
                x: 707_107,
                y: 707_107,
            },
        );
    }
    #[test]
    fn edge_snapping_uses_rotated_layer_offsets_and_actual_visible_stack_lands() {
        let mut board = Board::default();
        let mut pad = land(PadShape::Rectangle);
        pad.layer = MULTI_LAYER;
        if let Shape::Pad {
            rotation, position, ..
        } = &mut pad.shape
        {
            *rotation = 90.0;
            *position = Point::mm(10, 20);
        }
        pad.pad_stack = Some(Box::new(PadStack {
            mode: StackMode::Full,
            layers: BTreeMap::from([
                (
                    TOP,
                    PadGeometry {
                        size: Point::mm(2, 2),
                        shape: PadShape::Rectangle,
                        corner_percent: 0,
                        offset: Point {
                            x: 300_000,
                            y: 100_000,
                        },
                    },
                ),
                (
                    BOTTOM,
                    PadGeometry {
                        size: Point::mm(4, 4),
                        shape: PadShape::Rectangle,
                        corner_percent: 0,
                        offset: Point::default(),
                    },
                ),
            ]),
            ..Default::default()
        }));
        board.items.push(pad);
        let visible = BTreeSet::from([TOP, BOTTOM, MULTI_LAYER]);
        let mut options = edge_options();
        let raw = Point {
            x: 9_700_000,
            y: 21_350_000,
        };
        assert_eq!(
            board.snap_with_options(raw, 0, TOP, &visible, &options),
            Point {
                x: 9_700_000,
                y: 21_300_000
            }
        );
        let bottom = Point {
            x: 12_100_000,
            y: 20_000_000,
        };
        assert_eq!(
            board.snap_with_options(bottom, 0, TOP, &visible, &options),
            bottom
        );
        options.mode = SnapLayerMode::All;
        assert_eq!(
            board.snap_with_options(bottom, 0, TOP, &visible, &options),
            Point::mm(12, 20)
        );
        assert_eq!(
            board.snap_with_options(
                bottom,
                0,
                TOP,
                &BTreeSet::from([TOP, MULTI_LAYER]),
                &options
            ),
            bottom
        );
        options.mode = SnapLayerMode::Current;
        assert_eq!(
            board.snap_with_options(bottom, 0, BOTTOM, &visible, &options),
            Point::mm(12, 20)
        );
        board.items[0]
            .pad_stack
            .as_mut()
            .unwrap()
            .layers
            .get_mut(&BOTTOM)
            .unwrap()
            .size = Point::default();
        assert_eq!(
            board.snap_with_options(bottom, 0, BOTTOM, &visible, &options),
            bottom
        );
    }
}
