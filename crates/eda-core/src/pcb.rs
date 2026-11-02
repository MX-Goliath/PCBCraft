//! PCB scene and transactional edit helpers. Coordinates are integer nanometres.
//! No GUI, foreign EDA engine, or Altium runtime is used here.
use crate::{Document, DocumentKind};
use eda_geom::Point;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;
pub const TOP: u16 = 1;
pub const BOTTOM: u16 = 32;
pub const TOP_OVERLAY: u16 = 33;
pub const BOTTOM_OVERLAY: u16 = 34;
pub const KEEP_OUT: u16 = 56;
pub const MULTI_LAYER: u16 = 74;
pub const MAX_ITEMS: usize = 500_000;
const MAX_COORD: i64 = 100_000_000_000;
#[path = "pcb_snap.rs"]
mod snapping;
pub use snapping::{SnapLayerMode, SnapOptions};
#[path = "pcb_stack.rs"]
mod stack;
pub use stack::{DrillShape, PadGeometry, PadStack, StackMode, StackSurface, cutout_surface};
#[path = "pcb_mask.rs"]
mod mask;
pub use mask::dimension as dimension_nm;
pub use mask::{MaskKind, MaskMode, MaskSettings, MaskSide, ResolvedMask};
#[cfg(test)]
#[path = "pcb_stack_tests.rs"]
mod stack_tests;

pub type Fields = BTreeMap<String, String>;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PcbLayer {
    pub id: u16,
    pub name: String,
    pub copper: bool,
    /// Layer order from the source stack, not the numeric layer identifier.
    pub order: u16,
}
pub fn layer_name(id: u16) -> String {
    match id {
        1 => "Top Layer".into(),
        2..=31 => format!("Mid-Layer {}", id - 1),
        32 => "Bottom Layer".into(),
        33 => "Top Overlay".into(),
        34 => "Bottom Overlay".into(),
        35 => "Top Paste".into(),
        36 => "Bottom Paste".into(),
        37 => "Top Solder".into(),
        38 => "Bottom Solder".into(),
        39..=54 => format!("Internal Plane {}", id - 38),
        55 => "Drill Guide".into(),
        56 => "Keep-Out Layer".into(),
        57..=72 => format!("Mechanical {}", id - 56),
        73 => "Drill Drawing".into(),
        74 => "Multi-Layer".into(),
        75 => "Connect Layer".into(),
        76 => "Background Layer".into(),
        77 => "DRC Error Layer".into(),
        78 => "Highlight Layer".into(),
        79 => "Grid Color 1".into(),
        80 => "Grid Color 10".into(),
        81 => "Pad Hole Layer".into(),
        82 => "Via Hole Layer".into(),
        83..=1090 => format!("Mechanical {}", id - 66),
        _ => format!("Layer {id}"),
    }
}
/// Stable model IDs retain the legacy gap while covering modern mechanical layers.
pub fn mechanical_layer(number: u16) -> Option<u16> {
    match number {
        1..=16 => Some(56 + number),
        17..=1024 => Some(66 + number),
        _ => None,
    }
}
/// Modern Altium layer identities override the one-byte legacy fallback.
pub fn native_layer(id: u32) -> Option<u16> {
    match id {
        0x01000001 => Some(TOP),
        0x0100ffff => Some(BOTTOM),
        0x01020001..=0x01020400 => mechanical_layer((id & 0xffff) as u16),
        0x01000002..=0x0100001f => Some((id & 0xffff) as u16),
        0x01030006..=0x0103000b => Some(33 + (id & 0xffff) as u16 - 6),
        0x0103000c => Some(55),
        0x0103000d => Some(KEEP_OUT),
        0x0103000e => Some(73),
        0x0103000f => Some(MULTI_LAYER),
        _ => None,
    }
}
pub fn copper_layer(id: u16) -> bool {
    (1..=32).contains(&id) || (39..=54).contains(&id)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Net {
    pub index: u32,
    pub name: String,
    #[serde(default)]
    pub properties: Fields,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub id: Uuid,
    pub index: u32,
    pub designator: String,
    pub comment: String,
    pub footprint: String,
    /// Base-variant component parameters; None means legacy/uninterpreted data.
    #[serde(default)]
    pub parameters: Option<Fields>,
    pub position: Point,
    pub rotation: f64,
    pub layer: u16,
    pub locked: bool,
    #[serde(default)]
    pub properties: Fields,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContourVertex {
    pub position: Point,
    /// An arc begins at this vertex; end is the next vertex.
    pub arc: Option<ContourArc>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContourArc {
    pub centre: Point,
    pub radius: i64,
    pub start_angle: f64,
    pub end_angle: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub stream: String,
    pub index: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceArchive {
    pub filename: String,
    pub format: String,
    /// Original complete file. Saving PCBCraft JSON never discards unknown streams.
    pub bytes_base64: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Shape {
    Track {
        start: Point,
        end: Point,
        width: i64,
    },
    Arc {
        centre: Point,
        radius: i64,
        start_angle: f64,
        end_angle: f64,
        width: i64,
    },
    Pad {
        position: Point,
        size: Point,
        rotation: f64,
        shape: PadShape,
        hole: i64,
        plated: bool,
        name: String,
        #[serde(default)]
        bottom_size: Option<Point>,
        #[serde(default)]
        middle_size: Option<Point>,
    },
    Via {
        position: Point,
        diameter: i64,
        hole: i64,
        from_layer: u16,
        to_layer: u16,
    },
    Text {
        position: Point,
        text: String,
        height: i64,
        stroke: i64,
        rotation: f64,
        mirrored: bool,
        #[serde(default)]
        font: String,
    },
    Fill {
        start: Point,
        end: Point,
        rotation: f64,
    },
    Region {
        contour: Vec<Point>,
        #[serde(default)]
        holes: Vec<Vec<Point>>,
        #[serde(default)]
        cutout: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadShape {
    Round,
    Rectangle,
    Octagonal,
    RoundedRectangle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub layer: u16,
    pub net: Option<u32>,
    pub component: Option<u32>,
    pub locked: bool,
    #[serde(default)]
    pub keepout: bool,
    #[serde(default)]
    pub polygon: Option<u32>,
    pub shape: Shape,
    #[serde(default)]
    pub source: Option<SourceRecord>,
    #[serde(default)]
    pub properties: Fields,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad_stack: Option<Box<PadStack>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub masks: Option<Box<MaskSettings>>,
}
impl Item {
    /// A native region record may describe only a point or a line, with repeated
    /// vertices. Retain the record for native roundtrips, but it has no material.
    /// Test collinearity, not signed area: a self-touching ring may have area
    /// cancellation while still bounding real surfaces.
    pub fn is_empty_region(&self) -> bool {
        let Shape::Region { contour, .. } = &self.shape else {
            return false;
        };
        let Some(first) = contour.first() else {
            return true;
        };
        let Some(second) = contour.iter().find(|p| *p != first) else {
            return true;
        };
        if !first.valid() || !second.valid() {
            return false;
        }
        let (dx, dy) = (
            second.x as i128 - first.x as i128,
            second.y as i128 - first.y as i128,
        );
        contour.iter().all(|p| {
            p.valid()
                && dx * (p.y as i128 - first.y as i128) == dy * (p.x as i128 - first.x as i128)
        })
    }
    /// Altium percentages refer to half of the shortest pad side.
    pub fn pad_corner_percent(&self, layer: u16) -> u8 {
        self.properties
            .get(&format!("PCBCRAFT_PAD_RADIUS_{}", layer.clamp(1, 32)))
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(25)
            .min(100)
    }
    /// A mechanical through-hole with no copper land on any modelled layer.
    pub fn is_drill_only(&self) -> bool {
        self.layer == MULTI_LAYER
            && matches!(&self.shape,Shape::Pad{size,hole,plated:false,bottom_size,middle_size,..}
            if *size==Point::default()&&*hole>0&&[bottom_size,middle_size].iter().all(|s|s.is_none_or(|p|p==Point::default())))
            && self
                .pad_stack
                .as_ref()
                .is_none_or(|s| s.layers.values().all(|g| g.size == Point::default()))
    }

    pub fn new(layer: u16, net: Option<u32>, shape: Shape) -> Self {
        Self {
            id: Uuid::new_v4(),
            layer,
            net,
            component: None,
            locked: false,
            keepout: false,
            polygon: None,
            shape,
            source: None,
            properties: Fields::new(),
            pad_stack: None,
            masks: None,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self.shape {
            Shape::Track { .. } => "Track",
            Shape::Arc { .. } => "Arc",
            Shape::Pad { .. } => "Pad",
            Shape::Via { .. } => "Via",
            Shape::Text { .. } => "Text",
            Shape::Fill { .. } => "Fill",
            Shape::Region { .. } => "Region",
        }
    }
    pub fn anchor(&self) -> Point {
        match &self.shape {
            Shape::Track { start, .. } | Shape::Fill { start, .. } => *start,
            Shape::Arc { centre, .. } => *centre,
            Shape::Pad { position, .. }
            | Shape::Via { position, .. }
            | Shape::Text { position, .. } => *position,
            Shape::Region { contour, .. } => contour.first().copied().unwrap_or_default(),
        }
    }
    /// Move the primitive as a whole, preserving its dimensions and topology.
    /// A draft may use this before committing a validated Replace command.
    pub fn with_anchor(&self, anchor: Point) -> Result<Self, String> {
        if !valid_point(anchor) {
            return Err("Location is outside document coordinate limits".into());
        }
        let old = self.anchor();
        let mut next = self.clone();
        transform_item(
            &mut next,
            old,
            Point {
                x: anchor.x - old.x,
                y: anchor.y - old.y,
            },
            0.0,
            false,
        )?;
        Ok(next)
    }
    /// Conservative world bounds, used for view fitting and spatial rejection only.
    pub fn bounds(&self) -> Bounds {
        if let Some(stack) = &self.pad_stack {
            let mut bounds = Bounds::points(&self.drill_outline());
            for layer in stack.layers.keys().copied().chain([TOP, BOTTOM]) {
                bounds.union(Bounds::points(&self.pad_outline(layer)));
            }
            if !bounds.empty {
                return bounds;
            }
        }
        match &self.shape {
            Shape::Track { start, end, width } => {
                Bounds::points(&[*start, *end]).expanded(*width / 2)
            }
            Shape::Arc {
                centre,
                radius,
                width,
                ..
            } => Bounds::points(&[*centre]).expanded(radius + width / 2),
            Shape::Via {
                position, diameter, ..
            } => Bounds::points(&[*position]).expanded(diameter / 2),
            Shape::Pad {
                position,
                size,
                rotation,
                hole,
                ..
            } => {
                let mut bounds = Bounds::points(&rectangle(*position, *size, *rotation));
                if *hole > 0 {
                    bounds.union(Bounds::points(&[*position]).expanded((*hole + 1) / 2));
                }
                bounds
            }
            Shape::Fill {
                start,
                end,
                rotation,
            } => {
                let centre = midpoint(*start, *end);
                Bounds::points(&rectangle(
                    centre,
                    Point {
                        x: (end.x - start.x).abs(),
                        y: (end.y - start.y).abs(),
                    },
                    *rotation,
                ))
            }
            Shape::Text {
                position,
                text,
                height,
                rotation,
                mirrored,
                ..
            } => {
                let cache_valid = self.properties.get("PCBCRAFT_TEXT_CACHE_VALUE") == Some(text)
                    && self.properties.get("PCBCRAFT_TEXT_CACHE_SOURCE_HEIGHT")
                        == Some(&height.to_string());
                let metric = |key: &str, fallback: i64| {
                    if cache_valid {
                        self.properties
                            .get(key)
                            .and_then(|v| v.parse::<i64>().ok())
                            .filter(|v| *v > 0 && *v <= MAX_COORD)
                            .unwrap_or(fallback)
                    } else {
                        fallback
                    }
                };
                let w = metric(
                    "PCBCRAFT_TEXT_CACHE_WIDTH",
                    text.chars().count() as i64 * *height * 2,
                );
                let h = metric("PCBCRAFT_TEXT_CACHE_HEIGHT", *height * 2);
                let (s, c) = rotation.to_radians().sin_cos();
                let corners = [(0, 0), (w, 0), (w, h), (0, h)].map(|(x, y)| {
                    let x = (if *mirrored { -x } else { x }) as f64;
                    let y = y as f64;
                    Point {
                        x: position.x + (x * c - y * s).round() as i64,
                        y: position.y + (x * s + y * c).round() as i64,
                    }
                });
                Bounds::points(&corners).expanded(*height / 2)
            }
            Shape::Region { contour, .. } => Bounds::points(contour),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
    /// Shared, read-only meshes in model-local millimetres; native placement is
    /// retained on body records and transformed only when building a scene.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, std::sync::Arc<crate::pcb_3d::Model>>,
    #[serde(default)]
    pub layer_stack: Option<crate::layer_stack::Stack>,
    pub outline: Vec<ContourVertex>,
    pub layers: Vec<PcbLayer>,
    pub nets: Vec<Net>,
    pub components: Vec<Component>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub properties: Fields,
    #[serde(default)]
    pub rules: Vec<Fields>,
    #[serde(default)]
    pub classes: Vec<Fields>,
    #[serde(default)]
    pub differential_pairs: Vec<Fields>,
    #[serde(default)]
    pub polygons: Vec<Fields>,
    #[serde(default)]
    pub polygon_pours: Vec<crate::pcb_polygon::Polygon>,
    /// 0 means an old JSON document that still needs metadata migration.
    #[serde(default)]
    pub polygon_model_version: u8,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Framed native region records with no outer vertices or holes. They have
    /// no editable/material geometry; their bytes remain in the source archive.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub empty_region_records: Vec<SourceRecord>,
    #[serde(default)]
    pub pours_stale: bool,
    #[serde(default)]
    pub source: Option<std::sync::Arc<SourceArchive>>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
            models: BTreeMap::new(),
            layer_stack: None,
            outline: vec![],
            layers: [TOP, BOTTOM, TOP_OVERLAY, BOTTOM_OVERLAY, MULTI_LAYER]
                .into_iter()
                .enumerate()
                .map(|(order, id)| PcbLayer {
                    id,
                    name: layer_name(id),
                    copper: copper_layer(id),
                    order: order as u16,
                })
                .collect(),
            nets: vec![],
            components: vec![],
            items: vec![],
            properties: Fields::new(),
            rules: vec![],
            classes: vec![],
            differential_pairs: vec![],
            polygons: vec![],
            polygon_pours: vec![],
            polygon_model_version: 1,
            warnings: vec![],
            empty_region_records: vec![],
            pours_stale: false,
            source: None,
        }
    }
}
impl Board {
    pub fn from_legacy(doc: &Document) -> Self {
        let mut board = Self {
            outline: [
                Point { x: 0, y: 0 },
                Point {
                    x: doc.size.x,
                    y: 0,
                },
                doc.size,
                Point {
                    x: 0,
                    y: doc.size.y,
                },
            ]
            .into_iter()
            .map(|position| ContourVertex {
                position,
                arc: None,
            })
            .collect(),
            ..Self::default()
        };
        let layer = |l| match l {
            crate::Layer::Top => TOP,
            crate::Layer::Bottom => BOTTOM,
            crate::Layer::Overlay => TOP_OVERLAY,
            crate::Layer::Outline => 57,
        };
        for p in &doc.pads {
            let mut item = Item::new(
                layer(p.layer),
                None,
                Shape::Pad {
                    position: p.position,
                    size: p.size,
                    rotation: 0.0,
                    shape: PadShape::Rectangle,
                    hole: 0,
                    plated: true,
                    name: p.name.clone(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            item.id = p.id;
            board.items.push(item);
        }
        for t in &doc.tracks {
            let mut item = Item::new(
                layer(t.layer),
                None,
                Shape::Track {
                    start: t.start,
                    end: t.end,
                    width: t.width,
                },
            );
            item.id = t.id;
            board.items.push(item);
        }
        board
    }
    pub fn ensure_document(doc: &mut Document) {
        if doc.kind == DocumentKind::Board && doc.pcb.is_none() {
            doc.pcb = Some(Self::from_legacy(doc));
            // A single authoritative scene avoids diverging copies after an edit.

            doc.pads.clear();
            doc.tracks.clear();
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(stack) = &self.layer_stack {
            stack.validate_for_board(self)?;
        }
        if self.items.len() > MAX_ITEMS
            || self.empty_region_records.len() > MAX_ITEMS.saturating_sub(self.items.len())
            || self.components.len() > MAX_ITEMS
            || self.nets.len() > MAX_ITEMS
        {
            return Err("PCB entity limit exceeded".into());
        }
        let mut empty_records = BTreeSet::new();
        let item_records: BTreeSet<_> = if self.empty_region_records.is_empty() {
            BTreeSet::new()
        } else {
            self.items
                .iter()
                .filter_map(|item| item.source.as_ref().map(|s| (&s.stream, s.index)))
                .collect()
        };
        for record in &self.empty_region_records {
            if self.source.is_none()
                || !matches!(
                    record.stream.as_str(),
                    "Regions6"
                        | "ShapeBasedRegions6"
                        | "ComponentBodies6"
                        | "ShapeBasedComponentBodies6"
                )
                || record.index as usize >= MAX_ITEMS
                || !empty_records.insert((&record.stream, record.index))
                || item_records.contains(&(&record.stream, record.index))
            {
                return Err("Invalid archived empty region reference".into());
            }
        }
        if self.outline.len() > MAX_ITEMS || self.layers.len() > 4096 {
            return Err("PCB outline/layer limit exceeded".into());
        }
        let mut ids = HashSet::new();
        let mut nets = HashSet::new();
        let mut comps = HashSet::new();
        for n in &self.nets {
            if !nets.insert(n.index) {
                return Err("Duplicate net index".into());
            }
        }
        for c in &self.components {
            if !ids.insert(c.id)
                || !comps.insert(c.index)
                || !valid_point(c.position)
                || !c.rotation.is_finite()
            {
                return Err("Invalid component".into());
            }
        }
        if self.models.len() > 10_000 || self.models.values().any(|m| !m.valid()) {
            return Err("Invalid PCB 3D model geometry".into());
        }
        for v in &self.outline {
            if !valid_point(v.position) {
                return Err("Invalid board contour".into());
            }
            if let Some(a) = &v.arc
                && (!valid_point(a.centre)
                    || !positive(a.radius)
                    || !a.start_angle.is_finite()
                    || !a.end_angle.is_finite())
            {
                return Err("Invalid outline arc".into());
            }
        }
        let mut polygon_indices = HashSet::new();
        for p in &self.polygon_pours {
            if !ids.insert(p.id) || !polygon_indices.insert(p.index) {
                return Err("Duplicate polygon identifier/index".into());
            }
            if p.net.is_some_and(|n| !nets.contains(&n)) {
                return Err("Polygon refers to missing net".into());
            }
            p.validate()?;
        }
        for i in &self.items {
            if !ids.insert(i.id) {
                return Err("Duplicate PCB item identifier".into());
            }
            if i.net.is_some_and(|n| !nets.contains(&n)) {
                return Err(format!("{} refers to missing net", i.kind()));
            }
            if i.component.is_some_and(|c| !comps.contains(&c)) {
                return Err(format!("{} refers to missing component", i.kind()));
            }
            i.validate_geometry().map_err(|error| {
                format!(
                    "{error}; item={}, layer={}, source={:?}",
                    i.kind(),
                    i.layer,
                    i.source
                )
            })?;
        }
        Ok(())
    }
    pub fn net_name(&self, net: Option<u32>) -> &str {
        net.and_then(|id| self.nets.iter().find(|n| n.index == id))
            .map_or("No Net", |n| n.name.as_str())
    }
    /// Native pour fragments can omit NET and inherit it from their polygon.
    /// Keep the archived primitive unchanged; explicit primitive nets take priority.
    pub fn item_net(&self, item: &Item) -> Option<u32> {
        item.net.or_else(|| {
            item.polygon.and_then(|index| {
                if let Some(polygon) = self.polygon_pours.iter().find(|p| p.index == index) {
                    return polygon.net;
                }
                // A native outline can be read-only (for example a retraced arc),
                // while its saved copper fragments and net identity remain valid.
                // An editable polygon, including an explicitly unassigned net,
                // always takes precedence over this archived metadata.
                self.polygons
                    .iter()
                    .enumerate()
                    .find(|(n, p)| {
                        p.get("ID")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(*n as u32)
                            == index
                    })
                    .and_then(|(_, p)| p.get("NET"))
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|&net| net != 65535)
            })
        })
    }
    /// Native component strings can exist in Texts6 while their display is disabled.
    pub fn text_visible(&self, item: &Item) -> bool {
        if !matches!(item.shape, Shape::Text { .. }) {
            return true;
        }
        let Some(component) = item
            .component
            .and_then(|index| self.components.iter().find(|c| c.index == index))
        else {
            return true;
        };
        ![("DESIGNATOR", "NAMEON"), ("COMMENT", "COMMENTON")]
            .into_iter()
            .any(|(role, setting)| {
                item.properties.get(role).is_some_and(|v| v == "TRUE")
                    && component
                        .properties
                        .get(setting)
                        .is_some_and(|v| v.eq_ignore_ascii_case("FALSE"))
            })
    }
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::empty();
        for p in self.outline_points(48) {
            b.include(p);
        }
        for item in &self.items {
            if !self.text_visible(item) || item.is_empty_region() {
                continue;
            }
            b.union(item.bounds());
        }
        for p in &self.polygon_pours {
            b.union(Bounds::points(&p.points()));
        }
        if b.empty {
            Bounds {
                min: Point { x: 0, y: 0 },
                max: Point {
                    x: 80_000_000,
                    y: 50_000_000,
                },
                empty: false,
            }
        } else {
            b
        }
    }
    pub fn outline_points(&self, steps: usize) -> Vec<Point> {
        let mut p = Vec::new();
        for v in &self.outline {
            if let Some(a) = &v.arc {
                p.extend(contour_arc_points(v.position, a, steps));
            } else {
                p.push(v.position);
            }
        }
        p
    }
    pub fn item(&self, id: Uuid) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
    pub fn item_mut(&mut self, id: Uuid) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id == id)
    }
    pub fn resolve_selection(&self, selection: &[Uuid]) -> BTreeSet<Uuid> {
        if selection.is_empty() {
            return BTreeSet::new();
        }
        let selection: HashSet<_> = selection.iter().copied().collect();
        let component_indices: HashSet<_> = self
            .components
            .iter()
            .filter(|c| selection.contains(&c.id))
            .map(|c| c.index)
            .collect();
        let polygons: HashSet<_> = self
            .polygon_pours
            .iter()
            .filter(|p| selection.contains(&p.id))
            .map(|p| p.index)
            .collect();
        self.items
            .iter()
            .filter(|i| {
                selection.contains(&i.id)
                    || i.component.is_some_and(|c| component_indices.contains(&c))
                    || i.polygon.is_some_and(|p| polygons.contains(&p))
            })
            .map(|i| i.id)
            .collect()
    }
    pub fn selected_bounds(&self, selection: &[Uuid]) -> Option<Bounds> {
        let ids = self.resolve_selection(selection);
        let mut b = Bounds::empty();
        for i in self.items.iter().filter(|i| ids.contains(&i.id)) {
            b.union(i.bounds());
        }
        for p in self
            .polygon_pours
            .iter()
            .filter(|p| selection.contains(&p.id))
        {
            b.union(Bounds::points(&p.points()));
        }
        for c in self.components.iter().filter(|c| selection.contains(&c.id)) {
            b.include(c.position);
        }
        (!b.empty).then_some(b)
    }
    pub fn edited(&self, action: Edit) -> Result<Self, String> {
        let mut next = self.clone();
        let mut copper_changed = true;
        match action {
            Edit::Insert(items) => {
                if next.items.len().saturating_add(items.len()) > MAX_ITEMS {
                    return Err("PCB entity limit exceeded".into());
                }
                next.items.extend(items);
            }
            Edit::Replace(item) => {
                copper_changed = next.replace_item_unvalidated(item)?;
            }
            Edit::Delete(selection) => {
                let ids = self.resolve_selection(&selection);
                if self.selection_locked(&selection, &ids) {
                    return Err("Selection contains locked objects".into());
                }
                next.items.retain(|i| !ids.contains(&i.id));
                next.components.retain(|c| !selection.contains(&c.id));
                next.polygon_pours.retain(|p| !selection.contains(&p.id));
            }
            Edit::Transform {
                selection,
                pivot,
                translation,
                angle,
            } => {
                if !angle.is_finite() {
                    return Err("Invalid rotation".into());
                }
                let ids = self.resolve_selection(&selection);
                if self.selection_locked(&selection, &ids) {
                    return Err("Selection contains locked objects".into());
                }
                for item in next.items.iter_mut().filter(|i| ids.contains(&i.id)) {
                    let bottom = item
                        .component
                        .and_then(|i| self.components.iter().find(|c| c.index == i))
                        .is_some_and(|c| c.layer == BOTTOM);
                    transform_item(item, pivot, translation, angle, bottom)?;
                }
                for poly in next
                    .polygon_pours
                    .iter_mut()
                    .filter(|p| selection.contains(&p.id))
                {
                    for v in &mut poly.contour {
                        v.position = transform_point(v.position, pivot, translation, angle)?;
                        if let Some(a) = &mut v.arc {
                            a.centre = transform_point(a.centre, pivot, translation, angle)?;
                            a.start_angle += angle;
                            a.end_angle += angle;
                        }
                    }
                    for hole in &mut poly.holes {
                        for p in hole {
                            *p = transform_point(*p, pivot, translation, angle)?;
                        }
                    }
                }
                for c in next
                    .components
                    .iter_mut()
                    .filter(|c| selection.contains(&c.id))
                {
                    c.position = transform_point(c.position, pivot, translation, angle)?;
                    c.rotation = (c.rotation + angle).rem_euclid(360.0);
                }
            }
        }
        next.pours_stale |= copper_changed && !next.polygon_pours.is_empty();
        next.validate()?;
        Ok(next)
    }
    /// Internal transaction helper. Callers must validate the whole result
    /// before publishing it; all replacement/locking restrictions remain here.
    pub(crate) fn replace_item_unvalidated(&mut self, mut item: Item) -> Result<bool, String> {
        if let Some(old) = self.item(item.id) {
            item.reconcile_legacy_stack(old);
        }
        if let Some(old) = self.item(item.id)
            && old
                .polygon
                .is_some_and(|n| self.polygon_pours.iter().any(|p| p.index == n))
        {
            return Err("Edit the polygon definition, not its generated copper".into());
        }
        if self
            .item(item.id)
            .and_then(|old| old.component)
            .is_some_and(|owner| self.components.iter().any(|c| c.index == owner && c.locked))
        {
            return Err("Cannot edit a child of a locked component".into());
        }
        let old = self
            .item(item.id)
            .ok_or("Selected object no longer exists")?;
        // Mask overrides do not invalidate copper pours. Preserve conservative
        // invalidation for native primitive properties interpreted by pouring.
        let copper_changed = !old.same_copper_geometry(&item, self)
            || old.layer != item.layer
            || old.net != item.net
            || old.properties != item.properties;
        let old = self
            .item_mut(item.id)
            .ok_or("Selected object no longer exists")?;
        if old.locked && old != &item {
            let mut unlocked = old.clone();
            unlocked.locked = false;
            if item != unlocked {
                return Err("Apply Unlock on its own before changing a locked object".into());
            }
        }
        *old = item;
        Ok(copper_changed)
    }
    fn selection_locked(&self, selection: &[Uuid], ids: &BTreeSet<Uuid>) -> bool {
        // Primitive locks keep footprint children fixed relative to their
        // component. Moving/deleting the whole unlocked footprint is allowed.
        let components: HashSet<_> = self
            .components
            .iter()
            .filter(|c| selection.contains(&c.id))
            .map(|c| c.index)
            .collect();
        self.items.iter().any(|i| {
            ids.contains(&i.id)
                && (i.locked && !i.component.is_some_and(|owner| components.contains(&owner))
                    || i.component.is_some_and(|owner| {
                        self.components.iter().any(|c| c.index == owner && c.locked)
                    }))
        }) || self
            .components
            .iter()
            .any(|c| selection.contains(&c.id) && c.locked)
            || self
                .polygon_pours
                .iter()
                .any(|p| selection.contains(&p.id) && p.locked)
    }
    pub fn snap(
        &self,
        raw: Point,
        grid: i64,
        tolerance: i64,
        layer: u16,
        visible: &BTreeSet<u16>,
    ) -> Point {
        let mut result = Point {
            x: snap_coord(raw.x, grid),
            y: snap_coord(raw.y, grid),
        };
        let mut best = (tolerance.max(0) as f64).powi(2);
        for i in &self.items {
            if !crate::pcb_polygon::on_layer(self, i, layer) || !visible.contains(&i.layer) {
                continue;
            }
            let mut anchors = vec![i.anchor()];
            if let Shape::Track { end, .. } = i.shape {
                anchors.push(end);
            }
            for p in anchors {
                let d = distance_sq(p, raw);
                if d <= best {
                    best = d;
                    result = p;
                }
            }
        }
        result
    }
}
impl Item {
    pub(crate) fn validate_geometry(&self) -> Result<(), String> {
        self.validate_stack()?;
        self.validate_masks()?;
        let ok = match &self.shape {
            Shape::Track { start, end, width } => {
                valid_point(*start) && valid_point(*end) && positive(*width)
            }
            Shape::Arc {
                centre,
                radius,
                start_angle,
                end_angle,
                width,
            } => {
                valid_point(*centre)
                    && positive(*radius)
                    && (0..=MAX_COORD).contains(width)
                    && start_angle.is_finite()
                    && end_angle.is_finite()
            }
            Shape::Pad {
                position,
                size,
                rotation,
                hole,
                bottom_size,
                middle_size,
                ..
            } => {
                valid_point(*position)
                    && ((positive(size.x) && positive(size.y)) || self.is_drill_only())
                    && rotation.is_finite()
                    && *hole >= 0
                    && *hole <= MAX_COORD
                    && [bottom_size, middle_size].into_iter().all(|s| {
                        s.is_none_or(|p| {
                            (positive(p.x) && positive(p.y))
                                || (self.is_drill_only() && p == Point::default())
                        })
                    })
            }
            Shape::Via {
                position,
                diameter,
                hole,
                from_layer,
                to_layer,
            } => {
                valid_point(*position)
                    && positive(*diameter)
                    && positive(*hole)
                    && hole < diameter
                    && from_layer != to_layer
            }
            Shape::Text {
                position,
                height,
                stroke,
                rotation,
                text,
                ..
            } => {
                valid_point(*position)
                    && positive(*height)
                    && *stroke >= 0
                    && *stroke <= MAX_COORD
                    && rotation.is_finite()
                    && text.len() <= 1_000_000
            }
            Shape::Fill {
                start,
                end,
                rotation,
            } => valid_point(*start) && valid_point(*end) && rotation.is_finite(),
            Shape::Region { contour, holes, .. } => {
                contour.len() >= 3
                    && contour.len() <= MAX_ITEMS
                    && contour.iter().all(|p| valid_point(*p))
                    && holes.len() <= 10_000
                    && holes.iter().all(|h| {
                        h.len() >= 3 && h.len() <= MAX_ITEMS && h.iter().all(|p| valid_point(*p))
                    })
            }
        };
        if !ok {
            if let Shape::Region { contour, holes, .. } = &self.shape {
                return Err(format!(
                    "Invalid PCB Region geometry: outer vertices={}, holes={}, invalid holes={:?}, source={:?}",
                    contour.len(),
                    holes.len(),
                    holes
                        .iter()
                        .enumerate()
                        .filter(|(_, h)| h.len() < 3
                            || h.len() > MAX_ITEMS
                            || h.iter().any(|p| !valid_point(*p)))
                        .take(8)
                        .map(|(n, h)| (n, h.len(), h.iter().take(3).copied().collect::<Vec<_>>()))
                        .collect::<Vec<_>>(),
                    self.source
                ));
            }
            return Err(format!("Invalid PCB {} geometry", self.kind()));
        }
        Ok(())
    }
}

/// A validated transform transaction with reusable preview storage. Immutable
/// snapshots can be shared with render workers; copy-on-write protects them.
#[derive(Clone)]
pub struct MovePreview {
    board: std::sync::Arc<Board>,
    items: Vec<(usize, Item, bool)>,
    components: Vec<(usize, Component)>,
}
impl MovePreview {
    pub fn new(board: &Board, selection: &[Uuid]) -> Result<Self, String> {
        board.validate()?;
        if board
            .polygon_pours
            .iter()
            .any(|p| selection.contains(&p.id))
        {
            return Err("Polygon transforms require a full board transaction".into());
        }
        let ids = board.resolve_selection(selection);
        if board.selection_locked(selection, &ids) {
            return Err("Selection contains locked objects".into());
        }
        let items = board
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| ids.contains(&i.id))
            .map(|(index, i)| {
                (
                    index,
                    i.clone(),
                    i.component
                        .and_then(|index| board.components.iter().find(|c| c.index == index))
                        .is_some_and(|c| c.layer == BOTTOM),
                )
            })
            .collect();
        let components = board
            .components
            .iter()
            .enumerate()
            .filter(|(_, c)| selection.contains(&c.id))
            .map(|(index, c)| (index, c.clone()))
            .collect();
        Ok(Self {
            board: std::sync::Arc::new(board.clone()),
            items,
            components,
        })
    }
    pub fn update(
        &mut self,
        pivot: Point,
        translation: Point,
        angle: f64,
    ) -> Result<std::sync::Arc<Board>, String> {
        if !angle.is_finite() {
            return Err("Invalid rotation".into());
        }
        // Validate every changed entity before modifying even the private draft.
        // Identity, references, models and all other entities were checked at
        // construction and cannot be changed through this API.
        let items = self
            .items
            .iter()
            .map(|(index, original, bottom)| {
                let mut item = original.clone();
                transform_item(&mut item, pivot, translation, angle, *bottom)?;
                item.validate_geometry()?;
                Ok((*index, item))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let components = self
            .components
            .iter()
            .map(|(index, original)| {
                let mut component = original.clone();
                component.position =
                    transform_point(component.position, pivot, translation, angle)?;
                component.rotation = (component.rotation + angle).rem_euclid(360.);
                if !component.rotation.is_finite() {
                    return Err("Invalid component rotation".into());
                }
                Ok((*index, component))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let board = std::sync::Arc::make_mut(&mut self.board);
        for (index, item) in items {
            board.items[index] = item;
        }
        for (index, component) in components {
            board.components[index] = component;
        }
        board.pours_stale |= !board.polygon_pours.is_empty();
        Ok(self.board.clone())
    }
}

#[derive(Clone, Debug)]
pub enum Edit {
    Insert(Vec<Item>),
    Replace(Item),
    Delete(Vec<Uuid>),
    Transform {
        selection: Vec<Uuid>,
        pivot: Point,
        translation: Point,
        angle: f64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
    pub empty: bool,
}
impl Bounds {
    pub fn empty() -> Self {
        Self {
            min: Point::default(),
            max: Point::default(),
            empty: true,
        }
    }
    pub fn points(p: &[Point]) -> Self {
        let mut b = Self::empty();
        for p in p {
            b.include(*p);
        }
        b
    }
    pub fn include(&mut self, p: Point) {
        if self.empty {
            self.min = p;
            self.max = p;
            self.empty = false;
        } else {
            self.min.x = self.min.x.min(p.x);
            self.min.y = self.min.y.min(p.y);
            self.max.x = self.max.x.max(p.x);
            self.max.y = self.max.y.max(p.y);
        }
    }
    pub fn union(&mut self, b: Self) {
        if !b.empty {
            self.include(b.min);
            self.include(b.max);
        }
    }
    pub fn expanded(self, d: i64) -> Self {
        Self {
            min: Point {
                x: self.min.x.saturating_sub(d),
                y: self.min.y.saturating_sub(d),
            },
            max: Point {
                x: self.max.x.saturating_add(d),
                y: self.max.y.saturating_add(d),
            },
            empty: self.empty,
        }
    }
    pub fn contains(&self, p: Point) -> bool {
        !self.empty
            && p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
    }
    pub fn intersects(&self, b: Self) -> bool {
        !self.empty
            && !b.empty
            && self.min.x <= b.max.x
            && self.max.x >= b.min.x
            && self.min.y <= b.max.y
            && self.max.y >= b.min.y
    }
    pub fn centre(&self) -> Point {
        midpoint(self.min, self.max)
    }
}
fn positive(n: i64) -> bool {
    n > 0 && n <= MAX_COORD
}
fn valid_point(p: Point) -> bool {
    p.x.unsigned_abs() <= MAX_COORD as u64 && p.y.unsigned_abs() <= MAX_COORD as u64
}
pub fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: a.x + (b.x - a.x) / 2,
        y: a.y + (b.y - a.y) / 2,
    }
}
pub fn snap_coord(v: i64, step: i64) -> i64 {
    if step <= 0 {
        v
    } else {
        ((v as f64 / step as f64).round() * step as f64) as i64
    }
}
pub fn distance_sq(a: Point, b: Point) -> f64 {
    (a.x as f64 - b.x as f64).powi(2) + (a.y as f64 - b.y as f64).powi(2)
}
pub fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let den = dx * dx + dy * dy;
    let t = if den == 0.0 {
        0.0
    } else {
        (((p.x - a.x) as f64 * dx + (p.y - a.y) as f64 * dy) / den).clamp(0.0, 1.0)
    };
    ((p.x as f64 - a.x as f64 - t * dx).powi(2) + (p.y as f64 - a.y as f64 - t * dy).powi(2)).sqrt()
}
pub fn point_in_polygon(p: Point, points: &[Point]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = points.len() - 1;
    for (i, a) in points.iter().enumerate() {
        let b = points[j];
        if (a.y > p.y) != (b.y > p.y)
            && (p.x as f64)
                < (b.x - a.x) as f64 * (p.y - a.y) as f64 / (b.y - a.y) as f64 + a.x as f64
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}
pub fn rectangle(centre: Point, size: Point, angle: f64) -> Vec<Point> {
    let (s, c) = angle.to_radians().sin_cos();
    [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
        .into_iter()
        .map(|(x, y)| Point {
            x: centre.x + (x * size.x as f64 * c - y * size.y as f64 * s).round() as i64,
            y: centre.y + (x * size.x as f64 * s + y * size.y as f64 * c).round() as i64,
        })
        .collect()
}
pub fn arc_points(centre: Point, radius: i64, start: f64, end: f64, steps: usize) -> Vec<Point> {
    let sweep = arc_sweep(start, end);
    let n = ((steps as f64 * sweep / 360.0).ceil() as usize).clamp(2, 4096);
    (0..=n)
        .map(|j| {
            let a = (start + sweep * j as f64 / n as f64).to_radians();
            Point {
                x: centre.x + (a.cos() * radius as f64).round() as i64,
                y: centre.y + (a.sin() * radius as f64).round() as i64,
            }
        })
        .collect()
}
/// Native contour angles describe an arc's extent; its vertex selects traversal.
/// Clockwise rings often store their starting vertex at the arc's end angle.
pub fn contour_arc_points(position: Point, arc: &ContourArc, steps: usize) -> Vec<Point> {
    let mut points = arc_points(
        arc.centre,
        arc.radius,
        arc.start_angle,
        arc.end_angle,
        steps,
    );
    if distance_sq(position, *points.last().unwrap()) < distance_sq(position, points[0]) {
        points.reverse();
    }
    points[0] = position;
    points
}
pub fn arc_sweep(start: f64, end: f64) -> f64 {
    let d = (end - start).rem_euclid(360.0);
    if d.abs() < 1e-9 { 360.0 } else { d }
}
pub(crate) fn transform_point(
    p: Point,
    pivot: Point,
    delta: Point,
    angle: f64,
) -> Result<Point, String> {
    let (s, c) = angle.to_radians().sin_cos();
    let x = (p.x - pivot.x) as f64;
    let y = (p.y - pivot.y) as f64;
    let p = Point {
        x: (pivot.x as f64 + x * c - y * s + delta.x as f64).round() as i64,
        y: (pivot.y as f64 + x * s + y * c + delta.y as f64).round() as i64,
    };
    if valid_point(p) {
        Ok(p)
    } else {
        Err("Transformed geometry is out of range".into())
    }
}
pub(crate) fn transform_item(
    item: &mut Item,
    pivot: Point,
    delta: Point,
    angle: f64,
    model_bottom: bool,
) -> Result<(), String> {
    crate::pcb_3d::transform_model(item, pivot, delta, angle, false, model_bottom)?;
    let p = |v| transform_point(v, pivot, delta, angle);
    match &mut item.shape {
        Shape::Track { start, end, .. } => {
            *start = p(*start)?;
            *end = p(*end)?;
        }
        Shape::Arc {
            centre,
            start_angle,
            end_angle,
            ..
        } => {
            *centre = p(*centre)?;
            *start_angle += angle;
            *end_angle += angle;
        }
        Shape::Pad {
            position, rotation, ..
        }
        | Shape::Text {
            position, rotation, ..
        } => {
            *position = p(*position)?;
            *rotation = (*rotation + angle).rem_euclid(360.0);
        }
        Shape::Via { position, .. } => *position = p(*position)?,
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            let old = midpoint(*start, *end);
            let new = p(old)?;
            let size = Point {
                x: (end.x - start.x).abs(),
                y: (end.y - start.y).abs(),
            };
            *start = Point {
                x: new.x - size.x / 2,
                y: new.y - size.y / 2,
            };
            *end = Point {
                x: new.x + size.x / 2,
                y: new.y + size.y / 2,
            };
            *rotation = (*rotation + angle).rem_euclid(360.0);
        }
        Shape::Region { contour, holes, .. } => {
            for v in contour {
                *v = p(*v)?;
            }
            for h in holes {
                for v in h {
                    *v = p(*v)?;
                }
            }
        }
    }
    Ok(())
}
/// Distance to visible material, with correctly rotated pad hit-testing.
pub fn hit_distance(item: &Item, p: Point) -> f64 {
    if item.is_drill_only()
        && let Shape::Pad { position, hole, .. } = &item.shape
    {
        return (distance_sq(p, *position).sqrt() - *hole as f64 / 2.0).max(0.0);
    }
    match &item.shape {
        Shape::Track { start, end, width } => {
            (segment_distance(p, *start, *end) - *width as f64 / 2.0).max(0.0)
        }
        Shape::Via {
            position, diameter, ..
        } => (distance_sq(p, *position).sqrt() - *diameter as f64 / 2.0).max(0.0),
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            let a = ((p.y - centre.y) as f64)
                .atan2((p.x - centre.x) as f64)
                .to_degrees();
            if (a - start_angle).rem_euclid(360.0) <= arc_sweep(*start_angle, *end_angle) {
                ((distance_sq(p, *centre).sqrt() - *radius as f64).abs() - *width as f64 / 2.0)
                    .max(0.0)
            } else {
                arc_points(*centre, *radius, *start_angle, *end_angle, 1)
                    .into_iter()
                    .map(|a| distance_sq(p, a).sqrt())
                    .fold(f64::INFINITY, f64::min)
                    - *width as f64 / 2.0
            }
        }
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            ..
        } => {
            let a = (-rotation).to_radians();
            let (s, c) = a.sin_cos();
            let dx = (p.x - position.x) as f64;
            let dy = (p.y - position.y) as f64;
            let x = (dx * c - dy * s).abs();
            let y = (dx * s + dy * c).abs();
            let hw = size.x as f64 / 2.0;
            let hh = size.y as f64 / 2.0;
            match shape {
                PadShape::Round => {
                    let r = hw.min(hh);
                    let dx = (x - (hw - r)).max(0.0);
                    let dy = (y - (hh - r)).max(0.0);
                    (dx.hypot(dy) - r).max(0.0)
                }
                _ => polygon_distance(
                    p,
                    &crate::pcb_polygon::pad_outline_radius(
                        *position,
                        *size,
                        *rotation,
                        *shape,
                        item.pad_corner_percent(if item.layer == MULTI_LAYER {
                            TOP
                        } else {
                            item.layer
                        }),
                    ),
                ),
            }
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => polygon_distance(
            p,
            &rectangle(
                midpoint(*start, *end),
                Point {
                    x: (end.x - start.x).abs(),
                    y: (end.y - start.y).abs(),
                },
                *rotation,
            ),
        ),
        Shape::Region { contour, holes, .. } => {
            if let Some(h) = holes.iter().find(|h| point_in_polygon(p, h)) {
                polygon_edge_distance(p, h)
            } else {
                polygon_distance(p, contour)
            }
        }
        Shape::Text { .. } => {
            if item.bounds().contains(p) {
                0.0
            } else {
                distance_sq(p, item.anchor()).sqrt()
            }
        }
    }
}
fn polygon_edge_distance(p: Point, poly: &[Point]) -> f64 {
    if poly.is_empty() {
        return f64::INFINITY;
    }
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| segment_distance(p, *a, *b))
        .fold(f64::INFINITY, f64::min)
}
fn polygon_distance(p: Point, poly: &[Point]) -> f64 {
    if point_in_polygon(p, poly) {
        0.0
    } else {
        polygon_edge_distance(p, poly)
    }
}
/// Two-leg 45-degree route with a reversible elbow, not a push-and-shove router.
pub fn route_points(start: Point, end: Point, diagonal_first: bool) -> Vec<Point> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let d = dx.abs().min(dy.abs());
    let elbow = if diagonal_first {
        Point {
            x: start.x + dx.signum() * d,
            y: start.y + dy.signum() * d,
        }
    } else {
        Point {
            x: end.x - dx.signum() * d,
            y: end.y - dy.signum() * d,
        }
    };
    let mut p = vec![start];
    if elbow != start && elbow != end {
        p.push(elbow);
    }
    if end != start {
        p.push(end);
    }
    p
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_move_matches_full_validation_and_preserves_worker_snapshots() {
        use std::sync::Arc;
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(1, 1),
                end: Point::mm(3, 1),
                width: 200_000,
            },
        ));
        let selection = vec![b.items[0].id];
        let original = b.clone();
        let pivot = Point::mm(1, 1);
        let mut prepared = MovePreview::new(&b, &selection).unwrap();
        let mut snapshots = vec![];
        for (n, angle) in [0., 90., -90., 360., 17.].into_iter().enumerate() {
            let translation = Point::mm(n as i64, 2);
            let next = prepared.update(pivot, translation, angle).unwrap();
            let expected = b
                .edited(Edit::Transform {
                    selection: selection.clone(),
                    pivot,
                    translation,
                    angle,
                })
                .unwrap();
            assert_eq!(*next, expected);
            snapshots.push((next, expected));
        }
        for (snapshot, expected) in &snapshots {
            assert_eq!(**snapshot, *expected);
        }
        assert_eq!(b, original);
        assert!(prepared.update(pivot, Point::default(), f64::NAN).is_err());
        assert!(
            prepared
                .update(
                    pivot,
                    Point {
                        x: MAX_COORD * 2,
                        y: 0
                    },
                    0.
                )
                .is_err()
        );
        let next = prepared.update(pivot, Point::default(), 0.).unwrap();
        assert_eq!(*next, b);
        let ptr = Arc::as_ptr(&next);
        drop(next);
        let next = prepared.update(pivot, Point::mm(1, 1), 0.).unwrap();
        assert_eq!(
            ptr,
            Arc::as_ptr(&next),
            "unshared draft must reuse its allocation"
        );
        b.items[0].locked = true;
        assert!(MovePreview::new(&b, &selection).is_err());
        b.items[0].locked = false;
        if let Shape::Track { width, .. } = &mut b.items[0].shape {
            *width = -1;
        }
        assert!(MovePreview::new(&b, &selection).is_err());
    }
    #[test]
    fn clockwise_contour_arcs_start_at_their_vertex_without_a_diagonal_backtrack() {
        let arc = ContourArc {
            centre: Point::default(),
            radius: 1_000_000,
            start_angle: 0.0,
            end_angle: 90.0,
        };
        let start = Point::mm(0, 1);
        let pts = contour_arc_points(start, &arc, 32);
        assert_eq!(pts[0], start);
        assert_eq!(pts.last(), Some(&Point::mm(1, 0)));
        assert!(pts.windows(2).all(|p| p[1].x >= p[0].x && p[1].y <= p[0].y));
        assert!(
            pts.windows(2)
                .all(|p| distance_sq(p[0], p[1]).sqrt() < 250_000.0)
        );
        let b = Board {
            outline: vec![ContourVertex {
                position: start,
                arc: Some(arc.clone()),
            }],
            ..Default::default()
        };
        assert_eq!(b.outline_points(32), pts);
        let reversed = contour_arc_points(Point::mm(1, 0), &arc, 32);
        assert_eq!(reversed[0], Point::mm(1, 0));
        assert_eq!(reversed.last(), Some(&start));
    }
    #[test]
    fn hidden_component_strings_do_not_change_fit_and_manual_text_remains_visible() {
        let mut b = Board::default();
        b.components.push(Component {
            id: Uuid::new_v4(),
            index: 0,
            designator: "R1".into(),
            comment: "".into(),
            footprint: "R".into(),
            parameters: Some(Default::default()),
            position: Point::default(),
            rotation: 0.0,
            layer: TOP,
            locked: false,
            properties: Fields::from([
                ("NAMEON".into(), "FALSE".into()),
                ("COMMENTON".into(), "FALSE".into()),
            ]),
        });
        let mut text = Item::new(
            TOP,
            None,
            Shape::Text {
                position: Point::mm(1000, 1000),
                text: "R1".into(),
                height: 1_000_000,
                stroke: 100_000,
                rotation: 0.0,
                mirrored: false,
                font: "".into(),
            },
        );
        text.component = Some(0);
        text.properties.insert("DESIGNATOR".into(), "TRUE".into());
        assert!(!b.text_visible(&text));
        b.items.push(text.clone());
        assert!(b.bounds().max.x < 100_000_000);
        text.properties.clear();
        assert!(b.text_visible(&text));
        b.items.push(text);
        assert!(b.bounds().max.x > 1_000_000_000);
        b.components[0]
            .properties
            .insert("NAMEON".into(), "TRUE".into());
        assert!(b.text_visible(&b.items[0]));
    }
    #[test]
    fn native_pad_radius_percent_uses_half_the_shortest_side_in_geometry_and_picking() {
        let size = Point::mm(2, 1);
        let mut pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::default(),
                size,
                rotation: 0.0,
                shape: PadShape::RoundedRectangle,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.properties
            .insert("PCBCRAFT_PAD_RADIUS_1".into(), "20".into());
        let ring = crate::pcb_polygon::pad_outline_radius(
            Point::default(),
            size,
            0.0,
            PadShape::RoundedRectangle,
            pad.pad_corner_percent(TOP),
        );
        assert_eq!(
            ring[0],
            Point {
                x: 1_000_000,
                y: 400_000
            }
        );
        pad.properties
            .insert("PCBCRAFT_PAD_RADIUS_1".into(), "10".into());
        let corner = Point {
            x: 970_000,
            y: 470_000,
        };
        assert_eq!(hit_distance(&pad, corner), 0.0);
        pad.properties
            .insert("PCBCRAFT_PAD_RADIUS_1".into(), "80".into());
        assert!(hit_distance(&pad, corner) > 100_000.0);
    }
    fn p(x: i64, y: i64) -> Point {
        Point { x, y }
    }
    #[test]
    fn route_is_45_or_orthogonal() {
        for first in [true, false] {
            for x in -12..12 {
                for y in -12..12 {
                    let pts = route_points(p(0, 0), p(x, y), first);
                    assert_eq!(pts.last(), Some(&p(x, y)));
                    for a in pts.windows(2) {
                        let d = p(a[1].x - a[0].x, a[1].y - a[0].y);
                        assert!(d.x == 0 || d.y == 0 || d.x.abs() == d.y.abs());
                    }
                }
            }
        }
    }
    #[test]
    fn locked_edit_is_atomic() {
        let mut b = Board::default();
        let mut i = Item::new(
            TOP,
            None,
            Shape::Track {
                start: p(0, 0),
                end: p(1_000_000, 0),
                width: 100_000,
            },
        );
        i.locked = true;
        let id = i.id;
        b.items.push(i);
        assert!(b.edited(Edit::Delete(vec![id])).is_err());
        assert_eq!(b.items.len(), 1);
    }
    #[test]
    fn drill_only_pads_keep_zero_copper_but_have_physical_bounds_and_picking() {
        let hole = Item::new(
            MULTI_LAYER,
            None,
            Shape::Pad {
                position: Point::mm(1, 2),
                size: Point::default(),
                rotation: 270.0,
                shape: PadShape::Round,
                hole: 450_000,
                plated: false,
                name: "0".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        let mut b = Board {
            items: vec![hole.clone()],
            ..Default::default()
        };
        b.validate().unwrap();
        assert!(hole.is_drill_only());
        assert_eq!(
            hole.bounds().min,
            Point {
                x: 775_000,
                y: 1_775_000
            }
        );
        assert_eq!(
            hit_distance(
                &hole,
                Point {
                    x: 1_200_000,
                    y: 2_000_000
                }
            ),
            0.0
        );
        assert_eq!(
            hit_distance(
                &hole,
                Point {
                    x: 1_300_000,
                    y: 2_000_000
                }
            ),
            75_000.0
        );
        assert!(!crate::pcb_polygon::on_layer(&b, &hole, TOP));
        assert!(!crate::pcb_polygon::on_layer(&b, &hole, BOTTOM));
        for bad in 0..5 {
            b.items[0] = hole.clone();
            if bad == 0 {
                b.items[0].layer = TOP;
            } else if let Shape::Pad {
                size, hole, plated, ..
            } = &mut b.items[0].shape
            {
                match bad {
                    1 => *hole = 0,
                    2 => *plated = true,
                    3 => size.x = 1,
                    _ => size.y = -1,
                }
            }
            assert!(b.validate().is_err(), "case {bad}");
        }
    }
    #[test]
    fn rotated_pad_hit() {
        let i = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: p(0, 0),
                size: p(4_000_000, 1_000_000),
                rotation: 90.0,
                shape: PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        assert_eq!(hit_distance(&i, p(0, 1_500_000)), 0.0);
        assert!(hit_distance(&i, p(1_500_000, 0)) > 900_000.0);
    }
    #[test]
    fn holes_not_selectable_as_copper() {
        let i = Item::new(
            TOP,
            None,
            Shape::Region {
                contour: rectangle(p(0, 0), p(10_000_000, 10_000_000), 0.0),
                holes: vec![rectangle(p(0, 0), p(2_000_000, 2_000_000), 0.0)],
                cutout: false,
            },
        );
        assert!(hit_distance(&i, p(0, 0)) > 900_000.0);
        assert_eq!(hit_distance(&i, p(3_000_000, 0)), 0.0);
    }
    #[test]
    fn rejects_missing_net() {
        let mut b = Board::default();
        b.items.push(Item::new(
            TOP,
            Some(1),
            Shape::Track {
                start: p(0, 0),
                end: p(1, 1),
                width: 1,
            },
        ));
        assert!(b.validate().is_err());
    }
    #[test]
    fn group_move_updates_owner_and_children() {
        let mut b = Board::default();
        let cid = Uuid::new_v4();
        b.components.push(Component {
            id: cid,
            index: 4,
            designator: "U1".into(),
            comment: String::new(),
            footprint: String::new(),
            parameters: Some(Default::default()),
            position: p(0, 0),
            rotation: 0.0,
            layer: TOP,
            locked: false,
            properties: Fields::new(),
        });
        let mut i = Item::new(
            TOP,
            None,
            Shape::Track {
                start: p(0, 0),
                end: p(1_000_000, 0),
                width: 100_000,
            },
        );
        i.component = Some(4);
        b.items.push(i);
        let n = b
            .edited(Edit::Transform {
                selection: vec![cid],
                pivot: p(0, 0),
                translation: p(2_000_000, 0),
                angle: 90.0,
            })
            .unwrap();
        assert_eq!(n.components[0].position, p(2_000_000, 0));
        if let Shape::Track { end, .. } = n.items[0].shape {
            assert_eq!(end, p(2_000_000, 1_000_000));
        } else {
            panic!();
        }
    }
    #[test]
    fn snap_rounds_negative_coordinates() {
        assert_eq!(snap_coord(-16, 10), -20);
        assert_eq!(snap_coord(-14, 10), -10);
    }
    #[test]
    fn pcb_edit_history_roundtrip_and_source_archive_sharing() {
        let mut doc = Document::new(DocumentKind::Board, "test");
        Board::ensure_document(&mut doc);
        let board = doc.pcb.as_mut().unwrap();
        board.source = Some(std::sync::Arc::new(SourceArchive {
            filename: "source.PcbDoc".into(),
            format: "test".into(),
            bytes_base64: "AA==".into(),
        }));
        let original = doc.clone();
        let mut next = doc.clone();
        next.pcb = Some(
            next.pcb
                .as_ref()
                .unwrap()
                .edited(Edit::Insert(vec![Item::new(
                    TOP,
                    None,
                    Shape::Track {
                        start: p(0, 0),
                        end: p(1_000_000, 0),
                        width: 100_000,
                    },
                )]))
                .unwrap(),
        );
        assert!(std::sync::Arc::ptr_eq(
            original.pcb.as_ref().unwrap().source.as_ref().unwrap(),
            next.pcb.as_ref().unwrap().source.as_ref().unwrap()
        ));
        let mut history = crate::History::default();
        history.commit(&mut doc, next.clone()).unwrap();
        history.undo(&mut doc);
        assert_eq!(doc, original);
        history.redo(&mut doc);
        assert_eq!(doc, next);
    }
    #[test]
    fn unlock_cannot_silently_move_a_locked_object() {
        let mut board = Board::default();
        let mut item = Item::new(
            TOP,
            None,
            Shape::Track {
                start: p(0, 0),
                end: p(1_000_000, 0),
                width: 100_000,
            },
        );
        item.locked = true;
        board.items.push(item.clone());
        let mut invalid = item.clone();
        invalid.locked = false;
        invalid.layer = BOTTOM;
        assert!(board.edited(Edit::Replace(invalid)).is_err());
        item.locked = false;
        assert!(!board.edited(Edit::Replace(item)).unwrap().items[0].locked);
    }
}
