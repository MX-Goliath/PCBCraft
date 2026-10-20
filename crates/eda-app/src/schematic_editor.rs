use super::*;
use eda_core::schematic::{SchPrimitiveKind as K, Schematic};
use eda_core::schematic_edit::{self as edit, Edit, Fragment, Placement, Request};
use eda_geom::Point;

const CLIPBOARD_PREFIX: &str = "PCBCraft schematic clipboard v1\n";

#[cfg(test)]
#[path = "schematic_editor_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Editor {
    tool: Tool,
    pub kind: Option<Placement>,
    pub value: String,
    pub designator: String,
    pointer: Option<Point>,
}

#[derive(Default)]
enum Tool {
    #[default]
    Select,
    Place {
        turns: u32,
        revision: u64,
    },
    Path {
        points: Vec<Point>,
        vertical_first: bool,
        revision: u64,
    },
    Move {
        fragment: Fragment,
        id: Option<Uuid>,
        anchor: Point,
        turns: u32,
        revision: u64,
        drag: bool,
    },
}

impl Editor {
    pub fn active(&self) -> bool {
        !matches!(self.tool, Tool::Select)
    }
    pub fn cancel(&mut self, view: &mut View) {
        self.tool = Tool::Select;
        self.kind = None;
        view.sch_preview = None;
        view.sch_tool_active = false;
    }
}

impl OpenDocument {
    pub(super) fn edit_schematic(&mut self, command: Edit, revision: u64) -> Result<bool, String> {
        let document = self.model.id;
        let result = self.history.schematic_edit(
            &mut self.model,
            Request {
                document,
                expected_revision: revision,
                edit: command,
            },
        )?;
        if let Some(id) = self.view.selected
            && self
                .model
                .schematic
                .as_ref()
                .and_then(|s| edit::anchor(s, id))
                .is_none()
        {
            self.view.selected = None;
        }
        if let Some(s) = &self.model.schematic {
            self.view
                .sch_selection
                .retain(|id| edit::anchor(s, *id).is_some());
        }
        Ok(result)
    }
}

fn next_designator(s: &Schematic, prefix: &str) -> String {
    (1..)
        .map(|n| format!("{prefix}{n}"))
        .find(|candidate| {
            !s.components
                .iter()
                .any(|c| c.designator.eq_ignore_ascii_case(candidate))
        })
        .unwrap()
}

impl PcbCraft {
    pub(super) fn schematic_place_menu(&mut self, ui: &mut egui::Ui) {
        for label in ["Bus", "Bus Entry"] {
            ui.add_enabled(false, egui::Button::new(label));
        }
        ui.menu_button("Part…", |ui| {
            for (label, kind) in [
                ("Resistor", Placement::Resistor),
                ("Capacitor", Placement::Capacitor),
            ] {
                self.action(ui, label, "", Command::SchPlace(kind), true);
            }
        });
        for (label, shortcut, kind) in [
            ("Power Port", "", Placement::Ground),
            ("Wire", "Ctrl+W", Placement::Wire),
            ("Net Label", "L", Placement::NetLabel),
        ] {
            self.action(ui, label, shortcut, Command::SchPlace(kind), true);
        }
        for label in [
            "Port",
            "Off Sheet Connector",
            "Sheet Symbol",
            "Sheet Entry",
            "Device Sheet Symbol",
            "Reuse Block…",
            "Harness",
            "Directives",
        ] {
            ui.add_enabled(false, egui::Button::new(label));
        }
        self.action(
            ui,
            "Text String",
            "T",
            Command::SchPlace(Placement::Text),
            true,
        );
        for label in ["Text Frame", "Note"] {
            ui.add_enabled(false, egui::Button::new(label));
        }
        ui.menu_button("Drawing Tools", |ui| {
            for label in ["Arc", "Full Circle", "Elliptical Arc", "Ellipse"] {
                ui.add_enabled(false, egui::Button::new(label));
            }
            self.action(ui, "Line", "", Command::SchPlace(Placement::Line), true);
            for label in [
                "Rectangle",
                "Round Rectangle",
                "Polygon",
                "Bezier",
                "Graphic…",
            ] {
                ui.add_enabled(false, egui::Button::new(label));
            }
        });
        ui.add_enabled(false, egui::Button::new("Comment…"));
        ui.separator();
        self.action(
            ui,
            "Junction",
            "J",
            Command::SchPlace(Placement::Junction),
            true,
        );
    }

    pub(super) fn start_schematic_tool(&mut self, kind: Placement) {
        let Some(i) = self.active else {
            return;
        };
        let doc = &mut self.documents[i];
        let Some(s) = &doc.model.schematic else {
            return;
        };
        doc.sch_editor.cancel(&mut doc.view);
        doc.sch_editor.kind = Some(kind);
        doc.sch_editor.value = match kind {
            Placement::Resistor => "10k",
            Placement::Capacitor => "100nF",
            Placement::Ground => "GND",
            Placement::NetLabel => "Net1",
            Placement::Text => "Text",
            _ => "",
        }
        .into();
        doc.sch_editor.designator = next_designator(
            s,
            if kind == Placement::Capacitor {
                "C"
            } else {
                "R"
            },
        );
        doc.sch_editor.tool = if matches!(kind, Placement::Wire | Placement::Line) {
            Tool::Path {
                points: vec![],
                vertical_first: false,
                revision: doc.history.revision,
            }
        } else {
            Tool::Place {
                turns: 0,
                revision: doc.history.revision,
            }
        };
        doc.view.sch_tool_active = true;
    }

    pub(super) fn schematic_action(&mut self, action: SchAction, ctx: &egui::Context) {
        let Some(i) = self.active else {
            return;
        };
        if self.documents[i].model.kind != DocumentKind::Schematic {
            return;
        }
        if self.documents[i].view.sch_selection.len() > 1
            && !matches!(action, SchAction::Select | SchAction::Paste)
        {
            self.message(
                true,
                "Editing multiple schematic objects is not supported yet",
            );
            return;
        }
        let doc = &mut self.documents[i];
        let selected = doc.view.selected;
        let revision = doc.history.revision;
        let result = (|| -> Result<(), String> {
            match action {
                SchAction::Select => doc.sch_editor.cancel(&mut doc.view),
                SchAction::Delete => {
                    doc.sch_editor.cancel(&mut doc.view);
                    if let Some(id) = selected {
                        doc.edit_schematic(Edit::Delete(id), revision)?;
                    }
                }
                SchAction::Rotate => {
                    if let Some(id) = selected {
                        let pivot = edit::anchor(doc.model.schematic.as_ref().unwrap(), id)
                            .ok_or("Nothing selected")?;
                        doc.edit_schematic(
                            Edit::Transform {
                                id,
                                pivot,
                                quarter_turns: 1,
                                delta: Point::default(),
                            },
                            revision,
                        )?;
                    }
                }
                SchAction::Move => {
                    let id = selected.ok_or("Select an object to move")?;
                    let s = doc.model.schematic.as_ref().unwrap();
                    doc.sch_editor.tool = Tool::Move {
                        fragment: edit::extract(s, id)?,
                        id: Some(id),
                        anchor: edit::anchor(s, id).unwrap(),
                        turns: 0,
                        revision,
                        drag: false,
                    };
                    doc.view.sch_tool_active = true;
                }
                SchAction::Copy | SchAction::Cut | SchAction::Duplicate => {
                    doc.sch_editor.cancel(&mut doc.view);
                    let id = selected.ok_or("Select an object to copy")?;
                    let s = doc.model.schematic.as_ref().unwrap();
                    self.sch_clipboard =
                        Some((edit::extract(s, id)?, edit::anchor(s, id).unwrap()));
                    if matches!(action, SchAction::Cut) {
                        doc.edit_schematic(Edit::Delete(id), revision)?;
                    }
                }
                SchAction::Paste => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.message(true, e);
            return;
        }
        if matches!(action, SchAction::Copy | SchAction::Cut)
            && let Some(fragment) = &self.sch_clipboard
            && let Ok(json) = serde_json::to_string(fragment)
        {
            ctx.copy_text(format!("{CLIPBOARD_PREFIX}{json}"));
        }
        if matches!(action, SchAction::Paste | SchAction::Duplicate)
            && let Some((fragment, anchor)) = &self.sch_clipboard
        {
            let doc = &mut self.documents[i];
            let mut fragment = fragment.fresh_copy();
            for c in &mut fragment.components {
                let prefix = c.designator.trim_end_matches(|c: char| c.is_ascii_digit());
                let new_name = next_designator(
                    doc.model.schematic.as_ref().unwrap(),
                    if prefix.is_empty() { "U" } else { prefix },
                );
                let old = c.designator.clone();
                c.designator = new_name.clone();
                for p in &mut fragment.primitives {
                    if p.component == Some(c.id)
                        && let K::Text { text, .. } = &mut p.kind
                        && (p.text_role == Some(eda_core::schematic::TextRole::Designator)
                            || *text == old)
                    {
                        *text = new_name.clone();
                        p.presentation.raw_text = Some(new_name.clone());
                    }
                }
            }
            doc.sch_editor.kind = None;
            doc.sch_editor.tool = Tool::Move {
                fragment,
                id: None,
                anchor: *anchor,
                turns: 0,
                revision: doc.history.revision,
                drag: false,
            };
            doc.view.sch_tool_active = true;
        }
    }

    pub(super) fn schematic_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        let Some(i) = self.active else {
            return false;
        };
        if self.documents[i].model.kind != DocumentKind::Schematic {
            return false;
        }
        let clipboard_event = ctx.input_mut(|input| {
            let index = input.events.iter().position(|e| {
                matches!(
                    e,
                    egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                )
            })?;
            Some(input.events.remove(index))
        });
        if let Some(event) = clipboard_event {
            let action = match event {
                egui::Event::Copy => SchAction::Copy,
                egui::Event::Cut => SchAction::Cut,
                egui::Event::Paste(text) => {
                    if let Some(json) = text
                        .strip_prefix(CLIPBOARD_PREFIX)
                        .filter(|s| s.len() <= 16 * 1024 * 1024)
                    {
                        match serde_json::from_str::<(Fragment, Point)>(json) {
                            Ok(fragment) => self.sch_clipboard = Some(fragment),
                            Err(e) => {
                                self.message(true, format!("Invalid schematic clipboard: {e}"));
                                return true;
                            }
                        }
                    } else {
                        self.message(
                            true,
                            "Clipboard does not contain a PCBCraft schematic fragment",
                        );
                        return true;
                    }
                    SchAction::Paste
                }
                _ => unreachable!(),
            };
            self.schematic_action(action, ctx);
            return true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            let doc = &mut self.documents[i];
            if doc.sch_editor.active() {
                doc.sch_editor.cancel(&mut doc.view);
            } else {
                doc.view.selected = None;
                doc.view.sch_selection.clear();
            }
            return true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
            match &mut self.documents[i].sch_editor.tool {
                Tool::Place { turns, .. } | Tool::Move { turns, .. } => *turns = (*turns + 1) % 4,
                Tool::Path { vertical_first, .. } => *vertical_first = !*vertical_first,
                Tool::Select => self.schematic_action(SchAction::Rotate, ctx),
            }
            return true;
        }
        for (mods, key, action) in [
            (egui::Modifiers::CTRL, Key::C, SchAction::Copy),
            (egui::Modifiers::CTRL, Key::X, SchAction::Cut),
            (egui::Modifiers::CTRL, Key::V, SchAction::Paste),
            (egui::Modifiers::CTRL, Key::D, SchAction::Duplicate),
            (egui::Modifiers::NONE, Key::Delete, SchAction::Delete),
            (egui::Modifiers::NONE, Key::M, SchAction::Move),
        ] {
            if ctx.input_mut(|i| i.consume_key(mods, key)) {
                self.schematic_action(action, ctx);
                return true;
            }
        }
        for (key, kind) in [
            (Key::W, Placement::Wire),
            (Key::T, Placement::Text),
            (Key::L, Placement::NetLabel),
            (Key::J, Placement::Junction),
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                self.start_schematic_tool(kind);
                return true;
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::W)) {
            self.start_schematic_tool(Placement::Wire);
            return true;
        }
        false
    }

    pub(super) fn schematic_input(
        &mut self,
        index: usize,
        input: eda_render::SchematicInput,
        ctx: &egui::Context,
    ) {
        let doc = &mut self.documents[index];
        let Some(s) = doc.model.schematic.as_ref() else {
            return;
        };
        let step = s.sheet.snap_nm;
        let radius = ((8.0 / doc.view.camera.pixels_per_mm * 1e6) as i64)
            .min(s.sheet.options.snap_distance_nm);
        let snap = |p| edit::snap_point(s, p, step, radius);
        let point = input.point.map(snap);
        if point.is_some() {
            doc.sch_editor.pointer = point;
        }
        let mut command = None;
        let mut preview = None;
        let mut finish = false;
        let mut error = None;
        if !doc.sch_editor.active()
            && doc.view.sch_selection.len() <= 1
            && (input.drag_started || input.drag_stopped)
            && let (Some(id), Some(origin)) = (input.hit, input.press_origin)
        {
            match edit::extract(s, id) {
                Ok(fragment) => {
                    doc.view.selected = Some(id);
                    doc.sch_editor.tool = Tool::Move {
                        fragment,
                        id: Some(id),
                        anchor: snap(origin),
                        turns: 0,
                        revision: doc.history.revision,
                        drag: true,
                    };
                    doc.view.sch_tool_active = true;
                }
                Err(e) => error = Some(e),
            }
        }
        let kind = doc.sch_editor.kind;
        match &mut doc.sch_editor.tool {
            Tool::Select => {}
            Tool::Place { turns, revision } => {
                if let Some(at) = point {
                    match edit::place(
                        kind.unwrap(),
                        at,
                        *turns,
                        &doc.sch_editor.designator,
                        &doc.sch_editor.value,
                        s.sheet.system_font,
                    ) {
                        Ok(fragment) => {
                            if input.clicked {
                                command = Some((Edit::Insert(fragment.clone()), *revision));
                            }
                            preview = Some(fragment);
                        }
                        Err(e) => error = Some(e),
                    }
                }
                finish = input.secondary_clicked;
            }
            Tool::Path {
                points,
                vertical_first,
                revision,
            } => {
                let mut path = points.clone();
                if let Some(at) = point {
                    extend_path(
                        &mut path,
                        at,
                        *vertical_first,
                        kind == Some(Placement::Wire),
                    );
                    if input.clicked {
                        *points = path.clone();
                    }
                }
                let end = input.secondary_clicked
                    || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                if end {
                    // Finish at the last confirmed vertex; moving the pointer is only preview.
                    if points.len() > 1 {
                        let p = edit::primitive(
                            K::Polyline {
                                points: points.clone(),
                                closed: false,
                                electrical: if kind == Some(Placement::Wire) {
                                    "Wire"
                                } else {
                                    "Graphic"
                                }
                                .into(),
                            },
                            None,
                        );
                        command = Some((
                            Edit::Insert(Fragment {
                                primitives: vec![p],
                                ..Default::default()
                            }),
                            *revision,
                        ));
                    }
                    finish = true;
                }
                if path.len() > 1 {
                    preview = Some(Fragment {
                        primitives: vec![edit::primitive(
                            K::Polyline {
                                points: path,
                                closed: false,
                                electrical: "Wire".into(),
                            },
                            None,
                        )],
                        ..Default::default()
                    });
                }
            }
            Tool::Move {
                fragment,
                id,
                anchor,
                turns,
                revision,
                drag,
            } => {
                if let Some(at) = point.or(doc.sch_editor.pointer) {
                    let delta = Point {
                        x: at.x - anchor.x,
                        y: at.y - anchor.y,
                    };
                    match fragment.transformed(*anchor, *turns, delta) {
                        Ok(f) => {
                            if (input.clicked && !*drag) || (input.drag_stopped && *drag) {
                                command = Some((
                                    if let Some(id) = id {
                                        Edit::Transform {
                                            id: *id,
                                            pivot: *anchor,
                                            quarter_turns: *turns,
                                            delta,
                                        }
                                    } else {
                                        Edit::Insert(f.clone())
                                    },
                                    *revision,
                                ));
                                finish = true;
                            }
                            preview = Some(f);
                        }
                        Err(e) => {
                            error = Some(e);
                            if input.clicked || input.drag_stopped {
                                finish = true;
                            }
                        }
                    }
                }
                if input.secondary_clicked {
                    finish = true;
                }
            }
        }
        if let Some((command, revision)) = command {
            let is_place = matches!(doc.sch_editor.tool, Tool::Place { .. });
            match doc.edit_schematic(command, revision) {
                Ok(_) => {
                    if is_place {
                        if let Tool::Place { revision, .. } = &mut doc.sch_editor.tool {
                            *revision = doc.history.revision;
                        }
                        if matches!(kind, Some(Placement::Resistor | Placement::Capacitor)) {
                            doc.sch_editor.designator = next_designator(
                                doc.model.schematic.as_ref().unwrap(),
                                if kind == Some(Placement::Capacitor) {
                                    "C"
                                } else {
                                    "R"
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    error = Some(e);
                    finish = true;
                }
            }
        }
        if finish {
            doc.sch_editor.cancel(&mut doc.view);
        } else {
            doc.view.sch_preview = preview.map(|fragment| {
                let mut s = Schematic::blank();
                s.fonts = if fragment.fonts.is_empty() {
                    doc.model.schematic.as_ref().unwrap().fonts.clone()
                } else {
                    fragment.fonts
                };
                s.images = fragment.images;
                s.primitives = fragment.primitives;
                for p in &mut s.primitives {
                    p.color = 0xc08000;
                }
                s
            });
        }
        if let Some(e) = error {
            self.message(true, e);
        }
        if self.documents[index].sch_editor.active() {
            ctx.request_repaint();
        }
    }
}

pub(super) fn extend_path(
    points: &mut Vec<Point>,
    at: Point,
    vertical_first: bool,
    orthogonal: bool,
) {
    if let Some(last) = points.last().copied() {
        if last == at {
            return;
        }
        if orthogonal && last.x != at.x && last.y != at.y {
            points.push(if vertical_first {
                Point { x: last.x, y: at.y }
            } else {
                Point { x: at.x, y: last.y }
            });
        }
    }
    points.push(at);
}
