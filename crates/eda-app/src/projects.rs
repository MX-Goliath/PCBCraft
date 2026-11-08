//! Projects tree: native-style fixed-height rows, with independent expansion state.
use super::*;
use egui::{Align2, FontId, Id, Rect, Sense, Stroke, pos2, vec2};

#[derive(Clone, Copy)]
enum Icon {
    Group,
    Project,
    Folder,
    Pcb,
    Sch,
    File,
    Output,
    Bom,
}

struct Node {
    variant: Option<(PathBuf, Option<String>)>,
    project: Option<PathBuf>,
    id: Id,
    label: String,
    icon: Icon,
    path: Option<PathBuf>,
    document: Option<usize>,
    children: Vec<Node>,
    branch: bool,
    expanded: bool,
    enabled: bool,
    active: bool,
    dirty: bool,
    status: bool,
}

impl Node {
    fn folder(id: Id, label: &str, expanded: bool, children: Vec<Node>) -> Self {
        Self {
            variant: None,
            project: None,
            id,
            label: label.into(),
            icon: Icon::Folder,
            path: None,
            document: None,
            children,
            branch: true,
            expanded,
            enabled: true,
            active: false,
            dirty: false,
            status: false,
        }
    }
    fn matches(&self, query: &str) -> bool {
        self.label.to_lowercase().contains(query) || self.children.iter().any(|n| n.matches(query))
    }
}

fn file_icon(kind: &str) -> Icon {
    match kind {
        "pcbdoc" | "pcblib" => Icon::Pcb,
        "schdoc" | "schlib" => Icon::Sch,
        "outjob" => Icon::Output,
        "bomdoc" => Icon::Bom,
        _ => Icon::File,
    }
}
fn category(kind: &str) -> &'static str {
    match kind {
        "outjob" => "Output Job Files",
        "schlib" => "Schematic Library Documents",
        "pcblib" => "PCB Library Documents",
        "intlib" => "Integrated Libraries",
        "txt" | "rep" | "csv" | "log" => "Text Documents",
        _ => "Documents",
    }
}

impl PcbCraft {
    fn project_nodes(&self) -> Vec<Node> {
        let mut nodes = Vec::new();
        for project in &self.projects {
            let id = Id::new(("project", &project.path));
            let mut children = Vec::new();
            if !project.variants.is_empty() {
                let current = self
                    .settings
                    .active_variants
                    .get(&project.path)
                    .cloned()
                    .flatten();
                let variants = std::iter::once(("[No Variations]", None))
                    .chain(
                        project
                            .variants
                            .iter()
                            .map(|v| (v.name.as_str(), Some(v.id.clone()))),
                    )
                    .map(|(name, variant)| {
                        let mut n = Node::folder(id.with(("variant", name)), name, false, vec![]);
                        n.branch = false;
                        n.icon = Icon::Pcb;
                        n.enabled = true;
                        n.active = current == variant;
                        n.variant = Some((project.path.clone(), variant));
                        n.project = Some(project.path.clone());
                        n
                    })
                    .collect();
                children.push(Node::folder(
                    id.with("variants"),
                    "Variants",
                    true,
                    variants,
                ));
            }
            let mut sources = Vec::new();
            let mut settings = std::collections::BTreeMap::<&str, Vec<Node>>::new();
            for entry in &project.documents {
                let n = self.project_file(
                    entry,
                    id.with(&entry.path),
                    (entry.kind == "schdoc")
                        .then(|| project.sheet_number(&entry.path))
                        .flatten(),
                );
                if matches!(entry.kind.as_str(), "schdoc" | "pcbdoc" | "bomdoc") {
                    sources.push(n);
                } else {
                    settings.entry(category(&entry.kind)).or_default().push(n);
                }
            }
            if project.virtual_bom && !project.documents.iter().any(|e| e.kind == "bomdoc") {
                let mut bom = Node::folder(id.with("virtual_bom"), "ActiveBOM", false, vec![]);
                bom.icon = Icon::Bom;
                bom.branch = false;
                bom.enabled = false;
                sources.push(bom);
            }
            let sources = nest_sources(sources, &project.sheet_children);
            children.push(Node::folder(
                id.with("sources"),
                "Source Documents",
                true,
                sources,
            ));
            if !settings.is_empty() {
                children.push(Node::folder(
                    id.with("settings"),
                    "Settings",
                    false,
                    settings
                        .into_iter()
                        .map(|(name, entries)| {
                            Node::folder(id.with(("settings", name)), name, false, entries)
                        })
                        .collect(),
                ));
            }
            if !project.generated_documents.is_empty() {
                let mut groups = std::collections::BTreeMap::<&str, Vec<Node>>::new();
                for entry in &project.generated_documents {
                    groups
                        .entry(category(&entry.kind))
                        .or_default()
                        .push(self.project_file(entry, id.with(("generated", &entry.path)), None));
                }
                children.push(Node::folder(
                    id.with("generated"),
                    "Generated",
                    false,
                    groups
                        .into_iter()
                        .map(|(name, entries)| {
                            Node::folder(id.with(("generated", name)), name, false, entries)
                        })
                        .collect(),
                ));
            }
            let mut node = Node::folder(id, &project.name, true, children);
            node.icon = Icon::Project;
            node.status = true;
            node.project = Some(project.path.clone());
            node.dirty = self.variants.dirty(project);
            node.active = project.documents.iter().any(|entry| {
                self.active
                    .is_some_and(|i| document_path(&self.documents[i]) == Some(&entry.path))
            });
            nodes.push(node);
        }
        let free = self
            .documents
            .iter()
            .enumerate()
            .filter(|(_, doc)| {
                !doc.read_only
                    && !self.projects.iter().any(|p| {
                        p.documents
                            .iter()
                            .any(|entry| document_path(doc) == Some(&entry.path))
                    })
            })
            .map(|(i, doc)| Node {
                variant: None,
                project: None,
                id: Id::new(("free", doc.session_id)),
                label: doc.model.name.clone(),
                icon: if doc.model.kind == DocumentKind::Board {
                    Icon::Pcb
                } else {
                    Icon::Sch
                },
                path: document_path(doc).cloned(),
                document: Some(i),
                children: vec![],
                branch: false,
                expanded: false,
                enabled: true,
                active: self.active == Some(i),
                dirty: doc.dirty(),
                status: false,
            })
            .collect();
        nodes.push(Node::folder(
            Id::new("free_documents"),
            "Free Documents",
            true,
            vec![Node::folder(
                Id::new("free_sources"),
                "Source Documents",
                true,
                free,
            )],
        ));
        let mut root = Node::folder(
            Id::new("project_group"),
            &self.file_state.group_name(),
            true,
            nodes,
        );
        root.icon = Icon::Group;
        vec![root]
    }

    fn project_file(
        &self,
        entry: &eda_io::altium::ProjectEntry,
        id: Id,
        sheet: Option<usize>,
    ) -> Node {
        let document = self
            .documents
            .iter()
            .position(|doc| document_path(doc) == Some(&entry.path));
        let name = entry.path.file_name().unwrap_or_default().to_string_lossy();
        Node {
            variant: None,
            project: None,
            id,
            label: sheet.map_or_else(|| name.to_string(), |n| format!("[{n}] {name}")),
            icon: file_icon(&entry.kind),
            path: Some(entry.path.clone()),
            document,
            children: vec![],
            branch: false,
            expanded: false,
            enabled: entry.exists
                && matches!(
                    entry.kind.as_str(),
                    "pcbdoc" | "schdoc" | "schlib" | "pcblib" | "intlib"
                ),
            active: document.is_some() && document == self.active,
            dirty: document.is_some_and(|i| self.documents[i].dirty()),
            status: true,
        }
    }

    pub(super) fn projects_content(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let nodes = self.project_nodes();
        let mut open = None;
        let mut activate = None;
        let query_id = Id::new("projects_filter");
        let mut query = ctx.data_mut(|d| d.get_temp::<String>(query_id).unwrap_or_default());
        ui.scope(|ui| {
            // Keep the allocated width: nested bottom panels otherwise shrink
            // this frame to its minimum and leave its resize handle elsewhere.
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), 37.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(10.0);
                    for (n, icon, label, command) in [
                        (0, theme::Icon::Save, "Save document", Command::Save),
                        (1, theme::Icon::SaveAll, "Save All", Command::SaveAll),
                        (2, theme::Icon::Open, "Open document", Command::Open),
                        (3, theme::Icon::Folder, "Open project", Command::OpenProject),
                        (
                            4,
                            theme::Icon::Settings,
                            "Preferences",
                            Command::Preferences,
                        ),
                    ] {
                        let enabled = n > 1 || self.active.is_some();
                        let (r, response) =
                            ui.allocate_exact_size(vec2(28.0, 23.0), Sense::click());
                        theme::draw_icon(
                            ui.painter(),
                            Rect::from_center_size(r.center(), vec2(14.0, 14.0)),
                            icon,
                            enabled,
                        );
                        if enabled && response.clicked() {
                            self.command = Some(command);
                        }
                        response.on_hover_text(label);
                    }
                },
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                theme::search_field(ui, &mut query, ui.available_width() - 8.0);
            });
            ui.add_space(15.0);
            let selection = Id::new("project_tree_selection");
            ctx.data_mut(|d| d.insert_temp(Id::new("project_tree_rows"), Vec::<Id>::new()));
            let active_key = Id::new("project_tree_active");
            let active = self.active.map(|i| self.documents[i].session_id);
            ctx.data_mut(|d| {
                if d.get_temp::<Option<Uuid>>(active_key) != Some(active) {
                    d.remove::<Id>(selection);
                    d.insert_temp(active_key, active);
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("project_tree_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for node in &nodes {
                        draw_node(
                            ui,
                            node,
                            0,
                            &query.to_lowercase(),
                            selection,
                            &mut open,
                            &mut activate,
                        );
                    }
                });
            let rows = ctx.data(|d| {
                d.get_temp::<Vec<Id>>(Id::new("project_tree_rows"))
                    .unwrap_or_default()
            });
            if let Some(index) = ctx
                .memory(|m| m.focused())
                .and_then(|id| rows.iter().position(|r| *r == id))
            {
                let delta = ui.input_mut(|i| {
                    if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) {
                        1
                    } else if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) {
                        -1
                    } else {
                        0
                    }
                });
                if delta != 0 {
                    let next = (index as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
                    ctx.memory_mut(|m| m.request_focus(rows[next]));
                    ctx.data_mut(|d| {
                        d.insert_temp(selection, rows[next]);
                        d.insert_temp(Id::new("project_tree_reveal"), rows[next]);
                    });
                }
            }
        });
        ctx.data_mut(|d| d.insert_temp(query_id, query));
        if let Some((path, id)) = ctx.data_mut(|d| {
            d.remove_temp::<(PathBuf, Option<String>)>(Id::new("project_variant_select"))
        }) {
            self.select_variant(path, id);
        }
        if let Some(path) =
            ctx.data_mut(|d| d.remove_temp::<PathBuf>(Id::new("project_variant_manage")))
        {
            self.variants.request_project = Some(path);
            self.command = Some(Command::Variants);
        }
        if let Some(i) = activate {
            self.active = Some(i);
            self.pad_edit = None;
        }
        if let Some(path) = open {
            self.request_open_path(path);
        }
        if let Some(i) = ctx.data_mut(|d| d.remove_temp::<usize>(Id::new("project_tree_close")))
            && let Some(doc) = self.documents.get(i)
        {
            self.request_close(doc.session_id);
        }
    }
}

fn document_path(doc: &OpenDocument) -> Option<&PathBuf> {
    doc.source_path.as_ref().or(doc.path.as_ref())
}

fn nest_sources(
    nodes: Vec<Node>,
    links: &std::collections::BTreeMap<PathBuf, Vec<PathBuf>>,
) -> Vec<Node> {
    let indices: std::collections::BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.path.clone().map(|p| (p, i)))
        .collect();
    let referenced: std::collections::BTreeSet<_> = links
        .values()
        .flatten()
        .filter_map(|p| indices.get(p).copied())
        .collect();
    let mut slots: Vec<_> = nodes.into_iter().map(Some).collect();
    fn take(
        i: usize,
        depth: usize,
        slots: &mut [Option<Node>],
        links: &std::collections::BTreeMap<PathBuf, Vec<PathBuf>>,
        indices: &std::collections::BTreeMap<PathBuf, usize>,
    ) -> Option<Node> {
        let mut n = slots[i].take()?;
        if depth < 64
            && let Some(children) = n.path.as_ref().and_then(|p| links.get(p))
        {
            for path in children {
                if let Some(child) = indices
                    .get(path)
                    .and_then(|&j| take(j, depth + 1, slots, links, indices))
                {
                    n.children.push(child);
                }
            }
        }
        if !n.children.is_empty() {
            n.branch = true;
            n.expanded = true;
        }
        Some(n)
    }
    let mut result = Vec::new();
    for i in (0..slots.len())
        .filter(|i| !referenced.contains(i))
        .chain((0..slots.len()).filter(|i| referenced.contains(i)))
    {
        if let Some(node) = take(i, 0, &mut slots, links, &indices) {
            result.push(node);
        }
    }
    result
}

fn triangle(p: &egui::Painter, c: egui::Pos2, open: bool, color: Color32) {
    let offsets = if open {
        [vec2(-3.0, 2.0), vec2(3.0, 2.0), vec2(3.0, -4.0)]
    } else {
        [vec2(-2.0, -3.0), vec2(-2.0, 3.0), vec2(2.0, 0.0)]
    };
    p.add(egui::Shape::convex_polygon(
        offsets.map(|v| c + v).to_vec(),
        color,
        Stroke::NONE,
    ));
}

fn draw_icon(ui: &egui::Ui, r: Rect, icon: Icon) {
    let p = ui.painter();
    match icon {
        Icon::Pcb | Icon::Sch | Icon::Project => {
            super::document_bars::document_icon(
                ui,
                Rect::from_min_size(r.min - vec2(4.0, 0.0), vec2(22.0, 16.0)),
                if matches!(icon, Icon::Sch) {
                    DocumentKind::Schematic
                } else {
                    DocumentKind::Board
                },
            );
            if matches!(icon, Icon::Project) {
                p.rect_filled(
                    Rect::from_min_size(r.min + vec2(0.0, 7.0), vec2(6.0, 6.0)),
                    0.0,
                    Color32::from_rgb(205, 218, 239),
                );
            }
        }
        Icon::Folder => {
            let c = Color32::from_rgb(158, 200, 153);
            p.rect_filled(
                Rect::from_min_size(r.min + vec2(1.0, 3.0), vec2(6.0, 4.0)),
                1.0,
                c,
            );
            p.rect_filled(
                Rect::from_min_size(r.min + vec2(1.0, 5.0), vec2(13.0, 9.0)),
                1.0,
                c,
            );
            p.line_segment(
                [r.min + vec2(2.0, 6.0), r.min + vec2(12.0, 6.0)],
                Stroke::new(1.0, Color32::from_rgb(205, 225, 193)),
            );
        }
        _ => {
            let c = if matches!(icon, Icon::Output) {
                Color32::from_rgb(145, 186, 212)
            } else {
                Color32::from_rgb(214, 216, 195)
            };
            p.rect_filled(
                Rect::from_min_size(r.min + vec2(3.0, 2.0), vec2(10.0, 12.0)),
                0.0,
                c,
            );
            for y in [5.0, 7.0, 9.0, 11.0] {
                p.line_segment(
                    [r.min + vec2(5.0, y), r.min + vec2(11.0, y)],
                    Stroke::new(1.0, Color32::from_gray(94)),
                );
            }
            if matches!(icon, Icon::Group) {
                p.rect_filled(
                    Rect::from_min_size(r.min + vec2(7.0, 8.0), vec2(7.0, 7.0)),
                    0.0,
                    Color32::from_rgb(157, 187, 210),
                );
            }
        }
    }
}

fn draw_node(
    ui: &mut egui::Ui,
    node: &Node,
    depth: usize,
    query: &str,
    selection: Id,
    open_path: &mut Option<PathBuf>,
    activate: &mut Option<usize>,
) {
    if !query.is_empty() && !node.matches(query) {
        return;
    }
    let key = node.id.with("expanded");
    let mut expanded = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(key).unwrap_or(node.expanded));
    let selected = ui
        .ctx()
        .data(|d| d.get_temp::<Id>(selection))
        .map_or(node.active && !matches!(node.icon, Icon::Project), |id| {
            id == node.id
        });
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
    let response = ui.interact(row, node.id, Sense::click());
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Id>>(Id::new("project_tree_rows"))
            .push(node.id);
    });
    if ui
        .ctx()
        .data(|d| d.get_temp::<Id>(Id::new("project_tree_reveal")))
        == Some(node.id)
    {
        response.scroll_to_me(Some(egui::Align::Center));
        ui.ctx()
            .data_mut(|d| d.remove::<Id>(Id::new("project_tree_reveal")));
    }
    if response.has_focus() && node.branch {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, Key::ArrowRight) {
                expanded = true;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft) {
                expanded = false;
            }
        });
    }
    let x = row.left() + 15.0 + depth as f32 * 12.0;
    let text_x = x + 16.0;
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_max(pos2(text_x, row.top()), row.max),
            0.0,
            Color32::from_rgb(95, 130, 159),
        );
    } else if node.active && matches!(node.icon, Icon::Project) {
        ui.painter().rect_stroke(
            Rect::from_min_max(pos2(text_x, row.top()), row.max),
            0.0,
            Stroke::new(1.0, Color32::from_rgb(96, 124, 146)),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().line_segment(
        [pos2(text_x, row.bottom()), row.right_bottom()],
        Stroke::new(0.5, Color32::from_gray(52)),
    );
    if node.branch && depth > 0 {
        triangle(
            ui.painter(),
            pos2(x - 7.0, row.center().y),
            expanded || !query.is_empty(),
            Color32::from_gray(222),
        );
    }
    draw_icon(
        ui,
        Rect::from_min_size(pos2(x, row.top()), vec2(16.0, 16.0)),
        node.icon,
    );
    let font = FontId::new(
        11.0,
        if matches!(node.icon, Icon::Project | Icon::Group) || node.variant.is_some() && node.active
        {
            egui::FontFamily::Name("ui-bold".into())
        } else {
            egui::FontFamily::Proportional
        },
    );
    let label = format!("{}{}", node.label, if node.dirty { " *" } else { "" });
    let right_gutter = if matches!(node.icon, Icon::Bom) && node.path.is_none() {
        92.0
    } else if node.status || node.document.is_some() {
        35.0
    } else {
        2.0
    };
    let text_rect = Rect::from_min_max(
        pos2(text_x, row.top()),
        pos2((row.right() - right_gutter).max(text_x), row.bottom()),
    );
    ui.painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            pos2(text_x, row.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            Color32::from_gray(if node.enabled { 225 } else { 155 }),
        );
    if node.status {
        ui.painter().circle_stroke(
            pos2(row.right() - 8.0, row.center().y),
            3.0,
            Stroke::new(1.0, Color32::from_gray(130)),
        );
        ui.interact(
            Rect::from_center_size(pos2(row.right() - 8.0, row.center().y), vec2(12.0, 16.0)),
            node.id.with("status"),
            Sense::hover(),
        )
        .on_hover_text("Validation/version-control status is unavailable");
    }
    if node.document.is_some() {
        let r = Rect::from_min_size(pos2(row.right() - 30.0, row.top() + 4.0), vec2(6.0, 8.0));
        ui.painter().rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Color32::from_gray(210)),
            egui::StrokeKind::Inside,
        );
    }
    if matches!(node.icon, Icon::Bom) && node.path.is_none() && row.right() - text_x > 150.0 {
        ui.painter().text(
            pos2(row.right() - 36.0, row.center().y),
            Align2::RIGHT_CENTER,
            "+ Create",
            FontId::proportional(11.0),
            Color32::from_gray(125),
        );
    }
    let enter =
        response.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    if response.clicked() || enter {
        response.request_focus();
        ui.ctx().data_mut(|d| d.insert_temp(selection, node.id));
        if node.branch
            && (response.interact_pointer_pos().is_some_and(|p| p.x < x)
                || response.double_clicked())
        {
            expanded = !expanded;
        } else if node.enabled {
            if let Some(action) = &node.variant
                && (response.double_clicked() || enter)
            {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(Id::new("project_variant_select"), action.clone()));
            }
            if let Some(i) = node.document {
                *activate = Some(i);
            } else if let Some(path) = &node.path {
                *open_path = Some(path.clone());
            }
        }
    }
    response.context_menu(|ui| {
        if let Some(action) = &node.variant
            && ui.button("Set as Current").clicked()
        {
            ui.ctx()
                .data_mut(|d| d.insert_temp(Id::new("project_variant_select"), action.clone()));
            ui.close();
        }
        if let Some(project) = &node.project
            && ui.button("Variants…").clicked()
        {
            ui.ctx()
                .data_mut(|d| d.insert_temp(Id::new("project_variant_manage"), project.clone()));
            ui.close();
        }
        if node.branch
            && ui
                .button(if expanded { "Collapse" } else { "Expand" })
                .clicked()
        {
            expanded = !expanded;
            ui.close();
        }
        if node.path.is_some() {
            if ui
                .add_enabled(node.enabled, egui::Button::new("Open"))
                .clicked()
            {
                if let Some(i) = node.document {
                    *activate = Some(i);
                } else {
                    *open_path = node.path.clone();
                }
                ui.close();
            }
            if ui
                .add_enabled(node.document.is_some(), egui::Button::new("Close"))
                .clicked()
            {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::new("project_tree_close"), node.document.unwrap())
                });
                ui.close();
            }
            ui.separator();
            ui.add_enabled(false, egui::Button::new("Rename"));
            ui.add_enabled(false, egui::Button::new("Remove from Project"));
        }
    });
    response.on_hover_text(node.path.as_ref().map_or_else(
        || {
            if !node.enabled {
                "This feature is not implemented".into()
            } else {
                node.label.clone()
            }
        },
        |p| {
            format!(
                "{}{}",
                p.display(),
                if node.enabled {
                    ""
                } else {
                    "\nMissing file or unsupported document type"
                }
            )
        },
    ));
    if node.branch {
        ui.ctx().data_mut(|d| d.insert_persisted(key, expanded));
        if expanded || !query.is_empty() {
            let child_query = if node.label.to_lowercase().contains(query) {
                ""
            } else {
                query
            };
            for child in &node.children {
                draw_node(
                    ui,
                    child,
                    depth + 1,
                    child_query,
                    selection,
                    open_path,
                    activate,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_sizing_does_not_collapse_projects_or_change_width_between_editors() {
        let mut app = PcbCraft {
            dock_runtime: Default::default(),
            updates: Default::default(),
            git: Default::default(),
            library_editor: Default::default(),
            variants: Default::default(),
            file_state: Default::default(),
            sheet_sync: Default::default(),
            document_loader: Default::default(),
            cross_probe: Default::default(),
            file_dialog: rfd::FileDialog::new(),
            documents: vec![
                OpenDocument::new(Document::new(DocumentKind::Board, "PCB"), None),
                OpenDocument::new(Document::new(DocumentKind::Schematic, "SCH"), None),
            ],
            projects: vec![],
            active: Some(0),
            settings: Default::default(),
            search: String::new(),
            log: vec![],
            command: None,
            pending: None,
            preferences: false,
            about: false,
            layer_stack: None,
            annotation: None,
            design_sync: None,
            library_update: None,
            allow_exit: false,
            cursor_mm: None,
            pad_edit: None,
            property_ui: Default::default(),
            sch_clipboard: None,
            pcb_clipboard: None,
            components_ui: Default::default(),
        };
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            egui::FontFamily::Name("ui-bold".into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        for (width, active) in [(0.0, 0), (1920.0, 0), (1920.0, 1), (1920.0, 0)] {
            app.active = Some(active);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 1030.0))),
                    ..Default::default()
                },
                |ctx| {
                    app.left_dock(ctx);
                    if width > 0.0 {
                        assert!(
                            (ctx.available_rect().left() - 334.0).abs() < 1.0,
                            "{:?}",
                            ctx.available_rect()
                        );
                    }
                },
            );
        }
    }
    #[test]
    fn hierarchy_retains_all_documents_with_shared_children_and_cycles() {
        let nodes = ["main", "child", "other", "cycle-a", "cycle-b"]
            .map(|name| {
                let mut n = Node::folder(Id::new(name), name, false, vec![]);
                n.path = Some(name.into());
                n.branch = false;
                n
            })
            .into();
        let links = [
            ("main", vec!["child"]),
            ("other", vec!["child"]),
            ("cycle-a", vec!["cycle-b"]),
            ("cycle-b", vec!["cycle-a"]),
        ]
        .into_iter()
        .map(|(p, c)| (p.into(), c.into_iter().map(PathBuf::from).collect()))
        .collect();
        let tree = nest_sources(nodes, &links);
        assert_eq!(tree[0].label, "main");
        assert_eq!(tree[0].children[0].label, "child");
        fn labels(n: &Node, output: &mut Vec<String>) {
            output.push(n.label.clone());
            for c in &n.children {
                labels(c, output);
            }
        }
        let mut all = vec![];
        for n in &tree {
            labels(n, &mut all);
        }
        all.sort();
        assert_eq!(all, ["child", "cycle-a", "cycle-b", "main", "other"]);
    }
}
