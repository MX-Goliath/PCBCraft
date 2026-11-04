//! GUI-independent document model and history.
pub mod assembly_variants;
pub mod cross_probe;
pub mod design_sync;
pub mod library;
pub mod library_edit;
pub mod pcb;
pub mod pcb_3d;
pub mod pcb_arrange;
pub mod pcb_connectivity;
pub mod pcb_polygon;
pub mod pcb_route;
pub mod pcb_rules;
pub mod pcb_selection;
pub mod pcb_tools;
pub mod schematic;
pub mod schematic_arrange;
pub mod schematic_bus;
pub mod schematic_compile;
pub mod schematic_directives;
pub mod schematic_edit;
pub mod schematic_harness;
pub mod schematic_script;
pub mod schematic_simulation;
pub mod schematic_sync;
pub mod schematic_validation;
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
    /// The active library member uses the normal editable scene; other members
    /// live here. There is never a second copy of the active geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<library_edit::Book>,
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub kind: DocumentKind,
    pub size: Point,
    pub pads: Vec<Pad>,
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schematic: Option<schematic::Schematic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcb: Option<pcb::Board>,
}

impl Document {
    pub fn new(kind: DocumentKind, name: impl Into<String>) -> Self {
        Self {
            library: None,
            schema_version: 1,
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            size: match kind {
                DocumentKind::Board => Point::mm(80, 50),
                DocumentKind::Schematic => schematic::standard_sheet_size(0, false).unwrap(),
            },
            pads: vec![],
            tracks: vec![],
            schematic: (kind == DocumentKind::Schematic).then(schematic::Schematic::blank),
            pcb: None,
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
        if let Some(book) = &self.library {
            library_edit::validate(self, book)?;
        }
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
        if let Some(board) = &self.pcb {
            if self.kind != DocumentKind::Board || self.schematic.is_some() {
                return Err("PCB data requires a board document without schematic data".into());
            }
            if !self.pads.is_empty() || !self.tracks.is_empty() {
                return Err("A PCB scene cannot coexist with legacy pad/track arrays".into());
            }
            board.validate()?;
        }
        let mut ids = std::collections::HashSet::new();
        if let Some(schematic) = &self.schematic {
            if self.kind != DocumentKind::Schematic {
                return Err("Schematic data requires a schematic document".into());
            }
            schematic.validate()?;
        }
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

mod history;
pub use history::History;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schematic_standard_formats_keep_native_dimensions_orientation_and_history() {
        use schematic_edit::{Edit, Request};
        let mut doc = Document::new(DocumentKind::Schematic, "Sheet");
        assert_eq!(
            doc.size,
            Point {
                x: 292_100_000,
                y: 193_040_000
            }
        );
        assert_eq!(doc.schematic.as_ref().unwrap().sheet.format_name(), "A4");
        let original = doc.clone();
        let mut history = History::default();
        for portrait in [false, true] {
            for (index, (name, width, height)) in schematic::STANDARD_SHEETS.into_iter().enumerate()
            {
                let mut sheet = doc.schematic.as_ref().unwrap().sheet.clone();
                let mut size = if portrait {
                    Point::mm(100, 200)
                } else {
                    Point::mm(200, 100)
                };
                assert!(sheet.set_standard_format(&mut size, index));
                assert_eq!(sheet.format_name(), name);
                assert_eq!(
                    size,
                    if portrait {
                        Point {
                            x: height,
                            y: width,
                        }
                    } else {
                        Point {
                            x: width,
                            y: height,
                        }
                    }
                );
                let before = doc.clone();
                let changed = history
                    .schematic_edit(
                        &mut doc,
                        Request {
                            document: original.id,
                            expected_revision: history.revision,
                            edit: Edit::SheetProperties {
                                sheet: Box::new(sheet),
                                size,
                            },
                        },
                    )
                    .unwrap();
                assert_eq!(doc.size, size);
                if changed {
                    let after = doc.clone();
                    history.undo(&mut doc);
                    assert_eq!(doc, before);
                    history.redo(&mut doc);
                    assert_eq!(doc, after);
                }
            }
        }
        let mut sheet = doc.schematic.as_ref().unwrap().sheet.clone();
        let before = sheet.clone();
        let mut size = doc.size;
        assert!(!sheet.set_standard_format(&mut size, usize::MAX));
        assert_eq!(sheet, before);
        assert_eq!(size, doc.size);
    }

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

pub mod layer_stack;
mod layer_stack_edit;

pub mod annotation;
