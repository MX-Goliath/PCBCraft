use super::*;
use egui::{Event, Modifiers, PointerButton};
fn app() -> PcbCraft {
    super::super::schematic_editor::tests::app()
}
fn canvas() -> Rect {
    Rect::from_min_max(egui::pos2(334., 78.), egui::pos2(1390., 970.))
}
fn frame(
    app: &mut PcbCraft,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1707., 1026.))),
            time: Some(time),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| app.schematic_active_bar(ctx, canvas()),
    )
}
fn mouse(at: Pos2, button: PointerButton, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(at),
        Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        },
    ]
}
fn button(group: usize) -> Pos2 {
    egui::pos2(
        canvas().center().x - 476.0 / 2. + 4. + (group as f32 + 0.5) * 100. / 3.,
        canvas().top() + 23.,
    )
}
fn text(output: &egui::FullOutput, value: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                (t.galley.job.text == value).then_some(t.pos + t.galley.rect.center().to_vec2())
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("Missing text {value}"))
}
#[test]
fn device_selector_is_bounded_blocks_edits_and_escape_keeps_the_document_unchanged() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let before = app.documents[0].model.clone();
    app.schematic_bar_action(0, Action::DeviceSheet, &ctx);
    assert!(app.documents[0].sch_editor.bar.resources.modal());
    assert!(!app.documents[0].sch_editor.active());
    for n in 0..3 {
        frame(&mut app, &ctx, n as f64 / 60., vec![]);
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("select_device_sheet")))
        .unwrap();
    assert!(rect.width() < 900. && rect.height() < 700., "{rect:?}");
    let _ = ctx.run(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            focused: true,
            ..Default::default()
        },
        |ctx| {
            assert!(app.schematic_bar_shortcuts(0, ctx));
        },
    );
    assert!(!app.documents[0].sch_editor.bar.resources.modal());
    assert_eq!(app.documents[0].model, before);
    assert_eq!(app.documents[0].history.revision, 0);
}

#[test]
fn active_bar_drawing_and_simulation_menus_keep_live_order_and_keyboard_navigation() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    for n in 0..3 {
        frame(&mut app, &ctx, n as f64 / 60., vec![]);
    }
    for (group, labels, width) in [
        (
            12,
            vec![
                "Arc",
                "Full Circle",
                "Elliptical Arc",
                "Ellipse",
                "Line",
                "Rectangle",
                "Round Rectangle",
                "Polygon",
                "Bezier",
                "Graphic...",
            ],
            148.,
        ),
        (
            13,
            vec![
                "Voltage Source",
                "Current Source",
                "Simulation Generic Component",
                "Voltage Probe",
                "Voltage Diff Probe",
                "Current Probe",
                "Power Probe",
                "Initial Condition",
            ],
            224.,
        ),
    ] {
        frame(
            &mut app,
            &ctx,
            1.,
            mouse(button(group), PointerButton::Secondary, true),
        );
        frame(
            &mut app,
            &ctx,
            1.1,
            mouse(button(group), PointerButton::Secondary, false),
        );
        let output = frame(&mut app, &ctx, 1.2, vec![]);
        let menu = ctx
            .memory(|m| m.area_rect(egui::Id::new("schematic_active_bar_menu")))
            .unwrap();
        assert!((menu.width() - width).abs() < 1., "{menu:?}");
        assert!(
            (menu.height() - labels.len() as f32 * 88. / 3.).abs() < 1.,
            "{menu:?}"
        );
        let positions: Vec<_> = labels.iter().map(|label| text(&output, label)).collect();
        assert!(positions.windows(2).all(|p| p[0].y < p[1].y));
        app.documents[0].sch_editor.bar.menu = None;
    }
    app.documents[0].sch_editor.bar.menu = Some((13, egui::pos2(100., 100.)));
    app.documents[0].sch_editor.bar.menu_focus = None;
    for (time, key) in [
        (2., Key::ArrowDown),
        (2.1, Key::ArrowDown),
        (2.2, Key::Enter),
    ] {
        frame(
            &mut app,
            &ctx,
            time,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        frame(
            &mut app,
            &ctx,
            time + 0.01,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    assert_eq!(
        app.documents[0].sch_editor.kind,
        Some(Placement::CurrentSource)
    );
    assert_eq!(app.documents[0].sch_editor.bar.last[13], 1);
}

#[test]
fn active_bar_short_click_hold_right_click_last_used_and_filter_are_real_events() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    for n in 0..3 {
        frame(&mut app, &ctx, n as f64 / 60., vec![]);
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("active_bar")))
        .unwrap();
    assert!(
        (rect.width() - 476.0).abs() <= 1. && rect.height() <= 34.,
        "{rect:?}"
    );
    frame(
        &mut app,
        &ctx,
        1.,
        mouse(button(5), PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        1.2,
        mouse(button(5), PointerButton::Primary, false),
    );
    assert_eq!(app.documents[0].sch_editor.kind, Some(Placement::Wire));
    {
        let doc = &mut app.documents[0];
        doc.sch_editor.cancel(&mut doc.view);
    }
    frame(
        &mut app,
        &ctx,
        2.,
        mouse(button(5), PointerButton::Primary, true),
    );
    frame(&mut app, &ctx, 2.99, vec![]);
    assert!(app.documents[0].sch_editor.bar.menu.is_none());
    frame(&mut app, &ctx, 3.01, vec![]);
    assert_eq!(
        app.documents[0].sch_editor.bar.menu.map(|v| v.0),
        Some(5),
        "press={:?}; down={}; time={}",
        app.documents[0].sch_editor.bar.press,
        ctx.input(|i| i.pointer.primary_down()),
        ctx.input(|i| i.time)
    );
    let output = frame(
        &mut app,
        &ctx,
        3.1,
        mouse(button(5), PointerButton::Primary, false),
    );
    assert!(app.documents[0].sch_editor.kind.is_none());
    let bus = text(&output, "Bus");
    frame(
        &mut app,
        &ctx,
        3.2,
        mouse(bus, PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        3.3,
        mouse(bus, PointerButton::Primary, false),
    );
    assert_eq!(app.documents[0].sch_editor.kind, Some(Placement::Bus));
    assert_eq!(app.documents[0].sch_editor.bar.last[5], 1);
    frame(
        &mut app,
        &ctx,
        4.,
        mouse(button(0), PointerButton::Primary, true),
    );
    let output = frame(
        &mut app,
        &ctx,
        4.1,
        mouse(button(0), PointerButton::Primary, false),
    );
    let wires = text(&output, "Wires");
    frame(
        &mut app,
        &ctx,
        4.2,
        mouse(wires, PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        4.3,
        mouse(wires, PointerButton::Primary, false),
    );
    assert!(!app.documents[0].view.sch_filters[1]);
    app.documents[0].sch_editor.bar.menu = None;
    frame(
        &mut app,
        &ctx,
        5.,
        mouse(button(6), PointerButton::Secondary, true),
    );
    frame(
        &mut app,
        &ctx,
        5.1,
        mouse(button(6), PointerButton::Secondary, false),
    );
    assert_eq!(app.documents[0].sch_editor.bar.menu.map(|v| v.0), Some(6));
}

#[test]
fn active_bar_power_shape_region_alignment_and_stale_dialog_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            focused: true,
            ..Default::default()
        },
        |_| {},
    );
    let p = |at, clicked| eda_render::SchematicInput {
        point: Some(at),
        clicked,
        ..Default::default()
    };
    app.schematic_bar_action(0, Action::Power(2), &ctx);
    app.schematic_input(0, p(Point::mm(20, 20), true), &ctx);
    let s = app.documents[0].model.schematic.as_ref().unwrap();
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::PowerPort{text,style:2,orientation:1,..}if text=="+12"))
    );
    app.schematic_bar_action(0, Action::Place(Placement::Rectangle), &ctx);
    app.schematic_input(0, p(Point::mm(30, 50), true), &ctx);
    let before = app.documents[0].history.revision;
    app.schematic_input(0, p(Point::mm(50, 30), true), &ctx);
    assert_eq!(app.documents[0].history.revision, before + 1);
    app.schematic_bar_action(0, Action::Select(Region::Inside), &ctx);
    app.schematic_input(0, p(Point::mm(0, 0), true), &ctx);
    app.schematic_input(0, p(Point::mm(80, 80), true), &ctx);
    assert_eq!(selected(&app.documents[0]).len(), 2);
    let before = app.documents[0].model.clone();
    app.schematic_bar_action(0, Action::Align(Align::Left), &ctx);
    let doc = &mut app.documents[0];
    assert_ne!(doc.model, before);
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model, before);
    app.schematic_bar_action(0, Action::Offset, &ctx);
    assert!(app.documents[0].sch_editor.bar.dialog.is_some());
    let revision = app.documents[0].history.revision;
    app.schematic_input(0, p(Point::mm(90, 90), true), &ctx);
    assert_eq!(app.documents[0].history.revision, revision);
}

#[test]
fn active_bar_shape_keys_do_not_edit_the_previous_selection_and_arc_waits_for_four_points() {
    let mut app = app();
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            focused: true,
            ..Default::default()
        },
        |_| {},
    );
    let p = |at| eda_render::SchematicInput {
        point: Some(at),
        clicked: true,
        ..Default::default()
    };
    app.schematic_bar_action(0, Action::Power(0), &ctx);
    app.schematic_input(0, p(Point::mm(10, 10)), &ctx);
    let power = app.documents[0]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .last()
        .unwrap()
        .id;
    app.documents[0].view.selected = Some(power);
    app.schematic_bar_action(0, Action::Place(Placement::Arc), &ctx);
    app.schematic_input(0, p(Point::mm(30, 30)), &ctx);
    app.schematic_input(0, p(Point::mm(40, 30)), &ctx);
    let before = app.documents[0].model.clone();
    for (time, key) in [(1., Key::Backspace), (2., Key::Space)] {
        let _ = ctx.run(
            egui::RawInput {
                focused: true,
                time: Some(time),
                events: vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                assert!(app.schematic_shortcuts(ctx));
            },
        );
    }
    assert_eq!(app.documents[0].model, before);
    assert!(
        matches!(&app.documents[0].sch_editor.bar.interaction,Some(Interaction::Shape {points,..}) if points.len()==1)
    );
    app.schematic_input(0, p(Point::mm(40, 30)), &ctx);
    app.schematic_input(0, p(Point::mm(40, 30)), &ctx);
    assert_eq!(app.documents[0].model, before);
    app.schematic_input(0, p(Point::mm(30, 40)), &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .len(),
        before.schematic.as_ref().unwrap().primitives.len() + 1
    );
    assert!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitive(power)
            .is_some()
    );
}

#[test]
fn active_bar_panel_raster_from_actual_egui_meshes() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut texture = None;
    for n in 0..3 {
        let output = frame(&mut app, &ctx, n as f64 / 60., vec![]);
        for (id, delta) in output.textures_delta.set {
            if id == egui::TextureId::Managed(0) && delta.pos.is_none() {
                let egui::ImageData::Color(image) = delta.image;
                texture = Some(image);
            }
        }
    }
    let output = frame(&mut app, &ctx, 0.1, vec![]);
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("active_bar")))
        .unwrap();
    let clipped = ctx.tessellate(output.shapes, 1.);
    let scale = 1.5;
    let mut png = image::RgbaImage::from_pixel(
        (rect.width() * scale).ceil() as u32,
        (rect.height() * scale).ceil() as u32,
        image::Rgba([58, 58, 58, 255]),
    );
    for primitive in clipped {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        for tri in mesh.indices.chunks_exact(3) {
            let vertices = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            let points = vertices.map(|v| (v.pos - rect.min) * scale);
            let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
            let area = cross(points[1] - points[0], points[2] - points[0]);
            if area.abs() < 1e-5 {
                continue;
            }
            let minx = points
                .iter()
                .map(|v| v.x)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let miny = points
                .iter()
                .map(|v| v.y)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let maxx = points
                .iter()
                .map(|v| v.x)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .max(0.)
                .min(png.width() as f32) as u32;
            let maxy = points
                .iter()
                .map(|v| v.y)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .max(0.)
                .min(png.height() as f32) as u32;
            for y in miny..maxy {
                for x in minx..maxx {
                    let point = egui::vec2(x as f32 + 0.5, y as f32 + 0.5);
                    let w = [
                        cross(points[1] - point, points[2] - point) / area,
                        cross(points[2] - point, points[0] - point) / area,
                        cross(points[0] - point, points[1] - point) / area,
                    ];
                    if w.iter().any(|n| *n < 0.) {
                        continue;
                    }
                    let mut color = [0.; 4];
                    let mut uv = Vec2::ZERO;
                    for i in 0..3 {
                        uv += vertices[i].uv.to_vec2() * w[i];
                        for (c, target) in color.iter_mut().enumerate() {
                            *target += vertices[i].color[c] as f32 / 255. * w[i];
                        }
                    }
                    if let Some(tex) = &texture
                        && mesh.texture_id == egui::TextureId::Managed(0)
                    {
                        let tx =
                            (uv.x * tex.size[0] as f32).clamp(0., tex.size[0] as f32 - 1.) as usize;
                        let ty =
                            (uv.y * tex.size[1] as f32).clamp(0., tex.size[1] as f32 - 1.) as usize;
                        let t = tex.pixels[ty * tex.size[0] + tx];
                        for c in 0..4 {
                            color[c] *= t[c] as f32 / 255.;
                        }
                    }
                    let dst = png.get_pixel_mut(x, y);
                    for c in 0..3 {
                        dst[c] = ((color[c] + dst[c] as f32 / 255. * (1. - color[3])) * 255.)
                            .clamp(0., 255.) as u8;
                    }
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/routing-check");
    std::fs::create_dir_all(&root).unwrap();
    png.save(root.join("active-bar-panel.png")).unwrap();
    assert!((713..=716).contains(&png.width()));
}
