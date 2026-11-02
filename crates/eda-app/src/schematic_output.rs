//! Print preview and Smart PDF use snapshots of the current editable documents.
use super::*;
use eda_io::schematic_pdf::{Bookmark, Page};
use std::sync::mpsc;

pub(super) struct Dialog {
    smart: bool,
    step: usize,
    project: Vec<Document>,
    project_options: Result<eda_core::design_sync::ProjectOptions, String>,
    current: Document,
    all: bool,
    checked: Vec<bool>,
    page: usize,
    drawings: bool,
    print_layout: eda_render::schematic_output::Layout,
    paper: usize,
    landscape: bool,
    output: String,
    color: u8,
    dpi: usize,
    components: bool,
    nets: bool,
    error: String,
    job: Option<mpsc::Receiver<Result<JobResult, String>>>,
}
enum JobResult {
    Pdf(PathBuf),
    Print(Vec<Page>),
}

impl PcbCraft {
    pub(super) fn open_schematic_output(&mut self, smart: bool, _ctx: &egui::Context) {
        let Some(index) = self.active else { return };
        if self.documents[index].model.schematic.is_none() {
            return;
        }
        let current = self
            .variant_projection(index)
            .as_deref()
            .unwrap_or(&self.documents[index].model)
            .clone();
        let mut snapshots = match self.schematic_project_snapshots() {
            Ok(sheets) => sheets,
            Err(error) => {
                self.message(true, format!("Cannot prepare project output: {error}"));
                return;
            }
        };
        if let Err(e) = self.variant_output_snapshots(&mut snapshots, index) {
            self.message(true, e);
            return;
        }
        let project_options = if let Some(p) = self.file_project_index() {
            let sheets: Vec<_> = snapshots.iter().map(|(p, d)| (p.as_path(), d)).collect();
            self.projects[p].compile_options(&sheets)
        } else {
            Ok(Default::default())
        };
        let project = snapshots.into_iter().map(|(_, d)| d).collect::<Vec<_>>();
        let output = self.documents[index]
            .path
            .as_ref()
            .map(|p| p.with_extension("pdf"))
            .unwrap_or_else(|| PathBuf::from("Schematic.pdf"));
        let checked = vec![true; project.len()];
        let landscape = current.size.x >= current.size.y;
        let mut print_layout = eda_render::schematic_output::Layout::document(&current);
        print_layout.paper_mm = if landscape {
            [297., 210.]
        } else {
            [210., 297.]
        };
        print_layout.margin_mm = 5.;
        self.file_state.output = Some(Dialog {
            smart,
            step: 0,
            project,
            project_options,
            current,
            all: false,
            checked,
            page: 0,
            drawings: false,
            print_layout,
            paper: 0,
            landscape,
            output: output.display().to_string(),
            color: 0,
            dpi: 300,
            components: true,
            nets: true,
            error: String::new(),
            job: None,
        });
    }
    pub(super) fn schematic_output_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.file_state.output.take() else {
            return;
        };
        let mut keep = true;
        let mut start = false;
        egui::Modal::new(egui::Id::new("schematic_output")).show(ctx, |ui| {
            ui.set_min_size(if dialog.smart {
                egui::vec2(860., 510.)
            } else {
                egui::vec2(1020., 690.)
            });
            ui.heading(if dialog.smart {
                "Smart PDF"
            } else {
                "Preview SCH"
            });
            ui.separator();
            ui.add_enabled_ui(dialog.job.is_none(), |ui| {
                if dialog.smart {
                    smart_page(ui, &mut dialog, &self.file_dialog);
                } else {
                    print_page(ui, &mut dialog);
                }
            });
            if !dialog.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &dialog.error);
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    if dialog.job.is_some() {
                        ui.spinner();
                        ui.label("Rendering schematic pages…");
                    }
                    ui.add_space(
                        (ui.available_width() - if dialog.smart { 210. } else { 110. }).max(0.),
                    );
                    if !dialog.smart
                        && ui
                            .add_enabled(dialog.job.is_none(), egui::Button::new("Print…"))
                            .clicked()
                    {
                        start = true;
                    }
                    if ui
                        .add_enabled(dialog.job.is_none(), egui::Button::new("Cancel"))
                        .clicked()
                    {
                        keep = false;
                    }
                    if dialog.smart {
                        if ui
                            .add_enabled(
                                dialog.job.is_none() && dialog.step > 0,
                                egui::Button::new("Back"),
                            )
                            .clicked()
                        {
                            dialog.step -= 1;
                        }
                        if ui
                            .add_enabled(
                                dialog.job.is_none() && dialog.step < 4,
                                egui::Button::new("Next"),
                            )
                            .clicked()
                        {
                            dialog.step += 1;
                        }
                        if ui
                            .add_enabled(
                                dialog.job.is_none() && dialog.step == 4,
                                egui::Button::new("Finish"),
                            )
                            .clicked()
                        {
                            start = true;
                        }
                    }
                });
            });
        });
        if dialog.job.is_none()
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            keep = false;
        }
        if start {
            let docs = selected(&dialog);
            if docs.is_empty() {
                dialog.error = "Select at least one schematic drawing".into();
            } else {
                let path = if dialog.smart {
                    self.file_dialog
                        .clone()
                        .add_filter("PDF Document", &["pdf"])
                        .set_file_name(&dialog.output)
                        .save_file()
                } else {
                    Some(PathBuf::new())
                };
                if let Some(path) = path {
                    if dialog.smart
                        && let Err(error) =
                            super::schematic_netlist::export_destination(&path, "pdf")
                    {
                        dialog.error = error;
                        self.file_state.output = Some(dialog);
                        return;
                    }
                    let (tx, rx) = mpsc::sync_channel(1);
                    dialog.job = Some(rx);
                    dialog.error.clear();
                    let color = dialog.color;
                    let dpi = dialog.dpi;
                    let components = dialog.smart && dialog.components;
                    let nets = dialog.smart && dialog.nets;
                    let smart = dialog.smart;
                    let layout = (!smart).then_some(dialog.print_layout);
                    let net_documents = if dialog.all {
                        dialog.project.clone()
                    } else {
                        vec![dialog.current.clone()]
                    };
                    let options = if dialog.all {
                        dialog.project_options.clone()
                    } else {
                        Ok(Default::default())
                    };
                    let repaint = ctx.clone();
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(|| {
                            let netlist = if nets {
                                let options = options?;
                                let sheets: Vec<_> = net_documents.iter().collect();
                                let compiled =
                                    eda_core::design_sync::compile_project(&sheets, &options)?;
                                if !compiled.issues.is_empty() {
                                    return Err(format!(
                                        "Cannot generate PDF net information: {}",
                                        compiled.issues.join("; ")
                                    ));
                                }
                                Some(compiled)
                            } else {
                                None
                            };
                            render_pages(&docs, dpi, color, components, netlist.as_ref(), layout)
                                .and_then(|pages| {
                                    if smart {
                                        eda_io::schematic_pdf::save(&path, &pages)?;
                                        Ok(JobResult::Pdf(path))
                                    } else {
                                        Ok(JobResult::Print(pages))
                                    }
                                })
                        })
                        .unwrap_or_else(|_| Err("Schematic rendering worker failed".into()));
                        let _ = tx.send(result);
                        repaint.request_repaint();
                    });
                }
            }
        }
        if let Some(job) = &dialog.job {
            match job.try_recv() {
                Ok(result) => {
                    ctx.request_repaint();
                    dialog.job = None;
                    match result {
                        Ok(JobResult::Pdf(path)) => {
                            self.message(false, format!("Created PDF {}", path.display()));
                            keep = false;
                        }
                        Ok(JobResult::Print(pages)) => match print_native(&pages) {
                            Ok(true) => {
                                self.message(false, "Schematic print job submitted");
                                keep = false;
                            }
                            Ok(false) => {}
                            Err(e) => dialog.error = e,
                        },
                        Err(e) => dialog.error = e,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    dialog.job = None;
                    dialog.error = "Schematic output worker stopped".into();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
        }
        if keep {
            self.file_state.output = Some(dialog);
        }
    }
}

fn selected(d: &Dialog) -> Vec<Document> {
    if d.all {
        d.project
            .iter()
            .zip(&d.checked)
            .filter(|(_, yes)| **yes)
            .map(|(doc, _)| doc.clone())
            .collect()
    } else {
        vec![d.current.clone()]
    }
}
fn scope(ui: &mut egui::Ui, d: &mut Dialog) {
    ui.add_enabled_ui(!d.project.is_empty(), |ui| {
        ui.radio_value(&mut d.all, true, "Current Project")
    });
    ui.radio_value(
        &mut d.all,
        false,
        format!("Current Document ({})", d.current.name),
    );
}
fn color_controls(ui: &mut egui::Ui, d: &mut Dialog) {
    ui.label("Color Set");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut d.color, 1, "Gray");
        ui.selectable_value(&mut d.color, 2, "Mono");
        ui.selectable_value(&mut d.color, 0, "Color");
    });
}
fn smart_page(ui: &mut egui::Ui, d: &mut Dialog, file_dialog: &rfd::FileDialog) {
    ui.add_space(12.);
    match d.step {
        0 => {
            ui.add_space(130.);
            ui.heading("PCBCraft Smart PDF");
            ui.label(
                "This Wizard will guide you through the steps required to\nexport a design to PDF.",
            );
        }
        1 => {
            ui.strong("Choose Export Target");
            ui.label("Smart PDF can export the currently viewed document or the entire project.");
            ui.add_space(35.);
            scope(ui, d);
            ui.add_space(24.);
            ui.label("Output File Name:");
            ui.horizontal(|ui| {
                ui.add_sized([710., 22.], egui::TextEdit::singleline(&mut d.output));
                if ui.button("…").clicked()
                    && let Some(path) = file_dialog
                        .clone()
                        .add_filter("PDF Document", &["pdf"])
                        .set_file_name(&d.output)
                        .save_file()
                {
                    d.output = path.display().to_string();
                }
            });
        }
        2 => {
            ui.strong("Export Bill of Materials");
            ui.add_space(24.);
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut false, "Export a Bill of Materials"),
            );
            ui.label("Excel BOM templates are not available.");
        }
        3 => {
            ui.strong("Additional PDF Settings");
            ui.add_space(20.);
            ui.columns(2, |columns| {
                columns[0].strong("Additional Information");
                columns[0].checkbox(&mut d.components, "Include Component Parameters");
                columns[0].checkbox(&mut d.nets, "Generate nets information");
                columns[0]
                    .label("PDF bookmarks navigate to components, pins, net labels and ports.");
                color_controls(&mut columns[1], d);
                columns[1].label("Quality");
                egui::ComboBox::from_id_salt("pdf_dpi")
                    .selected_text(format!("{} dpi", d.dpi))
                    .show_ui(&mut columns[1], |ui| {
                        for dpi in [150, 300, 600] {
                            ui.selectable_value(&mut d.dpi, dpi, format!("{dpi} dpi"));
                        }
                    });
            });
        }
        _ => {
            ui.strong("Choose Documents to Export");
            ui.add_space(15.);
            if d.all {
                for (doc, checked) in d.project.iter().zip(&mut d.checked) {
                    ui.checkbox(checked, &doc.name);
                }
            } else {
                ui.label(&d.current.name);
            }
            ui.add_space(30.);
            ui.label("Click Finish to generate the PDF.");
            ui.label("The editable sheets are exported without grid or selection overlays.");
        }
    }
}
const PAPERS: [(&str, f64, f64); 7] = [
    ("A4", 210., 297.),
    ("A3", 297., 420.),
    ("A2", 420., 594.),
    ("A1", 594., 841.),
    ("A0", 841., 1189.),
    ("Letter", 215.9, 279.4),
    ("Legal", 215.9, 355.6),
];
fn print_page(ui: &mut egui::Ui, d: &mut Dialog) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut d.drawings, false, "General");
        ui.selectable_value(&mut d.drawings, true, "Drawings");
    });
    ui.columns(2, |columns| {
        columns[0].set_max_width(350.);
        if d.drawings {
            columns[0].strong("Documents");
            scope(&mut columns[0], d);
            if d.all {
                for (doc, checked) in d.project.iter().zip(&mut d.checked) {
                    columns[0].checkbox(checked, &doc.name);
                }
            }
        } else {
            columns[0].strong("Printer & Presets Settings");
            columns[0].label("Choose printer, copies and page range in the system Print dialog.");
            columns[0].add_space(18.);
            scope(&mut columns[0], d);
            columns[0].add_space(25.);
            columns[0].strong("Page Settings");
            color_controls(&mut columns[0], d);
            columns[0].horizontal(|ui| {
                ui.label("Page Size");
                egui::ComboBox::from_id_salt("print_page_size")
                    .selected_text(PAPERS[d.paper].0)
                    .show_ui(ui, |ui| {
                        for (n, paper) in PAPERS.iter().enumerate() {
                            ui.selectable_value(&mut d.paper, n, paper.0);
                        }
                    });
            });
            columns[0].horizontal(|ui| {
                ui.label("Orientation");
                ui.selectable_value(&mut d.landscape, false, "Portrait");
                ui.selectable_value(&mut d.landscape, true, "Landscape");
            });
            let paper = PAPERS[d.paper];
            d.print_layout.paper_mm = if d.landscape {
                [paper.2, paper.1]
            } else {
                [paper.1, paper.2]
            };
            columns[0].add_space(25.);
            columns[0].strong("Scale & Position Settings");
            columns[0].horizontal(|ui| {
                ui.label("Scale Mode");
                ui.selectable_value(&mut d.print_layout.fit, false, "Actual Size");
                ui.selectable_value(&mut d.print_layout.fit, true, "Fit On Page");
            });
            columns[0].add_enabled_ui(!d.print_layout.fit, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Scale");
                    ui.add(
                        egui::DragValue::new(&mut d.print_layout.scale_percent)
                            .range(1.0..=1000.0)
                            .suffix("%"),
                    );
                });
            });
            for (n, label) in ["Horizontal Offset", "Vertical Offset"].iter().enumerate() {
                columns[0].horizontal(|ui| {
                    ui.label(*label);
                    ui.add_enabled(
                        !d.print_layout.center[n],
                        egui::DragValue::new(&mut d.print_layout.offset_mm[n])
                            .range(-1000.0..=1000.0)
                            .suffix(" mm"),
                    );
                    if ui
                        .checkbox(&mut d.print_layout.center[n], "Center")
                        .changed()
                        && d.print_layout.center[n]
                    {
                        d.print_layout.offset_mm[n] = 0.;
                    }
                });
            }
        }
        let docs = selected(d);
        d.page = d.page.min(docs.len().saturating_sub(1));
        if let Some(doc) = docs.get(d.page) {
            let (available, _) = columns[1].allocate_exact_size(
                egui::vec2(columns[1].available_width(), 550.),
                egui::Sense::hover(),
            );
            let size = available.size();
            let ratio = d.print_layout.paper_mm[0] as f32 / d.print_layout.paper_mm[1] as f32;
            let width = size.x.min(size.y * ratio);
            let height = width / ratio;
            let rect = egui::Rect::from_center_size(available.center(), egui::vec2(width, height));
            let mut page_ui = columns[1].new_child(egui::UiBuilder::new().max_rect(available));
            eda_render::schematic_output::preview_layout(
                &mut page_ui,
                doc,
                rect,
                d.print_layout,
                d.color,
            );
            columns[1].horizontal(|ui| {
                if ui.add_enabled(d.page > 0, egui::Button::new("‹")).clicked() {
                    d.page -= 1;
                }
                ui.label(format!("Page {} of {}", d.page + 1, docs.len()));
                if ui
                    .add_enabled(d.page + 1 < docs.len(), egui::Button::new("›"))
                    .clicked()
                {
                    d.page += 1;
                }
            });
        }
    });
}

fn render_pages(
    docs: &[Document],
    dpi: usize,
    color: u8,
    components: bool,
    netlist: Option<&eda_core::design_sync::Netlist>,
    layout: Option<eda_render::schematic_output::Layout>,
) -> Result<Vec<Page>, String> {
    let mut pages = Vec::new();
    let mut net_marks = std::collections::BTreeMap::<String, Vec<(usize, usize, [f64; 4])>>::new();
    let mut total = 0usize;
    for doc in docs {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let layout = layout.unwrap_or_else(|| eda_render::schematic_output::Layout::document(doc));
        let width_pt = layout.paper_mm[0] * 72. / 25.4;
        let height_pt = layout.paper_mm[1] * 72. / 25.4;
        let mut width = (width_pt / 72. * dpi as f64).ceil() as usize;
        let mut height = (height_pt / 72. * dpi as f64).ceil() as usize;
        let factor = (24_000_000f64 / (width as f64 * height as f64))
            .sqrt()
            .min(1.);
        width = (width as f64 * factor).round() as usize;
        height = (height as f64 * factor).round() as usize;
        total = total.saturating_add(width * height * 3);
        if total > 512 * 1024 * 1024 {
            return Err(
                "Selected pages exceed the 512 MiB output memory limit; export a smaller range"
                    .into(),
            );
        }
        let mut raster =
            eda_render::schematic_output::raster_layout(&ctx, doc, width, height, layout)?;
        if color != 0 {
            for pixel in raster.rgb.chunks_exact_mut(3) {
                let gray =
                    (pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000;
                let v = if color == 2 {
                    if gray < 230 { 0 } else { 255 }
                } else {
                    gray as u8
                };
                pixel.fill(v);
            }
        }
        let mut bookmarks = Vec::new();
        let s = doc.schematic.as_ref().ok_or("Expected schematic")?;
        let rect = |bounds: eda_core::schematic_arrange::Bounds| {
            [
                bounds.min.x as f64 / doc.size.x as f64 * width_pt,
                bounds.min.y as f64 / doc.size.y as f64 * height_pt,
                bounds.max.x as f64 / doc.size.x as f64 * width_pt,
                bounds.max.y as f64 / doc.size.y as f64 * height_pt,
            ]
        };
        {
            for c in &s.components {
                if let Some(bounds) = eda_core::schematic_arrange::object_bounds(s, c.id) {
                    bookmarks.push(Bookmark {
                        destination: None,
                        title: c.designator.clone(),
                        rect: rect(bounds),
                        details: if components {
                            format!(
                                "{}\n{}\n{}",
                                c.designator,
                                c.description,
                                c.parameters
                                    .iter()
                                    .map(|(k, v)| format!("{k}: {v}"))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            )
                        } else {
                            c.designator.clone()
                        },
                    });
                }
            }
        }
        if let Some(netlist) = netlist {
            for p in &s.primitives {
                use eda_core::schematic::{SchPrimitiveKind as K, TextRole};
                let pin_net = if let K::Pin { number, .. } = &p.kind {
                    netlist
                        .pins
                        .iter()
                        .find(|pin| {
                            pin.sheet == doc.id
                                && Some(pin.component) == p.component
                                && pin.number == *number
                        })
                        .and_then(|pin| pin.net.as_deref())
                } else {
                    None
                };
                let title = match &p.kind {
                    K::Pin { number, .. } => p
                        .component
                        .and_then(|id| s.components.iter().find(|c| c.id == id))
                        .map(|c| {
                            let net = pin_net.unwrap_or("unconnected");
                            format!("{}.{} — {net}", c.designator, number)
                        }),
                    K::Text { text, .. } if p.text_role == Some(TextRole::NetLabel) => {
                        Some(text.clone())
                    }
                    K::Port { name, .. } | K::PowerPort { text: name, .. } => Some(name.clone()),
                    _ => None,
                };
                if let Some(title) = title
                    && let Some(bounds) = eda_core::schematic_arrange::primitive_bounds(s, p)
                {
                    if let Some(net) = pin_net {
                        net_marks.entry(net.into()).or_default().push((
                            pages.len(),
                            bookmarks.len(),
                            rect(bounds),
                        ));
                    }
                    bookmarks.push(Bookmark {
                        destination: None,
                        details: title.clone(),
                        title,
                        rect: rect(bounds),
                    });
                }
            }
        }
        pages.push(Page {
            title: doc.name.clone(),
            width_pt,
            height_pt,
            width: raster.width,
            height: raster.height,
            rgb: raster.rgb,
            bookmarks,
        });
    }
    // Cycle between visible physical pins of a compiled net. Prefer another sheet.
    // Unchecked drawing pages do not become dangling PDF destinations.
    for marks in net_marks.values() {
        for (index, (page, bookmark, _)) in marks.iter().enumerate() {
            let target = marks
                .iter()
                .cycle()
                .skip(index + 1)
                .take(marks.len().saturating_sub(1))
                .find(|(target, _, _)| target != page)
                .or_else(|| (marks.len() > 1).then(|| &marks[(index + 1) % marks.len()]));
            if let Some((target, _, rect)) = target {
                pages[*page].bookmarks[*bookmark].destination = Some((*target, *rect));
            }
        }
    }
    Ok(pages)
}

#[cfg(windows)]
fn print_native(pages: &[Page]) -> Result<bool, String> {
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        Graphics::Gdi::*,
        Storage::Xps::*,
        UI::{
            Controls::Dialogs::*,
            HiDpi::{GetThreadDpiAwarenessContext, SetThreadDpiAwarenessContext},
            Input::KeyboardAndMouse::GetActiveWindow,
        },
    };
    // The common dialog owns printer selection. No job is sent before Print.
    unsafe {
        let mut dialog: PRINTDLGW = std::mem::zeroed();
        dialog.hwndOwner = GetActiveWindow();
        dialog.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
        dialog.Flags = PD_RETURNDC | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOSELECTION;
        dialog.nMinPage = 1;
        dialog.nMaxPage = pages.len() as u16;
        dialog.nFromPage = 1;
        dialog.nToPage = pages.len() as u16;
        dialog.nCopies = 1;
        let dpi_context = GetThreadDpiAwarenessContext();
        let accepted = PrintDlgW(&mut dialog);
        SetThreadDpiAwarenessContext(dpi_context);
        if accepted == 0 {
            let error = CommDlgExtendedError();
            if !dialog.hDevMode.is_null() {
                GlobalFree(dialog.hDevMode);
            }
            if !dialog.hDevNames.is_null() {
                GlobalFree(dialog.hDevNames);
            }
            return if error == 0 {
                Ok(false)
            } else {
                Err(format!("Printer dialog failed: {error}"))
            };
        }
        let hdc = dialog.hDC;
        let name: Vec<u16> = "PCBCraft Schematic\0".encode_utf16().collect();
        let mut info: DOCINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<DOCINFOW>() as i32;
        info.lpszDocName = name.as_ptr();
        let result = (|| {
            if StartDocW(hdc, &info) <= 0 {
                return Err("Could not start printer document".into());
            }
            let width = GetDeviceCaps(hdc, HORZRES as i32);
            let height = GetDeviceCaps(hdc, VERTRES as i32);
            let from = if dialog.Flags & PD_PAGENUMS != 0 {
                dialog.nFromPage.saturating_sub(1) as usize
            } else {
                0
            };
            let to = if dialog.Flags & PD_PAGENUMS != 0 {
                dialog.nToPage as usize
            } else {
                pages.len()
            };
            SetStretchBltMode(hdc, HALFTONE);
            for _ in 0..dialog.nCopies.max(1) {
                for page in pages.iter().take(to).skip(from) {
                    if StartPage(hdc) <= 0 {
                        return Err("Could not start printer page".into());
                    }
                    let mut pixels = Vec::with_capacity(page.width * page.height * 4);
                    for rgb in page.rgb.chunks_exact(3) {
                        pixels.extend([rgb[2], rgb[1], rgb[0], 0]);
                    }
                    let mut bitmap: BITMAPINFO = std::mem::zeroed();
                    bitmap.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    bitmap.bmiHeader.biWidth = page.width as i32;
                    bitmap.bmiHeader.biHeight = -(page.height as i32);
                    bitmap.bmiHeader.biPlanes = 1;
                    bitmap.bmiHeader.biBitCount = 32;
                    bitmap.bmiHeader.biCompression = BI_RGB;
                    let scale =
                        (width as f64 / page.width as f64).min(height as f64 / page.height as f64);
                    let w = (page.width as f64 * scale) as i32;
                    let h = (page.height as f64 * scale) as i32;
                    if StretchDIBits(
                        hdc,
                        (width - w) / 2,
                        (height - h) / 2,
                        w,
                        h,
                        0,
                        0,
                        page.width as i32,
                        page.height as i32,
                        pixels.as_ptr().cast(),
                        &bitmap,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    ) == GDI_ERROR
                    {
                        return Err("Printer raster transfer failed".into());
                    }
                    if EndPage(hdc) <= 0 {
                        return Err("Could not finish printer page".into());
                    }
                }
            }
            if EndDoc(hdc) <= 0 {
                return Err("Could not finish printer document".into());
            }
            Ok(true)
        })();
        if result.is_err() {
            AbortDoc(hdc);
        }
        DeleteDC(hdc);
        if !dialog.hDevMode.is_null() {
            GlobalFree(dialog.hDevMode);
        }
        if !dialog.hDevNames.is_null() {
            GlobalFree(dialog.hDevNames);
        }
        result
    }
}
#[cfg(not(windows))]
fn print_native(pages: &[Page]) -> Result<bool, String> {
    let path = std::env::temp_dir().join(format!("pcbcraft-print-{}.pdf", Uuid::new_v4()));
    eda_io::schematic_pdf::save(&path, pages)?;
    let result = std::process::Command::new("lp")
        .arg(&path)
        .output()
        .map_err(|e| format!("CUPS lp is unavailable: {e}"));
    let _ = std::fs::remove_file(&path);
    let result = result?;
    if result.status.success() {
        Ok(true)
    } else {
        Err(String::from_utf8_lossy(&result.stderr).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_net_links_target_another_selected_sheet() {
        use eda_core::schematic::SchPrimitiveKind as K;
        use eda_core::schematic_edit::{self as edit, Edit};
        let mut docs = vec![];
        for i in 1..=2 {
            let doc = Document::new(DocumentKind::Schematic, format!("Sheet{i}.SchDoc"));
            let mut doc = edit::preview(
                &doc,
                &Edit::Insert(
                    edit::place(
                        Placement::Resistor,
                        eda_geom::Point::mm(20, 20),
                        0,
                        &format!("R{i}"),
                        "10k",
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
            let s = doc.schematic.as_mut().unwrap();
            let position = s
                .primitives
                .iter()
                .find_map(|p| {
                    matches!(p.kind, K::Pin { .. }).then(|| edit::electrical_endpoint(p).unwrap())
                })
                .unwrap();
            s.primitives.push(edit::primitive(
                K::PowerPort {
                    cross_sheet: false,
                    position,
                    orientation: 0,
                    style: 2,
                    text: "VCC".into(),
                    show_name: true,
                    font: 0,
                },
                None,
            ));
            docs.push(doc);
        }
        let netlist = eda_core::design_sync::compile(&docs.iter().collect::<Vec<_>>()).unwrap();
        assert!(netlist.issues.is_empty());
        let pages = render_pages(&docs, 150, 0, true, Some(&netlist), None).unwrap();
        for (i, page) in pages.iter().enumerate() {
            let mark = page
                .bookmarks
                .iter()
                .find(|m| m.title.ends_with("— VCC"))
                .unwrap();
            assert_eq!(mark.destination.as_ref().unwrap().0, 1 - i);
        }
        if let Some(path) = std::env::var_os("PCBCRAFT_FILE_QA_PROJECT_PDF") {
            eda_io::schematic_pdf::save(PathBuf::from(path).as_path(), &pages).unwrap();
        }
        let selected = render_pages(&docs[..1], 150, 0, true, Some(&netlist), None).unwrap();
        assert!(
            selected[0]
                .bookmarks
                .iter()
                .all(|m| m.destination.is_none_or(|(page, _)| page == 0))
        );
    }
    #[test]
    fn current_scene_exports_pdf_with_component_and_net_bookmarks() {
        let doc = eda_io::altium::import_schematic(
            include_bytes!("../../../fixtures/schematic/minimal.SchDoc"),
            "PrintReference.SchDoc",
        )
        .unwrap();
        let netlist = eda_core::design_sync::compile(&[&doc]).unwrap();
        let pages = render_pages(&[doc], 150, 0, true, Some(&netlist), None).unwrap();
        assert!(pages[0].bookmarks.iter().any(|m| m.title == "R1"));
        assert!(
            pages[0]
                .bookmarks
                .iter()
                .any(|m| m.title.starts_with("R1."))
        );
        let bytes = eda_io::schematic_pdf::serialize(&pages).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        if let Some(path) = std::env::var_os("PCBCRAFT_FILE_QA_PDF") {
            eda_io::project_files::write_atomic(std::path::Path::new(&path), &bytes).unwrap();
        }
    }
}
