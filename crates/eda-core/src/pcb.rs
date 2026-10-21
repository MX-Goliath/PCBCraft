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
        83..=98 => format!("Mechanical {}", id - 66),
        _ => format!("Layer {id}"),
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
}
impl Item {
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
    /// Conservative world bounds, used for view fitting and spatial rejection only.
    pub fn bounds(&self) -> Bounds {
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
                ..
            } => Bounds::points(&rectangle(*position, *size, *rotation)),
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
                ..
            } => {
                let size = Point {
                    x: (text.chars().count() as f64 * *height as f64 * 0.75) as i64,
                    y: *height,
                };
                Bounds::points(&rectangle(*position, size, *rotation)).expanded(*height)
            }
            Shape::Region { contour, .. } => Bounds::points(contour),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
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
    pub polygons: Vec<Fields>,
    #[serde(default)]
    pub polygon_pours: Vec<crate::pcb_polygon::Polygon>,
    /// 0 means an old JSON document that still needs metadata migration.
    #[serde(default)]
    pub polygon_model_version: u8,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub pours_stale: bool,
    #[serde(default)]
    pub source: Option<std::sync::Arc<SourceArchive>>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
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
            polygons: vec![],
            polygon_pours: vec![],
            polygon_model_version: 1,
            warnings: vec![],
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
        if self.items.len() > MAX_ITEMS
            || self.components.len() > MAX_ITEMS
            || self.nets.len() > MAX_ITEMS
        {
            return Err("PCB entity limit exceeded".into());
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
            let ok = match &i.shape {
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
                        && positive(*width)
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
                        && positive(size.x)
                        && positive(size.y)
                        && rotation.is_finite()
                        && *hole >= 0
                        && *hole <= MAX_COORD
                        && [bottom_size, middle_size]
                            .into_iter()
                            .all(|s| s.is_none_or(|p| positive(p.x) && positive(p.y)))
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
                            h.len() >= 3
                                && h.len() <= MAX_ITEMS
                                && h.iter().all(|p| valid_point(*p))
                        })
                }
            };
            if !ok {
                return Err(format!("Invalid PCB {} geometry", i.kind()));
            }
        }
        Ok(())
    }
    pub fn net_name(&self, net: Option<u32>) -> &str {
        net.and_then(|id| self.nets.iter().find(|n| n.index == id))
            .map_or("No Net", |n| n.name.as_str())
    }
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::empty();
        for p in self.outline_points(48) {
            b.include(p);
        }
        for item in &self.items {
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
                p.extend(arc_points(
                    a.centre,
                    a.radius,
                    a.start_angle,
                    a.end_angle,
                    steps,
                ));
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
        match action {
            Edit::Insert(items) => {
                if next.items.len().saturating_add(items.len()) > MAX_ITEMS {
                    return Err("PCB entity limit exceeded".into());
                }
                next.items.extend(items);
            }
            Edit::Replace(item) => {
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
                    .is_some_and(|owner| {
                        self.components.iter().any(|c| c.index == owner && c.locked)
                    })
                {
                    return Err("Cannot edit a child of a locked component".into());
                }
                let old = next
                    .item_mut(item.id)
                    .ok_or("Selected object no longer exists")?;
                if old.locked && old != &item {
                    let mut unlocked = old.clone();
                    unlocked.locked = false;
                    if item != unlocked {
                        return Err(
                            "Apply Unlock on its own before changing a locked object".into()
                        );
                    }
                }
                *old = item;
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
                    transform_item(item, pivot, translation, angle)?;
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
        next.pours_stale |= !next.polygon_pours.is_empty();
        next.validate()?;
        Ok(next)
    }
    fn selection_locked(&self, selection: &[Uuid], ids: &BTreeSet<Uuid>) -> bool {
        self.items.iter().any(|i| {
            ids.contains(&i.id)
                && (i.locked
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
) -> Result<(), String> {
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
                    &crate::pcb_polygon::pad_outline(*position, *size, *rotation, *shape),
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
