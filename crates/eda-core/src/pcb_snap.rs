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
        for item in &self.items {
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
                Shape::Pad { position, .. } if options.pad_centres => consider(*position),
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
}
