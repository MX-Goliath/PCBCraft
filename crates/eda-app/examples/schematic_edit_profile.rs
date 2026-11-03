//! Read-only schematic interaction probe; never writes the input document.
use eda_core::schematic_edit::{self as edit, Edit, Fragment, Request, Selection};
use eda_geom::Point;
use std::time::Instant;

fn measure<T>(label: &str, mut operation: impl FnMut() -> T) {
    let start = Instant::now();
    for _ in 0..5 {
        std::hint::black_box(operation());
    }
    println!("{label}_ms={:.3}", start.elapsed().as_secs_f64() * 200.);
}

fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Pass a SchDoc or pcbcraft.json")?;
    let doc = eda_io::load(std::path::Path::new(&path))?;
    let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
    println!(
        "components={} primitives={} parameters={}",
        s.components.len(),
        s.primitives.len(),
        s.parameter_objects.len()
    );
    measure("clone", || doc.clone());
    measure("validate", || doc.validate().unwrap());
    let id = s
        .components
        .first()
        .map(|c| c.id)
        .or_else(|| s.primitives.first().map(|p| p.id))
        .ok_or("Empty sheet")?;
    let selection = Selection {
        ids: vec![id],
        segments: vec![],
    };
    let pivot = edit::selection_anchor(s, &selection).ok_or("No anchor")?;
    let move_edit = Edit::TransformSelection {
        selection,
        pivot,
        quarter_turns: 0,
        delta: Point::mm(1, 1),
        connected: false,
    };
    let mut drag_edit = move_edit.clone();
    if let Edit::TransformSelection { connected, .. } = &mut drag_edit {
        *connected = true;
    }
    let wire = Edit::Insert(Fragment {
        primitives: vec![edit::primitive(
            eda_core::schematic::SchPrimitiveKind::Polyline {
                points: vec![Point::mm(10, 10), Point::mm(20, 10)],
                electrical: "Wire".into(),
                closed: false,
            },
            None,
        )],
        ..Default::default()
    });
    measure("move_preview", || {
        edit::preview_fragment(&doc, &move_edit).unwrap()
    });
    measure("drag_preview", || {
        edit::preview_fragment(&doc, &drag_edit).unwrap()
    });
    if !s.components.is_empty() {
        let mut group = move_edit.clone();
        if let Edit::TransformSelection { selection, .. } = &mut group {
            selection.ids = s.components.iter().map(|c| c.id).collect();
        }
        measure("group_move_preview", || {
            edit::preview_fragment(&doc, &group).unwrap()
        });
        if let Edit::TransformSelection { connected, .. } = &mut group {
            *connected = true;
        }
        measure("group_drag_preview", || {
            edit::preview_fragment(&doc, &group).unwrap()
        });
        if let Edit::TransformSelection {
            selection,
            pivot,
            delta,
            ..
        } = &group
        {
            measure("group_drag_geometry", || {
                edit::connected_transform(s, &selection.ids, *pivot, 0, *delta).unwrap()
            });
        }
    }
    measure("wire_preview", || {
        edit::preview_fragment(&doc, &wire).unwrap()
    });
    let duplicate = Edit::Insert(edit::extract(s, id)?.fresh_copy().transformed(
        pivot,
        0,
        Point::mm(5, 5),
    )?);
    measure("duplicate_preview", || {
        edit::preview_fragment(&doc, &duplicate).unwrap()
    });
    measure("delete_preview", || {
        edit::preview_fragment(&doc, &Edit::Delete(id)).unwrap()
    });
    measure("rotate_preview", || {
        edit::preview_fragment(
            &doc,
            &Edit::Transform {
                id,
                pivot,
                quarter_turns: 1,
                delta: Point::default(),
            },
        )
        .unwrap()
    });
    if let Some(p) = s.primitives.iter().find(|p| {
        p.component.is_none()
            && matches!(p.kind, eda_core::schematic::SchPrimitiveKind::Text { .. })
    }) {
        measure("text_preview", || {
            edit::preview_fragment(
                &doc,
                &Edit::Text {
                    id: p.id,
                    text: "Performance probe".into(),
                },
            )
            .unwrap()
        });
    }
    if let Some(p) = s.primitives.iter().find(|p| p.component.is_none() && matches!(&p.kind, eda_core::schematic::SchPrimitiveKind::Polyline { points, electrical, .. } if electrical == "Wire" && points.len() >= 2)) {
        measure("wire_drag_preview", || edit::preview_fragment(&doc, &Edit::DragWire { id: p.id, handle: edit::WireHandle::Vertex(0), delta: Point::mm(1, 1) }).unwrap());
    }
    measure("commit", || {
        let mut current = doc.clone();
        let mut history = eda_core::History::default();
        history
            .schematic_edit(
                &mut current,
                Request {
                    document: doc.id,
                    expected_revision: 0,
                    edit: move_edit.clone(),
                },
            )
            .unwrap();
    });
    let mut current = doc.clone();
    let mut history = eda_core::History::default();
    history.schematic_edit(
        &mut current,
        Request {
            document: doc.id,
            expected_revision: 0,
            edit: move_edit.clone(),
        },
    )?;
    measure("undo_redo", || {
        history.undo(&mut current);
        history.redo(&mut current);
    });
    if let Some(p) = s.primitives.first() {
        let mut value = p.clone();
        value.presentation.hidden = !value.presentation.hidden;
        let visibility = Edit::PrimitiveProperties {
            id: p.id,
            value: Box::new(value),
        };
        measure("visibility_preview", || {
            edit::preview_fragment(&doc, &visibility).unwrap()
        });
    }
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    for face in [
        "sch-sans",
        "sch-serif",
        "sch-mono",
        "sch-gost",
        "sch-altium-gost",
    ] {
        for suffix in ["", "-bold", "-italic", "-bold-italic"] {
            fonts.families.insert(
                egui::FontFamily::Name(format!("{face}{suffix}").into()),
                fonts.families[&egui::FontFamily::Proportional].clone(),
            );
        }
    }
    ctx.set_fonts(fonts);
    let mut view = eda_render::View::default();
    let frame = |view: &mut eda_render::View| {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 900.),
                )),
                focused: true,
                events: vec![egui::Event::PointerMoved(egui::pos2(500., 400.))],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    eda_render::canvas(ui, &doc, view);
                });
            },
        );
        ctx.tessellate(output.shapes, output.pixels_per_point)
    };
    frame(&mut view);
    measure("frame", || frame(&mut view));
    let patch = edit::preview_fragment(&doc, &drag_edit)?;
    let mut preview = eda_core::schematic::Schematic::blank();
    preview.fonts = patch.fonts;
    preview.primitives = patch.primitives;
    view.sch_preview = Some(preview);
    frame(&mut view);
    measure("preview_frame", || frame(&mut view));
    Ok(())
}
