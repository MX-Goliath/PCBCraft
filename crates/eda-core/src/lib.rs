//! GUI-independent document model and history. No native Altium reader exists yet.
use eda_geom::Point;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentKind {
    Board,
    Schematic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Units {
    #[default]
    Millimetres,
    Mils,
}

impl Units {
    pub fn label(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Mils => "mil",
        }
    }
    pub fn from_nm(self, nm: f64) -> f64 {
        nm / match self {
            Self::Millimetres => 1_000_000.0,
            Self::Mils => 25_400.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    Top,
    Bottom,
    Overlay,
    Outline,
}

impl Layer {
    pub const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Overlay, Self::Outline];
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Top Layer",
            Self::Bottom => "Bottom Layer",
            Self::Overlay => "Top Overlay",
            Self::Outline => "Board Outline",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pad {
    pub id: Uuid,
    pub name: String,
    pub position: Point,
    pub size: Point,
    pub layer: Layer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: Uuid,
    pub start: Point,
    pub end: Point,
    pub width: i64,
    pub layer: Layer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub kind: DocumentKind,
    pub size: Point,
    pub pads: Vec<Pad>,
    pub tracks: Vec<Track>,
}

impl Document {
    pub fn new(kind: DocumentKind, name: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            size: match kind {
                DocumentKind::Board => Point::mm(80, 50),
                DocumentKind::Schematic => Point::mm(297, 210),
            },
            pads: vec![],
            tracks: vec![],
        }
    }

    pub fn example() -> Self {
        let mut doc = Self::new(DocumentKind::Board, "Controller.pcbcraft.json");
        doc.size = Point::mm(60, 40);
        for (name, x, y) in [
            ("J1.1", 10, 15),
            ("J1.2", 10, 25),
            ("R1.1", 27, 15),
            ("R1.2", 33, 15),
            ("J2.1", 50, 15),
            ("J2.2", 50, 25),
        ] {
            doc.pads.push(Pad {
                id: Uuid::new_v4(),
                name: name.into(),
                position: Point::mm(x, y),
                size: Point::mm(3, 2),
                layer: Layer::Top,
            });
        }
        for ((x1, y1), (x2, y2)) in [
            ((10, 15), (27, 15)),
            ((33, 15), (50, 15)),
            ((10, 25), (50, 25)),
        ] {
            doc.tracks.push(Track {
                id: Uuid::new_v4(),
                start: Point::mm(x1, y1),
                end: Point::mm(x2, y2),
                width: 500_000,
                layer: Layer::Top,
            });
        }
        doc
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported document version: {}",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() {
            return Err("Document name cannot be empty".into());
        }
        if !self.size.valid() || self.size.x <= 0 || self.size.y <= 0 {
            return Err("Invalid document dimensions".into());
        }
        let mut ids = std::collections::HashSet::new();
        for pad in &self.pads {
            if !ids.insert(pad.id)
                || pad.name.trim().is_empty()
                || !pad.position.valid()
                || !pad.size.valid()
                || pad.size.x <= 0
                || pad.size.y <= 0
            {
                return Err("Invalid or duplicate pad".into());
            }
        }
        for track in &self.tracks {
            if !ids.insert(track.id)
                || !track.start.valid()
                || !track.end.valid()
                || !(1..=1_000_000_000).contains(&track.width)
            {
                return Err("Invalid or duplicate track".into());
            }
        }
        Ok(())
    }
}

/// Snapshot history for the initial small documents; revision never moves backwards.
#[derive(Default)]
pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    pub revision: u64,
}

impl History {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn commit(&mut self, current: &mut Document, next: Document) -> Result<bool, String> {
        if current.id != next.id || current.kind != next.kind {
            return Err("An edit cannot replace document identity or kind".into());
        }
        next.validate()?;
        if *current == next {
            return Ok(false);
        }
        self.undo.push(std::mem::replace(current, next));
        self.redo.clear();
        self.revision += 1;
        Ok(true)
    }
    pub fn undo(&mut self, doc: &mut Document) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(doc, previous));
            self.revision += 1;
        }
    }
    pub fn redo(&mut self, doc: &mut Document) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(doc, next));
            self.revision += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_edit_is_atomic_and_undo_keeps_monotonic_revision() {
        let mut doc = Document::example();
        let original = doc.clone();
        let mut history = History::default();
        let mut invalid = doc.clone();
        invalid.pads[0].size.x = -1;
        assert!(history.commit(&mut doc, invalid).is_err());
        assert_eq!(doc, original);
        assert_eq!(history.revision, 0);
        let mut next = doc.clone();
        next.pads[0].position.x += 1_000_000;
        history.commit(&mut doc, next.clone()).unwrap();
        history.undo(&mut doc);
        assert_eq!(doc, original);
        history.redo(&mut doc);
        assert_eq!(doc, next);
        assert_eq!(history.revision, 3);
        history.undo(&mut doc);
        let mut branch = doc.clone();
        branch.name = "Branch".into();
        history.commit(&mut doc, branch).unwrap();
        assert!(!history.can_redo());
    }
}
