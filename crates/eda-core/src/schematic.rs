//! Independent schematic scene. Native bytes and fields remain available for audit.
use crate::Point;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

/// Native standard sheet drawing areas, in nm; labels observed in AD 24.4.1.
pub const STANDARD_SHEETS: [(&str, i64, i64); 18] = [
    ("A4", 1150 * 254_000, 760 * 254_000),
    ("A3", 1550 * 254_000, 1110 * 254_000),
    ("A2", 2230 * 254_000, 1570 * 254_000),
    ("A1", 3150 * 254_000, 2230 * 254_000),
    ("A0", 4460 * 254_000, 3150 * 254_000),
    ("A", 950 * 254_000, 750 * 254_000),
    ("B", 1500 * 254_000, 950 * 254_000),
    ("C", 2000 * 254_000, 1500 * 254_000),
    ("D", 3200 * 254_000, 2000 * 254_000),
    ("E", 4200 * 254_000, 3200 * 254_000),
    ("Letter", 1100 * 254_000, 850 * 254_000),
    ("Legal", 1400 * 254_000, 850 * 254_000),
    ("Tabloid", 1700 * 254_000, 1100 * 254_000),
    ("OrCAD A", 990 * 254_000, 790 * 254_000),
    ("OrCAD B", 1540 * 254_000, 990 * 254_000),
    ("OrCAD C", 2060 * 254_000, 1560 * 254_000),
    ("OrCAD D", 3260 * 254_000, 2060 * 254_000),
    ("OrCAD E", 4280 * 254_000, 3280 * 254_000),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schematic {
    /// The native archive is historical once the editable scene has changed.
    #[serde(default)]
    pub source_modified: bool,
    pub sheet: Sheet,
    pub fonts: Vec<SchFont>,
    pub components: Vec<Component>,
    #[serde(default)]
    pub compile_masks: Vec<CompileMask>,
    pub primitives: Vec<SchPrimitive>,
    /// Hidden parameter objects have identity and editable presentation as well.
    #[serde(default)]
    pub parameter_objects: Vec<SchPrimitive>,
    pub records: Arc<Vec<SourceRecord>>,
    #[serde(default)]
    pub images: Arc<BTreeMap<String, String>>,
    pub source: Option<Arc<NativeSource>>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSource {
    pub format: String,
    pub filename: String,
    /// Unmodified original file, including unknown streams; native writers must
    /// verify supported scene changes against this archive before replacing it.
    pub original_base64: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileMask {
    pub a: Point,
    pub b: Point,
    pub collapsed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub index: u32,
    pub stream: String,
    pub record_type: u32,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sheet {
    #[serde(default)]
    pub options: SheetOptions,
    pub background: u32,
    pub border: bool,
    pub title_block: bool,
    pub zones: [u32; 2],
    #[serde(default)]
    pub zone_origin_lower_right: bool,
    #[serde(default)]
    pub system_font: usize,
    pub margin_nm: i64,
    pub grid_nm: i64,
    pub snap_nm: i64,
    pub grid_visible: bool,
    pub style: String,
    pub parameters: BTreeMap<String, String>,
}

impl Sheet {
    pub fn format_name(&self) -> &str {
        if let Some(value) = self.style.strip_prefix("Standard ") {
            if let Ok(n) = value.parse::<usize>()
                && let Some((name, _, _)) = STANDARD_SHEETS.get(n)
            {
                return name;
            }
            if STANDARD_SHEETS.iter().any(|(name, _, _)| *name == value) {
                return value;
            }
        }
        &self.style
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchFont {
    pub name: String,
    pub height_nm: i64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    #[serde(default)]
    pub settings: ComponentSettings,
    pub id: Uuid,
    pub source_index: u32,
    pub position: Point,
    pub designator: String,
    pub comment: String,
    pub library_reference: String,
    pub description: String,
    pub part: u32,
    pub display_mode: u32,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchPrimitive {
    #[serde(default)]
    pub presentation: Presentation,
    #[serde(default)]
    pub text_role: Option<TextRole>,
    pub id: Uuid,
    pub source_index: u32,
    pub component: Option<Uuid>,
    pub color: u32,
    pub fill: Option<u32>,
    pub width_nm: i64,
    pub line_style: u32,
    #[serde(default)]
    pub start_shape: u32,
    #[serde(default)]
    pub end_shape: u32,
    #[serde(default)]
    pub line_shape_size: u32,
    pub kind: SchPrimitiveKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextRole {
    Designator,
    Comment,
    NetLabel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SheetOptions {
    pub border_color: u32,
    pub show_zones: bool,
    pub snap_enabled: bool,
    pub electrical_snap: bool,
    pub snap_distance_nm: i64,
}
impl Default for SheetOptions {
    fn default() -> Self {
        Self {
            border_color: 0,
            show_zones: true,
            snap_enabled: true,
            electrical_snap: true,
            snap_distance_nm: 2_540_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComponentSettings {
    pub models: Vec<ComponentModel>,
    pub rotation: u32,
    pub part_count: u32,
    pub kind: u32,
    pub source: String,
    pub design_item_id: String,
    pub designator_locked: bool,
    pub part_locked: bool,
}
impl Default for ComponentSettings {
    fn default() -> Self {
        Self {
            models: vec![],
            rotation: 0,
            part_count: 1,
            kind: 0,
            source: String::new(),
            design_item_id: String::new(),
            designator_locked: false,
            part_locked: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentModel {
    pub name: String,
    pub kind: String,
    pub current: bool,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Presentation {
    pub symbol_type: String,
    /// Sheet Symbol ownership for entries and its designator/filename texts.
    pub parent: Option<Uuid>,
    pub alignment: Option<u32>,
    /// Port label ink is independent of its outline colour.
    pub text_color: Option<u32>,
    pub harness_type: String,
    pub record_type: u32,
    pub url: String,
    pub name: String,
    pub raw_text: Option<String>,
    pub hidden: bool,
    pub show_name: bool,
    pub auto_position: bool,
    pub locked: bool,
    pub allow_sync: bool,
}

pub fn parameter_text(
    text: &str,
    local: &BTreeMap<String, String>,
    sheet: &BTreeMap<String, String>,
) -> String {
    let mut value = text.to_owned();
    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..32 {
        let Some(key) = value.strip_prefix('=') else {
            break;
        };
        let key = key.trim().trim_matches('"');
        if !visited.insert(key.to_lowercase()) {
            return text.to_owned();
        }
        let lookup = |values: &BTreeMap<String, String>| {
            values
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.clone())
        };
        let Some(next) = lookup(local).or_else(|| lookup(sheet)) else {
            return value;
        };
        value = next;
    }
    value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SchPrimitiveKind {
    ParameterSet {
        position: Point,
        orientation: u32,
        label: String,
        style: u32,
    },
    Blanket {
        points: Vec<Point>,
        collapsed: bool,
    },
    CompileMask {
        a: Point,
        b: Point,
        collapsed: bool,
    },
    HarnessConnector {
        a: Point,
        b: Point,
        side: u32,
        primary_nm: i64,
        bus_text_style: String,
    },
    HarnessEntry {
        position: Point,
        side: u32,
        name: String,
        font: usize,
        text_color: u32,
    },
    Polyline {
        points: Vec<Point>,
        closed: bool,
        electrical: String,
    },
    Rectangle {
        a: Point,
        b: Point,
        radius: Point,
    },
    SheetSymbol {
        a: Point,
        b: Point,
        designator: String,
        filename: String,
    },
    SheetEntry {
        position: Point,
        side: u32,
        io_type: u32,
        name: String,
        font: usize,
        text_color: u32,
        arrow_kind: String,
    },
    Arc {
        centre: Point,
        radii: Point,
        start: f64,
        end: f64,
    },
    Bezier {
        points: Vec<Point>,
    },
    Text {
        position: Point,
        text: String,
        font: usize,
        orientation: u32,
        justification: u32,
    },
    TextFrame {
        a: Point,
        b: Point,
        text: String,
        font: usize,
        alignment: u32,
        word_wrap: bool,
        show_border: bool,
    },
    Image {
        a: Point,
        b: Point,
        filename: String,
        keep_aspect: bool,
        #[serde(default)]
        embedded: bool,
    },
    Pin {
        position: Point,
        length_nm: i64,
        orientation: u32,
        name: String,
        number: String,
        show_name: bool,
        show_number: bool,
        font: usize,
        electrical: u32,
        #[serde(default)]
        number_font: Option<usize>,
        outer_symbol: u32,
        inner_symbol: u32,
    },
    PowerPort {
        #[serde(default)]
        cross_sheet: bool,
        position: Point,
        orientation: u32,
        style: u32,
        text: String,
        show_name: bool,
        font: usize,
    },
    Port {
        position: Point,
        width_nm: i64,
        height_nm: i64,
        style: u32,
        io_type: u32,
        name: String,
        font: usize,
    },
    Junction {
        position: Point,
        radius_nm: i64,
    },
    NoErc {
        position: Point,
    },
    Ieee {
        position: Point,
        symbol: u32,
        orientation: u32,
        scale_nm: i64,
        mirrored: bool,
    },
    Unsupported {
        a: Point,
        b: Point,
        description: String,
    },
}

impl Schematic {
    pub fn blank() -> Self {
        Self {
            source_modified: false,
            sheet: Sheet {
                options: Default::default(),
                background: 0xf8fcff,
                border: true,
                title_block: true,
                zones: [4, 4],
                zone_origin_lower_right: true,
                system_font: 0,
                margin_nm: 5_080_000,
                grid_nm: 2_540_000,
                snap_nm: 2_540_000,
                grid_visible: true,
                style: "A4".into(),
                parameters: BTreeMap::new(),
            },
            fonts: vec![SchFont {
                name: "Arial".into(),
                height_nm: 2_540_000,
                bold: false,
                italic: false,
                underline: false,
            }],
            components: vec![],
            compile_masks: vec![],
            primitives: vec![],
            parameter_objects: vec![],
            records: Arc::default(),
            images: Arc::default(),
            source: None,
            warnings: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.compile_masks.len() > 500_000
            || self
                .compile_masks
                .iter()
                .any(|m| !m.a.valid() || !m.b.valid())
        {
            return Err("Invalid schematic compile mask".into());
        }
        if self.records.len() > 500_000
            || self.primitives.len() + self.parameter_objects.len() > 500_000
            || self.components.len() > 100_000
        {
            return Err("Schematic object limit exceeded".into());
        }
        if !(1..=1_000_000_000).contains(&self.sheet.grid_nm)
            || !(1..=1_000_000_000).contains(&self.sheet.snap_nm)
        {
            return Err("Invalid schematic grid".into());
        }
        if !(0..=1_000_000_000).contains(&self.sheet.options.snap_distance_nm) {
            return Err("Invalid electrical snap distance".into());
        }
        let mut ids = std::collections::HashSet::new();
        if self.fonts.is_empty()
            || self.sheet.system_font >= self.fonts.len()
            || self.fonts.len() > 128
            || self
                .fonts
                .iter()
                .any(|f| !(1..=254_000_000).contains(&f.height_nm))
        {
            return Err("Invalid schematic font size".into());
        }
        if self.sheet.zones.iter().any(|n| !(1..=64).contains(n))
            || !(0..=100_000_000).contains(&self.sheet.margin_nm)
        {
            return Err("Invalid sheet border or zones".into());
        }
        for c in &self.components {
            if !ids.insert(c.id)
                || !c.position.valid()
                || c.settings.rotation > 3
                || !(1..=1024).contains(&c.settings.part_count)
            {
                return Err("Invalid schematic component".into());
            }
        }
        let component_ids = ids.clone();
        for p in self.primitives.iter().chain(&self.parameter_objects) {
            let font_valid = match &p.kind {
                SchPrimitiveKind::Text { font, .. }
                | SchPrimitiveKind::TextFrame { font, .. }
                | SchPrimitiveKind::PowerPort { font, .. }
                | SchPrimitiveKind::Port { font, .. } => *font < self.fonts.len(),
                SchPrimitiveKind::SheetEntry { font, .. }
                | SchPrimitiveKind::HarnessEntry { font, .. } => *font < self.fonts.len(),
                SchPrimitiveKind::Pin {
                    font, number_font, ..
                } => *font < self.fonts.len() && number_font.is_none_or(|n| n < self.fonts.len()),
                _ => true,
            };
            if !ids.insert(p.id)
                || !font_valid
                || p.width_nm < 0
                || p.width_nm > 1_000_000_000
                || p.component.is_some_and(|id| !component_ids.contains(&id))
            {
                return Err("Invalid schematic primitive or owner".into());
            }
            let valid = match &p.kind {
                SchPrimitiveKind::ParameterSet {
                    position,
                    orientation,
                    style,
                    ..
                } => position.valid() && *orientation < 4 && *style <= 1,
                SchPrimitiveKind::Blanket { points, .. } => {
                    (3..=100_000).contains(&points.len()) && points.iter().all(|p| p.valid())
                }
                SchPrimitiveKind::CompileMask { a, b, .. } => a.valid() && b.valid() && a != b,
                SchPrimitiveKind::HarnessConnector {
                    a,
                    b,
                    side,
                    primary_nm,
                    ..
                } => {
                    a.valid()
                        && b.valid()
                        && a.x < b.x
                        && a.y > b.y
                        && *side <= 3
                        && (0..=if *side <= 1 { a.y - b.y } else { b.x - a.x }).contains(primary_nm)
                }
                SchPrimitiveKind::HarnessEntry {
                    position,
                    side,
                    name,
                    ..
                } => position.valid() && *side <= 3 && !name.trim().is_empty(),
                SchPrimitiveKind::Polyline { points, .. } => {
                    points.len() <= 100_000 && points.iter().all(|p| p.valid())
                }
                SchPrimitiveKind::Rectangle { a, b, radius } => {
                    a.valid() && b.valid() && radius.valid()
                }
                SchPrimitiveKind::SheetSymbol { a, b, .. } => {
                    a.valid() && b.valid() && a.x < b.x && a.y > b.y
                }
                SchPrimitiveKind::SheetEntry {
                    position,
                    side,
                    io_type,
                    ..
                } => position.valid() && *side <= 3 && *io_type <= 3,
                SchPrimitiveKind::Arc {
                    centre,
                    radii,
                    start,
                    end,
                } => {
                    centre.valid()
                        && radii.valid()
                        && radii.x >= 0
                        && radii.y >= 0
                        && start.is_finite()
                        && end.is_finite()
                        && start.abs() <= 360_000.0
                        && end.abs() <= 360_000.0
                }
                SchPrimitiveKind::Bezier { points } => {
                    points.len() >= 4
                        && points.len() <= 100_000
                        && (points.len() - 1).is_multiple_of(3)
                        && points.iter().all(|p| p.valid())
                }
                SchPrimitiveKind::Text { position, .. } | SchPrimitiveKind::NoErc { position } => {
                    position.valid()
                }
                SchPrimitiveKind::Pin {
                    position,
                    length_nm,
                    ..
                } => position.valid() && (0..=1_000_000_000).contains(length_nm),
                SchPrimitiveKind::Port {
                    position,
                    width_nm,
                    height_nm,
                    ..
                } => {
                    position.valid()
                        && (0..=1_000_000_000).contains(width_nm)
                        && (0..=1_000_000_000).contains(height_nm)
                }
                SchPrimitiveKind::PowerPort { position, .. } => position.valid(),
                SchPrimitiveKind::Junction {
                    position,
                    radius_nm,
                } => position.valid() && (0..=1_000_000_000).contains(radius_nm),
                SchPrimitiveKind::Ieee {
                    position, scale_nm, ..
                } => position.valid() && (0..=1_000_000_000).contains(scale_nm),
                SchPrimitiveKind::Unsupported { a, b, .. }
                | SchPrimitiveKind::TextFrame { a, b, .. }
                | SchPrimitiveKind::Image { a, b, .. } => a.valid() && b.valid(),
            };
            if !valid {
                return Err(format!(
                    "Invalid schematic geometry at record {}",
                    p.source_index
                ));
            }
            if let Some(parent) = p.presentation.parent {
                if p.component.is_some()
                    || !self.primitives.iter().any(|p| {
                        p.id == parent
                            && matches!(
                                p.kind,
                                SchPrimitiveKind::SheetSymbol { .. }
                                    | SchPrimitiveKind::ParameterSet { .. }
                                    | SchPrimitiveKind::HarnessConnector { .. }
                            )
                    })
                {
                    return Err("Invalid Sheet Symbol owner".into());
                }
            } else if matches!(
                p.kind,
                SchPrimitiveKind::SheetEntry { .. } | SchPrimitiveKind::HarnessEntry { .. }
            ) {
                return Err("Sheet Entry requires a Sheet Symbol owner".into());
            }
            if let SchPrimitiveKind::HarnessEntry { position, side, .. } = p.kind {
                let parent = self
                    .primitive(p.presentation.parent.ok_or("Missing Harness Connector")?)
                    .ok_or("Missing Harness Connector")?;
                if let SchPrimitiveKind::HarnessConnector { a, b, .. } = parent.kind {
                    let on_edge = match side {
                        0 => position.x == a.x && (b.y..=a.y).contains(&position.y),
                        1 => position.x == b.x && (b.y..=a.y).contains(&position.y),
                        2 => position.y == a.y && (a.x..=b.x).contains(&position.x),
                        _ => position.y == b.y && (a.x..=b.x).contains(&position.x),
                    };
                    if !on_edge {
                        return Err("Harness Entry must be on its connector edge".into());
                    }
                } else {
                    return Err("Harness Entry requires a Harness Connector owner".into());
                }
            }
        }
        Ok(())
    }

    pub fn primitive(&self, id: Uuid) -> Option<&SchPrimitive> {
        self.primitives
            .iter()
            .chain(&self.parameter_objects)
            .find(|p| p.id == id)
    }
}
