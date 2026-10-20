//! Schematic edits in document coordinates. GUI previews and commits use the same commands.
#[cfg(test)]
#[path = "schematic_edit_tests.rs"]
mod tests;
use crate::schematic::{
    Component, SchFont, SchPrimitive, SchPrimitiveKind as K, Schematic, TextRole,
};
use crate::{Document, DocumentKind, History, Point};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const AUTHORED: u32 = u32::MAX;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fragment {
    pub fonts: Vec<SchFont>,
    pub images: std::collections::BTreeMap<String, String>,
    pub components: Vec<Component>,
    pub primitives: Vec<SchPrimitive>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    SheetFont(crate::schematic::SchFont),
    Font {
        id: Uuid,
        font: crate::schematic::SchFont,
    },
    SheetProperties {
        sheet: Box<crate::schematic::Sheet>,
        size: Point,
    },
    PrimitiveProperties {
        id: Uuid,
        value: Box<SchPrimitive>,
    },
    ComponentDetails {
        id: Uuid,
        description: String,
        settings: crate::schematic::ComponentSettings,
    },
    ParameterValue {
        component: Option<Uuid>,
        name: String,
        value: Option<String>,
    },
    Insert(Fragment),
    Delete(Uuid),
    Transform {
        id: Uuid,
        pivot: Point,
        quarter_turns: u32,
        delta: Point,
    },
    ComponentProperties {
        id: Uuid,
        designator: String,
        comment: String,
    },
    Text {
        id: Uuid,
        text: String,
    },
    Width {
        id: Uuid,
        width_nm: i64,
    },
    Batch(Vec<Edit>),
}

#[derive(Clone, Debug)]
pub struct Request {
    pub document: Uuid,
    pub expected_revision: u64,
    pub edit: Edit,
}

impl History {
    pub fn schematic_edit(&mut self, doc: &mut Document, request: Request) -> Result<bool, String> {
        if request.document != doc.id || request.expected_revision != self.revision {
            return Err("The document changed during the operation. Start it again.".into());
        }
        let next = preview(doc, &request.edit)?;
        self.commit(doc, next)
    }
}

pub fn preview(doc: &Document, edit: &Edit) -> Result<Document, String> {
    if doc.kind != DocumentKind::Schematic {
        return Err("A schematic document is required".into());
    }
    let mut next = doc.clone();
    let s = next
        .schematic
        .as_mut()
        .ok_or("Schematic scene is unavailable")?;
    apply(s, &mut next.size, edit, 0)?;
    refresh_parameters(s);
    if s != doc.schematic.as_ref().unwrap() && s.source.is_some() {
        s.source_modified = true;
    }
    next.validate()?;
    Ok(next)
}

fn apply(s: &mut Schematic, size: &mut Point, edit: &Edit, depth: usize) -> Result<(), String> {
    if depth > 16 {
        return Err("Edit nesting exceeds limit".into());
    }
    match edit {
        Edit::SheetFont(font) => {
            let index = if let Some(i) = s.fonts.iter().position(|f| f == font) {
                i
            } else {
                s.fonts.push(font.clone());
                s.fonts.len() - 1
            };
            s.sheet.system_font = index;
        }
        Edit::Font { id, font } => {
            let index = if let Some(i) = s.fonts.iter().position(|f| f == font) {
                i
            } else {
                s.fonts.push(font.clone());
                s.fonts.len() - 1
            };
            let p = s
                .primitives
                .iter_mut()
                .chain(&mut s.parameter_objects)
                .find(|p| p.id == *id)
                .ok_or("Missing object")?;
            match &mut p.kind {
                K::Text { font, .. }
                | K::TextFrame { font, .. }
                | K::Port { font, .. }
                | K::PowerPort { font, .. }
                | K::Pin { font, .. } => *font = index,
                _ => return Err("Object has no font".into()),
            }
        }
        Edit::SheetProperties {
            sheet,
            size: new_size,
        } => {
            s.sheet = *sheet.clone();
            *size = *new_size;
        }
        Edit::ComponentDetails {
            id,
            description,
            settings,
        } => {
            let c = s
                .components
                .iter_mut()
                .find(|c| c.id == *id)
                .ok_or("Component no longer exists")?;
            if c.settings.rotation != settings.rotation {
                return Err("Use Transform to rotate component geometry".into());
            }
            c.description = description.clone();
            c.settings = settings.clone();
        }
        Edit::PrimitiveProperties { id, value } => {
            let p = s
                .primitives
                .iter_mut()
                .chain(&mut s.parameter_objects)
                .find(|p| p.id == *id)
                .ok_or("Object no longer exists")?;
            if value.id != *id
                || value.component != p.component
                || value.source_index != p.source_index
                || std::mem::discriminant(&value.kind) != std::mem::discriminant(&p.kind)
                || value.text_role != p.text_role
                || value.presentation.record_type != p.presentation.record_type
            {
                return Err("Object identity cannot change through Properties".into());
            }
            let old = p.clone();
            if !old
                .presentation
                .name
                .eq_ignore_ascii_case(&value.presentation.name)
            {
                let values = if let Some(owner) = p.component {
                    &s.components
                        .iter()
                        .find(|c| c.id == owner)
                        .ok_or("Missing component")?
                        .parameters
                } else {
                    &s.sheet.parameters
                };
                if values
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case(&value.presentation.name))
                {
                    return Err("A parameter with this name already exists".into());
                }
            }
            if (!old.presentation.name.is_empty() && value.presentation.name.trim().is_empty())
                || (p.text_role == Some(TextRole::Designator)
                    && value
                        .presentation
                        .raw_text
                        .as_ref()
                        .is_some_and(|s| s.trim().is_empty()))
            {
                return Err("Parameter name and designator cannot be empty".into());
            }
            *p = *value.clone();
            if let Some(text) = &p.presentation.raw_text {
                if let Some(owner) = p.component {
                    let c = s
                        .components
                        .iter_mut()
                        .find(|c| c.id == owner)
                        .ok_or("Missing component")?;
                    if p.text_role == Some(TextRole::Designator) {
                        c.designator = text.clone();
                    } else if !p.presentation.name.is_empty() {
                        if old.presentation.name != p.presentation.name {
                            c.parameters.remove(&old.presentation.name);
                        }
                        c.parameters
                            .insert(p.presentation.name.clone(), text.clone());
                    }
                } else if !p.presentation.name.is_empty() {
                    if old.presentation.name != p.presentation.name {
                        s.sheet.parameters.remove(&old.presentation.name);
                    }
                    s.sheet
                        .parameters
                        .insert(p.presentation.name.clone(), text.clone());
                }
            }
        }
        Edit::ParameterValue {
            component,
            name,
            value,
        } => {
            if name.trim().is_empty() {
                return Err("Parameter name cannot be empty".into());
            }
            let values = if let Some(id) = component {
                &mut s
                    .components
                    .iter_mut()
                    .find(|c| c.id == *id)
                    .ok_or("Missing component")?
                    .parameters
            } else {
                &mut s.sheet.parameters
            };
            let key = values
                .keys()
                .find(|k| k.eq_ignore_ascii_case(name))
                .cloned()
                .unwrap_or_else(|| name.clone());
            if let Some(value) = value {
                values.insert(key, value.clone());
            } else {
                values.remove(&key);
                let retain = |p: &SchPrimitive| {
                    p.component != *component || !p.presentation.name.eq_ignore_ascii_case(name)
                };
                s.primitives.retain(retain);
                s.parameter_objects.retain(retain);
            }
            for p in s
                .primitives
                .iter_mut()
                .chain(&mut s.parameter_objects)
                .filter(|p| {
                    p.component == *component && p.presentation.name.eq_ignore_ascii_case(name)
                })
            {
                if let Some(value) = value {
                    p.presentation.raw_text = Some(value.clone());
                }
            }
        }
        Edit::Insert(f) => {
            let mut map = vec![];
            for font in &f.fonts {
                let index = s.fonts.iter().position(|v| v == font).unwrap_or_else(|| {
                    s.fonts.push(font.clone());
                    s.fonts.len() - 1
                });
                map.push(index);
            }
            let mut primitives = f.primitives.clone();
            for p in &mut primitives {
                remap_fonts(&mut p.kind, &map)?;
            }
            for (key, data) in &f.images {
                if s.images.get(key).is_some_and(|old| old != data) {
                    return Err("Clipboard image name conflicts with an existing resource".into());
                }
                s.images.insert(key.clone(), data.clone());
            }
            s.components.extend(f.components.clone());
            s.primitives.extend(primitives);
        }
        Edit::Delete(id) => {
            let found = s.components.iter().any(|c| c.id == *id) || s.primitive(*id).is_some();
            if !found {
                return Err("Selected object no longer exists".into());
            }
            s.components.retain(|c| c.id != *id);
            s.primitives
                .retain(|p| p.id != *id && p.component != Some(*id));
            s.parameter_objects
                .retain(|p| p.id != *id && p.component != Some(*id));
        }
        Edit::Transform {
            id,
            pivot,
            quarter_turns,
            delta,
        } => {
            let mut found = false;
            for c in &mut s.components {
                if c.id == *id {
                    c.position = transform_point(c.position, *pivot, *quarter_turns, *delta)?;
                    c.settings.rotation = (c.settings.rotation + quarter_turns) % 4;
                    found = true;
                }
            }
            for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
                if p.id == *id || p.component == Some(*id) {
                    transform_primitive(p, *pivot, *quarter_turns, *delta)?;
                    found = true;
                }
            }
            if !found {
                return Err("Selected object no longer exists".into());
            }
        }
        Edit::ComponentProperties {
            id,
            designator,
            comment,
        } => {
            if designator.trim().is_empty() {
                return Err("Designator cannot be empty".into());
            }
            let c = s
                .components
                .iter_mut()
                .find(|c| c.id == *id)
                .ok_or("Component no longer exists")?;
            let raw_comment = c
                .parameters
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("Comment"))
                .map_or(c.comment.as_str(), |(_, v)| v.as_str());
            if c.designator == *designator && raw_comment == comment {
                return Ok(());
            }
            c.designator = designator.clone();
            c.comment = comment.clone();
            let key = c
                .parameters
                .keys()
                .find(|k| k.eq_ignore_ascii_case("Comment"))
                .cloned()
                .unwrap_or_else(|| "Comment".into());
            c.parameters.insert(key, comment.clone());
            for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
                if p.component != Some(*id) {
                    continue;
                }
                let role = p.text_role.or_else(|| {
                    s.records.get(p.source_index as usize).and_then(|r| {
                        if r.record_type == 34 {
                            Some(TextRole::Designator)
                        } else if r.record_type == 41
                            && r.fields
                                .get("NAME")
                                .is_some_and(|n| n.eq_ignore_ascii_case("Comment"))
                        {
                            Some(TextRole::Comment)
                        } else {
                            None
                        }
                    })
                });
                if let K::Text { text, .. } = &mut p.kind {
                    match role {
                        Some(TextRole::Designator) => {
                            *text = designator.clone();
                            p.presentation.raw_text = Some(designator.clone());
                        }
                        Some(TextRole::Comment) => {
                            *text = comment.clone();
                            p.presentation.raw_text = Some(comment.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
        Edit::Text { id, text } => {
            let p = s
                .primitives
                .iter_mut()
                .find(|p| p.id == *id)
                .ok_or("Text object no longer exists")?;
            if p.component.is_some() {
                return Err("Edit the owning component's properties".into());
            }
            match &mut p.kind {
                K::Text { text: value, .. }
                | K::TextFrame { text: value, .. }
                | K::PowerPort { text: value, .. } => *value = text.clone(),
                K::Port { name, .. } => *name = text.clone(),
                _ => return Err("Selected object has no editable text".into()),
            }
            if p.presentation.raw_text.is_some() {
                p.presentation.raw_text = Some(text.clone());
            }
        }
        Edit::Width { id, width_nm } => {
            if !(1..=1_000_000_000).contains(width_nm) {
                return Err("Invalid line width".into());
            }
            let p = s
                .primitives
                .iter_mut()
                .find(|p| p.id == *id)
                .ok_or("Object no longer exists")?;
            if p.component.is_some() || !matches!(p.kind, K::Polyline { .. }) {
                return Err("Select a wire or line".into());
            }
            p.width_nm = *width_nm;
        }
        Edit::Batch(edits) => {
            if edits.len() > 10_000 {
                return Err("Too many edits".into());
            }
            for edit in edits {
                apply(s, size, edit, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn refresh_parameters(s: &mut Schematic) {
    use crate::schematic::parameter_text;
    for c in &mut s.components {
        if let Some(value) = c
            .parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Comment"))
            .map(|(_, v)| v)
        {
            c.comment = parameter_text(value, &c.parameters, &s.sheet.parameters);
        }
    }
    for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
        let Some(raw) = &p.presentation.raw_text else {
            continue;
        };
        if let K::Text { text, .. } = &mut p.kind {
            let c = p
                .component
                .and_then(|id| s.components.iter().find(|c| c.id == id));
            *text = if p.text_role == Some(TextRole::Designator) {
                c.map_or_else(|| raw.clone(), |c| c.designator.clone())
            } else {
                parameter_text(
                    raw,
                    c.map_or(&s.sheet.parameters, |c| &c.parameters),
                    &s.sheet.parameters,
                )
            };
        }
    }
}

pub fn transform_point(p: Point, pivot: Point, turns: u32, delta: Point) -> Result<Point, String> {
    let x = p.x as i128 - pivot.x as i128;
    let y = p.y as i128 - pivot.y as i128;
    let (x, y) = match turns % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    };
    let result = Point {
        x: i64::try_from(x + pivot.x as i128 + delta.x as i128)
            .map_err(|_| "Coordinate overflow")?,
        y: i64::try_from(y + pivot.y as i128 + delta.y as i128)
            .map_err(|_| "Coordinate overflow")?,
    };
    if !result.valid() {
        return Err("Object exceeds the document coordinate range".into());
    }
    Ok(result)
}

pub fn transform_primitive(
    p: &mut SchPrimitive,
    pivot: Point,
    turns: u32,
    delta: Point,
) -> Result<(), String> {
    let turns = turns % 4;
    let tx = |p: &mut Point| -> Result<(), String> {
        *p = transform_point(*p, pivot, turns, delta)?;
        Ok(())
    };
    match &mut p.kind {
        K::Polyline { points, .. } => {
            for p in points {
                tx(p)?;
            }
        }
        K::Bezier { points } => {
            for p in points {
                tx(p)?;
            }
        }
        K::Rectangle { a, b, radius } => {
            tx(a)?;
            tx(b)?;
            if turns % 2 == 1 {
                std::mem::swap(&mut radius.x, &mut radius.y);
            }
        }
        K::Arc {
            centre,
            radii,
            start,
            end,
        } => {
            tx(centre)?;
            if turns % 2 == 1 {
                std::mem::swap(&mut radii.x, &mut radii.y);
            }
            let sweep = *end - *start;
            *start = (*start + turns as f64 * 90.0).rem_euclid(360.0);
            *end = *start + sweep;
        }
        K::Text {
            position,
            orientation,
            ..
        }
        | K::Pin {
            position,
            orientation,
            ..
        }
        | K::PowerPort {
            position,
            orientation,
            ..
        }
        | K::Ieee {
            position,
            orientation,
            ..
        } => {
            tx(position)?;
            *orientation = (*orientation + turns) % 4;
        }
        K::Junction { position, .. } | K::NoErc { position } => tx(position)?,
        K::Port { position, .. } => {
            if turns != 0 {
                return Err("Rotation of this port style is not supported yet".into());
            }
            tx(position)?;
        }
        K::TextFrame { a, b, .. } | K::Image { a, b, .. } | K::Unsupported { a, b, .. } => {
            if turns != 0 {
                return Err(
                    "Rotation of images, text frames and unsupported shapes is not supported yet"
                        .into(),
                );
            }
            tx(a)?;
            tx(b)?;
        }
    }
    Ok(())
}

pub fn anchor(s: &Schematic, id: Uuid) -> Option<Point> {
    s.components
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.position)
        .or_else(|| {
            s.primitive(id).and_then(|p| match &p.kind {
                K::Polyline { points, .. } => points.first().copied(),
                K::Bezier { points } => Some(points[0]),
                K::Rectangle { a, .. }
                | K::TextFrame { a, .. }
                | K::Image { a, .. }
                | K::Unsupported { a, .. } => Some(*a),
                K::Arc { centre, .. } => Some(*centre),
                K::Text { position, .. }
                | K::Pin { position, .. }
                | K::PowerPort { position, .. }
                | K::Port { position, .. }
                | K::Junction { position, .. }
                | K::NoErc { position }
                | K::Ieee { position, .. } => Some(*position),
            })
        })
}

pub fn extract(s: &Schematic, id: Uuid) -> Result<Fragment, String> {
    let mut f = Fragment {
        fonts: s.fonts.clone(),
        images: s.images.clone(),
        components: s
            .components
            .iter()
            .filter(|c| c.id == id)
            .cloned()
            .collect(),
        primitives: s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| p.id == id || p.component == Some(id))
            .cloned()
            .collect(),
    };
    for p in &mut f.primitives {
        if p.text_role.is_none()
            && let Some(r) = s.records.get(p.source_index as usize)
        {
            p.text_role = if r.record_type == 34 {
                Some(TextRole::Designator)
            } else if r.record_type == 41
                && r.fields
                    .get("NAME")
                    .is_some_and(|n| n.eq_ignore_ascii_case("Comment"))
            {
                Some(TextRole::Comment)
            } else {
                None
            };
        }
    }
    if f.components.is_empty() && f.primitives.is_empty() {
        return Err("Nothing selected".into());
    }
    Ok(f)
}

fn remap_fonts(kind: &mut K, map: &[usize]) -> Result<(), String> {
    if map.is_empty() {
        return Ok(());
    }
    let remap = |font: &mut usize| -> Result<(), String> {
        *font = *map.get(*font).ok_or("Clipboard font is missing")?;
        Ok(())
    };
    match kind {
        K::Text { font, .. }
        | K::TextFrame { font, .. }
        | K::PowerPort { font, .. }
        | K::Port { font, .. } => remap(font)?,
        K::Pin {
            font, number_font, ..
        } => {
            remap(font)?;
            if let Some(n) = number_font {
                remap(n)?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl Fragment {
    pub fn fresh_copy(&self) -> Self {
        let mut f = self.clone();
        let mut ids = std::collections::HashMap::new();
        for c in &mut f.components {
            let old = c.id;
            c.id = Uuid::new_v4();
            ids.insert(old, c.id);
            c.source_index = AUTHORED;
        }
        for p in &mut f.primitives {
            p.id = Uuid::new_v4();
            p.component = p.component.and_then(|id| ids.get(&id).copied());
            p.source_index = AUTHORED;
        }
        f
    }
    pub fn transformed(&self, pivot: Point, turns: u32, delta: Point) -> Result<Self, String> {
        let mut f = self.clone();
        for c in &mut f.components {
            c.position = transform_point(c.position, pivot, turns, delta)?;
            c.settings.rotation = (c.settings.rotation + turns) % 4;
        }
        for p in &mut f.primitives {
            transform_primitive(p, pivot, turns, delta)?;
        }
        Ok(f)
    }
}

pub fn primitive(kind: K, component: Option<Uuid>) -> SchPrimitive {
    SchPrimitive {
        presentation: Default::default(),
        id: Uuid::new_v4(),
        source_index: AUTHORED,
        component,
        color: 0x800000,
        fill: None,
        width_nm: 254_000,
        line_style: 0,
        start_shape: 0,
        end_shape: 0,
        line_shape_size: 0,
        text_role: None,
        kind,
    }
}

/// Grid and electrical endpoint/segment snapping in document coordinates.
pub fn snap_point(s: &Schematic, raw: Point, step: i64, radius: i64) -> Point {
    let step = step.max(1) as i128;
    let round = |n: i64| (((n as i128 + step / 2).div_euclid(step)) * step) as i64;
    let mut result = if s.sheet.options.snap_enabled {
        Point {
            x: round(raw.x),
            y: round(raw.y),
        }
    } else {
        raw
    };
    let mut best = (radius.max(0) as f64).powi(2);
    let mut consider = |p: Point| {
        let dx = p.x as f64 - raw.x as f64;
        let dy = p.y as f64 - raw.y as f64;
        let d = dx * dx + dy * dy;
        if p.valid() && d <= best {
            best = d;
            result = p;
        }
    };
    for p in s
        .primitives
        .iter()
        .filter(|p| !p.presentation.hidden && s.sheet.options.electrical_snap)
    {
        match &p.kind {
            K::Pin {
                position,
                length_nm,
                orientation,
                ..
            } => {
                let (x, y) = match orientation % 4 {
                    0 => (*length_nm, 0),
                    1 => (0, *length_nm),
                    2 => (-*length_nm, 0),
                    _ => (0, -*length_nm),
                };
                consider(Point {
                    x: position.x.saturating_add(x),
                    y: position.y.saturating_add(y),
                });
            }
            K::PowerPort { position, .. } | K::Junction { position, .. } => consider(*position),
            K::Polyline {
                points, electrical, ..
            } if electrical == "Wire" || electrical == "Bus" => {
                for p in points {
                    consider(*p);
                }
                for segment in points.windows(2) {
                    let [a, b] = [segment[0], segment[1]];
                    if a.x == b.x {
                        consider(Point {
                            x: a.x,
                            y: raw.y.clamp(a.y.min(b.y), a.y.max(b.y)),
                        });
                    } else if a.y == b.y {
                        consider(Point {
                            x: raw.x.clamp(a.x.min(b.x), a.x.max(b.x)),
                            y: a.y,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Resistor,
    Capacitor,
    Wire,
    NetLabel,
    Text,
    Ground,
    Junction,
    Line,
}
impl Placement {
    pub fn label(self) -> &'static str {
        match self {
            Self::Resistor => "Resistor",
            Self::Capacitor => "Capacitor",
            Self::Wire => "Wire",
            Self::NetLabel => "Net Label",
            Self::Text => "Text String",
            Self::Ground => "Power Port",
            Self::Junction => "Junction",
            Self::Line => "Line",
        }
    }
}

pub fn place(
    kind: Placement,
    at: Point,
    turns: u32,
    designator: &str,
    value: &str,
    font: usize,
) -> Result<Fragment, String> {
    let mut f = Fragment::default();
    let text = |position, text: String, role, component| {
        let raw_text = text.clone();
        let mut p = primitive(
            K::Text {
                position,
                text,
                font,
                orientation: 0,
                justification: 0,
            },
            component,
        );
        p.text_role = role;
        p.presentation.raw_text = Some(raw_text);
        p.presentation.name = match role {
            Some(TextRole::Designator) => "Designator",
            Some(TextRole::Comment) => "Comment",
            _ => "",
        }
        .into();
        p.presentation.auto_position = component.is_some();
        p.presentation.allow_sync = true;
        p
    };
    match kind {
        Placement::Resistor | Placement::Capacitor => {
            if designator.trim().is_empty() {
                return Err("Designator cannot be empty".into());
            }
            let id = Uuid::new_v4();
            f.components.push(Component {
                settings: Default::default(),
                id,
                source_index: AUTHORED,
                position: Point::default(),
                designator: designator.into(),
                comment: value.into(),
                library_reference: kind.label().into(),
                description: "PCBCraft basic symbol".into(),
                part: 1,
                display_mode: 0,
                parameters: Default::default(),
            });
            for (x, orientation, number) in [(-5_080_000, 2, "1"), (5_080_000, 0, "2")] {
                f.primitives.push(primitive(
                    K::Pin {
                        position: Point { x, y: 0 },
                        length_nm: 5_080_000,
                        orientation,
                        name: String::new(),
                        number: number.into(),
                        show_name: false,
                        show_number: false,
                        font,
                        number_font: Some(font),
                        electrical: 4,
                        outer_symbol: 0,
                        inner_symbol: 0,
                    },
                    Some(id),
                ));
            }
            if kind == Placement::Resistor {
                f.primitives.push(primitive(
                    K::Rectangle {
                        a: Point {
                            x: -5_080_000,
                            y: -1_270_000,
                        },
                        b: Point {
                            x: 5_080_000,
                            y: 1_270_000,
                        },
                        radius: Point::default(),
                    },
                    Some(id),
                ));
            } else {
                // Capacitor plates and leads are one component, never independent placement objects.
                for (a, b) in [
                    (
                        Point {
                            x: -5_080_000,
                            y: 0,
                        },
                        Point { x: -762_000, y: 0 },
                    ),
                    (Point { x: 762_000, y: 0 }, Point { x: 5_080_000, y: 0 }),
                    (
                        Point {
                            x: -762_000,
                            y: -2_540_000,
                        },
                        Point {
                            x: -762_000,
                            y: 2_540_000,
                        },
                    ),
                    (
                        Point {
                            x: 762_000,
                            y: -2_540_000,
                        },
                        Point {
                            x: 762_000,
                            y: 2_540_000,
                        },
                    ),
                ] {
                    f.primitives.push(primitive(
                        K::Polyline {
                            points: vec![a, b],
                            closed: false,
                            electrical: "Graphic".into(),
                        },
                        Some(id),
                    ));
                }
            }
            f.primitives.push(text(
                Point {
                    x: -5_080_000,
                    y: 3_048_000,
                },
                designator.into(),
                Some(TextRole::Designator),
                Some(id),
            ));
            f.primitives.push(text(
                Point {
                    x: -5_080_000,
                    y: -5_080_000,
                },
                value.into(),
                Some(TextRole::Comment),
                Some(id),
            ));
        }
        Placement::Text | Placement::NetLabel => f.primitives.push(text(
            Point::default(),
            value.into(),
            (kind == Placement::NetLabel).then_some(TextRole::NetLabel),
            None,
        )),
        Placement::Ground => {
            let mut p = primitive(
                K::PowerPort {
                    position: Point::default(),
                    orientation: 3,
                    style: 2,
                    text: value.into(),
                    show_name: true,
                    font,
                },
                None,
            );
            p.color = 128;
            f.primitives.push(p);
        }
        Placement::Junction => f.primitives.push(primitive(
            K::Junction {
                position: Point::default(),
                radius_nm: 508_000,
            },
            None,
        )),
        Placement::Wire | Placement::Line => {
            return Err("A wire requires at least two points".into());
        }
    }
    f.transformed(Point::default(), turns, at)
}
