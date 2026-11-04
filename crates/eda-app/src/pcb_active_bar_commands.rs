use super::*;
use eda_core::pcb_arrange::{self, Align};

fn fixed_alignment(
    mut board: Board,
    selection: &[Uuid],
    grid: i64,
    mut operations: Vec<Align>,
) -> Result<(Board, Vec<Align>), String> {
    while let Some(&mode) = operations.first() {
        if matches!(mode, Align::CentreX | Align::CentreY) {
            break;
        }
        board = pcb_arrange::objects(&board, selection, mode, grid)?;
        operations.remove(0);
    }
    Ok((board, operations))
}

fn begin_alignment(
    doc: &mut OpenDocument,
    selection: Vec<Uuid>,
    operations: Vec<Align>,
) -> Result<(), String> {
    let (next, operations) = fixed_alignment(
        doc.model.pcb.as_ref().unwrap().clone(),
        &selection,
        doc.view.grid_nm,
        operations,
    )?;
    if operations.is_empty() {
        commit(doc, next)?;
    } else {
        doc.view.pcb.set_preview(Some(next.clone()));
        doc.pcb_editor.bar.align_pick = Some(AlignmentPick {
            selection,
            revision: doc.history.revision,
            operations,
            draft: std::sync::Arc::new(next),
        });
        doc.view.pcb.tool_active = true;
    }
    Ok(())
}

pub(super) fn cancel_alignment(doc: &mut OpenDocument) -> Result<(), String> {
    if let Some(pick) = doc.pcb_editor.bar.align_pick.take() {
        doc.view.pcb.clear_gesture();
        if pick.revision != doc.history.revision {
            return Err("Document changed before choosing the alignment reference".into());
        }
        commit(doc, (*pick.draft).clone())?;
    }
    Ok(())
}

fn alignment(id: &str) -> Option<Align> {
    Some(match id {
        "AlignComponentsLeft" => Align::Left,
        "AlignComponentsRight" => Align::Right,
        "AlignComponentsTop" => Align::Top,
        "AlignComponentsBottom" => Align::Bottom,
        "AlignComponentsCenterHorizontal" => Align::CentreX,
        "AlignComponentsCenterVertical" => Align::CentreY,
        "AlignComponentsSpreadHorizontal" => Align::DistributeX,
        "AlignComponentsSpreadVertical" => Align::DistributeY,
        "AlignComponentsExpandHorizontal" => Align::ExpandX,
        "AlignComponentsContractHorizontal" => Align::ContractX,
        "AlignComponentsExpandVertical" => Align::ExpandY,
        "AlignComponentsContractVertical" => Align::ContractY,
        "MoveComponentsToGrid" | "MoveAllComponentsOriginToGrid" => Align::Grid,
        _ => return None,
    })
}
pub(super) fn enabled(id: &str, doc: &OpenDocument) -> bool {
    let Some(board) = &doc.model.pcb else {
        return false;
    };
    let selection = &doc.view.pcb.selected;
    if id == "MoveAllComponentsOriginToGrid" {
        return !board.components.is_empty();
    }
    if let Some(mode) = alignment(id) {
        if mode != Align::Grid {
            return selection.len() >= 2;
        }
        let count = board
            .components
            .iter()
            .filter(|c| selection.contains(&c.id) && !c.locked)
            .count();
        return count >= if mode == Align::Grid { 1 } else { 2 };
    }
    match id {
        "AlignComponentsDialog" => selection.len() >= 2,
        "OffsetSelection" | "RotateSelectedObjects" | "MoveSelection" => !selection.is_empty(),
        "FlipSelectedObjects" => {
            selection.len() == 1
                && board
                    .components
                    .iter()
                    .any(|c| c.id == selection[0] && !c.locked)
        }
        "NextSelection" => !doc.pcb_editor.last_hits.is_empty(),
        "NextTopologySelection" => !selection.is_empty(),
        "MoveObject"
        | "MoveDrag"
        | "MoveComponent"
        | "SelectLasso"
        | "SelectInsideArea"
        | "SelectOutsideArea"
        | "SelectTouchingRectangle"
        | "SelectTouchingLine"
        | "SelectAll"
        | "SelectBoard"
        | "SelectNet"
        | "SelectConnectedCopper"
        | "SelectPhysicalConnection"
        | "SelectPhysicalConnectionOnLayer"
        | "SelectComponentConnections"
        | "SelectComponentNets"
        | "SelectAllOnLayer"
        | "SelectFreeObjects"
        | "SelectAllLocked"
        | "SelectOffGridPads"
        | "ToggleSelection"
        | "PlaceComponent"
        | "AdvancedRouting"
        | "PlaceVia"
        | "PlacePad"
        | "PlacePolygonPlane"
        | "PlaceCutoutRegion"
        | "PlaceKeepoutTrack"
        | "PlaceKeepoutArcCenter"
        | "PlaceKeepoutSolidRegion"
        | "PlaceKeepoutFill"
        | "PlaceString"
        | "PlaceLine"
        | "PlaceArcCenter"
        | "PlaceArcEdge"
        | "PlaceArcAnyAngle"
        | "PlaceFullCircle"
        | "PlaceFill"
        | "PlaceSolidRegion"
        | "PlaceRectangle" => true,
        _ => false,
    }
}

impl PcbCraft {
    pub(super) fn pcb_bar_command(
        &mut self,
        index: usize,
        id: &str,
        _ctx: &egui::Context,
    ) -> Result<(), String> {
        let units = self.settings.units;
        if !enabled(id, &self.documents[index]) {
            return Ok(());
        }
        cancel_alignment(&mut self.documents[index])?;
        let tool = match id {
            "MoveObject" | "MoveComponent" | "MoveSelection" => Some(Tool::Move),
            "MoveDrag" => Some(Tool::Drag),
            "AdvancedRouting" => Some(Tool::Route),
            "PlaceVia" => Some(Tool::Via),
            "PlacePad" => Some(Tool::Pad),
            "PlacePolygonPlane" => Some(Tool::Polygon),
            "PlaceCutoutRegion" => Some(Tool::PourCutout),
            "PlaceKeepoutTrack" | "PlaceLine" => Some(Tool::Line),
            "PlaceKeepoutArcCenter" | "PlaceArcCenter" | "PlaceArcEdge" | "PlaceArcAnyAngle" => {
                Some(Tool::Arc)
            }
            "PlaceKeepoutSolidRegion" | "PlaceSolidRegion" => Some(Tool::Region),
            "PlaceKeepoutFill" | "PlaceFill" => Some(Tool::Fill),
            "PlaceString" => Some(Tool::Text),
            "PlaceFullCircle" => Some(Tool::Circle),
            "PlaceRectangle" => Some(Tool::Rectangle),
            _ => None,
        };
        if let Some(tool) = tool {
            self.pcb_action(Action::Tool(tool));
            let ed = &mut self.documents[index].pcb_editor;
            if ed.tool == tool {
                ed.keepout = id.starts_with("PlaceKeepout");
                ed.bar.component_pick = id == "MoveComponent";
                if tool == Tool::Arc {
                    ed.arc_mode = match id {
                        "PlaceArcEdge" => pcb_tools::ArcPlacement::Edge,
                        "PlaceArcAnyAngle" => pcb_tools::ArcPlacement::AnyAngle,
                        _ => pcb_tools::ArcPlacement::CentreRadiusStartEnd,
                    };
                }
            }
            return Ok(());
        }
        if id == "PlaceComponent" {
            self.settings.right_components = true;
            self.settings.right_messages = false;
            self.settings.properties = false;
            return Ok(());
        }
        let action = match id {
            "SelectAll" => Some(Action::SelectAll),
            "SelectBoard" => Some(Action::SelectBoard),
            "SelectNet" => Some(Action::SelectNet),
            "SelectConnectedCopper" => Some(Action::SelectCopper),
            "FlipSelectedObjects" => Some(Action::Flip),
            _ => None,
        };
        if let Some(action) = action {
            self.pcb_action(action);
            return Ok(());
        }
        let doc = &mut self.documents[index];
        if doc.pcb_editor.has_pending() {
            return Err("Finish or cancel the current placement before using this command".into());
        }
        doc.pcb_editor.cancel(&mut doc.view);
        let board = doc.model.pcb.as_ref().unwrap();
        if let Some(mode) = alignment(id) {
            if matches!(mode, Align::CentreX | Align::CentreY) {
                return begin_alignment(doc, doc.view.pcb.selected.clone(), vec![mode]);
            }
            let ids = if id == "MoveAllComponentsOriginToGrid" {
                board.components.iter().map(|c| c.id).collect()
            } else {
                doc.view.pcb.selected.clone()
            };
            let next = if id == "MoveAllComponentsOriginToGrid" {
                pcb_arrange::all_component_origins_to_grid(board, doc.view.grid_nm)?
            } else if mode == Align::Grid {
                pcb_arrange::components(board, &ids, mode, doc.view.grid_nm)?
            } else {
                pcb_arrange::objects(board, &ids, mode, doc.view.grid_nm)?
            };
            commit(doc, next)?;
            return Ok(());
        }
        let region = match id {
            "SelectLasso" => Some(RegionSelection::Lasso),
            "SelectInsideArea" => Some(RegionSelection::Inside),
            "SelectOutsideArea" => Some(RegionSelection::Outside),
            "SelectTouchingRectangle" => Some(RegionSelection::Touching),
            "SelectTouchingLine" => Some(RegionSelection::Line),
            _ => None,
        };
        if let Some(mode) = region {
            doc.pcb_editor.bar.region = Some(SelectionGesture {
                mode,
                points: vec![],
                revision: doc.history.revision,
            });
            doc.view.pcb.tool_active = true;
            return Ok(());
        }
        match id {
            "SelectPhysicalConnection"
            | "SelectPhysicalConnectionOnLayer"
            | "SelectComponentConnections"
            | "SelectComponentNets" => {
                doc.pcb_editor.selection_pick = Some(match id {
                    "SelectPhysicalConnection" => ConnectionSelection::Physical,
                    "SelectPhysicalConnectionOnLayer" => ConnectionSelection::PhysicalOnLayer,
                    "SelectComponentConnections" => ConnectionSelection::ComponentConnections,
                    _ => ConnectionSelection::ComponentNets,
                });
                doc.view.pcb.tool_active = true;
            }
            "AlignComponentsDialog" => {
                doc.pcb_editor.bar.align_dialog = Some(AlignDialog {
                    selection: doc.view.pcb.selected.clone(),
                    revision: doc.history.revision,
                    h: 0,
                    v: 0,
                    error: None,
                })
            }
            "NextSelection" => {
                let ed = &mut doc.pcb_editor;
                ed.overlap = (ed.overlap + 1) % ed.last_hits.len();
                doc.view.pcb.selected =
                    vec![picked(board, &doc.view.pcb, ed.last_hits[ed.overlap])];
            }
            "NextTopologySelection" => extend_route_selection(doc),
            "OffsetSelection" | "RotateSelectedObjects" => {
                doc.pcb_editor.bar.dialog = Some(TransformDialog {
                    rotate: id == "RotateSelectedObjects",
                    selection: doc.view.pcb.selected.clone(),
                    revision: doc.history.revision,
                    x: format!("0{}", units.label()),
                    y: format!("0{}", units.label()),
                    angle: "0.00000".into(),
                    error: None,
                    units,
                    focused: false,
                });
            }
            _ => {
                let previous: BTreeSet<_> = doc.view.pcb.selected.iter().copied().collect();
                let mut view = doc.view.pcb.clone();
                if id == "SelectAllLocked" {
                    view.select_locked = true;
                }
                let mut selected = BTreeSet::new();
                for item in &board.items {
                    if !view.selectable_on_board(board, item) {
                        continue;
                    }
                    let chosen = match id {
                        "SelectAllOnLayer" => pcb_polygon::on_layer(board, item, view.active_layer),
                        "SelectFreeObjects" => item.component.is_none() && item.polygon.is_none(),
                        "SelectAllLocked" => {
                            item.locked
                                || item.component.is_some_and(|n| {
                                    board.components.iter().any(|c| c.index == n && c.locked)
                                })
                        }
                        "SelectOffGridPads" => {
                            matches!(item.shape, Shape::Pad { .. })
                                && doc.view.grid_nm > 0
                                && (item.anchor().x.rem_euclid(doc.view.grid_nm) != 0
                                    || item.anchor().y.rem_euclid(doc.view.grid_nm) != 0)
                        }
                        "ToggleSelection" => !previous.contains(&picked(board, &view, item.id)),
                        _ => false,
                    };
                    if chosen {
                        selected.insert(picked(board, &view, item.id));
                    }
                }
                doc.view.pcb.selected = selected.into_iter().collect();
            }
        }
        Ok(())
    }
    pub(super) fn pcb_bar_dialog(&mut self, index: usize, ctx: &egui::Context) {
        let doc = &mut self.documents[index];
        if let Some(mut dialog) = doc.pcb_editor.bar.align_dialog.take() {
            let (accept, cancel) = align_form(ctx, &mut dialog);
            if accept {
                let result = (|| -> Result<(), String> {
                    if dialog.revision != doc.history.revision {
                        return Err("Document changed while alignment was open".into());
                    }
                    let horizontal = [
                        None,
                        Some(Align::Left),
                        Some(Align::CentreX),
                        Some(Align::Right),
                        Some(Align::DistributeX),
                    ][dialog.h as usize];
                    let vertical = [
                        None,
                        Some(Align::Top),
                        Some(Align::CentreY),
                        Some(Align::Bottom),
                        Some(Align::DistributeY),
                    ][dialog.v as usize];
                    let operations: Vec<_> = horizontal.into_iter().chain(vertical).collect();
                    begin_alignment(doc, dialog.selection.clone(), operations)
                })();
                match result {
                    Ok(()) => return,
                    Err(e) => dialog.error = Some(e),
                }
            }
            if !cancel {
                doc.pcb_editor.bar.align_dialog = Some(dialog);
            }
            return;
        }
        let Some(mut dialog) = doc.pcb_editor.bar.dialog.take() else {
            return;
        };
        let (accept, cancel, interactive) = transform_form(ctx, &mut dialog);
        if interactive {
            doc.pcb_editor.bar.offset_pick = Some((dialog, None));
            doc.view.pcb.tool_active = true;
            return;
        }
        if accept {
            let result = (|| -> Result<(), String> {
                if dialog.revision != doc.history.revision {
                    return Err("Document changed while this dialog was open".into());
                }
                let board = doc.model.pcb.as_ref().unwrap();
                let pivot = board
                    .selected_bounds(&dialog.selection)
                    .ok_or("Selection no longer exists")?
                    .centre();
                if dialog.rotate {
                    let angle = dialog
                        .angle
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|a| a.is_finite() && a.abs() <= 360.)
                        .ok_or("Enter a rotation between -360 and 360 degrees")?;
                    doc.pcb_editor.bar.rotate_pick =
                        Some((dialog.selection.clone(), dialog.revision, angle));
                    doc.view.pcb.tool_active = true;
                    return Ok(());
                }
                let translation = Point {
                    x: offset(&dialog.x, dialog.units)?,
                    y: offset(&dialog.y, dialog.units)?,
                };
                let next = board.edited(Edit::Transform {
                    selection: dialog.selection.clone(),
                    pivot,
                    translation,
                    angle: 0.,
                })?;
                commit(doc, next)?;
                Ok(())
            })();
            match result {
                Ok(()) => return,
                Err(error) => dialog.error = Some(error),
            }
        }
        if !cancel {
            doc.pcb_editor.bar.dialog = Some(dialog);
        }
    }
}

pub(in crate::app::pcb_editor) fn input(
    doc: &mut OpenDocument,
    ed: &mut Editor,
    i: &Input,
    ctx: &egui::Context,
) -> Result<bool, String> {
    if ed.bar.menu.is_some() || ed.bar.dialog.is_some() || ed.bar.align_dialog.is_some() {
        return Ok(true);
    }
    if let Some(pick) = ed.bar.align_pick.clone() {
        if pick.revision != doc.history.revision {
            ed.bar.align_pick = None;
            doc.view.pcb.clear_gesture();
            return Err("Document changed before choosing the alignment reference".into());
        }
        if i.secondary_clicked || i.cancelled {
            ed.bar.align_pick = None;
            doc.view.pcb.clear_gesture();
            commit(doc, (*pick.draft).clone())?;
            return Ok(true);
        }
        if !i.clicked {
            return Ok(true);
        }
        let board = &*pick.draft;
        let reference = i
            .hit
            .and_then(|id| pcb_arrange::reference_object(board, &pick.selection, id));
        if let Some(reference) = reference {
            let next = pcb_arrange::objects_at_reference(
                board,
                &pick.selection,
                pick.operations[0],
                doc.view.grid_nm,
                reference,
            )?;
            let (next, operations) = fixed_alignment(
                next,
                &pick.selection,
                doc.view.grid_nm,
                pick.operations[1..].to_vec(),
            )?;
            if operations.is_empty() {
                commit(doc, next)?;
                ed.bar.align_pick = None;
                doc.view.pcb.clear_gesture();
            } else {
                doc.view.pcb.set_preview(Some(next.clone()));
                ed.bar.align_pick = Some(AlignmentPick {
                    draft: std::sync::Arc::new(next),
                    operations,
                    ..pick
                });
            }
        }
        return Ok(true);
    }
    if let Some((selection, revision, angle)) = ed.bar.rotate_pick.clone() {
        if revision != doc.history.revision {
            ed.bar.rotate_pick = None;
            doc.view.pcb.clear_gesture();
            return Err("Document changed before choosing the rotation reference".into());
        }
        if i.secondary_clicked || i.cancelled {
            ed.bar.rotate_pick = None;
            doc.view.pcb.clear_gesture();
            return Ok(true);
        }
        if let Some(pivot) = i.point {
            let next = doc.model.pcb.as_ref().unwrap().edited(Edit::Transform {
                selection,
                pivot,
                translation: Point::default(),
                angle,
            })?;
            if i.clicked {
                commit(doc, next)?;
                ed.bar.rotate_pick = None;
                doc.view.pcb.clear_gesture();
            } else {
                doc.view.pcb.set_preview(Some(next));
            }
        }
        return Ok(true);
    }
    if let Some((mut dialog, start)) = ed.bar.offset_pick.take() {
        if dialog.revision != doc.history.revision {
            doc.view.pcb.clear_gesture();
            return Err("Document changed during offset measurement".into());
        }
        if i.secondary_clicked || i.cancelled {
            ed.bar.dialog = Some(dialog);
            doc.view.pcb.clear_gesture();
            return Ok(true);
        }
        if let Some(p) = i.point {
            if let Some(start) = start {
                doc.view.pcb.draft_outline = vec![start, p];
                if i.clicked {
                    dialog.x = format!(
                        "{:.6}{}",
                        dialog.units.from_nm((p.x - start.x) as f64),
                        dialog.units.label()
                    );
                    dialog.y = format!(
                        "{:.6}{}",
                        dialog.units.from_nm((p.y - start.y) as f64),
                        dialog.units.label()
                    );
                    ed.bar.dialog = Some(dialog);
                    doc.view.pcb.clear_gesture();
                    return Ok(true);
                }
            } else if i.clicked {
                ed.bar.offset_pick = Some((dialog, Some(p)));
                return Ok(true);
            }
        }
        ed.bar.offset_pick = Some((dialog, start));
        return Ok(true);
    }
    if ed.bar.component_pick && ed.tool == Tool::Move && ed.tool_base.is_none() && i.clicked {
        let board = doc.model.pcb.as_ref().unwrap();
        let component = i.hit.and_then(|id| {
            board.components.iter().find(|c| c.id == id).or_else(|| {
                board.item(id).and_then(|item| {
                    board
                        .components
                        .iter()
                        .find(|c| Some(c.index) == item.component)
                })
            })
        });
        let Some(component) = component else {
            return Ok(true);
        };
        doc.view.pcb.selected = vec![component.id];
    }
    let Some(mut region) = ed.bar.region.take() else {
        return Ok(false);
    };
    if region.revision != doc.history.revision {
        doc.view.pcb.clear_gesture();
        return Err("Document changed during region selection".into());
    }
    if i.cancelled || i.drag_stopped && !i.hovered {
        doc.view.pcb.clear_gesture();
        return Ok(true);
    }
    let Some(p) = i.raw_point.or(i.point) else {
        ed.bar.region = Some(region);
        return Ok(true);
    };
    let mut finish = false;
    if region.mode == RegionSelection::Lasso {
        if (i.clicked || i.dragging)
            && region.points.last() != Some(&p)
            && region.points.len() < 4096
        {
            region.points.push(p);
        }
        finish =
            (i.double_clicked || i.secondary_clicked || i.drag_stopped) && region.points.len() >= 3;
        doc.view.pcb.draft_outline = region
            .points
            .iter()
            .copied()
            .chain(std::iter::once(p))
            .collect();
    } else {
        let started = !region.points.is_empty();
        if !started && (i.clicked || i.drag_started || i.drag_stopped) {
            region.points.push(i.press_origin.unwrap_or(p));
        }
        if let Some(a) = region.points.first().copied() {
            doc.view.pcb.selection_box = Some((a, p));
            if started && i.clicked || i.drag_stopped {
                region.points.push(p);
                finish = true;
            }
        }
        if i.secondary_clicked {
            doc.view.pcb.clear_gesture();
            return Ok(true);
        }
    }
    if finish {
        let board = doc.model.pcb.as_ref().unwrap();
        let mut candidates: BTreeSet<_> = board
            .items
            .iter()
            .filter(|item| doc.view.pcb.selectable_on_board(board, item))
            .map(|item| picked(board, &doc.view.pcb, item.id))
            .collect();
        if doc.view.pcb.filters[7] {
            candidates.extend(
                board
                    .polygon_pours
                    .iter()
                    .filter(|p| {
                        doc.view.pcb.visible.contains(&p.layer)
                            && (!p.locked || doc.view.pcb.select_locked)
                    })
                    .map(|p| p.id),
            );
        }
        let mut selected: BTreeSet<_> = eda_core::pcb_selection::in_region(
            board,
            &candidates,
            &region.points,
            region.mode,
            doc.view.pcb.active_layer,
        )
        .into_iter()
        .collect();
        if i.modifiers
            .unwrap_or_else(|| ctx.input(|i| i.modifiers))
            .shift
        {
            selected.extend(doc.view.pcb.selected.iter().copied());
        }
        doc.view.pcb.selected = selected.into_iter().collect();
        doc.view.pcb.selection_box = None;
        doc.view.pcb.draft_outline.clear();
        region.points.clear();
    }
    ed.bar.region = Some(region);
    Ok(true)
}

fn offset(value: &str, units: Units) -> Result<i64, String> {
    if let Ok(n) = value.trim().parse::<f64>() {
        let n = n * if units == Units::Mils { 25_400. } else { 1e6 };
        if n.is_finite() && n.abs() <= 1e11 {
            return Ok(n.round() as i64);
        }
        return Err("Offset is outside document limits".into());
    }
    pcb::dimension_nm(value)
}
fn close_control(ui: &mut egui::Ui, rect: Rect) -> bool {
    // The title control extends into the frame margin. ui.put would include
    // that negative content coordinate in layout and grow the entire dialog.
    let response = ui.interact(rect, ui.id().with("title_close"), Sense::click());
    ui.painter().rect_filled(
        rect,
        0.,
        if response.hovered() {
            Color32::from_rgb(195, 35, 35)
        } else {
            Color32::from_rgb(175, 25, 25)
        },
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "\u{00d7}",
        egui::FontId::proportional(15.),
        Color32::LIGHT_GRAY,
    );
    response.clicked()
}
fn align_form(ctx: &egui::Context, d: &mut AlignDialog) -> (bool, bool) {
    let mut accept = false;
    let mut cancel = false;
    let frame = egui::Frame::NONE
        .fill(Color32::from_gray(76))
        .inner_margin(7)
        .stroke(Stroke::new(1., Color32::from_gray(43)));
    egui::Modal::new(egui::Id::new("pcb_bar_align"))
        .frame(frame)
        .backdrop_color(Color32::TRANSPARENT)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (r, _) = ui.allocate_exact_size(egui::vec2(294., 224.), Sense::hover());
            let at = |x, y, w, h| Rect::from_min_size(r.min + egui::vec2(x, y), egui::vec2(w, h));
            ui.painter().text(
                at(0., 0., 294., 16.).center(),
                egui::Align2::CENTER_CENTER,
                "Align Objects",
                egui::FontId::proportional(12.),
                Color32::LIGHT_GRAY,
            );
            if close_control(ui, at(258., -7., 35., 22.)) {
                cancel = true;
            }
            ui.painter()
                .rect_filled(at(0., 22., 294., 170.), 0., Color32::from_gray(61));
            for (x, title, labels, value) in [
                (
                    19.,
                    "Horizontal",
                    ["No Change", "Left", "Center", "Right", "Space equally"],
                    &mut d.h,
                ),
                (
                    150.,
                    "Vertical",
                    ["No change", "Top", "Center", "Bottom", "Space equally"],
                    &mut d.v,
                ),
            ] {
                ui.painter().rect_stroke(
                    at(x, 44., 124., 132.),
                    0.,
                    Stroke::new(1., Color32::from_gray(43)),
                    egui::StrokeKind::Inside,
                );
                ui.painter()
                    .rect_filled(at(x + 10., 35., 77., 18.), 0., Color32::from_gray(61));
                ui.painter().text(
                    at(x + 13., 35., 100., 18.).left_center(),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(12.),
                    Color32::LIGHT_GRAY,
                );
                for (n, label) in labels.into_iter().enumerate() {
                    if ui
                        .scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(at(x + 14., 55. + n as f32 * 24., 108., 22.))
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| ui.radio(*value == n as u8, label),
                        )
                        .inner
                        .clicked()
                    {
                        *value = n as u8;
                    }
                }
            }
            if let Some(e) = &d.error {
                ui.put(
                    at(3., 178., 288., 18.),
                    egui::Label::new(egui::RichText::new(e).color(Color32::LIGHT_RED)).truncate(),
                )
                .on_hover_text(e);
            }
            accept = ui
                .put(at(138., 199., 75., 25.), egui::Button::new("OK"))
                .clicked();
            cancel |= ui
                .put(at(219., 199., 73., 25.), egui::Button::new("Cancel"))
                .clicked();
        });
    accept |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    cancel |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
    (accept, cancel)
}
fn transform_form(ctx: &egui::Context, d: &mut TransformDialog) -> (bool, bool, bool) {
    let mut accept = false;
    let mut cancel = false;
    let mut interactive = false;
    let size = if d.rotate {
        egui::vec2(242., 95.)
    } else {
        egui::vec2(266., 212.)
    };
    let frame = egui::Frame::NONE
        .fill(Color32::from_gray(76))
        .inner_margin(7)
        .stroke(Stroke::new(1., Color32::from_gray(43)));
    egui::Modal::new(egui::Id::new("pcb_bar_transform"))
        .frame(frame)
        .backdrop_color(Color32::TRANSPARENT)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            let at = |x, y, w, h| Rect::from_min_size(r.min + egui::vec2(x, y), egui::vec2(w, h));
            let title = if d.rotate {
                "Rotation Angle (Degrees)".into()
            } else {
                format!("Get X / Y Offsets [{}]", d.units.label())
            };
            ui.painter().text(
                at(0., 0., size.x, 16.).center(),
                egui::Align2::CENTER_CENTER,
                title,
                egui::FontId::proportional(12.),
                Color32::LIGHT_GRAY,
            );
            if close_control(ui, at(size.x - 28., -7., 28., 22.)) {
                cancel = true;
            }
            let body = at(0., 22., size.x, if d.rotate { 39. } else { 158. });
            ui.painter().rect_filled(body, 0., Color32::from_gray(61));
            let field = if d.rotate {
                at(7., 31., size.x - 14., 21.)
            } else {
                at(83., 38., 137., 21.)
            };
            let response = if d.rotate {
                ui.put(
                    field,
                    egui::TextEdit::singleline(&mut d.angle).font(egui::FontId::proportional(12.)),
                )
            } else {
                ui.put(
                    field,
                    egui::TextEdit::singleline(&mut d.x).font(egui::FontId::proportional(12.)),
                )
            };
            if !d.focused {
                response.request_focus();
                d.focused = true;
            }
            if !d.rotate {
                ui.put(at(14., 38., 65., 21.), egui::Label::new("X Offset"));
                ui.put(at(14., 71., 65., 21.), egui::Label::new("Y Offset"));
                ui.put(
                    at(83., 71., 137., 21.),
                    egui::TextEdit::singleline(&mut d.y).font(egui::FontId::proportional(12.)),
                );
                for (y, value) in [(38., &mut d.x), (71., &mut d.y)] {
                    if ui
                        .put(at(227., y, 20., 21.), egui::Button::new("-"))
                        .clicked()
                    {
                        match offset(value, d.units) {
                            Ok(n) => {
                                *value =
                                    format!("{}{}", d.units.from_nm(-n as f64), d.units.label())
                            }
                            Err(e) => d.error = Some(e),
                        }
                    }
                }
                if ui
                    .put(
                        at(15., 104., 114., 24.),
                        egui::Button::new("Toggle X/Y Offsets"),
                    )
                    .clicked()
                {
                    std::mem::swap(&mut d.x, &mut d.y);
                }
                if ui
                    .put(
                        at(135., 104., 114., 24.),
                        egui::Button::new("Reset X/Y Offsets"),
                    )
                    .clicked()
                {
                    d.x = format!("0{}", d.units.label());
                    d.y = d.x.clone();
                }
                interactive = ui
                    .put(
                        at(15., 138., 234., 24.),
                        egui::Button::new("Define X/Y Offsets Interactively..."),
                    )
                    .clicked();
            }
            if let Some(error) = &d.error {
                ui.put(
                    at(0., size.y - 47., size.x, 20.),
                    egui::Label::new(egui::RichText::new(error).color(Color32::LIGHT_RED))
                        .truncate(),
                )
                .on_hover_text(error);
            }
            accept = ui
                .put(
                    at(size.x - 154., size.y - 25., 75., 25.),
                    egui::Button::new("OK"),
                )
                .clicked();
            cancel |= ui
                .put(
                    at(size.x - 73., size.y - 25., 73., 25.),
                    egui::Button::new("Cancel"),
                )
                .clicked();
        });
    cancel |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
    accept |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    if !d.rotate
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Q))
        && let (Ok(x), Ok(y)) = (offset(&d.x, d.units), offset(&d.y, d.units))
    {
        d.units = if d.units == Units::Mils {
            Units::Millimetres
        } else {
            Units::Mils
        };
        d.x = format!("{:.6}{}", d.units.from_nm(x as f64), d.units.label());
        d.y = format!("{:.6}{}", d.units.from_nm(y as f64), d.units.label());
    }
    (accept, cancel, interactive)
}
