use super::*;
use eda_core::schematic::{SchPrimitiveKind as K, Schematic};
use eda_core::schematic_edit::{self as edit, Edit, Fragment, Placement, Request, Selection};
use eda_geom::Point;

const CLIPBOARD_PREFIX: &str = "PCBCraft schematic clipboard v1\n";
#[path = "schematic_harness_ui.rs"]
mod harness_ui;

fn place_sheet_entry(s: &Schematic, at: Point, name: &str) -> Result<Fragment, String> {
    let p=s.primitives.iter().find(|p|matches!(p.kind,K::SheetSymbol{a,b,..} if (a.x..=b.x).contains(&at.x)&&(b.y..=a.y).contains(&at.y))).ok_or("Sheet Entry must be placed inside a Sheet Symbol")?;
    let K::SheetSymbol { a, b, .. } = p.kind else {
        unreachable!()
    };
    let side = [
        (at.x - a.x).abs(),
        (at.x - b.x).abs(),
        (at.y - a.y).abs(),
        (at.y - b.y).abs(),
    ]
    .iter()
    .enumerate()
    .min_by_key(|(_, d)| *d)
    .unwrap()
    .0 as u32;
    let distance = if side <= 1 { a.y - at.y } else { at.x - a.x };
    let mut e = edit::primitive(
        K::SheetEntry {
            position: eda_core::schematic_harness::edge(a, b, side, distance),
            side,
            io_type: 0,
            name: name.into(),
            font: s.sheet.system_font,
            text_color: 128,
            arrow_kind: "Block & Triangle".into(),
        },
        None,
    );
    e.presentation.parent = Some(p.id);
    e.presentation.record_type = 16;
    e.color = 128;
    e.fill = Some(0x80ffff);
    e.width_nm = 101_600;
    Ok(Fragment {
        primitives: vec![e],
        ..Default::default()
    })
}

#[cfg(test)]
#[path = "schematic_editor_tests.rs"]
pub(super) mod tests;

#[derive(Default)]
pub(super) struct Editor {
    pub(super) compiled_path: Option<String>,
    pub(super) bar: super::schematic_active_bar::State,
    pub(super) hierarchy_pick: bool,
    tool: Tool,
    move_popup: Option<egui::Pos2>,
    pub kind: Option<Placement>,
    pub value: String,
    pub directive: Option<eda_core::schematic::SchPrimitive>,
    pub designator: String,
    pointer: Option<Point>,
    preview_cache: PreviewCache,
    preview_revision: u64,
    pub(super) harness_width: i64,
    pub(super) harness_color: u32,
    pub(super) paused: bool,
    pub(super) place_prefix: Option<u8>,
    pub(super) predefined: bool,
    pub(super) predefined_selected: usize,
    pub(super) predefined_line: bool,
    pub(super) predefined_port: bool,
    pub(super) predefined_name: String,
    pub(super) predefined_width: i64,
    pub(super) predefined_length: i64,
    pub(super) predefined_auto: bool,
    pub(super) predefined_sort: bool,
    pub(super) predefined_filter: String,
}

/// Repaint and camera/UI changes must not rerun a validated document edit.
/// History revisions also invalidate this cache after Undo/Redo.
#[derive(Default)]
struct PreviewCache {
    entry: Option<(Uuid, u64, Edit, Result<Fragment, String>)>,
}

impl PreviewCache {
    fn fragment(
        &mut self,
        document: &Document,
        revision: u64,
        command: &Edit,
    ) -> Result<Fragment, String> {
        if let Some((id, old_revision, old_command, result)) = &self.entry
            && *id == document.id
            && *old_revision == revision
            && old_command == command
        {
            return result.clone();
        }
        let result = edit::preview_fragment(document, command);
        self.entry = Some((document.id, revision, command.clone(), result.clone()));
        result
    }
}

#[derive(Default)]
enum Tool {
    #[default]
    Select,
    Graphic,
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
    Area {
        points: Vec<Point>,
        revision: u64,
    },
    AreaResize {
        id: Uuid,
        handle: usize,
        anchor: Point,
        revision: u64,
    },
    Place {
        turns: u32,
        revision: u64,
    },
    Connector {
        start: Option<Point>,
        turns: u32,
        revision: u64,
    },
    Path {
        points: Vec<Point>,
        vertical_first: bool,
        angle_mode: u8,
        revision: u64,
    },
    Move {
        fragment: Fragment,
        reflected: bool,
        selection: Selection,
        connected: bool,
        anchor: Point,
        turns: u32,
        revision: u64,
        drag: bool,
    },
}

impl Editor {
    pub(super) fn path_status(&self) -> Option<&'static str> {
        if let Tool::Path { angle_mode, .. } = self.tool {
            Some(match angle_mode {
                0 => "90 Degrees | Shift+Space: mode | Space: corner",
                1 => "45 Degrees | Shift+Space: mode | Space: corner",
                _ => "Any Angle | Shift+Space: mode",
            })
        } else {
            None
        }
    }
    pub fn active(&self) -> bool {
        self.bar.interaction.is_some() || self.hierarchy_pick || !matches!(self.tool, Tool::Select)
    }
    pub fn cancel(&mut self, view: &mut View) {
        self.preview_cache.entry = None;
        self.bar.interaction = None;
        self.hierarchy_pick = false;
        self.tool = Tool::Select;
        self.move_popup = None;
        self.kind = None;
        self.paused = false;
        self.place_prefix = None;
        view.sch_preview = None;
        view.sch_tool_active = false;
        view.cancel_schematic_gesture();
    }
}

impl OpenDocument {
    fn rotate_schematic_selection(&mut self, turns: u32) -> Result<(), String> {
        let selection = view_selection(&self.view);
        if !selection.is_empty() {
            let pivot = edit::selection_anchor(self.model.schematic.as_ref().unwrap(), &selection)
                .ok_or("Nothing selected")?;
            self.edit_schematic(
                Edit::TransformSelection {
                    selection,
                    pivot,
                    quarter_turns: turns,
                    delta: Point::default(),
                    connected: false,
                },
                self.history.revision,
            )?;
        }
        Ok(())
    }
    pub(super) fn edit_schematic(&mut self, command: Edit, revision: u64) -> Result<bool, String> {
        if self.sch_editor.compiled_path.is_some() {
            return Err("The physical variant view is read-only; switch to Editor to change the base schematic".into());
        }
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
    pub(super) fn start_schematic_fragment(
        &mut self,
        index: usize,
        fragment: Fragment,
        anchor: Point,
    ) {
        let doc = &mut self.documents[index];
        doc.sch_editor.cancel(&mut doc.view);
        doc.sch_editor.tool = Tool::Move {
            reflected: false,
            fragment,
            selection: Selection::default(),
            connected: false,
            anchor,
            turns: 0,
            revision: doc.history.revision,
            drag: false,
        };
        doc.view.sch_tool_active = true;
    }
    pub(super) fn schematic_graphic_tool(&mut self, index: usize) {
        let doc = &mut self.documents[index];
        doc.sch_editor.cancel(&mut doc.view);
        doc.sch_editor.tool = Tool::Graphic;
        doc.view.sch_tool_active = true;
    }
    pub(super) fn finish_schematic_interaction(&mut self) {
        for (i, doc) in self.documents.iter_mut().enumerate() {
            if self.active != Some(i)
                && (matches!(
                    doc.sch_editor.tool,
                    Tool::Move { .. }
                        | Tool::WireDrag { .. }
                        | Tool::Pick { .. }
                        | Tool::Path { .. }
                        | Tool::Area { .. }
                        | Tool::AreaResize { .. }
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
        self.action(ui, "Bus", "P, B", Command::SchPlace(Placement::Bus), true);
        self.action(
            ui,
            "Bus Entry",
            "P, U",
            Command::SchPlace(Placement::BusEntry),
            true,
        );
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
        self.action(ui, "Port", "P, R", Command::SchPlace(Placement::Port), true);
        for kind in [Placement::OffSheetConnector, Placement::SheetSymbol] {
            self.action(ui, kind.label(), "", Command::SchPlace(kind), true);
        }
        self.action(
            ui,
            "Sheet Entry",
            "P, E",
            Command::SchPlace(Placement::SheetEntry),
            true,
        );
        for (label, device) in [("Device Sheet Symbol", true), ("Reuse Block…", false)] {
            if ui.button(label).clicked() {
                if let Some(i) = self.active {
                    let doc = &mut self.documents[i];
                    if device {
                        doc.sch_editor.cancel(&mut doc.view);
                    }
                    let resources = &mut self.documents[i].sch_editor.bar.resources;
                    if device {
                        resources.open_device();
                    } else {
                        resources.open_reuse();
                    }
                }
                ui.close();
            }
        }
        ui.menu_button("Harness", |ui| {
            for (label, key, kind) in [
                ("Signal Harness", "P, H, H", Placement::SignalHarness),
                ("Harness Connector", "P, H, C", Placement::HarnessConnector),
                ("Harness Entry", "P, H, E", Placement::HarnessEntry),
            ] {
                self.action(ui, label, key, Command::SchPlace(kind), true);
            }
            if ui.button("Predefined Harness Connector").clicked() {
                if let Some(i) = self.active {
                    let editor = &mut self.documents[i].sch_editor;
                    if editor.predefined_width == 0 {
                        editor.predefined_width = 12_700_000;
                        editor.predefined_length = 12_700_000;
                        editor.predefined_name = "HarnessPort".into();
                        editor.predefined_line = true;
                        editor.predefined_port = true;
                        editor.predefined_sort = true;
                    }
                    editor.predefined = true;
                }
                ui.close();
            }
        });
        ui.menu_button("Directives", |ui| {
            for kind in [
                Placement::ParameterSet,
                Placement::NoErc,
                Placement::DifferentialPair,
                Placement::Blanket,
                Placement::CompileMask,
            ] {
                self.action(ui, kind.label(), "", Command::SchPlace(kind), true);
            }
        });
        self.action(
            ui,
            "Text String",
            "T",
            Command::SchPlace(Placement::Text),
            true,
        );
        for kind in [Placement::TextFrame, Placement::Note] {
            self.action(ui, kind.label(), "", Command::SchPlace(kind), true);
        }
        ui.menu_button("Drawing Tools", |ui| {
            for kind in [
                Placement::Arc,
                Placement::FullCircle,
                Placement::EllipticalArc,
                Placement::Ellipse,
                Placement::Line,
                Placement::Rectangle,
                Placement::RoundRectangle,
                Placement::Polygon,
                Placement::Bezier,
            ] {
                self.action(ui, kind.label(), "", Command::SchPlace(kind), true);
            }
            if ui.button("Graphic…").clicked() {
                if let Some(i) = self.active {
                    self.schematic_graphic_tool(i);
                }
                ui.close();
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
        doc.sch_editor.bar.power = None;
        doc.sch_editor.bar.shape_turns = 0;
        doc.sch_editor.kind = Some(kind);
        doc.sch_editor.directive = match kind {
            Placement::Blanket => Some(eda_core::schematic_directives::primitive(K::Blanket {
                points: eda_core::schematic_directives::rectangle(
                    Point::default(),
                    Point::mm(10, 10),
                ),
                collapsed: false,
            })),
            Placement::CompileMask => {
                Some(eda_core::schematic_directives::primitive(K::CompileMask {
                    a: Point::default(),
                    b: Point::mm(10, 10),
                    collapsed: false,
                }))
            }
            Placement::ParameterSet | Placement::DifferentialPair | Placement::NoErc => {
                edit::place(kind, Point::default(), 0, "", "", s.sheet.system_font)
                    .ok()
                    .and_then(|f| f.primitives.into_iter().next())
            }
            _ => None,
        };
        doc.sch_editor.value = match kind {
            Placement::Resistor => "10k",
            Placement::Capacitor => "100nF",
            Placement::Ground => "GND",
            Placement::OffSheetConnector => "OffSheet",
            Placement::NetLabel => "Net1",
            Placement::Port => "Port",
            Placement::SheetEntry => "Entry",
            Placement::HarnessEntry => "0",
            Placement::HarnessConnector => "*",
            Placement::Text | Placement::TextFrame | Placement::Note => "Text",
            Placement::SheetSymbol => "Sheet1.SchDoc",
            Placement::ParameterSet => "Parameter Set",
            Placement::DifferentialPair => "DIFFPAIR",
            Placement::VoltageSource => "5",
            Placement::CurrentSource => "1m",
            Placement::VoltageDiffProbe => "VD+",
            _ => "",
        }
        .into();
        if matches!(
            kind,
            Placement::SheetSymbol
                | Placement::TextFrame
                | Placement::Note
                | Placement::Rectangle
                | Placement::RoundRectangle
                | Placement::Arc
                | Placement::FullCircle
                | Placement::EllipticalArc
                | Placement::Ellipse
                | Placement::Polygon
                | Placement::Bezier
        ) {
            doc.sch_editor.bar.interaction =
                Some(super::schematic_active_bar::Interaction::Shape {
                    kind,
                    points: vec![],
                });
            doc.view.sch_tool_active = true;
            return;
        }
        doc.sch_editor.designator = next_designator(
            s,
            match kind {
                Placement::Capacitor => "C",
                Placement::VoltageSource => "V",
                Placement::CurrentSource => "I",
                Placement::InitialCondition => "IC",
                _ => "R",
            },
        );
        if kind == Placement::VoltageDiffProbe {
            doc.sch_editor.designator = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
        }
        doc.sch_editor.harness_width = 508_000;
        doc.sch_editor.harness_color = eda_core::schematic_harness::SIGNAL_COLOR;
        doc.sch_editor.tool = if kind == Placement::HarnessConnector {
            Tool::Connector {
                start: None,
                turns: 0,
                revision: doc.history.revision,
            }
        } else if matches!(
            kind,
            Placement::Wire | Placement::Bus | Placement::SignalHarness | Placement::Line
        ) {
            Tool::Path {
                points: vec![],
                vertical_first: false,
                angle_mode: 0,
                revision: doc.history.revision,
            }
        } else if matches!(kind, Placement::Blanket | Placement::CompileMask) {
            Tool::Area {
                points: vec![],
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
                SchAction::Rotate => doc.rotate_schematic_selection(1)?,
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
                        reflected: false,
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
                reflected: false,
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
        if self.documents[i].sch_editor.compiled_path.is_some() {
            return false;
        }
        if !self.documents[i].sch_editor.active() && self.selection_shortcuts(ctx) {
            return true;
        }
        if self.schematic_bar_shortcuts(i, ctx) {
            return true;
        }
        if self.documents[i].sch_editor.predefined {
            return false;
        }
        if self.documents[i].sch_editor.active()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Tab))
        {
            self.documents[i].sch_editor.paused = true;
            return true;
        }
        if let Some(prefix) = self.documents[i].sch_editor.place_prefix {
            let mut chosen = None;
            for (key, kind) in if prefix == 1 {
                vec![
                    (Key::H, Placement::SignalHarness),
                    (Key::C, Placement::HarnessConnector),
                    (Key::E, Placement::HarnessEntry),
                ]
            } else {
                vec![
                    (Key::B, Placement::Bus),
                    (Key::U, Placement::BusEntry),
                    (Key::R, Placement::Port),
                    (Key::E, Placement::SheetEntry),
                    (Key::W, Placement::Wire),
                ]
            } {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    chosen = Some(kind);
                    break;
                }
            }
            if prefix == 0 && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::H)) {
                self.documents[i].sch_editor.place_prefix = Some(1);
                return true;
            }
            if let Some(kind) = chosen {
                self.start_schematic_tool(kind);
                return true;
            }
        }
        if !self.documents[i].sch_editor.active()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::P))
        {
            self.documents[i].sch_editor.place_prefix = Some(0);
            return true;
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
            if doc.sch_editor.paused {
                doc.sch_editor.paused = false;
                return true;
            }
            if let Tool::Path {
                points, revision, ..
            } = &doc.sch_editor.tool
                && points.len() > 1
            {
                let primitive = path_primitive(
                    doc.sch_editor.kind,
                    points.clone(),
                    doc.sch_editor.harness_width,
                    doc.sch_editor.harness_color,
                );
                let revision = *revision;
                match doc.edit_schematic(
                    Edit::Insert(Fragment {
                        primitives: vec![primitive],
                        ..Default::default()
                    }),
                    revision,
                ) {
                    Ok(_) => {
                        if let Tool::Path {
                            points, revision, ..
                        } = &mut doc.sch_editor.tool
                        {
                            points.clear();
                            *revision = doc.history.revision;
                        }
                        doc.view.sch_preview = None;
                    }
                    Err(error) => self.message(true, error),
                }
                return true;
            }
            if doc.sch_editor.active() || doc.sch_editor.move_popup.is_some() {
                doc.sch_editor.cancel(&mut doc.view);
            } else {
                doc.view.selected = None;
                doc.view.sch_selection.clear();
                doc.view.sch_segments.clear();
                doc.view.cancel_schematic_gesture();
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
        if matches!(self.documents[i].sch_editor.tool, Tool::Path { .. })
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, Key::Space))
        {
            if let Tool::Path { angle_mode, .. } = &mut self.documents[i].sch_editor.tool {
                *angle_mode = (*angle_mode + 1) % 3;
            }
            return true;
        }
        for key in [Key::X, Key::Y] {
            if let Tool::Move {
                fragment,
                reflected,
                turns,
                ..
            } = &mut self.documents[i].sch_editor.tool
                && eda_core::schematic_directives::area_fragment(fragment)
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
            {
                // Mx R(t) = R(-t) Mx; My = R(2) Mx.
                *turns = (if key == Key::X { 4 } else { 6 } - *turns) % 4;
                *reflected = !*reflected;
                return true;
            }
        }
        let clockwise = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, Key::Space));
        if clockwise || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
            let turns_to_add = if clockwise { 3 } else { 1 };
            match &mut self.documents[i].sch_editor.tool {
                Tool::Connector { turns, .. }
                | Tool::Place { turns, .. }
                | Tool::Move { turns, .. }
                | Tool::Library { turns, .. } => *turns = (*turns + turns_to_add) % 4,
                Tool::Path { vertical_first, .. } => *vertical_first = !*vertical_first,
                Tool::Select => {
                    if let Err(e) = self.documents[i].rotate_schematic_selection(turns_to_add) {
                        self.message(true, e);
                    }
                }
                Tool::Graphic
                | Tool::Pick { .. }
                | Tool::WireDrag { .. }
                | Tool::Area { .. }
                | Tool::AreaResize { .. } => {}
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
        if input.panning || input.cancelled {
            let doc = &mut self.documents[index];
            if input.cancelled && !input.panning
                || matches!(
                    doc.sch_editor.tool,
                    Tool::Move { drag: true, .. }
                        | Tool::WireDrag { drag: true, .. }
                        | Tool::AreaResize { .. }
                )
            {
                doc.sch_editor.cancel(&mut doc.view);
            }
            return;
        }
        if input.clicked
            && input
                .modifiers
                .unwrap_or_else(|| ctx.input(|i| i.modifiers))
                .alt
            && !self.documents[index].sch_editor.active()
        {
            self.highlight_schematic_net(index, input.hit);
            return;
        }
        if !self.documents[index].sch_editor.active()
            && input.modifiers.is_some_and(|m| m.shift)
            && (input.drag_started || input.dragging || input.drag_stopped)
        {
            return;
        }
        if self.schematic_bar_input(index, &input, ctx) {
            return;
        }
        if self.documents[index].sch_editor.hierarchy_pick && input.clicked {
            if let Some(id) = input.hit {
                self.navigate_hierarchy(index, id);
            }
            return;
        }
        let doc = &mut self.documents[index];
        if doc.sch_editor.paused || doc.sch_editor.predefined || doc.sch_editor.bar.dialog.is_some()
        {
            return;
        }
        // Escape is processed by schematic_shortcuts, including finishing a path.
        // Do not immediately cancel the next path during the same held key.
        if ctx.input(|i| i.key_down(Key::Escape)) {
            return;
        }
        if ctx.input(|i| !i.focused) {
            if matches!(
                doc.sch_editor.tool,
                Tool::Move { .. }
                    | Tool::Connector { .. }
                    | Tool::WireDrag { .. }
                    | Tool::Pick { .. }
                    | Tool::Path { .. }
                    | Tool::Area { .. }
                    | Tool::AreaResize { .. }
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
        let modifiers = ctx.input(|i| i.modifiers);
        let snap = |p| {
            if modifiers.ctrl && modifiers.shift {
                p
            } else {
                edit::snap_point(s, p, step, radius)
            }
        };
        let point = input.point.map(snap);
        // The follow-up frame paints the preview computed on the input frame.
        // Placement tools allocate fresh identities, so comparing their output
        // alone would otherwise schedule another frame forever while idle.
        if point == doc.sch_editor.pointer
            && doc.view.sch_preview.is_some()
            && doc.sch_editor.preview_revision == doc.history.revision
            && !input.clicked
            && !input.secondary_clicked
            && !input.drag_started
            && !input.drag_stopped
            // Shortcuts consume processed events before this function runs.
            // Raw events must still invalidate a stationary preview on Space/X/Y.
            && ctx.input(|i| i.raw.events.is_empty())
        {
            return;
        }
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
                            reflected: false,
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
            && input.clicked
            && let (Some(id), Some(at)) = (input.hit, input.point)
            && let Some(p) = s.primitive(id)
        {
            let points = match &p.kind {
                K::Blanket { points, .. } => Some(points.clone()),
                K::CompileMask { a, b, .. } => {
                    Some(eda_core::schematic_directives::rectangle(*a, *b))
                }
                _ => None,
            };
            if let Some(points) = points {
                let top = Point {
                    x: points.iter().map(|p| p.x).min().unwrap(),
                    y: points.iter().map(|p| p.y).max().unwrap(),
                };
                let size = 1_524_000.max((3.0 / doc.view.camera.pixels_per_mm * 1e6) as i64);
                if at.x >= top.x - radius / 3
                    && at.x <= top.x + size
                    && at.y <= top.y + radius / 3
                    && at.y >= top.y - size
                {
                    let mut value = p.clone();
                    if let K::Blanket { collapsed, .. } | K::CompileMask { collapsed, .. } =
                        &mut value.kind
                    {
                        *collapsed = !*collapsed;
                    }
                    let revision = doc.history.revision;
                    if let Err(e) = doc.edit_schematic(
                        Edit::PrimitiveProperties {
                            id,
                            value: Box::new(value),
                        },
                        revision,
                    ) {
                        self.message(true, e);
                    }
                    return;
                }
            }
        }
        if !doc.sch_editor.active()
            && (input.drag_started || input.drag_stopped)
            && let (Some(id), Some(origin)) = (input.hit, input.press_origin)
            && doc.view.selected == Some(id)
            && let Some(p) = s.primitive(id)
        {
            let h = eda_core::schematic_directives::handles(&p.kind);
            if let Some((handle, _)) = h
                .iter()
                .enumerate()
                .map(|(i, p)| (i, (p.x - origin.x).abs() + (p.y - origin.y).abs()))
                .filter(|(_, d)| *d <= radius * 2)
                .min_by_key(|(_, d)| *d)
            {
                doc.sch_editor.tool = Tool::AreaResize {
                    id,
                    handle,
                    anchor: snap(origin),
                    revision: doc.history.revision,
                };
                doc.view.sch_tool_active = true;
            }
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
                            reflected: false,
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
        let directive = doc.sch_editor.directive.clone();
        let styled_directive = |shape: K| {
            let mut p = directive
                .clone()
                .unwrap_or_else(|| eda_core::schematic_directives::primitive(shape.clone()));
            p.id = Uuid::new_v4();
            p.kind = shape;
            p
        };
        match &mut doc.sch_editor.tool {
            Tool::Graphic => {
                if input.secondary_clicked {
                    finish = true;
                } else if input.clicked
                    && let Some(at) = point
                {
                    if let Some(path) = self
                        .file_dialog
                        .clone()
                        .set_title("Choose Graphic")
                        .add_filter(
                            "Image Files",
                            &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "ico"],
                        )
                        .pick_file()
                    {
                        match eda_io::schematic_graphic::load(&path, false) {
                            Ok(fragment) => {
                                doc.sch_editor.tool = Tool::Move {
                                    reflected: false,
                                    fragment,
                                    selection: Selection::default(),
                                    connected: false,
                                    anchor: Point::default(),
                                    turns: 0,
                                    revision: doc.history.revision,
                                    drag: false,
                                };
                                doc.sch_editor.pointer = Some(at);
                            }
                            Err(e) => {
                                error = Some(e);
                                finish = true;
                            }
                        }
                    } else {
                        finish = true;
                    }
                }
            }
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
                    let mut delta = Point {
                        x: at.x - anchor.x,
                        y: at.y - anchor.y,
                    };
                    if ctx.input(|i| i.modifiers.alt) {
                        if delta.x.abs() >= delta.y.abs() {
                            delta.y = 0;
                        } else {
                            delta.x = 0;
                        }
                    }
                    let edit = Edit::DragWire {
                        id: *id,
                        handle: *handle,
                        delta,
                    };
                    match doc.sch_editor.preview_cache.fragment(
                        &doc.model,
                        doc.history.revision,
                        &edit,
                    ) {
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
            Tool::AreaResize {
                id,
                handle,
                anchor,
                revision,
            } => {
                if let Some(at) = input.drop_point.map(snap).or(point)
                    && let Some(original) = s.primitive(*id)
                {
                    let mut value = original.clone();
                    let result = eda_core::schematic_directives::resize(
                        &mut value.kind,
                        *handle,
                        Point {
                            x: at.x - anchor.x,
                            y: at.y - anchor.y,
                        },
                    );
                    if result.is_ok() {
                        let edit = Edit::PrimitiveProperties {
                            id: *id,
                            value: Box::new(value),
                        };
                        if let Ok(f) = doc.sch_editor.preview_cache.fragment(
                            &doc.model,
                            doc.history.revision,
                            &edit,
                        ) {
                            preview = Some(f);
                        }
                        if input.drag_stopped {
                            command = Some((edit, *revision));
                        }
                    } else if input.drag_stopped {
                        error = result.err();
                    }
                }
                finish = input.secondary_clicked || input.drag_stopped;
            }
            Tool::Area { points, revision } => {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Backspace)) {
                    points.pop();
                }
                let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                let mask = kind == Some(Placement::CompileMask);
                let anchor = input.clicked || (enter && (mask || points.len() < 2));
                if let Some(at) = point {
                    let mut path = points.clone();
                    if path.last() != Some(&at) {
                        path.push(at);
                    }
                    let mask = kind == Some(Placement::CompileMask);
                    let shape = if path.len() >= 2 {
                        Some(if mask {
                            K::CompileMask {
                                a: path[0],
                                b: path[1],
                                collapsed: false,
                            }
                        } else {
                            K::Blanket {
                                points: if path.len() == 2 {
                                    eda_core::schematic_directives::rectangle(path[0], path[1])
                                } else {
                                    path.clone()
                                },
                                collapsed: false,
                            }
                        })
                    } else {
                        None
                    };
                    if let Some(shape) = shape {
                        let p = styled_directive(shape);
                        let fragment = Fragment {
                            primitives: vec![p],
                            ..Default::default()
                        };
                        if edit::preview(&doc.model, &Edit::Insert(fragment.clone())).is_ok() {
                            preview = Some(fragment);
                        }
                    }
                    if anchor && points.last() != Some(&at) {
                        points.push(at);
                    }
                }
                let mask = kind == Some(Placement::CompileMask);
                let end = if mask {
                    anchor && points.len() == 2
                } else {
                    input.secondary_clicked || enter
                };
                if end && points.len() >= 2 {
                    let shape = if mask {
                        K::CompileMask {
                            a: points[0],
                            b: points[1],
                            collapsed: false,
                        }
                    } else {
                        K::Blanket {
                            points: if points.len() == 2 {
                                eda_core::schematic_directives::rectangle(points[0], points[1])
                            } else {
                                points.clone()
                            },
                            collapsed: false,
                        }
                    };
                    let p = styled_directive(shape);
                    command = Some((
                        Edit::Insert(Fragment {
                            primitives: vec![p],
                            ..Default::default()
                        }),
                        *revision,
                    ));
                    points.clear();
                } else if input.secondary_clicked {
                    finish = true;
                }
            }
            Tool::Connector {
                start,
                turns,
                revision,
            } => {
                if let Some(at) = point {
                    let clicked = input.clicked
                        || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                    if let Some(origin) = *start {
                        let a = Point {
                            x: origin.x.min(at.x),
                            y: origin.y.max(at.y),
                        };
                        let b = Point {
                            x: origin.x.max(at.x),
                            y: origin.y.min(at.y),
                        };
                        if let Ok(mut fragment) = eda_core::schematic_harness::connector(
                            a,
                            b,
                            s.sheet.system_font,
                            &doc.sch_editor.value,
                        ) {
                            if let K::HarnessConnector {
                                side, primary_nm, ..
                            } = &mut fragment.primitives[0].kind
                            {
                                for _ in 0..*turns {
                                    *side = [3, 2, 0, 1][*side as usize];
                                }
                                *primary_nm = if *side <= 1 {
                                    (a.y - b.y) / 2
                                } else {
                                    (b.x - a.x) / 2
                                };
                            }
                            if clicked {
                                command = Some((Edit::Insert(fragment.clone()), *revision));
                                *start = None;
                            }
                            preview = Some(fragment);
                        }
                    } else if clicked {
                        *start = Some(at);
                    }
                }
                finish = input.secondary_clicked;
            }
            Tool::Place { turns, revision } => {
                if let Some(at) = point {
                    let result = if kind == Some(Placement::HarnessEntry) {
                        s.primitives
                            .iter()
                            .filter(|p| matches!(p.kind, K::HarnessConnector { .. }))
                            .find_map(|p| {
                                eda_core::schematic_harness::entry(
                                    s,
                                    p.id,
                                    at,
                                    &doc.sch_editor.value,
                                )
                                .ok()
                            })
                            .map(|p| Fragment {
                                primitives: vec![p],
                                ..Default::default()
                            })
                            .ok_or("Harness Entry must be placed inside a Harness Connector".into())
                    } else if kind == Some(Placement::SheetEntry) {
                        place_sheet_entry(s, at, &doc.sch_editor.value)
                    } else {
                        edit::place(
                            kind.unwrap(),
                            at,
                            *turns,
                            &doc.sch_editor.designator,
                            &doc.sch_editor.value,
                            s.sheet.system_font,
                        )
                    };
                    let result = result.map(|mut f| {
                        if kind == Some(Placement::Ground)
                            && let Some(n) = doc.sch_editor.bar.power
                        {
                            let (_, style, orientation) = super::schematic_active_bar::POWER[n];
                            for p in &mut f.primitives {
                                if let K::PowerPort {
                                    style: s,
                                    orientation: o,
                                    ..
                                } = &mut p.kind
                                {
                                    *s = style;
                                    *o = (orientation + *turns) % 4;
                                }
                            }
                        }
                        f
                    });
                    match result {
                        Ok(mut fragment) => {
                            if let Some(template) = &directive {
                                let mut p = template.clone();
                                p.id = Uuid::new_v4();
                                if edit::transform_primitive(&mut p, Point::default(), *turns, at)
                                    .is_ok()
                                {
                                    fragment.primitives = vec![p];
                                }
                            }
                            if input.clicked
                                || ctx
                                    .input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
                            {
                                command = Some((Edit::Insert(fragment.clone()), *revision));
                            }
                            preview = Some(fragment);
                        }
                        Err(e) => {
                            if input.clicked {
                                error = Some(e);
                            }
                        }
                    }
                }
                finish = input.secondary_clicked;
            }
            Tool::Path {
                points,
                vertical_first,
                angle_mode,
                revision,
            } => {
                let mut path = points.clone();
                let anchor = input.clicked
                    || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                if let Some(at) = point {
                    extend_harness_path(
                        &mut path,
                        at,
                        *vertical_first,
                        if matches!(
                            kind,
                            Some(Placement::Wire | Placement::Bus | Placement::SignalHarness)
                        ) {
                            *angle_mode
                        } else {
                            2
                        },
                    );
                    if anchor {
                        *points = path.clone();
                    }
                }
                repeat_path = matches!(
                    kind,
                    Some(Placement::Wire | Placement::Bus | Placement::SignalHarness)
                ) && anchor
                    && points.len() > 1
                    && point.is_some_and(|at| {
                        if kind == Some(Placement::SignalHarness) {
                            eda_core::schematic_harness::connection_at(s, at)
                        } else {
                            edit::connection_at(s, at)
                        }
                    });
                let end = repeat_path || input.secondary_clicked;
                if end {
                    // Finish at the last confirmed vertex; moving the pointer is only preview.
                    if points.len() > 1 {
                        let mut p = edit::primitive(
                            K::Polyline {
                                points: points.clone(),
                                closed: false,
                                electrical: if kind == Some(Placement::Bus) {
                                    "Bus"
                                } else if kind == Some(Placement::SignalHarness) {
                                    "Signal Harness"
                                } else if kind == Some(Placement::Wire) {
                                    "Wire"
                                } else {
                                    "Graphic"
                                }
                                .into(),
                            },
                            None,
                        );
                        if kind == Some(Placement::Bus) {
                            p.presentation.record_type = 26;
                            p.width_nm = 508_000;
                        }
                        if kind == Some(Placement::SignalHarness) {
                            p.presentation.record_type = 218;
                            p.width_nm = doc.sch_editor.harness_width;
                            p.color = doc.sch_editor.harness_color;
                        }
                        command = Some((
                            Edit::Insert(Fragment {
                                primitives: vec![p],
                                ..Default::default()
                            }),
                            *revision,
                        ));
                    }
                    repeat_path |= points.len() > 1;
                    finish = !repeat_path;
                }
                if path.len() > 1 {
                    let mut fragment = Fragment {
                        primitives: vec![edit::primitive(
                            K::Polyline {
                                points: path,
                                closed: false,
                                electrical: if kind == Some(Placement::Bus) {
                                    "Bus"
                                } else if kind == Some(Placement::SignalHarness) {
                                    "Signal Harness"
                                } else if kind == Some(Placement::Wire) {
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
                    if kind == Some(Placement::Bus) {
                        fragment.primitives[0].presentation.record_type = 26;
                        fragment.primitives[0].width_nm = 508_000;
                    }
                    if kind == Some(Placement::SignalHarness) {
                        fragment.primitives[0].presentation.record_type = 218;
                        fragment.primitives[0].width_nm = doc.sch_editor.harness_width;
                        fragment.primitives[0].color = doc.sch_editor.harness_color;
                    }
                    match doc.sch_editor.preview_cache.fragment(
                        &doc.model,
                        doc.history.revision,
                        &Edit::Insert(fragment),
                    ) {
                        Ok(f) => preview = Some(f),
                        Err(e) => error = Some(e),
                    }
                }
            }
            Tool::Move {
                fragment,
                reflected,
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
                    let mut delta = Point {
                        x: at.x - anchor.x,
                        y: at.y - anchor.y,
                    };
                    if ctx.input(|i| i.modifiers.alt) {
                        if delta.x.abs() >= delta.y.abs() {
                            delta.y = 0;
                        } else {
                            delta.x = 0;
                        }
                    }
                    let edit = if *reflected {
                        eda_core::schematic_directives::transform_areas(
                            fragment, *anchor, *turns, true, delta,
                        )
                        .map(|f| {
                            if selection.is_empty() {
                                Edit::Insert(f)
                            } else {
                                Edit::Batch(
                                    f.primitives
                                        .into_iter()
                                        .map(|p| Edit::PrimitiveProperties {
                                            id: p.id,
                                            value: Box::new(p),
                                        })
                                        .collect(),
                                )
                            }
                        })
                    } else if selection.is_empty() {
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
                        doc.sch_editor
                            .preview_cache
                            .fragment(&doc.model, doc.history.revision, &edit)
                            .map(|f| (edit, f))
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
                Tool::Place { .. }
                    | Tool::Library { .. }
                    | Tool::Connector { .. }
                    | Tool::Area { .. }
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
                        if let Tool::Place { revision, .. }
                        | Tool::Area { revision, .. }
                        | Tool::Connector { revision, .. }
                        | Tool::Library { revision, .. } = &mut doc.sch_editor.tool
                        {
                            *revision = doc.history.revision;
                        }
                        if kind == Some(Placement::VoltageDiffProbe) {
                            if doc.sch_editor.value == "VD+" {
                                doc.sch_editor.value = "VD-".into();
                            } else {
                                doc.sch_editor.value = "VD+".into();
                                doc.sch_editor.designator =
                                    Uuid::new_v4().simple().to_string()[..8].to_uppercase();
                            }
                        }
                        if kind == Some(Placement::HarnessEntry) {
                            let n = doc
                                .sch_editor
                                .value
                                .parse::<u32>()
                                .ok()
                                .and_then(|n| n.checked_add(1));
                            if let Some(n) = n {
                                doc.sch_editor.value = n.to_string();
                            }
                        }
                        if matches!(
                            kind,
                            Some(
                                Placement::Resistor
                                    | Placement::Capacitor
                                    | Placement::VoltageSource
                                    | Placement::CurrentSource
                                    | Placement::InitialCondition
                            )
                        ) {
                            doc.sch_editor.designator = next_designator(
                                doc.model.schematic.as_ref().unwrap(),
                                match kind {
                                    Some(Placement::Capacitor) => "C",
                                    Some(Placement::VoltageSource) => "V",
                                    Some(Placement::CurrentSource) => "I",
                                    Some(Placement::InitialCondition) => "IC",
                                    _ => "R",
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
            if doc.view.sch_preview.is_some() {
                ctx.request_repaint();
            }
            doc.sch_editor.cancel(&mut doc.view);
        } else {
            let next_preview = preview.map(|fragment| {
                let mut s = Schematic::blank();
                s.fonts = if fragment.fonts.is_empty() {
                    doc.model.schematic.as_ref().unwrap().fonts.clone()
                } else {
                    fragment.fonts
                };
                s.images = fragment.images;
                s.primitives = fragment.primitives;
                for p in &mut s.primitives {
                    if !matches!(p.kind,K::HarnessConnector{..}|K::HarnessEntry{..}) && p.presentation.record_type!=217 && !matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Signal Harness") {p.color = 0xc08000;}
                }
                s
            });
            if doc.view.sch_preview != next_preview {
                doc.view.sch_preview = next_preview;
                ctx.request_repaint();
            }
            doc.sch_editor.preview_revision = doc.history.revision;
        }
        if let Some(e) = error {
            self.message(true, e);
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
    s.primitive(id).is_some_and(|p| p.component.is_none() && matches!(&p.kind, K::Polyline { electrical, closed: false, .. } if matches!(electrical.as_str(),"Wire"|"Signal Harness"|"Bus")))
}

fn path_primitive(
    kind: Option<Placement>,
    points: Vec<Point>,
    width: i64,
    color: u32,
) -> eda_core::schematic::SchPrimitive {
    let mut p = edit::primitive(
        K::Polyline {
            points,
            closed: false,
            electrical: match kind {
                Some(Placement::Wire) => "Wire",
                Some(Placement::Bus) => "Bus",
                Some(Placement::SignalHarness) => "Signal Harness",
                _ => "Graphic",
            }
            .into(),
        },
        None,
    );
    match kind {
        Some(Placement::Bus) => {
            p.presentation.record_type = 26;
            p.width_nm = 508_000;
        }
        Some(Placement::SignalHarness) => {
            p.presentation.record_type = 218;
            p.width_nm = width;
            p.color = color;
        }
        _ => {}
    }
    p
}

fn extend_harness_path(points: &mut Vec<Point>, at: Point, vertical_first: bool, mode: u8) {
    if mode == 1
        && let Some(last) = points.last().copied()
    {
        let dx = at.x - last.x;
        let dy = at.y - last.y;
        let diagonal = dx.abs().min(dy.abs());
        let corner = if vertical_first {
            Point {
                x: last.x + dx.signum() * diagonal,
                y: last.y + dy.signum() * diagonal,
            }
        } else if dx.abs() > dy.abs() {
            Point {
                x: at.x - dx.signum() * diagonal,
                y: last.y,
            }
        } else {
            Point {
                x: last.x,
                y: at.y - dy.signum() * diagonal,
            }
        };
        if corner != last && corner != at {
            points.push(corner);
        }
    }
    extend_path(points, at, vertical_first, mode == 0);
}

fn extend_path(points: &mut Vec<Point>, at: Point, vertical_first: bool, orthogonal: bool) {
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
