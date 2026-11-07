//! Variant Management follows the observed AD24 two-grid dialog and child dialogs.
use super::*;

pub(super) struct VariantEditor {
    original: Option<String>,
    value: Variant,
    parameter: Option<ParameterProperties>,
    selected: Option<String>,
    error: String,
}

impl Row {
    fn component(&self) -> &eda_core::schematic::Component {
        self.document
            .schematic
            .as_ref()
            .unwrap()
            .components
            .iter()
            .find(|c| c.id == self.component)
            .unwrap()
    }
    fn reference(&self) -> model::ComponentRef {
        model::ComponentRef {
            uid: self.uid.clone(),
            designator: self.designator.clone(),
        }
    }
    fn base_parameters(&self) -> BTreeMap<String, String> {
        let c = self.component();
        let mut params = c.parameters.clone();
        params.retain(|k, _| {
            !k.eq_ignore_ascii_case("Comment") && !k.eq_ignore_ascii_case("Description")
        });
        params.insert("Comment".into(), c.comment.clone());
        params.insert("Description".into(), c.description.clone());
        params
    }
    fn inherited_parameters(&self, variant: &Variant) -> BTreeMap<String, String> {
        if let Some(c) = variant
            .variation(&self.uid, &self.designator)
            .filter(|c| c.kind == 2)
            .and_then(|c| c.alternate.as_ref())
            .and_then(|a| a.components.first())
        {
            let mut params = c.parameters.clone();
            params.insert("Comment".into(), c.comment.clone());
            params.insert("Description".into(), c.description.clone());
            params
        } else {
            self.base_parameters()
        }
    }
}

impl Dialog {
    fn commit(&mut self, edit: model::Edit) {
        let result = model::edited(&self.draft, edit)
            .and_then(|next| self.history.commit(&mut self.draft, next));
        match result {
            Ok(_) => self.error.clear(),
            Err(e) => self.error = e,
        }
        self.variant = self.variant.min(self.draft.len().saturating_sub(1));
    }
    fn targets(&self) -> Vec<model::ComponentRef> {
        self.selected
            .iter()
            .filter_map(|n| self.rows.get(*n))
            .map(Row::reference)
            .collect()
    }
    fn select_row(&mut self, n: usize, visible: &[usize], modifiers: egui::Modifiers) {
        if modifiers.shift {
            let a = visible.iter().position(|v| *v == self.row).unwrap_or(0);
            let b = visible.iter().position(|v| *v == n).unwrap_or(a);
            if !modifiers.command {
                self.selected.clear();
            }
            self.selected
                .extend(visible[a.min(b)..=a.max(b)].iter().copied());
        } else if modifiers.command {
            if !self.selected.remove(&n) {
                self.selected.insert(n);
            }
        } else {
            self.selected = BTreeSet::from([n]);
        }
        self.row = n;
    }
    fn visible(&self) -> Vec<usize> {
        let filter = self.filter.to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                let c = row.component();
                format!(
                    "{} {} {} {} {}",
                    row.path, row.designator, c.comment, c.library_reference, row.document.name
                )
                .to_lowercase()
                .contains(&filter)
                    && (!self.only_varied
                        || self.draft.iter().any(|v| {
                            v.variation(&row.uid, &row.designator)
                                .is_some_and(|c| c.kind != 0 || !c.parameters.is_empty())
                        }))
            })
            .map(|(n, _)| n)
            .collect()
    }
    fn duplicate(&mut self) {
        if let Some(mut v) = self.draft.get(self.variant).cloned() {
            v.id = Uuid::new_v4().to_string().to_uppercase();
            v.name = Variant::unique_name(&self.draft, &format!("{} Copy", v.name));
            self.editor = Some(VariantEditor {
                original: None,
                value: v,
                parameter: None,
                selected: None,
                error: String::new(),
            });
        }
    }
}

fn title(ui: &mut egui::Ui, text: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(w - 28., 22.),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.label(text);
            },
        );
        close = ui
            .add_sized([24., 22.], egui::Button::new("×").frame(false))
            .clicked();
    });
    ui.separator();
    close
}
fn cell(ui: &mut egui::Ui, text: &str, width: f32, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 20.), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.,
            if selected {
                theme::ACCENT
            } else {
                Color32::from_gray(77)
            },
        );
    }
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.left_center() + egui::vec2(4., 0.),
            egui::Align2::LEFT_CENTER,
            text,
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        );
    response.on_hover_text(text)
}
fn field<'a>(params: &'a BTreeMap<String, String>, name: &str) -> Option<&'a String> {
    params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v)
}

fn columns(ui: &egui::Ui, d: &Dialog) -> ([f32; 6], f32) {
    let weights = [175., 132., 125., 135., 176., 220.];
    if !d.fit_width {
        return (weights, 280.);
    }
    let available = (ui.available_width() - 16.).max(360.);
    let variant_width = (available * 0.288).clamp(100., 280.);
    let original = (available - variant_width * d.draft.len() as f32 - 6.).max(360.);
    (weights.map(|w| w * original / 963.), variant_width)
}

impl PcbCraft {
    pub(in crate::app) fn variants_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.variants.dialog.take() else {
            return;
        };
        let mut keep = true;
        let mut accept = false;
        let child = d.editor.is_some()
            || d.style_editor.is_some()
            || d.report.is_some()
            || d.links
            || d.library.is_some()
            || d.component_kind.is_some();
        egui::Modal::new(egui::Id::new("variant_management"))
            .frame(egui::Frame::popup(&ctx.style()).inner_margin(egui::Margin{left:14,right:14,top:4,bottom:4}))
            .show(ctx, |ui| {
            ui.set_width(994.0_f32.min(ctx.available_rect().width() - 68.).max(600.));
            ui.spacing_mut().item_spacing = egui::vec2(3., 3.);
            ui.add_enabled_ui(!child, |ui| {
                if title(ui,"Variant Management") { keep = false; }
                ui.label(format!("Components for project {}", d.project.file_name().unwrap_or_default().to_string_lossy()));
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.add_sized([220., 20.], egui::TextEdit::singleline(&mut d.filter));
                    ui.checkbox(&mut d.only_varied, "Only Show Varied Components");
                    let count = d.selected.len();
                    if count > 1 { ui.label(format!("{count} selected")); }
                });
                let visible = d.visible();
                // Filtering also bounds bulk operations to rows that are still visible.
                d.selected.retain(|n| visible.contains(n));
                if !visible.contains(&d.row) && let Some(n) = visible.first() {
                    d.row = *n;
                    d.selected.insert(*n);
                }
                let height = ((ctx.available_rect().height() - 210.) * 0.47).clamp(120.,264.);
                let mut edit = None;
                egui::Frame::group(ui.style()).inner_margin(0).show(ui, |ui| {
                    egui::ScrollArea::both().id_salt("variant_components").max_height(height)
                        .min_scrolled_height(height).show(ui, |ui| {
                        let (widths,vw)=columns(ui,&d);
                        ui.horizontal(|ui| {
                            cell(ui,"Project Components",widths.iter().sum::<f32>()+5.,false);
                            for (n,v) in d.draft.iter().enumerate() {
                                if cell(ui,&v.name,vw,d.variant==n).clicked(){d.variant=n;}
                            }
                        });
                        egui::Grid::new("variant_components_grid").spacing([1.,0.]).striped(true).show(ui, |ui| {
                            for (label,w) in ["Hierarchy Path","Logical","Name","Designator","Document","Original Library Link"].iter().zip(widths) {
                                cell(ui,label,w,false);
                            }
                            for _ in &d.draft {cell(ui,"Component Variation",vw,false);}
                            ui.end_row();
                            for &n in &visible {
                                let row = d.rows[n].clone();
                                let c = row.component();
                                let hierarchy = if row.path.is_empty() { row.document.name.trim_end_matches(".SchDoc") } else { &row.path };
                                let original = if c.settings.source.is_empty() { &c.library_reference } else { &c.settings.source };
                                for (text,w) in [hierarchy, &row.designator, &c.comment, &c.designator, &row.document.name, original].into_iter().zip(widths) {
                                    let response = cell(ui,text,w,d.selected.contains(&n));
                                    if response.clicked() { d.select_row(n,&visible,ui.input(|i|i.modifiers)); }
                                    response.context_menu(|ui| {
                                        if !d.selected.contains(&n) { d.selected = BTreeSet::from([n]); d.row=n; }
                                        if let Some(v) = d.draft.get(d.variant) {
                                            for (label,kind) in [("Fitted",0),("Not Fitted",1)] {
                                                if ui.button(label).clicked() {
                                                    edit=Some(model::Edit::SetKind{id:v.id.clone(),components:d.targets(),kind}); ui.close();
                                                }
                                            }
                                            if ui.button("Reset All Variations for Selection").clicked() {
                                                edit=Some(model::Edit::Reset{id:v.id.clone(),components:d.targets()}); ui.close();
                                            }
                                        }
                                    });
                                }
                                for vn in 0..d.draft.len() {
                                    let v=&d.draft[vn];
                                    let change=v.variation(&row.uid,&row.designator);
                                    let kind=change.map_or(0,|c|c.kind);
                                    let label=match kind { 1=>"Not Fitted",2=>change.and_then(|c|c.alternate.as_ref()).and_then(|a|a.components.first()).map_or("Alternate Part",|c|c.library_reference.as_str()),_=>"" };
                                    let response=cell(ui,label,vw,d.variant==vn&&d.selected.contains(&n));
                                    if response.clicked() {
                                        d.variant=vn;
                                        if !d.selected.contains(&n) {d.selected=BTreeSet::from([n]);}
                                        d.row=n;
                                        d.component_kind=Some(kind);
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
                if let Some(edit)=edit {d.commit(edit);}
                ui.label("Component Parameters");
                let mut parameter_edit=None;
                egui::Frame::group(ui.style()).inner_margin(0).show(ui,|ui| {
                    egui::ScrollArea::both().id_salt("variant_parameters").max_height(height)
                        .min_scrolled_height(height).show(ui,|ui| {
                        if let Some(row)=d.rows.get(d.row) {
                            let (widths,vw)=columns(ui,&d);
                            let source_width=widths.iter().sum::<f32>()+4.;
                            let name_width=source_width*0.388;
                            let original_width=source_width-name_width;
                            ui.horizontal(|ui| {
                                cell(ui,&format!("Parameter Variations: {}",row.designator),source_width+1.,false);
                                for v in &d.draft {cell(ui,&v.name,vw,false);}
                            });
                            let mut params=row.base_parameters();
                            for v in &d.draft {
                                for key in row.inherited_parameters(v).keys() {
                                    if !params.keys().any(|k|k.eq_ignore_ascii_case(key)) {params.insert(key.clone(),String::new());}
                                }
                                if let Some(change)=v.variation(&row.uid,&row.designator) {
                                    for key in change.parameters.keys() {
                                        if !params.keys().any(|k|k.eq_ignore_ascii_case(key)) {params.insert(key.clone(),String::new());}
                                    }
                                }
                            }
                            egui::Grid::new("variant_parameters_grid").spacing([1.,0.]).striped(true).show(ui,|ui| {
                                cell(ui,"Parameter Name",name_width,false);
                                cell(ui,"Original Value",original_width,false);
                                for _ in &d.draft {cell(ui,"New Value",vw,false);}
                                ui.end_row();
                                for (name,base) in params {
                                    cell(ui,&name,name_width,false);cell(ui,&base,original_width,false);
                                    for v in &d.draft {
                                        let change=v.variation(&row.uid,&row.designator);
                                        let varied=change.and_then(|c|field(&c.parameters,&name));
                                        let inherited=row.inherited_parameters(v);
                                        let fallback=field(&inherited,&name).unwrap_or(&base);
                                        let mut value=varied.unwrap_or(fallback).clone();
                                        let mut text=egui::TextEdit::singleline(&mut value).desired_width(vw).margin(egui::vec2(4.,2.));
                                        if varied.is_some() {text=text.text_color(Color32::from_rgb(120,193,126));}
                                        let response=ui.add_sized([vw,20.],text);
                                        if response.changed() {
                                            parameter_edit=Some(model::Edit::SetParameter{id:v.id.clone(),components:d.targets(),name:name.clone(),value:Some(value)});
                                        }
                                        response.context_menu(|ui| {
                                            if ui.add_enabled(varied.is_some(),egui::Button::new("Reset to Original")).clicked() {
                                                parameter_edit=Some(model::Edit::SetParameter{id:v.id.clone(),components:d.targets(),name:name.clone(),value:None});ui.close();
                                            }
                                        });
                                    }
                                    ui.end_row();
                                }
                            });
                        }
                    });
                });
                if let Some(edit)=parameter_edit {d.commit(edit);}
                ui.horizontal(|ui| {
                    ui.label("Parameter");ui.add_sized([140.,20.],egui::TextEdit::singleline(&mut d.parameter));
                    ui.label("Value");ui.add_sized([180.,20.],egui::TextEdit::singleline(&mut d.value));
                    let id=d.draft.get(d.variant).map(|v|v.id.clone());
                    if ui.add_enabled(id.is_some()&&!d.selected.is_empty()&&!d.parameter.trim().is_empty(),egui::Button::new("Set for Selection")).clicked() {
                        d.commit(model::Edit::SetParameter{id:id.unwrap(),components:d.targets(),name:d.parameter.clone(),value:Some(d.value.clone())});
                    }
                    if ui.button("Component Links…").clicked(){d.links=true;}
                    if ui.add_enabled(d.history.can_undo(),egui::Button::new("Undo")).clicked(){d.history.undo(&mut d.draft);}
                    if ui.add_enabled(d.history.can_redo(),egui::Button::new("Redo")).clicked(){d.history.redo(&mut d.draft);}
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Add Variant…").clicked() {
                        let stem=format!("Variant of {}",d.project.file_stem().unwrap_or_default().to_string_lossy());
                        d.editor=Some(VariantEditor{original:None,value:Variant::new(Variant::unique_name(&d.draft,&stem)),parameter:None,selected:None,error:String::new()});
                    }
                    ui.menu_button("▾",|ui| {
                        if ui.add_enabled(!d.draft.is_empty(),egui::Button::new("Clone Selected Variant…")).clicked(){d.duplicate();ui.close();}
                    });
                    if ui.add_enabled(!d.draft.is_empty(),egui::Button::new("Delete Variant…")).clicked() {
                        d.commit(model::Edit::Remove(d.draft[d.variant].id.clone()));
                    }
                    if ui.add_enabled(!d.draft.is_empty(),egui::Button::new("Edit Variant…")).clicked() {
                        let v=d.draft[d.variant].clone();
                        d.editor=Some(VariantEditor{original:Some(v.id.clone()),value:v,parameter:None,selected:None,error:String::new()});
                    }
                    if ui.button("Detailed Report…").clicked(){d.report=Some(detailed_report(&d));}
                    if ui.button("Drawing Style…").clicked(){d.style_editor=Some(d.style.clone());}
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        if ui.add_sized([76.,24.],egui::Button::new("Cancel")).clicked(){keep=false;}
                        if ui.add_sized([76.,24.],egui::Button::new("OK")).clicked(){accept=true;}
                        ui.checkbox(&mut d.fit_width,"Fit To Width");
                    });
                });
                if !d.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&d.error);}
                if !ui.ctx().wants_keyboard_input() {
                    if ui.input_mut(|i|i.consume_key(egui::Modifiers::COMMAND,egui::Key::Z)){d.history.undo(&mut d.draft);}
                    if ui.input_mut(|i|i.consume_key(egui::Modifiers::COMMAND,egui::Key::Y)){d.history.redo(&mut d.draft);}
                    if ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)){keep=false;}
                }
                d.variant=d.variant.min(d.draft.len().saturating_sub(1));
            });
        });
        self.component_variation_editor(ctx, &mut d);
        variant_editor(ctx, &mut d);
        drawing_style(ctx, &mut d);
        component_links(ctx, &mut d);
        alternate_editor(ctx, &mut d);
        if let Some(report) = d.report.clone() {
            let mut close = false;
            egui::Modal::new(egui::Id::new("variant_report")).show(ctx, |ui| {
                ui.set_width(850.0_f32.min(ctx.available_rect().width() - 80.));
                close = title(ui, "Detailed Variant Report");
                egui::ScrollArea::both().max_height(480.).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut report.as_str())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Bill of Materials CSV…").clicked() {
                        match bom_report(&d) {
                            Ok(bom) => {
                                if let Some(path) = self
                                    .file_dialog
                                    .clone()
                                    .set_file_name("Variant BOM.csv")
                                    .add_filter("CSV", &["csv"])
                                    .save_file()
                                    && let Err(e) =
                                        eda_io::project_files::write_atomic(&path, bom.as_bytes())
                                {
                                    d.error = e;
                                }
                            }
                            Err(e) => d.error = e,
                        }
                    }
                    if ui.button("Save CSV…").clicked()
                        && let Some(path) = self
                            .file_dialog
                            .clone()
                            .set_file_name("Variant Report.csv")
                            .add_filter("CSV", &["csv"])
                            .save_file()
                        && let Err(e) =
                            eda_io::project_files::write_atomic(&path, report.as_bytes())
                    {
                        d.error = e;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
            if close {
                d.report = None;
            }
        }
        if accept {
            let result = (|| {
                model::validate(&d.draft)?;
                // Refuse a manager that was left open while a base component changed.
                let i = self
                    .projects
                    .iter()
                    .position(|p| p.path == d.project)
                    .ok_or("Project was closed")?;
                if self.projects[i].variants != d.original {
                    return Err("Variants changed; reopen the manager".into());
                }
                self.variants.catalog.borrow_mut().take();
                let current = self.variant_rows(i)?;
                if current.len() != d.rows.len()
                    || current
                        .iter()
                        .zip(d.rows.iter())
                        .any(|(a, b)| a.uid != b.uid || a.document != b.document)
                {
                    return Err("Project components changed; reopen the manager".into());
                }
                self.apply_variants(i, d.draft.clone())?;
                self.settings.variant_style = d.style.clone();
                self.variants.cache = None;
                Ok(())
            })();
            match result {
                Ok(()) => keep = false,
                Err(e) => d.error = e,
            }
        }
        if keep {
            self.variants.dialog = Some(d);
        }
    }

    fn component_variation_editor(&mut self, ctx: &egui::Context, d: &mut Dialog) {
        let Some(mut kind) = d.component_kind.take() else {
            return;
        };
        let mut keep = true;
        let mut choose = false;
        egui::Modal::new(egui::Id::new("edit_component_variation")).show(ctx, |ui| {
            ui.set_width(370.);
            if title(ui, "Edit Component Variation") {
                keep = false;
            }
            ui.label("Variation Kind");
            ui.horizontal(|ui| {
                for (value, label) in [(0, "Fitted"), (1, "Not Fitted"), (2, "Alternate Part")] {
                    if ui
                        .add_sized(
                            [118., 24.],
                            egui::Button::new(label).selected(kind == value),
                        )
                        .clicked()
                    {
                        kind = value;
                    }
                }
            });
            if kind == 2 {
                let alternate = d
                    .rows
                    .get(d.row)
                    .and_then(|r| {
                        d.draft
                            .get(d.variant)
                            .and_then(|v| v.variation(&r.uid, &r.designator))
                    })
                    .and_then(|c| c.alternate.as_ref())
                    .and_then(|a| a.components.first());
                if let Some(c) = alternate {
                    egui::Grid::new("alternate_component_details").show(ui, |ui| {
                        for (key, value) in [
                            ("Design Item ID", c.library_reference.as_str()),
                            ("Comment", c.comment.as_str()),
                            ("Description", c.description.as_str()),
                            ("Source", c.settings.source.as_str()),
                        ] {
                            ui.label(key);
                            ui.label(value);
                            ui.end_row();
                        }
                    });
                }
                choose = ui.button("Replace Component?").clicked();
            }
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
                if ui.button("OK").clicked()
                    && let Some(v) = d.draft.get(d.variant)
                {
                    if kind < 2 {
                        d.commit(model::Edit::SetKind {
                            id: v.id.clone(),
                            components: d.targets(),
                            kind,
                        });
                        keep = false;
                    } else {
                        let mut next = d.draft.clone();
                        let result = (|| {
                            for c in d.targets() {
                                let change = next[d.variant].variation_mut(&c.uid, &c.designator);
                                if change.alternate.is_none() {
                                    return Err("Select an alternate component first".into());
                                }
                                change.kind = 2;
                            }
                            d.history.commit(&mut d.draft, next)?;
                            Ok::<_, String>(())
                        })();
                        match result {
                            Ok(()) => {
                                keep = false;
                                d.error.clear();
                            }
                            Err(e) => d.error = e,
                        }
                    }
                }
            });
            if !d.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &d.error);
            }
        });
        if choose {
            self.choose_variant_alternate(d, d.row);
            if d.library.is_some() {
                keep = false;
            }
        }
        if keep {
            d.component_kind = Some(kind);
        }
    }

    fn choose_variant_alternate(&mut self, d: &mut Dialog, n: usize) {
        let Some(path) = self
            .file_dialog
            .clone()
            .add_filter("Schematic Library", &["SchLib", "IntLib", "PrjPcbVariants"])
            .pick_file()
        else {
            return;
        };
        match eda_io::altium::library::load(&path) {
            Ok(lib) => {
                d.library = Some(lib);
                d.row = n;
                d.error.clear();
            }
            Err(e) => d.error = e,
        }
    }
}

fn alternate_editor(ctx: &egui::Context, d: &mut Dialog) {
    let Some(lib) = d.library.take() else { return };
    let mut keep = true;
    let mut selected = None;
    egui::Modal::new(egui::Id::new("variant_alternate")).show(ctx, |ui| {
        ui.set_width(620.0_f32.min(ctx.available_rect().width() - 80.));
        if title(ui, "Choose Alternate Part") {
            keep = false;
        }
        ui.label(&lib.name);
        egui::ScrollArea::vertical()
            .max_height(400.)
            .show(ui, |ui| {
                for (n, symbol) in lib.symbols.iter().enumerate() {
                    if ui
                        .selectable_label(
                            false,
                            format!("{} — {}", symbol.name, symbol.description),
                        )
                        .clicked()
                    {
                        selected = Some(n);
                    }
                }
            });
        for warning in &lib.warnings {
            ui.colored_label(Color32::YELLOW, warning);
        }
        if !d.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, &d.error);
        }
        if ui.button("Cancel").clicked() {
            keep = false;
        }
    });
    if let Some(n) = selected {
        let result = (|| {
            let symbol = &lib.symbols[n];
            let mut next = d.draft.clone();
            let v = next.get_mut(d.variant).ok_or("Select a variant")?;
            for n in &d.selected {
                let row = &d.rows[*n];
                let c = row.component();
                let s = row.document.schematic.as_ref().unwrap();
                let a = symbol.instantiate(c.part, c.display_mode, s)?;
                model::alternate_for(s, c, &a)?;
                let change = v.variation_mut(&row.uid, &row.designator);
                change.kind = 2;
                change.alternate = Some(a);
                change
                    .fields
                    .insert("AltLibLink_DesignItemID".into(), symbol.name.clone());
                change
                    .fields
                    .insert("AlternatePart".into(), symbol.name.clone());
            }
            d.history.commit(&mut d.draft, next)?;
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => {
                keep = false;
                d.error.clear();
            }
            Err(e) => d.error = e,
        }
    }
    if keep {
        d.library = Some(lib);
    }
}

struct ParameterProperties {
    original: Option<String>,
    name: String,
    value: String,
}
fn variant_editor(ctx: &egui::Context, d: &mut Dialog) {
    let Some(mut editor) = d.editor.take() else {
        return;
    };
    let mut keep = true;
    egui::Modal::new(egui::Id::new("edit_project_variant")).show(ctx, |ui| {
        ui.set_width(620.0_f32.min(ctx.available_rect().width() - 80.));
        ui.add_enabled_ui(editor.parameter.is_none(), |ui| {
            if title(ui, "Edit Project Variant") {
                keep = false;
            }
            ui.label("Description");
            ui.add(egui::TextEdit::singleline(&mut editor.value.name).desired_width(f32::INFINITY));
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut false, "Allow variation of fabrication outputs"),
            );
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut false, "Allow variation for paste mask"),
            );
            ui.label("Parameters");
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_height(270.);
                egui::ScrollArea::vertical()
                    .max_height(300.)
                    .show(ui, |ui| {
                        egui::Grid::new("global_variant_parameters")
                            .spacing([1., 0.])
                            .striped(true)
                            .show(ui, |ui| {
                                cell(ui, "Name", 290., false);
                                cell(ui, "Value", 290., false);
                                ui.end_row();
                                for (name, value) in &editor.value.parameters {
                                    for text in [name, value] {
                                        let response = cell(
                                            ui,
                                            text,
                                            290.,
                                            editor.selected.as_ref() == Some(name),
                                        );
                                        if response.clicked() {
                                            editor.selected = Some(name.clone());
                                        }
                                        if response.double_clicked() {
                                            editor.parameter = Some(ParameterProperties {
                                                original: Some(name.clone()),
                                                name: name.clone(),
                                                value: value.clone(),
                                            });
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            });
            ui.horizontal(|ui| {
                if ui.button("Add?").clicked() {
                    editor.parameter = Some(ParameterProperties {
                        original: None,
                        name: String::new(),
                        value: String::new(),
                    });
                }
                let selected = editor
                    .selected
                    .clone()
                    .filter(|name| editor.value.parameters.contains_key(name));
                if ui
                    .add_enabled(selected.is_some(), egui::Button::new("Remove?"))
                    .clicked()
                {
                    editor.value.parameters.remove(selected.as_ref().unwrap());
                    editor.selected = None;
                }
                if ui
                    .add_enabled(selected.is_some(), egui::Button::new("Edit?"))
                    .clicked()
                {
                    let name = selected.unwrap();
                    editor.parameter = Some(ParameterProperties {
                        original: Some(name.clone()),
                        value: editor.value.parameters[&name].clone(),
                        name,
                    });
                }
            });
            if !editor.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &editor.error);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
                if ui.button("OK").clicked() {
                    editor.value.name = editor.value.name.trim().into();
                    let edit = match &editor.original {
                        Some(id) => model::Edit::Replace {
                            id: id.clone(),
                            value: editor.value.clone(),
                        },
                        None => model::Edit::Add(editor.value.clone()),
                    };
                    match model::edited(&d.draft, edit)
                        .and_then(|next| d.history.commit(&mut d.draft, next))
                    {
                        Ok(_) => {
                            d.variant = d
                                .draft
                                .iter()
                                .position(|v| v.id == editor.value.id)
                                .unwrap_or(0);
                            d.error.clear();
                            keep = false;
                        }
                        Err(e) => editor.error = e,
                    }
                }
            });
        });
    });
    if let Some(mut parameter) = editor.parameter.take() {
        let mut parameter_keep = true;
        egui::Modal::new(egui::Id::new("variant_parameter_properties")).show(ctx, |ui| {
            ui.set_width(465.);
            if title(ui, "Parameter Properties") {
                parameter_keep = false;
            }
            egui::Grid::new("variant_parameter_properties_fields").show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut parameter.name).desired_width(410.));
                ui.end_row();
                ui.label("Value");
                ui.add(egui::TextEdit::singleline(&mut parameter.value).desired_width(410.));
                ui.end_row();
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    parameter_keep = false;
                }
                if ui.button("OK").clicked() {
                    let name = parameter.name.trim();
                    if name.is_empty()
                        || editor.value.parameters.keys().any(|k| {
                            k.eq_ignore_ascii_case(name) && Some(k) != parameter.original.as_ref()
                        })
                    {
                        editor.error = "Parameter names must be nonempty and unique".into();
                    } else {
                        if let Some(old) = &parameter.original {
                            editor.value.parameters.remove(old);
                            if let Some(fields) =
                                editor.value.parameter_fields.remove(&old.to_lowercase())
                            {
                                editor
                                    .value
                                    .parameter_fields
                                    .insert(name.to_lowercase(), fields);
                            }
                        }
                        editor
                            .value
                            .parameters
                            .insert(name.into(), parameter.value.clone());
                        editor.selected = Some(name.into());
                        editor.error.clear();
                        parameter_keep = false;
                    }
                }
            });
            if !editor.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &editor.error);
            }
        });
        if parameter_keep {
            editor.parameter = Some(parameter);
        }
    }
    if keep {
        d.editor = Some(editor);
    }
}

fn drawing_style(ctx: &egui::Context, d: &mut Dialog) {
    let Some(mut style) = d.style_editor.take() else {
        return;
    };
    let mut keep = true;
    egui::Modal::new(egui::Id::new("variant_drawing_style")).show(ctx, |ui| {
        ui.set_width(690.0_f32.min(ctx.available_rect().width() - 80.));
        if title(ui, "Drawing Style") {
            keep = false;
        }
        ui.group(|ui| {
            ui.strong("Schematic Drawing Options");
            ui.label("Graphics drawn over components that are not fitted.");
            ui.checkbox(&mut style.dim, "Mask Not Fitted Components");
            let mut graphics = style.cross || style.mesh;
            if ui.checkbox(&mut graphics, "Use Graphics").changed() {
                style.cross = graphics;
                style.mesh = false;
            }
            ui.add_enabled_ui(graphics, |ui| {
                if ui.radio(style.cross, "Use Red Cross").clicked() {
                    style.cross = true;
                    style.mesh = false;
                }
                if ui.radio(style.mesh, "Use Gray Box and Mesh").clicked() {
                    style.mesh = true;
                    style.cross = false;
                }
            });
            let mut text = !style.not_fitted_text.is_empty();
            if ui.checkbox(&mut text, "Use Text").changed() {
                style.not_fitted_text = if text {
                    "Not Fitted".into()
                } else {
                    String::new()
                };
            }
            ui.add_enabled(text, egui::TextEdit::singleline(&mut style.not_fitted_text));
            ui.horizontal(|ui| {
                ui.label("Graphics Color");
                color_edit(ui, &mut style.color);
                ui.label("Line Width (mm)");
                let mut mm = style.width_nm as f64 / 1e6;
                if ui
                    .add(egui::DragValue::new(&mut mm).range(0.01..=10.).speed(0.01))
                    .changed()
                {
                    style.width_nm = (mm * 1e6).round() as i64;
                }
            });
            ui.separator();
            ui.label("Appearance of parameter variations");
            ui.checkbox(&mut style.modify_parameters, "Modify Text and Font");
            ui.add_enabled_ui(style.modify_parameters, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Suffix");
                    ui.text_edit_singleline(&mut style.parameter_suffix);
                    ui.label("Font Color");
                    color_edit(ui, &mut style.parameter_color);
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut style.parameter_italic, "Italic");
                    ui.checkbox(&mut style.parameter_bold, "Bold");
                });
            });
        });
        ui.group(|ui| {
            ui.strong("PCB Drawing Options");
            ui.add_enabled(
                false,
                egui::RadioButton::new(true, "Hide Not Fitted Components"),
            );
            ui.add_enabled(
                false,
                egui::RadioButton::new(false, "Display Not Fitted Components in Draft Mode"),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Cancel").clicked() {
                keep = false;
            }
            if ui.button("OK").clicked() {
                d.style = style.clone();
                keep = false;
            }
        });
    });
    if keep {
        d.style_editor = Some(style);
    }
}
fn color_edit(ui: &mut egui::Ui, color: &mut u32) {
    let mut rgb = [*color as u8, (*color >> 8) as u8, (*color >> 16) as u8];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        *color = rgb[0] as u32 | ((rgb[1] as u32) << 8) | ((rgb[2] as u32) << 16);
    }
}

fn component_links(ctx: &egui::Context, d: &mut Dialog) {
    if !d.links {
        return;
    }
    let refs = d.rows.iter().map(Row::reference).collect::<Vec<_>>();
    let mut edit = None;
    egui::Modal::new(egui::Id::new("variant_component_links")).show(ctx, |ui| {
        ui.set_width(760.0_f32.min(ctx.available_rect().width() - 80.));
        if title(ui, "Component Links") {
            d.links = false;
        }
        ui.label("Unresolved variation links. Select the physical component to relink explicitly.");
        egui::ScrollArea::both().max_height(400.).show(ui, |ui| {
            let mut count = 0;
            for v in &d.draft {
                for n in model::unresolved(v, &refs) {
                    count += 1;
                    let c = &v.variations[n];
                    ui.horizontal(|ui| {
                        ui.label(format!("{} / {}", v.name, c.designator))
                            .on_hover_text(&c.uid);
                        egui::ComboBox::from_id_salt(("repair_variant_link", &v.id, n))
                            .selected_text("Choose component…")
                            .show_ui(ui, |ui| {
                                for r in d.rows.iter() {
                                    if ui
                                        .selectable_label(
                                            false,
                                            format!(
                                                "{} — {} {}",
                                                r.designator, r.document.name, r.path
                                            ),
                                        )
                                        .clicked()
                                    {
                                        edit = Some(model::Edit::Rebind {
                                            id: v.id.clone(),
                                            index: n,
                                            component: r.reference(),
                                        });
                                        ui.close();
                                    }
                                }
                            });
                    });
                }
            }
            if count == 0 {
                ui.label("All variation links are resolved.");
            }
        });
        if ui.button("Close").clicked() {
            d.links = false;
        }
        if !d.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, &d.error);
        }
    });
    if let Some(edit) = edit {
        d.commit(edit);
    }
}

fn csv_row(output: &mut String, values: &[&str]) {
    output.push_str(
        &values
            .iter()
            .map(|v| format!("\"{}\"", v.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(","),
    );
    output.push_str("\r\n");
}
fn detailed_report(d: &Dialog) -> String {
    let mut out = String::from("\u{feff}");
    csv_row(
        &mut out,
        &[
            "Variant",
            "Hierarchy Path",
            "Designator",
            "Document",
            "Status",
            "Parameter",
            "Original Value",
            "Variant Value",
        ],
    );
    for v in &d.draft {
        for row in d.rows.iter() {
            let change = v.variation(&row.uid, &row.designator);
            let state = match change.map_or(0, |c| c.kind) {
                1 => "Not Fitted",
                2 => "Alternate Part",
                _ => "Fitted",
            };
            csv_row(
                &mut out,
                &[
                    &v.name,
                    &row.path,
                    &row.designator,
                    &row.document.name,
                    state,
                    "",
                    "",
                    "",
                ],
            );
            if let Some(change) = change {
                let base = row.base_parameters();
                for (key, value) in &change.parameters {
                    csv_row(
                        &mut out,
                        &[
                            &v.name,
                            &row.path,
                            &row.designator,
                            &row.document.name,
                            state,
                            key,
                            field(&base, key).map_or("", String::as_str),
                            value,
                        ],
                    );
                }
            }
        }
        for (name, value) in &v.parameters {
            csv_row(
                &mut out,
                &[&v.name, "", "", "", "Variant Parameter", name, "", value],
            );
        }
        for n in model::unresolved(v, &d.rows.iter().map(Row::reference).collect::<Vec<_>>()) {
            let c = &v.variations[n];
            csv_row(
                &mut out,
                &[
                    &v.name,
                    &c.uid,
                    &c.designator,
                    "",
                    "Unresolved Link",
                    "",
                    "",
                    "",
                ],
            );
        }
    }
    out
}

fn bom_report(d: &Dialog) -> Result<String, String> {
    let variant = d.draft.get(d.variant);
    let refs = d.rows.iter().map(Row::reference).collect::<Vec<_>>();
    if variant.is_some_and(|v| !model::unresolved(v, &refs).is_empty()) {
        return Err("Resolve component links before generating the assembly BOM".into());
    }
    let mut out = String::from("\u{feff}");
    csv_row(
        &mut out,
        &[
            "Variant",
            "Hierarchy Path",
            "Designator",
            "Comment",
            "Library Reference",
            "Description",
            "Footprint",
            "Quantity",
        ],
    );
    let mut seen = BTreeSet::new();
    for row in d.rows.iter() {
        let s = row.document.schematic.as_ref().unwrap();
        let Some(c) = model::assembly_component(s, row.component(), variant, &row.path)? else {
            continue;
        };
        if matches!(c.settings.kind, 2 | 4 | 5) {
            continue;
        }
        if c.settings.part_count > 1 {
            return Err(
                "BOM for multipart components requires complete physical part folding".into(),
            );
        }
        if !seen.insert(row.uid.to_uppercase()) {
            continue;
        }
        let footprint = c
            .settings
            .models
            .iter()
            .find(|m| m.current && m.kind.eq_ignore_ascii_case("PCBLIB"))
            .map_or("", |m| m.name.as_str());
        csv_row(
            &mut out,
            &[
                variant.map_or("[No Variations]", |v| v.name.as_str()),
                &row.path,
                &row.designator,
                &c.comment,
                &c.library_reference,
                &c.description,
                footprint,
                "1",
            ],
        );
    }
    Ok(out)
}
