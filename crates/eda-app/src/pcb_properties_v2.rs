//! Properties are a draft until Apply. A failed edit never mutates the live board.
use super::*;

fn shape_combo(ui: &mut egui::Ui, shape: &mut PadShape) {
    egui::ComboBox::from_id_salt("pad_shape")
        .selected_text(format!("{shape:?}"))
        .show_ui(ui, |ui| {
            for value in [
                PadShape::Round,
                PadShape::Rectangle,
                PadShape::Octagonal,
                PadShape::RoundedRectangle,
            ] {
                ui.selectable_value(shape, value, format!("{value:?}"));
            }
        });
}
fn copper_layer_ui(ui: &mut egui::Ui, board: &Board, caption: &str, layer: &mut u16) {
    ui.push_id(caption, |ui| {
        ui.label(caption);
        egui::ComboBox::from_id_salt("layer")
            .selected_text(pcb::layer_name(*layer))
            .show_ui(ui, |ui| {
                for l in board.layers.iter().filter(|l| l.copper) {
                    ui.selectable_value(layer, l.id, &l.name);
                }
            });
    });
}
fn optional_size(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<Point>,
    fallback: Point,
    units: Units,
) {
    ui.push_id(label, |ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *value = on.then_some(fallback);
        }
        if let Some(p) = value {
            dimension_ui(ui, "X", &mut p.x, units);
            dimension_ui(ui, "Y", &mut p.y, units);
        }
    });
}
fn vertices_ui(ui: &mut egui::Ui, id: &str, points: &mut Vec<Point>, units: Units) {
    let mut remove = None;
    let mut insert = None;
    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(190.0)
        .show(ui, |ui| {
            for n in 0..points.len() {
                ui.push_id(n, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}", n + 1));
                        if ui
                            .small_button("+")
                            .on_hover_text("Insert a vertex after this one")
                            .clicked()
                        {
                            insert = Some(n);
                        }
                        if ui
                            .add_enabled(points.len() > 3, egui::Button::new("−"))
                            .clicked()
                        {
                            remove = Some(n);
                        }
                    });
                    point_ui(ui, "", &mut points[n], units);
                });
            }
        });
    if let Some(n) = remove {
        points.remove(n);
    } else if let Some(n) = insert {
        let p = pcb::midpoint(points[n], points[(n + 1) % points.len()]);
        points.insert(n + 1, p);
    }
}
fn holes_ui(ui: &mut egui::Ui, holes: &mut Vec<Vec<Point>>, outline: &[Point], units: Units) {
    let mut remove = None;
    for (n, hole) in holes.iter_mut().enumerate() {
        ui.push_id(("hole", n), |ui| {
            egui::CollapsingHeader::new(format!("Cutout {}", n + 1)).show(ui, |ui| {
                vertices_ui(ui, "vertices", hole, units);
                if ui.button("Delete cutout").clicked() {
                    remove = Some(n);
                }
            });
        });
    }
    if let Some(n) = remove {
        holes.remove(n);
    }
    if ui.button("Add rectangular cutout").clicked() && outline.len() >= 3 {
        let b = Bounds::points(outline);
        let centre = b.centre();
        let dx = ((b.max.x - b.min.x) / 10).max(1);
        let dy = ((b.max.y - b.min.y) / 10).max(1);
        holes.push(vec![
            Point {
                x: centre.x - dx,
                y: centre.y - dy,
            },
            Point {
                x: centre.x + dx,
                y: centre.y - dy,
            },
            Point {
                x: centre.x + dx,
                y: centre.y + dy,
            },
            Point {
                x: centre.x - dx,
                y: centre.y + dy,
            },
        ]);
    }
}
fn item_ui(ui: &mut egui::Ui, item: &mut Item, board: &Board, units: Units) {
    ui.checkbox(&mut item.locked, "Locked");
    ui.checkbox(&mut item.keepout, "Keepout");
    layer_ui(ui, board, &mut item.layer);
    ui.push_id("item_net", |ui| net_ui(ui, board, &mut item.net));
    match &mut item.shape {
        Shape::Track { start, end, width } => {
            point_ui(ui, "Start", start, units);
            point_ui(ui, "End", end, units);
            dimension_ui(ui, "Width", width, units);
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            point_ui(ui, "Centre", centre, units);
            dimension_ui(ui, "Radius", radius, units);
            angle_ui(ui, "Start angle", start_angle);
            angle_ui(ui, "End angle", end_angle);
            dimension_ui(ui, "Width", width, units);
        }
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            hole,
            plated,
            name,
            bottom_size,
            middle_size,
        } => {
            ui.horizontal(|ui| {
                ui.label("Designator");
                ui.text_edit_singleline(name);
            });
            point_ui(ui, "Location", position, units);
            dimension_ui(ui, "Size X", &mut size.x, units);
            dimension_ui(ui, "Size Y", &mut size.y, units);
            angle_ui(ui, "Rotation", rotation);
            shape_combo(ui, shape);
            dimension_ui(ui, "Drill", hole, units);
            ui.checkbox(plated, "Plated");
            optional_size(ui, "Middle layer size", middle_size, *size, units);
            optional_size(ui, "Bottom layer size", bottom_size, *size, units);
            if *hole > 0 && item.layer != pcb::MULTI_LAYER {
                ui.label("For a through-hole pad choose Multi-Layer.");
            }
        }
        Shape::Via {
            position,
            diameter,
            hole,
            from_layer,
            to_layer,
        } => {
            item.layer = pcb::MULTI_LAYER;
            point_ui(ui, "Location", position, units);
            dimension_ui(ui, "Diameter", diameter, units);
            dimension_ui(ui, "Drill", hole, units);
            copper_layer_ui(ui, board, "From layer", from_layer);
            copper_layer_ui(ui, board, "To layer", to_layer);
        }
        Shape::Text {
            position,
            text,
            height,
            stroke,
            rotation,
            mirrored,
            font,
        } => {
            ui.text_edit_multiline(text);
            point_ui(ui, "Location", position, units);
            dimension_ui(ui, "Height", height, units);
            dimension_ui(ui, "Stroke", stroke, units);
            angle_ui(ui, "Rotation", rotation);
            ui.checkbox(mirrored, "Mirrored");
            ui.horizontal(|ui| {
                ui.label("Font");
                ui.text_edit_singleline(font);
            });
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            point_ui(ui, "Corner 1", start, units);
            point_ui(ui, "Corner 2", end, units);
            angle_ui(ui, "Rotation", rotation);
        }
        Shape::Region {
            contour,
            holes,
            cutout,
        } => {
            ui.checkbox(cutout, "Polygon cutout (no copper)");
            vertices_ui(ui, "region_vertices", contour, units);
            holes_ui(ui, holes, contour, units);
        }
    }
}
fn polygon_ui(ui: &mut egui::Ui, p: &mut Polygon, board: &Board, units: Units) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut p.name);
    });
    ui.checkbox(&mut p.locked, "Locked");
    ui.checkbox(&mut p.shelved, "Shelved");
    copper_layer_ui(ui, board, "Layer", &mut p.layer);
    ui.push_id("polygon_net", |ui| net_ui(ui, board, &mut p.net));
    ui.horizontal(|ui| {
        ui.label("Pour order (smaller first)");
        ui.add(egui::DragValue::new(&mut p.priority));
    });
    dimension_ui(ui, "Clearance", &mut p.clearance, units);
    egui::ComboBox::from_id_salt("connection")
        .selected_text(format!("{:?}", p.connection))
        .show_ui(ui, |ui| {
            for v in [Connection::Direct, Connection::Thermal, Connection::None] {
                ui.selectable_value(&mut p.connection, v, format!("{v:?}"));
            }
        });
    if p.connection == Connection::Thermal {
        dimension_ui(ui, "Thermal gap", &mut p.thermal_gap, units);
        dimension_ui(ui, "Spoke width", &mut p.spoke_width, units);
        angle_ui(ui, "Spoke angle", &mut p.spoke_angle);
    }
    ui.checkbox(&mut p.remove_islands, "Remove unconnected islands");
    ui.horizontal(|ui| {
        ui.label("Minimum island area");
        ui.add(
            egui::DragValue::new(&mut p.min_island_area_mm2)
                .range(0.0..=1e6)
                .speed(0.1)
                .suffix(" mm²"),
        );
    });
    ui.separator();
    if p.contour.iter().any(|v| v.arc.is_some()) {
        ui.label("This outline has arcs. Canvas vertex edits straighten adjacent arcs.");
        if ui
            .button("Convert outline arcs to straight vertices")
            .clicked()
        {
            p.contour = pcb_polygon::straight_contour(&p.points());
        }
    } else {
        let mut points = p.points();
        vertices_ui(ui, "polygon_vertices", &mut points, units);
        p.contour = pcb_polygon::straight_contour(&points);
    }
    let points = p.points();
    holes_ui(ui, &mut p.holes, &points, units);
    ui.label("Clearance/thermal values here belong to PCBCraft's pour settings, not a complete Altium rule evaluator.");
}
pub(super) fn show(app: &mut PcbCraft, ui: &mut egui::Ui) -> bool {
    let Some(index) = app.active else {
        return false;
    };
    if app.documents[index].model.pcb.is_none() {
        return false;
    }
    let units = app.settings.units;
    let doc = &mut app.documents[index];
    let mut ed = std::mem::take(&mut doc.pcb_editor);
    let mut edit: Option<Result<Board, String>> = None;
    let mut action = None;
    ui.label(RichText::new("PCB Properties").strong());
    egui::ScrollArea::vertical().id_salt("pcb_properties_v2").show(ui,|ui|{
        let board=doc.model.pcb.as_ref().unwrap();
        if ed.tool!=Tool::Select {
            properties::section(ui,"Interactive Tool",|ui|{
                ui.label(format!("{:?}",ed.tool));
                if let Some(error)=&ed.last_error{ui.colored_label(Color32::LIGHT_RED,error);}
                if matches!(ed.tool,Tool::Route|Tool::Arc){dimension_ui(ui,"Track width",&mut ed.width,units);}
                if matches!(ed.tool,Tool::Via|Tool::Route){dimension_ui(ui,"Via diameter",&mut ed.diameter,units);dimension_ui(ui,"Via drill",&mut ed.drill,units);}
                if ed.tool==Tool::Via{copper_layer_ui(ui,board,"From layer",&mut ed.via_from);copper_layer_ui(ui,board,"To layer",&mut ed.via_to);}
                if ed.tool==Tool::Pad {
                    ui.horizontal(|ui|{ui.label("Pad number");ui.text_edit_singleline(&mut ed.pad_name);});
                    dimension_ui(ui,"Size X",&mut ed.pad_size.x,units);dimension_ui(ui,"Size Y",&mut ed.pad_size.y,units);shape_combo(ui,&mut ed.pad_shape);
                    dimension_ui(ui,"Hole",&mut ed.pad_hole,units);ui.checkbox(&mut ed.pad_plated,"Plated");angle_ui(ui,"Rotation",&mut ed.rotation);
                }
                if ed.tool==Tool::Text{ui.text_edit_multiline(&mut ed.text);dimension_ui(ui,"Text height",&mut ed.text_height,units);}
                ui.add_enabled_ui(ed.route_session.is_none(),|ui|{net_ui(ui,board,&mut ed.net);});
                if ed.route_session.is_some(){
                    ui.label("Space: elbow · Shift+Space: corner mode · V: next copper layer + via · Backspace: last leg");
                    if ui.button("Finish route").clicked(){action=Some(Action::FinishRoute);}
                }
                if matches!(ed.tool,Tool::Polygon|Tool::PolygonCutout|Tool::Region|Tool::Outline){ui.label("Click vertices; right-click/double-click finishes. Backspace removes the last vertex.");}
            });
        }
        let selected=doc.view.pcb.selected.clone();
        ui.add_enabled_ui(!ed.has_pending(),|ui|{
            if selected.len()==1 {
                let id=selected[0];
                if let Some(item)=board.item(id) {
                    if item.polygon.is_some(){ui.label("Select the polygon definition to edit its generated copper.");}
                    else {
                        if ed.draft.as_ref().is_none_or(|d|d.id!=id||d.revision!=doc.history.revision){ed.draft=Some(Draft{id,revision:doc.history.revision,item:item.clone()});}
                        let d=ed.draft.as_mut().unwrap();
                        properties::section(ui,item.kind(),|ui|{
                            item_ui(ui,&mut d.item,board,units);
                            ui.horizontal(|ui|{
                                if ui.button("Apply").clicked(){edit=Some(board.edited(Edit::Replace(d.item.clone())));}
                                if ui.button("Revert").clicked(){d.item=item.clone();}
                            });
                        });
                    }
                }else if let Some(c)=board.components.iter().find(|c|c.id==id) {
                    if ed.component_draft.as_ref().is_none_or(|d|d.value.id!=id||d.revision!=doc.history.revision){ed.component_draft=Some(ComponentDraft{revision:doc.history.revision,value:c.clone()});}
                    let d=&mut ed.component_draft.as_mut().unwrap().value;
                    properties::section(ui,"Component",|ui|{
                        ui.horizontal(|ui|{ui.label("Designator");ui.text_edit_singleline(&mut d.designator);});
                        ui.horizontal(|ui|{ui.label("Comment");ui.text_edit_singleline(&mut d.comment);});
                        ui.label(format!("Footprint: {}",d.footprint));ui.checkbox(&mut d.locked,"Locked");
                        point_ui(ui,"Location",&mut d.position,units);angle_ui(ui,"Rotation",&mut d.rotation);ui.label(pcb::layer_name(d.layer));
                        ui.horizontal(|ui|{
                            if ui.button("Apply").clicked(){edit=Some(pcb_tools::edit_component(board,d.clone()));}
                            if ui.button("Revert").clicked(){*d=c.clone();}
                            if ui.button("Flip board side").clicked(){action=Some(Action::Flip);}
                        });
                    });
                }else if let Some(p)=board.polygon_pours.iter().find(|p|p.id==id) {
                    if ed.polygon_draft.as_ref().is_none_or(|d|d.value.id!=id||d.revision!=doc.history.revision){ed.polygon_draft=Some(PolygonDraft{revision:doc.history.revision,value:p.clone()});}
                    let d=&mut ed.polygon_draft.as_mut().unwrap().value;
                    properties::section(ui,"Polygon Pour",|ui|{
                        polygon_ui(ui,d,board,units);
                        ui.horizontal(|ui|{
                            if ui.button("Apply and repour").clicked(){edit=Some(pcb_tools::replace_polygon(board,d.clone()));}
                            if ui.button("Revert").clicked(){*d=p.clone();}
                        });
                        if ui.button("Draw cutout").clicked(){action=Some(Action::Tool(Tool::PolygonCutout));}
                    });
                }
            }else{ui.label(format!("{} objects selected",selected.len()));}
        });
        properties::section(ui,"Polygons",|ui|{
            for p in &board.polygon_pours {
                if ui.selectable_label(selected.contains(&p.id),format!("{} · {}{}",p.name,board.net_name(p.net),if p.shelved{" (shelved)"}else{""})).clicked(){doc.view.pcb.selected=vec![p.id];}
            }
            ui.horizontal(|ui|{
                if ui.button("Repour all").clicked(){action=Some(Action::Repour);}
                if ui.button("Shelve").clicked(){action=Some(Action::Shelve);}
                if ui.button("Restore").clicked(){action=Some(Action::Restore);}
            });
        });
        properties::section(ui,"Selection Filter",|ui|{
            ui.checkbox(&mut doc.view.pcb.component_selection,"Select whole components");ui.checkbox(&mut doc.view.pcb.select_locked,"Select locked objects");
            for(n,label)in ["Tracks","Arcs","Pads","Vias","Texts","Fills","Regions","Polygons"].into_iter().enumerate(){ui.checkbox(&mut doc.view.pcb.filters[n],label);}
        });
        properties::section(ui,"Nets",|ui|{
            ui.text_edit_singleline(&mut ed.new_net_name);
            if ui.add_enabled(!ed.has_pending()&&!ed.new_net_name.trim().is_empty(),egui::Button::new("Add net")).clicked(){
                let name=ed.new_net_name.trim().to_owned();
                edit=Some((||->Result<Board,String>{
                    if board.nets.iter().any(|n|n.name==name){return Err("A net with this name already exists".into());}
                    let index=board.nets.iter().map(|n|n.index).max().map_or(Some(0),|n|n.checked_add(1)).ok_or("Net index overflow")?;
                    let mut b=board.clone();b.nets.push(pcb::Net{index,name,properties:pcb::Fields::new()});b.validate()?;Ok(b)
                })());
            }
        });
        properties::section(ui,"Board",|ui|{
            ui.label(format!("{} objects · {} components · {} nets",board.items.len(),board.components.len(),board.nets.len()));
            ui.label("Save .PcbDoc writes supported edits in the original binary/ASCII representation. Complex dependent streams can prevent native save; JSON remains available.");
            if !board.warnings.is_empty(){egui::CollapsingHeader::new("Import / compatibility warnings").show(ui,|ui|{for w in &board.warnings{ui.label(w);}});}
        });
    });
    let result = match edit {
        Some(Ok(next)) => commit(doc, next).map(|_| ()),
        Some(Err(e)) => Err(e),
        None => Ok(()),
    };
    doc.pcb_editor = ed;
    if let Err(e) = result {
        app.message(true, e);
    }
    if let Some(action) = action {
        app.command = Some(Command::Pcb(action));
    }
    true
}
