//! Independent schematic scene. Native bytes and fields remain available for audit.
use crate::Point;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schematic {
    pub sheet: Sheet,
    pub fonts: Vec<SchFont>,
    pub components: Vec<Component>,
    pub primitives: Vec<SchPrimitive>,
    pub records: Vec<SourceRecord>,
    #[serde(default)]
    pub images: BTreeMap<String, String>,
    pub source: Option<NativeSource>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSource {
    pub format: String,
    pub filename: String,
    /// Unmodified original file, including unknown streams. Never a native writer.
    pub original_base64: String,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SchPrimitiveKind {
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
    Arc {
        centre: Point,
        radii: Point,
        start: f64,
        end: f64,
    },
    Bezier {
        points: [Point; 4],
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
    pub fn validate(&self) -> Result<(), String> {
        if self.records.len() > 500_000
            || self.primitives.len() > 500_000
            || self.components.len() > 100_000
        {
            return Err("Schematic object limit exceeded".into());
        }
        if !(1..=1_000_000_000).contains(&self.sheet.grid_nm)
            || !(1..=1_000_000_000).contains(&self.sheet.snap_nm)
        {
            return Err("Invalid schematic grid".into());
        }
        let mut ids = std::collections::HashSet::new();
        if self.fonts.len() > 128
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
            if !ids.insert(c.id) || !c.position.valid() {
                return Err("Invalid schematic component".into());
            }
        }
        let component_ids = ids.clone();
        for p in &self.primitives {
            if !ids.insert(p.id)
                || p.width_nm < 0
                || p.width_nm > 1_000_000_000
                || p.component.is_some_and(|id| !component_ids.contains(&id))
            {
                return Err("Invalid schematic primitive or owner".into());
            }
            let valid = match &p.kind {
                SchPrimitiveKind::Polyline { points, .. } => {
                    points.len() <= 100_000 && points.iter().all(|p| p.valid())
                }
                SchPrimitiveKind::Rectangle { a, b, radius } => {
                    a.valid() && b.valid() && radius.valid()
                }
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
                SchPrimitiveKind::Bezier { points } => points.iter().all(|p| p.valid()),
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
        }
        Ok(())
    }
}
