//! Native view preferences are read into the view, without modifying the board.
use eda_core::pcb::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct Settings {
    pub(super) opacity: BTreeMap<u16, [u8; 23]>,
    pub colors: BTreeMap<u16, egui::Color32>,
    pub options_tab: bool,
    pub panel: super::configuration::Panel,
    pub hidden: [bool; 23],
    pub draft: [bool; 23],
    pub highlight_dim: u8,
    pub masked_dim: u8,
    pub selection_color: egui::Color32,
    pub highlight_color: egui::Color32,
    pub background_end: egui::Color32,
    pub board_line: egui::Color32,
    pub via_hole_color: egui::Color32,
    pub origin_color: egui::Color32,
    pub origin_visible: bool,
    pub board_visible: bool,
    pub pad_holes_visible: bool,
    pub via_holes_visible: bool,
    pub all_connections: bool,
    pub layer_connections: bool,
    pub via_span: bool,
    pub track_names: bool,
    pub status_info: bool,
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
            dim: 153,
            highlight_dim: 102,
            masked_dim: 128,
            selection_color: egui::Color32::WHITE,
            highlight_color: egui::Color32::from_rgb(255, 255, 200),
            background_end: egui::Color32::from_rgb(109, 106, 105),
            board_line: egui::Color32::BLACK,
            via_hole_color: egui::Color32::from_rgb(128, 96, 0),
            origin_color: egui::Color32::WHITE,
            origin_visible: true,
            board_visible: true,
            pad_holes_visible: true,
            via_holes_visible: true,
            all_connections: fields.get("CFG2D.ALLCONNECTIONSINSINGLELAYERMODE") != Some(&"FALSE"),
            layer_connections: fields.get("CFG2D.MULTICOLOREDCONNECTIONS") == Some(&"TRUE"),
            via_span: fields.get("CFG2D.SHOWVIASPAN") == Some(&"TRUE"),
            track_names: fields
                .get("CFG2D.DISPLAYNETNAMESONTRACKS")
                .is_some_and(|v| *v != "0"),
            status_info: fields.get("CFG2D.SHOWSTATUSINFO") != Some(&"FALSE"),
            background: egui::Color32::from_rgb(109, 106, 105),
            board_color: egui::Color32::BLACK,
            grid_color: egui::Color32::from_gray(65),
            hole_color: egui::Color32::from_rgb(0, 128, 128),
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
        let catalog = super::catalog::Catalog::from_board(board);
        for layer in &catalog.layers {
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
        if let Some(raw) = fields.get("CFG2D.PRIMDRAWMODE") {
            for (flag, mode) in s.draft.iter_mut().zip(raw.chars()) {
                *flag = mode == '1';
            }
        }
        s.panel.catalog = std::sync::Arc::new(catalog);
        s
    }
    pub fn kind(item: &Item) -> usize {
        if item.keepout {
            return 13;
        }
        if item.properties.contains_key("PCBCRAFT_BODY_PROJECTION") {
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
        }
    }
    pub fn alpha(&self, item: &Item) -> u8 {
        let kind = Self::kind(item);
        if self.hidden[kind] {
            0
        } else {
            self.opacity.get(&item.layer).map_or(255, |v| v[kind])
        }
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
