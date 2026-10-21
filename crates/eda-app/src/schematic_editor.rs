use super::*;
use eda_core::schematic::{SchPrimitiveKind as K, Schematic};
use eda_core::schematic_edit::{self as edit, Edit, Fragment, Placement, Request, Selection};
use eda_geom::Point;

const CLIPBOARD_PREFIX: &str = "PCBCraft schematic clipboard v1\n";

#[cfg(test)]
#[path = "schematic_editor_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Editor {
    tool: Tool,
    move_popup: Option<egui::Pos2>,
    pub kind: Option<Placement>,
    pub value: String,
    pub designator: String,
    pointer: Option<Point>,
}

#[derive(Default)]
enum Tool {
    #[default]
    Select,
    Pick {
        connected: bool,
    },
    WireDrag {
        id: Uuid,
        handle: edit::WireHandle,
        anchor: Point,
        revision: u64,
        drag: bool,
    },
    Library {
        drag: bool,
        symbol: Box<eda_core::library::Symbol>,
        part: u32,
        display_mode: u32,
        turns: u32,
        revision: u64,
    },
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
        selection: Selection,
        connected: bool,
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
        self.move_popup = None;
        self.kind = None;
        view.sch_preview = None;
        view.sch_tool_active = false;
        view.cancel_schematic_gesture();
    }
}

impl OpenDocument {
    pub(super) fn edit_schematic(&mut self, command: Edit, revision: u64) -> Result<bool, String> {
        let partial_primary = self
            .view
            .selected
            .is_some_and(|id| self.view.sch_segments.iter().any(|span| span.id == id));
        let transformed_spans = if let Edit::TransformSelection {
            selection,
            pivot,
            quarter_turns,
            delta,
            ..
        } = &command
        {
            Some(
                selection
                    .segments
                    .iter()
                    .map(|span| {
                        Ok(edit::WireSegment {
                            id: span.id,
                            a: edit::transform_point(span.a, *pivot, *quarter_turns, *delta)?,
                            b: edit::transform_point(span.b, *pivot, *quarter_turns, *delta)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            )
        } else {
            None
        };
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
            if partial_primary {
                self.view.selected = None;
            }
            if let Some(spans) = transformed_spans {
                self.view.sch_segments=spans.into_iter().filter_map(|mut span| {
                    span.id=s.primitives.iter().find(|p|p.component.is_none()&&matches!(&p.kind,K::Polyline {points,electrical,..} if electrical=="Wire"&&points.windows(2).any(|e|span.a.on_segment(e[0],e[1])&&span.b.on_segment(e[0],e[1]))))?.id;
                    Some(span)
                }).collect();
            }
            self.view
                .sch_selection
                .retain(|id| edit::anchor(s, *id).is_some());
            self.view.sch_segments.retain(|span| {
                edit::extract_selection(
                    s,
                    &Selection {
                        ids: vec![],
                        segments: vec![*span],
                    },
                )
                .is_ok()
            });
            self.view.selected = self
                .view
                .selected
                .or_else(|| self.view.sch_selection.first().copied())
                .or_else(|| self.view.sch_segments.first().map(|span| span.id));
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
    pub(super) fn finish_schematic_interaction(&mut self) {
        for (i, doc) in self.documents.iter_mut().enumerate() {
            if self.active != Some(i)
                && (matches!(
                    doc.sch_editor.tool,
                    Tool::Move { .. }
                        | Tool::WireDrag { .. }
                        | Tool::Pick { .. }
                        | Tool::Path { .. }
                ) || doc.sch_editor.move_popup.is_some())
            {
                doc.sch_editor.cancel(&mut doc.view);
            }
        }
    }
    pub(super) fn move_menu_entries(&mut self, ui: &mut egui::Ui) {
        let can_drag = self.active.is_some();
        self.action(
            ui,
            "Drag",
            "M, D",
            Command::SchEdit(SchAction::Drag),
            can_drag,
        );
        self.action(ui, "Move", "M, M", Command::SchEdit(SchAction::Move), true);
        let selected = self
            .active
            .is_some_and(|i| self.documents[i].view.selected.is_some());
        self.action(
            ui,
            "Move Selection",
            "",
            Command::SchEdit(SchAction::Move),
            selected,
        );
        ui.add_enabled(false, egui::Button::new("Move Selection By X, Y…"));
        self.action(
            ui,
            "Drag Selection",
            "",
            Command::SchEdit(SchAction::Drag),
            selected && can_drag,
        );
        ui.add_enabled(false, egui::Button::new("Move To Front"));
        self.action(
            ui,
            "Rotate Selection",
            "Space",
            Command::SchEdit(SchAction::Rotate),
            selected,
        );
    }
    pub(super) fn schematic_move_menu(&mut self, ctx: &egui::Context) {
        let Some(i) = self.active else {
            return;
        };
        let Some(at) = self.documents[i].sch_editor.move_popup else {
            return;
        };
        let shown = egui::Area::new(egui::Id::new("schematic_move_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(250.0);
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                        self.move_menu_entries(ui)
                    });
                });
            });
        if self.command.is_some()
            || ctx.input(|input| {
                input.pointer.any_pressed()
                    && input
                        .pointer
                        .latest_pos()
                        .is_some_and(|p| !shown.response.rect.contains(p))
            })
        {
            self.documents[i].sch_editor.move_popup = None;
        }
    }
    pub(super) fn start_library_tool(
        &mut self,
        symbol: eda_core::library::Symbol,
        part: u32,
        display_mode: u32,
    ) {
        let Some(index) = self.active else {
            return;
        };
        let doc = &mut self.documents[index];
        if doc.model.kind != DocumentKind::Schematic {
            return;
        }
        doc.sch_editor.cancel(&mut doc.view);
        doc.sch_editor.tool = Tool::Library {
            drag: false,
            symbol: Box::new(symbol),
            part,
            display_mode,
            turns: 0,
            revision: doc.history.revision,
        };
        doc.view.sch_tool_active = true;
        doc.view.selected = None;
        doc.view.sch_selection.clear();
        doc.view.sch_segments.clear();
    }
    pub(super) fn start_library_drag(
        &mut self,
        symbol: eda_core::library::Symbol,
        part: u32,
        mode: u32,
        ctx: &egui::Context,
    ) {
        self.start_library_tool(symbol, part, mode);
        if let Some(i) = self.active
            && let Tool::Library { drag, .. } = &mut self.documents[i].sch_editor.tool
        {
            *drag = true;
            let target = self.documents[i].session_id;
            self.components_ui.drag_document = Some(target);
            egui::DragAndDrop::set_payload(ctx, target);
        }
    }
    pub(super) fn finish_library_drag(&mut self, ctx: &egui::Context) {
        if let Some(target) = self.components_ui.drag_document {
            let active = self
                .active
                .is_some_and(|i| self.documents[i].session_id == target);
            let cancelled = ctx
                .input(|i| !i.focused || i.key_pressed(Key::Escape) || !i.pointer.primary_down())
                || egui::DragAndDrop::payload::<Uuid>(ctx).is_none();
            if !active || cancelled {
                if let Some(doc) = self.documents.iter_mut().find(|d| d.session_id == target) {
                    doc.sch_editor.cancel(&mut doc.view);
                }
                self.components_ui.drag_document = None;
                egui::DragAndDrop::clear_payload(ctx);
                ctx.request_repaint();
            }
        }
    }
    pub(super) fn schematic_place_menu(&mut self, ui: &mut egui::Ui) {
        for label in ["Bus", "Bus Entry"] {
            ui.add_enabled(false, egui::Button::new(label));
        }
        ui.menu_button("Part…", |ui| {
            self.action(ui, "Components", "", Command::ShowComponents, true);
            ui.separator();
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
        let doc = &mut self.documents[i];
        let selected = doc.view.selected;
        let selection = view_selection(&doc.view);
        let revision = doc.history.revision;
        let result = (|| -> Result<(), String> {
            match action {
                SchAction::Select => doc.sch_editor.cancel(&mut doc.view),
                SchAction::Delete => {
                    doc.sch_editor.cancel(&mut doc.view);
                    if !selection.is_empty() {
                        doc.edit_schematic(Edit::DeleteSelection(selection.clone()), revision)?;
                    }
                }
                SchAction::Rotate => {
                    if !selection.is_empty() {
                        let pivot = edit::selection_anchor(
                            doc.model.schematic.as_ref().unwrap(),
                            &selection,
                        )
                        .ok_or("Nothing selected")?;
                        doc.edit_schematic(
                            Edit::TransformSelection {
                                selection: selection.clone(),
                                pivot,
                                quarter_turns: 1,
                                delta: Point::default(),
                                connected: false,
                            },
                            revision,
                        )?;
                    }
                }
                SchAction::Move | SchAction::Drag => {
                    let connected = matches!(action, SchAction::Drag);
                    doc.sch_editor.cancel(&mut doc.view);
                    let Some(id) = selected else {
                        doc.sch_editor.tool = Tool::Pick { connected };
                        doc.view.sch_tool_active = true;
                        return Ok(());
                    };
                    let s = doc.model.schematic.as_ref().unwrap();
                    if connected
                        && selection.ids.len() == 1
                        && selection.segments.is_empty()
                        && is_wire(s, id)
                    {
                        doc.sch_editor.tool = Tool::Pick { connected: true };
                        doc.view.sch_tool_active = true;
                        return Ok(());
                    }
                    let anchor = edit::selection_anchor(s, &selection).ok_or("Nothing selected")?;
                    doc.sch_editor.tool = Tool::Move {
                        fragment: edit::extract_selection(s, &selection)?,
                        selection: selection.clone(),
                        connected,
                        anchor,
                        turns: 0,
                        revision,
                        drag: false,
                    };
                    doc.view.sch_tool_active = true;
                }
                SchAction::Copy | SchAction::Cut | SchAction::Duplicate => {
                    doc.sch_editor.cancel(&mut doc.view);
                    let s = doc.model.schematic.as_ref().unwrap();
                    self.sch_clipboard = Some((
                        edit::extract_selection(s, &selection)?,
                        edit::selection_anchor(s, &selection).ok_or("Select objects to copy")?,
                    ));
                    if matches!(action, SchAction::Cut) {
                        doc.edit_schematic(Edit::DeleteSelection(selection.clone()), revision)?;
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
            let mut reserved: std::collections::BTreeSet<_> = doc
                .model
                .schematic
                .as_ref()
                .unwrap()
                .components
                .iter()
                .map(|c| c.designator.to_ascii_uppercase())
                .collect();
            for c in &mut fragment.components {
                let prefix = c.designator.trim_end_matches(|c: char| c.is_ascii_digit());
                let prefix = if prefix.is_empty() { "U" } else { prefix };
                let new_name = (1..)
                    .map(|n| format!("{prefix}{n}"))
                    .find(|name| reserved.insert(name.to_ascii_uppercase()))
                    .unwrap();
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
                selection: Selection::default(),
                connected: false,
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
        if self.documents[i].sch_editor.move_popup.is_some() {
            for (key, action) in [(Key::D, SchAction::Drag), (Key::M, SchAction::Move)] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    self.documents[i].sch_editor.move_popup = None;
                    self.schematic_action(action, ctx);
                    return true;
                }
            }
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::M)) {
            self.documents[i].sch_editor.move_popup =
                Some(ctx.pointer_latest_pos().unwrap_or(egui::pos2(400.0, 200.0)));
            return true;
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
            if doc.sch_editor.active() || doc.sch_editor.move_popup.is_some() {
                doc.sch_editor.cancel(&mut doc.view);
            } else {
                doc.view.selected = None;
                doc.view.sch_selection.clear();
                doc.view.sch_segments.clear();
            }
            return true;
        }
        if matches!(self.documents[i].sch_editor.tool, Tool::Path { .. })
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::Backspace)
                    || i.consume_key(egui::Modifiers::NONE, Key::Delete)
            })
        {
            if let Tool::Path { points, .. } = &mut self.documents[i].sch_editor.tool {
                points.pop();
            }
            return true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
            match &mut self.documents[i].sch_editor.tool {
                Tool::Place { turns, .. }
                | Tool::Move { turns, .. }
                | Tool::Library { turns, .. } => *turns = (*turns + 1) % 4,
                Tool::Path { vertical_first, .. } => *vertical_first = !*vertical_first,
                Tool::Select => self.schematic_action(SchAction::Rotate, ctx),
                Tool::Pick { .. } | Tool::WireDrag { .. } => {}
            }
            return true;
        }
        for (mods, key, action) in [
            (egui::Modifiers::CTRL, Key::C, SchAction::Copy),
            (egui::Modifiers::CTRL, Key::X, SchAction::Cut),
            (egui::Modifiers::CTRL, Key::V, SchAction::Paste),
            (egui::Modifiers::CTRL, Key::D, SchAction::Duplicate),
            (egui::Modifiers::NONE, Key::Delete, SchAction::Delete),
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
        if ctx.input(|i| !i.focused || i.key_down(Key::Escape)) {
            if matches!(
                doc.sch_editor.tool,
                Tool::Move { .. } | Tool::WireDrag { .. } | Tool::Pick { .. } | Tool::Path { .. }
            ) {
                doc.sch_editor.cancel(&mut doc.view);
            }
            return;
        }
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
        let mut repeat_path = false;
        let mut error = None;
        if let Tool::Pick { connected } = doc.sch_editor.tool {
            if input.secondary_clicked {
                doc.sch_editor.cancel(&mut doc.view);
            } else if input.clicked
                && let (Some(id), Some(at)) = (input.hit, point)
            {
                doc.view.selected = Some(id);
                doc.view.sch_selection.clear();
                doc.view.sch_segments.clear();
                if let Ok(fragment) = edit::extract(s, id) {
                    doc.sch_editor.tool = if connected && is_wire(s, id) {
                        let handle = input.wire_handle.or_else(|| {
                            edit::wire_handle_at(s, id, input.point.unwrap_or(at), radius)
                        });
                        let Some(handle) = handle else {
                            return;
                        };
                        Tool::WireDrag {
                            id,
                            handle,
                            anchor: at,
                            revision: doc.history.revision,
                            drag: false,
                        }
                    } else {
                        Tool::Move {
                            fragment,
                            selection: Selection {
                                ids: vec![id],
                                segments: vec![],
                            },
                            connected,
                            anchor: at,
                            turns: 0,
                            revision: doc.history.revision,
                            drag: false,
                        }
                    };
                }
            }
            return;
        }
        if !doc.sch_editor.active()
            && !ctx.input(|i| !i.focused || i.key_down(Key::Escape))
            && (input.drag_started || input.drag_stopped)
            && let (Some(id), Some(origin)) = (input.hit, input.press_origin)
        {
            let selection = if doc.view.sch_selection.contains(&id)
                || doc.view.sch_segments.iter().any(|span| span.id == id)
            {
                view_selection(&doc.view)
            } else {
                doc.view.sch_selection = vec![id];
                doc.view.sch_segments.clear();
                Selection {
                    ids: vec![id],
                    segments: vec![],
                }
            };
            match edit::extract_selection(s, &selection) {
                Ok(fragment) => {
                    doc.view.selected = Some(id);
                    doc.sch_editor.tool = if selection.ids.len() == 1
                        && selection.segments.is_empty()
                        && is_wire(s, id)
                    {
                        let Some(handle) = input
                            .wire_handle
                            .or_else(|| edit::wire_handle_at(s, id, origin, radius))
                        else {
                            return;
                        };
                        Tool::WireDrag {
                            id,
                            handle,
                            anchor: snap(origin),
                            revision: doc.history.revision,
                            drag: true,
                        }
                    } else {
                        Tool::Move {
                            fragment,
                            selection,
                            connected: true,
                            anchor: snap(origin),
                            turns: 0,
                            revision: doc.history.revision,
                            drag: true,
                        }
                    };
                    doc.view.sch_tool_active = true;
                }
                Err(e) => error = Some(e),
            }
        }
        let kind = doc.sch_editor.kind;
        match &mut doc.sch_editor.tool {
            Tool::Select | Tool::Pick { .. } => {}
            Tool::WireDrag {
                id,
                handle,
                anchor,
                revision,
                drag,
            } => {
                let at = if *drag {
                    input.drop_point.map(snap).or(point)
                } else {
                    point.or(doc.sch_editor.pointer)
                };
                if let Some(at) = at {
                    let delta = Point {
                        x: at.x - anchor.x,
                        y: at.y - anchor.y,
                    };
                    let edit = Edit::DragWire {
                        id: *id,
                        handle: *handle,
                        delta,
                    };
                    match edit::preview_fragment(&doc.model, &edit) {
                        Ok(fragment) => {
                            preview = Some(fragment);
                            if (input.clicked && !*drag) || (input.drag_stopped && *drag) {
                                command = Some((
                                    Edit::DragWire {
                                        id: *id,
                                        handle: *handle,
                                        delta,
                                    },
                                    *revision,
                                ));
                                finish = true;
                            }
                        }
                        Err(e) => {
                            error = Some(e);
                            finish = true;
                        }
                    }
                }
                finish |= input.secondary_clicked || (*drag && input.drag_stopped);
            }
            Tool::Library {
                drag,
                symbol,
                part,
                display_mode,
                turns,
                revision,
            } => {
                let at = if *drag {
                    input.drop_point.map(snap)
                } else {
                    point
                };
                if let Some(at) = at {
                    match symbol
                        .instantiate(*part, *display_mode, s)
                        .and_then(|f| f.transformed(Point::default(), *turns, at))
                    {
                        Ok(fragment) => {
                            if if *drag {
                                ctx.input(|i| {
                                    i.pointer.primary_released()
                                        && i.focused
                                        && !i.key_pressed(Key::Escape)
                                }) && egui::DragAndDrop::payload::<Uuid>(ctx)
                                    .is_some_and(|id| *id == doc.session_id)
                            } else {
                                input.clicked
                            } {
                                command = Some((Edit::Insert(fragment.clone()), *revision));
                                finish = *drag;
                            }
                            preview = Some(fragment);
                        }
                        Err(e) => {
                            error = Some(e);
                            finish = true;
                        }
                    }
                }
                finish |= input.secondary_clicked;
                if *drag && ctx.input(|i| !i.pointer.primary_down() || !i.focused) {
                    finish = true;
                }
            }
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
                repeat_path = kind == Some(Placement::Wire)
                    && input.clicked
                    && points.len() > 1
                    && point.is_some_and(|at| edit::connection_at(s, at));
                let end = repeat_path
                    || input.secondary_clicked
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
                    finish = !repeat_path;
                }
                if path.len() > 1 {
                    let fragment = Fragment {
                        primitives: vec![edit::primitive(
                            K::Polyline {
                                points: path,
                                closed: false,
                                electrical: if kind == Some(Placement::Wire) {
                                    "Wire"
                                } else {
                                    "Graphic"
                                }
                                .into(),
                            },
                            None,
                        )],
                        ..Default::default()
                    };
                    match edit::preview_fragment(&doc.model, &Edit::Insert(fragment)) {
                        Ok(f) => preview = Some(f),
                        Err(e) => error = Some(e),
                    }
                }
            }
            Tool::Move {
                fragment,
                selection,
                connected,
                anchor,
                turns,
                revision,
                drag,
            } => {
                let at = if *drag {
                    input.drop_point.map(snap).or(point)
                } else {
                    point.or(doc.sch_editor.pointer)
                };
                let cancelled = ctx.input(|i| !i.focused || i.key_down(Key::Escape));
                if let Some(at) = at.filter(|_| !cancelled) {
                    let delta = Point {
                        x: at.x - anchor.x,
                        y: at.y - anchor.y,
                    };
                    let edit = if selection.is_empty() {
                        fragment
                            .transformed(*anchor, *turns, delta)
                            .map(Edit::Insert)
                    } else {
                        Ok(Edit::TransformSelection {
                            selection: selection.clone(),
                            pivot: *anchor,
                            quarter_turns: *turns,
                            delta,
                            connected: *connected,
                        })
                    };
                    match edit.and_then(|edit| {
                        edit::preview_fragment(&doc.model, &edit).map(|f| (edit, f))
                    }) {
                        Ok((edit, f)) => {
                            if (input.clicked && !*drag) || (input.drag_stopped && *drag) {
                                command = Some((edit, *revision));
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
                finish |= cancelled || (*drag && input.drag_stopped);
            }
        }
        if let Some((command, revision)) = command {
            let is_place = matches!(
                doc.sch_editor.tool,
                Tool::Place { .. } | Tool::Library { .. }
            );
            match doc.edit_schematic(command, revision) {
                Ok(_) => {
                    if repeat_path {
                        if let Tool::Path {
                            points, revision, ..
                        } = &mut doc.sch_editor.tool
                        {
                            points.clear();
                            *revision = doc.history.revision;
                        }
                        preview = None;
                    }
                    if is_place {
                        if let Tool::Place { revision, .. } | Tool::Library { revision, .. } =
                            &mut doc.sch_editor.tool
                        {
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

fn view_selection(view: &View) -> Selection {
    let mut ids = view.sch_selection.clone();
    if ids.is_empty() && view.sch_segments.is_empty() {
        ids.extend(view.selected);
    }
    ids.retain(|id| !view.sch_segments.iter().any(|span| span.id == *id));
    Selection {
        ids,
        segments: view.sch_segments.clone(),
    }
}

fn is_wire(s: &Schematic, id: Uuid) -> bool {
    s.primitive(id).is_some_and(|p| p.component.is_none() && matches!(&p.kind, K::Polyline { electrical, closed: false, .. } if electrical == "Wire"))
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
