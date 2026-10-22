//! Native view preferences are read into the view, without modifying the board.
use eda_core::pcb::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct Settings {
    pub(super) opacity: BTreeMap<u16, [u8; 23]>,
    pub colors: BTreeMap<u16, egui::Color32>,
    pub options_tab: bool,
    pub all_layers: bool,
    pub connections: bool,
    pub connection_color: egui::Color32,
    pub dim: u8,
    pub background: egui::Color32,
    pub board_color: egui::Color32,
    pub grid_color: egui::Color32,
    pub hole_color: egui::Color32,
    pub visible: Option<BTreeSet<u16>>,
    pub pad_numbers: bool,
    pub pad_nets: bool,
    pub via_nets: bool,
    pub snap_grid_nm: Option<i64>,
}

fn layer_key(layer: u16) -> String {
    match layer {
        TOP => "TOPLAYER".into(),
        BOTTOM => "BOTTOMLAYER".into(),
        33 => "TOPOVERLAY".into(),
        34 => "BOTTOMOVERLAY".into(),
        35 => "TOPPASTE".into(),
        36 => "BOTTOMPASTE".into(),
        37 => "TOPSOLDER".into(),
        38 => "BOTTOMSOLDER".into(),
        55 => "DRILLGUIDE".into(),
        56 => "KEEPOUTLAYER".into(),
        73 => "DRILLDRAWING".into(),
        74 => "MULTILAYER".into(),
        57..=72 => format!("MECHANICAL{}", layer - 56),
        83..=1090 => format!("MECHANICAL{}", layer - 66),
        2..=31 => format!("MIDLAYER{}", layer - 1),
        39..=54 => format!("INTERNALPLANE{}", layer - 38),
        _ => format!("LAYER{layer}"),
    }
}
impl Settings {
    pub fn from_board(board: &Board) -> Self {
        let raw = board
            .properties
            .get("2DCONFIGURATION")
            .map_or("", String::as_str);
        let fields: BTreeMap<_, _> = raw.split('`').filter_map(|s| s.split_once('=')).collect();
        let mut s = Self {
            connections: true,
            connection_color: egui::Color32::from_rgb(220, 220, 220),
            dim: 50,
            background: egui::Color32::from_rgb(109, 106, 105),
            board_color: egui::Color32::BLACK,
            grid_color: egui::Color32::from_gray(65),
            hole_color: egui::Color32::from_rgb(210, 210, 190),
            pad_numbers: fields.get("CFG2D.SHOWPADNUMBERS") == Some(&"TRUE"),
            pad_nets: fields.get("CFG2D.SHOWPADNETS") == Some(&"TRUE"),
            via_nets: fields.get("CFG2D.SHOWVIANETS") == Some(&"TRUE"),
            snap_grid_nm: fields
                .get("CFG2D.SNAPGRIDSIZE")
                .copied()
                .or_else(|| board.properties.get("SNAPGRIDSIZE").map(String::as_str))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0 && *v <= 1e10)
                .map(|v| (v * 2.54).round() as i64)
                .filter(|v| *v > 0),
            ..Default::default()
        };
        if let Some(raw) = fields.get("CFG2D.TOGGLELAYERS.SET") {
            let mut ids: BTreeSet<_> = raw
                .split([',', '_'])
                .filter_map(|s| s.split_once('='))
                .filter(|(_, v)| *v == "1")
                .filter_map(|(id, _)| id.parse().ok().and_then(native_layer))
                .collect();
            for layer in &board.layers {
                let group = if layer.copper {
                    "Signal"
                } else if matches!(layer.id,57..=72|83..=1090) {
                    "Mechanical"
                } else {
                    "Standard"
                };
                if raw.contains(&format!("{group}.All~1")) {
                    ids.insert(layer.id);
                }
            }
            if !ids.is_empty() || raw.contains(".All~0") {
                s.visible = Some(ids);
            }
        }
        for layer in &board.layers {
            let key = format!("CFG2D.LAYEROPACITY.{}", layer_key(layer.id));
            if let Some(raw) = fields.get(key.as_str()) {
                let mut values = [255; 23];
                for (out, value) in values.iter_mut().zip(raw.split('?')) {
                    if let Ok(value) = value.parse::<f32>()
                        && value.is_finite()
                    {
                        *out = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
                s.opacity.insert(layer.id, values);
            }
        }
        s
    }
    pub fn alpha(&self, item: &Item) -> u8 {
        let kind = if item.properties.contains_key("PCBCRAFT_BODY_PROJECTION") {
            12
        } else if item.polygon.is_some() {
            10
        } else {
            match item.shape {
                Shape::Arc { .. } => 1,
                Shape::Pad { .. } => 2,
                Shape::Via { .. } => 3,
                Shape::Track { .. } => 4,
                Shape::Text { .. } => 5,
                Shape::Fill { .. } => 6,
                Shape::Region { .. } => 11,
            }
        };
        self.opacity.get(&item.layer).map_or(255, |v| v[kind])
    }
}

impl super::BoardView {
    pub fn color(&self, layer: u16) -> egui::Color32 {
        self.display
            .colors
            .get(&layer)
            .copied()
            .unwrap_or_else(|| super::layer_color(layer))
    }
    /// Document-local display controls; never edit the electrical model or history.
    pub fn configuration_panel(
        &mut self,
        ui: &mut egui::Ui,
        board: &Board,
        grid: &mut bool,
    ) -> Option<u16> {
        let mut requested_layer = None;
        self.initialize(board);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.display.options_tab, false, "Layers & Colors");
            ui.selectable_value(&mut self.display.options_tab, true, "View Options");
        });
        ui.separator();
        if self.display.connections {
            ui.small(if self.connections.calculating() {
                "Updating connections…".into()
            } else {
                format!("{} connections", self.connections.lines.len())
            });
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !self.display.options_tab {
                ui.collapsing("Configuration", |ui| {
                    ui.label("Current document");
                    ui.add_enabled(false, egui::Button::new("Load / Save Configuration…"))
                        .on_hover_text("Native view configuration profiles are not supported yet");
                });
                for (group, title) in [
                    (0, "Signal Layers"),
                    (1, "Internal Planes"),
                    (2, "Mechanical Layers"),
                    (3, "Other Layers"),
                ] {
                    egui::CollapsingHeader::new(title)
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut layers: Vec<_> = board
                                .layers
                                .iter()
                                .filter(|l| {
                                    let g = if (39..=54).contains(&l.id) {
                                        1
                                    } else if l.copper {
                                        0
                                    } else if matches!(l.id,57..=72|83..=1090) {
                                        2
                                    } else {
                                        3
                                    };
                                    g == group
                                })
                                .collect();
                            layers.sort_by_key(|layer| layer.order);
                            ui.horizontal(|ui| {
                                for (label, on) in [("All On", true), ("All Off", false)] {
                                    if ui.small_button(label).clicked() {
                                        for l in &layers {
                                            if on {
                                                self.visible.insert(l.id);
                                            } else {
                                                self.visible.remove(&l.id);
                                            }
                                        }
                                    }
                                }
                            });
                            for layer in layers {
                                ui.horizontal(|ui| {
                                    let mut color = self.color(layer.id);
                                    if ui.color_edit_button_srgba(&mut color).changed() {
                                        self.display.colors.insert(layer.id, color);
                                    }
                                    let mut visible = self.visible.contains(&layer.id);
                                    if ui.checkbox(&mut visible, "").changed() {
                                        if visible {
                                            self.visible.insert(layer.id);
                                        } else {
                                            self.visible.remove(&layer.id);
                                        }
                                    }
                                    if ui
                                        .selectable_label(
                                            self.active_layer == layer.id,
                                            &layer.name,
                                        )
                                        .clicked()
                                    {
                                        requested_layer = Some(layer.id);
                                        self.visible.insert(layer.id);
                                    }
                                });
                            }
                        });
                }
                egui::CollapsingHeader::new("System Colors")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut self.display.connection_color);
                            ui.checkbox(&mut self.display.connections, "Connections");
                        });
                        ui.checkbox(grid, "Visible Grid");
                        for (label, color) in [
                            ("Workspace", &mut self.display.background),
                            ("Board Area", &mut self.display.board_color),
                            ("Grid", &mut self.display.grid_color),
                            ("Pad / Via Holes", &mut self.display.hole_color),
                        ] {
                            ui.horizontal(|ui| {
                                ui.color_edit_button_srgba(color);
                                ui.label(label);
                            });
                        }
                    });
            } else {
                if self.three_d.enabled {
                    egui::CollapsingHeader::new("3D Settings")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.checkbox(&mut self.three_d.show_components, "Show Component Bodies");
                            ui.horizontal(|ui| {
                                if ui.button("Top").clicked() {
                                    self.three_d.top();
                                }
                                if ui.button("Bottom").clicked() {
                                    self.three_d.bottom();
                                }
                                if ui.button("Fit Board").clicked() {
                                    self.three_d.fit();
                                }
                            });
                            ui.label("2D display settings apply when returning to 2D.");
                        });
                }
                egui::CollapsingHeader::new("Object Visibility")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut self.display.all_layers,
                                false,
                                "Active layer",
                            );
                            ui.selectable_value(&mut self.display.all_layers, true, "All layers");
                        });
                        ui.label("Transparency (%)");
                        for (kind, label) in [
                            (1, "Arcs"),
                            (2, "Pads"),
                            (3, "Vias"),
                            (4, "Tracks"),
                            (5, "Texts"),
                            (6, "Fills"),
                            (10, "Polygons"),
                            (11, "Regions"),
                            (12, "Component Bodies"),
                        ] {
                            let alpha = self
                                .display
                                .opacity
                                .get(&self.active_layer)
                                .map_or(255, |v| v[kind]);
                            let mut transparency = 100.0 - f32::from(alpha) * 100.0 / 255.0;
                            if ui
                                .add(egui::Slider::new(&mut transparency, 0.0..=100.0).text(label))
                                .changed()
                            {
                                let alpha = ((100.0 - transparency) * 255.0 / 100.0).round() as u8;
                                for layer in &board.layers {
                                    if self.display.all_layers || layer.id == self.active_layer {
                                        self.display.opacity.entry(layer.id).or_insert([255; 23])
                                            [kind] = alpha;
                                    }
                                }
                            }
                        }
                    });
                egui::CollapsingHeader::new("Additional Options")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.display.connections, "Show Connections");
                        ui.checkbox(&mut self.display.pad_numbers, "Show Pad Numbers");
                        ui.checkbox(&mut self.display.pad_nets, "Show Pad Nets");
                        ui.checkbox(&mut self.display.via_nets, "Show Via Nets");
                        ui.checkbox(&mut self.polygons_visible, "Show Polygon Copper");
                        ui.checkbox(grid, "Show Grid");
                    });
                egui::CollapsingHeader::new("Mask and Dim Settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("Single Layer Mode (Shift+S)");
                        ui.horizontal(|ui| {
                            for (n, s) in [(0, "Off"), (1, "Dim"), (2, "Hide")] {
                                ui.selectable_value(&mut self.single_layer, n, s);
                            }
                        });
                        ui.add(egui::Slider::new(&mut self.display.dim, 0..=255).text("Dim level"));
                    });
            }
        });
        requested_layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    #[test]
    fn saved_view_controls_modern_layers_transparency_labels_and_grid() {
        let mut b = Board::default();
        b.properties
            .insert("SNAPGRIDSIZE".into(), "10000.000000".into());
        b.layers.push(PcbLayer {
            id: 84,
            name: "Carbon".into(),
            copper: false,
            order: 9,
        });
        b.properties.insert("2DCONFIGURATION".into(),"CFG2D.TOGGLELAYERS.SET=Signal.All~0_Signal.Include~SerializeLayerHash.Version=2,ClassName=TLayerHash,16777217=1_Mechanical.All~0_Mechanical.Include~SerializeLayerHash.Version=2,ClassName=TLayerHash,16908306=1_Standard.All~0`CFG2D.LAYEROPACITY.TOPLAYER=1?1?0.8?1?1?1?1?1?1?1?0.8?0.8`CFG2D.SHOWPADNUMBERS=TRUE`CFG2D.SHOWPADNETS=FALSE`CFG2D.SHOWVIANETS=TRUE`CFG2D.SNAPGRIDSIZE=10000".into());
        let s = Settings::from_board(&b);
        assert_eq!(s.visible, Some(BTreeSet::from([TOP, 84])));
        assert!(s.pad_numbers && !s.pad_nets && s.via_nets);
        assert_eq!(s.snap_grid_nm, Some(25_400));
        let mut i = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point::mm(1, 0),
                width: 100_000,
            },
        );
        assert_eq!(s.alpha(&i), 255);
        i.polygon = Some(0);
        assert_eq!(s.alpha(&i), 204);
        b.properties.insert(
            "2DCONFIGURATION".into(),
            "CFG2D.TOGGLELAYERS.SET=Signal.All~1_Mechanical.All~1_Standard.All~0".into(),
        );
        assert_eq!(
            Settings::from_board(&b).visible,
            Some(BTreeSet::from([TOP, BOTTOM, 84]))
        );
        b.properties.insert(
            "2DCONFIGURATION".into(),
            "CFG2D.TOGGLELAYERS.SET=Signal.All~0_Mechanical.All~0_Standard.All~0".into(),
        );
        assert_eq!(Settings::from_board(&b).visible, Some(BTreeSet::new()));
        assert_eq!(Settings::from_board(&b).snap_grid_nm, Some(25_400));
    }
}
