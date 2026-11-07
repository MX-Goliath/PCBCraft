//! Semantic review uses the same SCH/PCB geometry and camera as the editors.
use crate::*;
use eda_core::document_diff::{Change, Diff, Kind};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// One immutable comparison's artwork. Camera translation reuses resident meshes.
#[derive(Clone, Default)]
pub(crate) struct Cache {
    key: Option<u64>,
    scale: f64,
    origin: Pos2,
    meshes: Vec<Arc<egui::epaint::Mesh>>,
    gpu: Option<Arc<crate::pcb::gpu::Scene>>,
    gpu_slots: Vec<Option<usize>>,
    atlas: Option<egui::TextureHandle>,
    visible_changes: Vec<(usize, eda_core::pcb::Bounds)>,
    builds: usize,
    zoom_changed: Option<(f64, std::time::Instant)>,
}
impl Cache {
    fn paint(&self, painter: &egui::Painter, view: &View, rect: Rect) {
        let scale = (view.camera.pixels_per_mm / self.scale) as f32;
        let delta =
            view.camera.screen(Point::default(), rect).to_vec2() - self.origin.to_vec2() * scale;
        for (slot, mesh) in self.meshes.iter().enumerate() {
            if let (Some(gpu), Some(gpu_slot)) = (&self.gpu, self.gpu_slots[slot]) {
                painter.add(gpu.callback(gpu_slot, rect, delta, scale, false));
            } else if scale == 1. && delta == Vec2::ZERO {
                painter.add(egui::Shape::Mesh(mesh.clone()));
            } else {
                let mut moved = (**mesh).clone();
                for vertex in &mut moved.vertices {
                    vertex.pos = (vertex.pos.to_vec2() * scale + delta).to_pos2();
                }
                painter.add(egui::Shape::mesh(moved));
            }
        }
    }
}

pub fn color(kind: Kind) -> Color32 {
    match kind {
        Kind::Removed => Color32::from_rgb(238, 83, 83),
        Kind::Moved => Color32::from_rgb(67, 154, 255),
        Kind::Modified => Color32::from_rgb(242, 158, 51),
        Kind::Added => Color32::from_rgb(58, 188, 104),
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Overlay,
    Before,
    After,
}

pub fn bounds(change: &Change) -> eda_core::pcb::Bounds {
    let mut bounds = eda_core::pcb::Bounds::empty();
    for target in [&change.before, &change.after].into_iter().flatten() {
        bounds.union(target.bounds);
    }
    bounds
}
pub fn fit(view: &mut View, bounds: eda_core::pcb::Bounds, rect: Rect) {
    if bounds.empty || rect.width() < 80. || rect.height() < 80. {
        return;
    }
    view.camera.centre_mm = [
        (bounds.min.x as f64 + bounds.max.x as f64) / 2e6,
        (bounds.min.y as f64 + bounds.max.y as f64) / 2e6,
    ];
    view.camera.pixels_per_mm = ((rect.width() as f64 - 100.)
        / ((bounds.max.x - bounds.min.x).max(8_000_000) as f64 / 1e6))
        .min(
            (rect.height() as f64 - 100.)
                / ((bounds.max.y - bounds.min.y).max(8_000_000) as f64 / 1e6),
        )
        .clamp(0.01, 500.);
    view.camera.fitted = true;
}
fn target_visible(
    target: &eda_core::document_diff::Target,
    visible: Option<&HashSet<Uuid>>,
) -> bool {
    visible.is_none_or(|ids| target.ids.iter().any(|id| ids.contains(id)))
}
fn overlay(
    painter: &egui::Painter,
    doc: &Document,
    view: &mut View,
    rect: Rect,
    ids: &HashSet<Uuid>,
    color: Color32,
) {
    if let Some(board) = &doc.pcb {
        let items: Vec<_> = board
            .items
            .iter()
            .filter(|i| ids.contains(&i.id) && crate::pcb::artwork_visible(board, i, &view.pcb))
            .collect();
        for item in &items {
            crate::pcb::paint_item(
                painter,
                &view.camera,
                rect,
                item,
                view.pcb.active_layer,
                crate::pcb::review_tint(color, item, &view.pcb),
                &mut view.pcb,
            );
        }
        // Copper tinting must retain drill openings and native pad/net labels.
        for item in &items {
            crate::pcb::paint_details(painter, board, &view.camera.clone(), view, rect, item);
        }
    } else {
        crate::schematic::diff_overlay(painter, doc, &view.camera, rect, ids, color);
    }
}
#[allow(clippy::too_many_arguments)]
pub fn canvas(
    ui: &mut egui::Ui,
    before: Option<&Document>,
    after: Option<&Document>,
    diff: &Diff,
    view: &mut View,
    mode: Mode,
    visible: [bool; 4],
    selected: Option<usize>,
    fit_request: bool,
) -> Option<usize> {
    let rect = ui.available_rect_before_wrap();
    if (mode == Mode::Before && before.is_none()) || (mode == Mode::After && after.is_none()) {
        ui.centered_and_justified(|ui| {
            ui.label("File / library member does not exist in this revision")
        });
        return None;
    }
    let Some(doc) = (if mode == Mode::Before {
        before.or(after)
    } else {
        after.or(before)
    }) else {
        ui.centered_and_justified(|ui| ui.label("No document in this revision"));
        return None;
    };
    // The native Selected Objects command targets the selected semantic change,
    // including its old position; native edit selections are intentionally empty.
    let selection_fit = selected.is_some()
        && !view.pcb.three_d.enabled
        && matches!(
            view.pcb.requested_fit,
            Some(crate::pcb::FitTarget::Selection)
        );
    if selection_fit {
        view.pcb.requested_fit = None;
    }
    let fit_request = fit_request || selection_fit;
    if fit_request || !view.camera.fitted {
        let mut b = eda_core::pcb::Bounds::empty();
        if fit_request && let Some(c) = selected.and_then(|i| diff.changes.get(i)) {
            b = bounds(c);
        } else {
            for c in &diff.changes {
                b.union(bounds(c));
            }
        }
        if b.empty {
            if let Some(board) = &doc.pcb {
                b = board.bounds();
            } else {
                b = eda_core::pcb::Bounds::points(&[Point::default(), doc.size]);
            }
        }
        fit(view, b, rect);
    }
    view.selected = None;
    view.sch_selection.clear();
    view.sch_segments.clear();
    view.pcb.selected.clear();
    view.pcb.input = None;
    if let Some(board) = &doc.pcb {
        view.pcb.initialize(board);
    }
    // Input is view-only: callers never route this canvas to editor commands.
    view.review_overlay = mode == Mode::Overlay && !view.pcb.three_d.enabled;
    let _ = crate::canvas(ui, doc, view);
    view.review_overlay = false;
    view.pcb.input = None;
    if mode != Mode::Overlay || view.pcb.three_d.enabled {
        return None;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        0.,
        if doc.kind == DocumentKind::Schematic {
            Color32::from_white_alpha(160)
        } else {
            Color32::from_black_alpha(155)
        },
    );
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (
        crate::pcb::review_paint_key(view, ui.ctx()),
        visible,
        diff as *const Diff as usize,
        diff.changes.len(),
        before.map(|d| (d.id, d as *const Document as usize)),
        after.map(|d| (d.id, d as *const Document as usize)),
    )
        .hash(&mut hash);
    let key = hash.finish();
    let mut cache = std::mem::take(&mut view.review_cache);
    let zoom = view.camera.pixels_per_mm / cache.scale;
    // Scale resident artwork through a gesture, then refine stroke minimums,
    // labels and glyph rasterization once the zoom settles, just like native PCB.
    let refine = if view.camera.pixels_per_mm != cache.scale {
        let (scale, changed) = cache
            .zoom_changed
            .get_or_insert((view.camera.pixels_per_mm, std::time::Instant::now()));
        if *scale != view.camera.pixels_per_mm {
            *scale = view.camera.pixels_per_mm;
            *changed = std::time::Instant::now();
        }
        let remaining = std::time::Duration::from_millis(120).saturating_sub(changed.elapsed());
        ui.ctx().request_repaint_after(remaining);
        remaining.is_zero()
    } else {
        false
    };
    if cache.key != Some(key) || !(0.8..=1.25).contains(&zoom) || refine {
        let layer = painter.layer_id();
        let saved = ui.ctx().graphics_mut(|g| std::mem::take(g.entry(layer)));
        let capture = painter.with_clip_rect(Rect::EVERYTHING);
        // Native layer colors (especially red Top Layer) must not masquerade as
        // change classifications. Repaint the context in a neutral ink first.
        let context_ids = if let Some(board) = &doc.pcb {
            board.items.iter().map(|item| item.id).collect()
        } else if let Some(sch) = &doc.schematic {
            sch.primitives
                .iter()
                .chain(&sch.parameter_objects)
                .map(|item| item.id)
                .chain(sch.components.iter().map(|item| item.id))
                .collect()
        } else {
            HashSet::new()
        };
        overlay(
            &capture,
            doc,
            view,
            rect,
            &context_ids,
            Color32::from_gray(if doc.kind == DocumentKind::Schematic {
                175
            } else {
                95
            }),
        );
        for kind in [Kind::Removed, Kind::Moved, Kind::Modified, Kind::Added] {
            if !visible[kind_index(kind)] {
                continue;
            }
            let mut old = HashSet::new();
            let mut new = HashSet::new();
            for c in diff.changes.iter().filter(|c| c.kind == kind) {
                if let Some(t) = &c.before {
                    old.extend(t.ids.iter().copied());
                }
                if let Some(t) = &c.after {
                    new.extend(t.ids.iter().copied());
                }
            }
            if let Some(doc) = before {
                overlay(
                    &capture,
                    doc,
                    view,
                    rect,
                    &old,
                    color(kind).gamma_multiply(if kind == Kind::Removed { 1. } else { 0.55 }),
                );
            }
            if let Some(doc) = after {
                overlay(&capture, doc, view, rect, &new, color(kind));
            }
        }

        let visible_ids = |doc: Option<&Document>| {
            doc.and_then(|d| d.pcb.as_ref()).map(|board| {
                board
                    .items
                    .iter()
                    .filter(|item| crate::pcb::artwork_visible(board, item, &view.pcb))
                    .map(|item| item.id)
                    .collect::<HashSet<_>>()
            })
        };
        let old_visible = visible_ids(before);
        let new_visible = visible_ids(after);
        cache.visible_changes = diff
            .changes
            .iter()
            .enumerate()
            .filter(|(_, c)| visible[kind_index(c.kind)])
            .filter(|(_, c)| {
                c.before
                    .as_ref()
                    .is_some_and(|t| target_visible(t, old_visible.as_ref()))
                    || c.after
                        .as_ref()
                        .is_some_and(|t| target_visible(t, new_visible.as_ref()))
            })
            .map(|(i, c)| (i, bounds(c)))
            .collect();
        let shapes = ui.ctx().graphics_mut(|g| {
            std::mem::replace(g.entry(layer), saved)
                .all_entries()
                .cloned()
                .collect()
        });
        // Isolate UVs from ordinary UI text growing/resetting egui's font atlas.
        let image = ui.ctx().fonts(|f| f.image());
        let atlas = ui.ctx().load_texture(
            "git-review-artwork",
            image.clone(),
            egui::TextureOptions::LINEAR,
        );
        cache.meshes = ui
            .ctx()
            .tessellate(shapes, ui.ctx().pixels_per_point())
            .into_iter()
            .filter_map(|p| {
                if let egui::epaint::Primitive::Mesh(mut mesh) = p.primitive {
                    if mesh.texture_id == egui::TextureId::default() {
                        mesh.texture_id = atlas.id();
                    }
                    Some(Arc::new(mesh))
                } else {
                    None
                }
            })
            .collect();
        let mut gpu_meshes = Vec::new();
        cache.gpu_slots = cache
            .meshes
            .iter()
            .map(|mesh| {
                if mesh.texture_id == atlas.id() {
                    let slot = gpu_meshes.len();
                    gpu_meshes.push(mesh.clone());
                    Some(slot)
                } else {
                    None
                }
            })
            .collect();
        cache.gpu = crate::pcb::gpu_format(ui.ctx())
            .map(|format| crate::pcb::gpu::Scene::new(gpu_meshes, image, format));
        cache.atlas = Some(atlas);
        cache.origin = view.camera.screen(Point::default(), rect);
        cache.scale = view.camera.pixels_per_mm;
        cache.key = Some(key);
        cache.zoom_changed = None;
        cache.builds += 1;
    }
    cache.paint(&painter, view, rect);
    let pointer = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|p| rect.contains(*p));
    let mut hovered = None;
    for &(i, b) in &cache.visible_changes {
        let c = &diff.changes[i];
        if b.empty {
            continue;
        }
        let box_rect = Rect::from_two_pos(
            view.camera.screen(b.min, rect),
            view.camera.screen(b.max, rect),
        )
        .expand(5.);
        if pointer.is_some_and(|p| box_rect.contains(p)) {
            hovered = Some(i);
        }
        if selected != Some(i) {
            continue;
        }
        if let Some(target) = &c.after
            && selected == Some(i)
            && !target.bounds.empty
        {
            let next_rect = Rect::from_two_pos(
                view.camera.screen(target.bounds.min, rect),
                view.camera.screen(target.bounds.max, rect),
            )
            .expand(3.);
            painter.rect_stroke(
                next_rect,
                0.,
                Stroke::new(0.75, color(c.kind)),
                StrokeKind::Outside,
            );
        }
        if let Some(old) = &c.before
            && selected == Some(i)
            && (c.kind == Kind::Removed || c.moved)
            && !old.bounds.empty
        {
            let old_rect = Rect::from_two_pos(
                view.camera.screen(old.bounds.min, rect),
                view.camera.screen(old.bounds.max, rect),
            )
            .expand(4.);
            let tint = color(if c.moved { Kind::Moved } else { Kind::Removed });
            for [a, b] in [
                old_rect.left_top(),
                old_rect.right_top(),
                old_rect.right_bottom(),
                old_rect.left_bottom(),
                old_rect.left_top(),
            ]
            .windows(2)
            .map(|p| [p[0], p[1]])
            {
                let length = a.distance(b);
                let count = (length / 9.).ceil().min(2000.) as usize;
                for j in 0..count {
                    let at = j as f32 * 9.;
                    painter.line_segment(
                        [
                            a.lerp(b, (at / length).min(1.)),
                            a.lerp(b, ((at + 5.) / length).min(1.)),
                        ],
                        Stroke::new(1., tint),
                    );
                }
            }
        }
        if selected == Some(i) {
            painter.rect_stroke(
                box_rect,
                0.,
                Stroke::new(1.5, color(c.kind)),
                StrokeKind::Outside,
            );
            if c.moved
                && let (Some(a), Some(b)) = (&c.before, &c.after)
            {
                let centre = |b: eda_core::pcb::Bounds| Point {
                    x: b.min.x + (b.max.x - b.min.x) / 2,
                    y: b.min.y + (b.max.y - b.min.y) / 2,
                };
                let a = view.camera.screen(centre(a.bounds), rect);
                let b = view.camera.screen(centre(b.bounds), rect);
                painter.arrow(a, b - a, Stroke::new(2., color(Kind::Moved)));
            }
        }
    }
    view.review_cache = cache;
    let response = ui.interact(rect, ui.id().with("git-change-picking"), Sense::click());
    let clicked = response.clicked().then_some(hovered).flatten();
    if let Some(i) = hovered {
        let c = &diff.changes[i];
        response.on_hover_text(format!(
            "{} · {}{}",
            c.kind.label(),
            c.label,
            if c.moved && c.kind != Kind::Moved {
                " · also moved"
            } else {
                ""
            }
        ));
    }
    clicked
}
pub fn kind_index(kind: Kind) -> usize {
    match kind {
        Kind::Removed => 0,
        Kind::Moved => 1,
        Kind::Modified => 2,
        Kind::Added => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_changes_are_pickable_and_pan_does_not_leave_stale_hit_boxes() {
        let mut doc = Document::new(DocumentKind::Board, "picking");
        doc.pcb = Some(eda_core::pcb::Board::default());
        doc.pcb
            .as_mut()
            .unwrap()
            .items
            .push(eda_core::pcb::Item::new(
                eda_core::pcb::TOP,
                None,
                eda_core::pcb::Shape::Track {
                    start: Point::mm(0, 5),
                    end: Point::mm(10, 5),
                    width: 300_000,
                },
            ));
        let diff = eda_core::document_diff::compare(None, Some(&doc));
        let ctx = egui::Context::default();
        let mut view = View::default();
        view.camera.centre_mm = [5., 5.];
        view.camera.pixels_per_mm = 20.;
        view.camera.fitted = true;
        let pointer = Pos2::new(450., 325.);
        let frame = |view: &mut View, events| {
            let mut picked = None;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 650.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        picked = canvas(
                            ui,
                            None,
                            Some(&doc),
                            &diff,
                            view,
                            Mode::Overlay,
                            [true; 4],
                            None,
                            false,
                        );
                    });
                },
            );
            picked
        };
        frame(&mut view, vec![egui::Event::PointerMoved(pointer)]);
        for expected in [Some(0), None] {
            frame(
                &mut view,
                vec![egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert_eq!(
                frame(
                    &mut view,
                    vec![egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE
                    }]
                ),
                expected
            );
            view.camera.centre_mm[0] += 100.;
            frame(&mut view, vec![egui::Event::PointerMoved(pointer)]);
        }
        assert_eq!(view.review_cache.builds, 1);
    }
    #[test]
    fn large_pcb_and_schematic_reviews_reuse_artwork_during_navigation() {
        for pcb in [true, false] {
            let mut doc = Document::new(
                if pcb {
                    DocumentKind::Board
                } else {
                    DocumentKind::Schematic
                },
                "large additions",
            );
            if pcb {
                doc.pcb = Some(eda_core::pcb::Board::default());
            }
            for i in 0..5000 {
                let a = Point::mm(i % 100, i / 100);
                let b = Point::mm(i % 100 + 1, i / 100 + 1);
                if let Some(board) = &mut doc.pcb {
                    board.items.push(eda_core::pcb::Item::new(
                        eda_core::pcb::TOP,
                        None,
                        eda_core::pcb::Shape::Track {
                            start: a,
                            end: b,
                            width: 100_000,
                        },
                    ));
                } else {
                    doc.schematic.as_mut().unwrap().primitives.push(
                        eda_core::schematic_edit::primitive(
                            eda_core::schematic::SchPrimitiveKind::Rectangle {
                                a,
                                b,
                                radius: Point::default(),
                            },
                            None,
                        ),
                    );
                }
            }
            let diff = eda_core::document_diff::compare(None, Some(&doc));
            assert!(diff.changes.len() >= 5000); // SCH also compares sheet metadata.
            let ctx = egui::Context::default();
            let mut fonts = egui::FontDefinitions::default();
            fonts.families.insert(
                egui::FontFamily::Name("sch-sans".into()),
                fonts.families[&egui::FontFamily::Proportional].clone(),
            );
            ctx.set_fonts(fonts);
            let mut view = View::default();
            let frame = |view: &mut View, visible| {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 650.))),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            canvas(
                                ui,
                                None,
                                Some(&doc),
                                &diff,
                                view,
                                Mode::Overlay,
                                visible,
                                None,
                                false,
                            );
                        });
                    },
                );
            };
            for _ in 0..12 {
                frame(&mut view, [true; 4]);
                view.camera.centre_mm[0] += 1.;
                view.camera.pixels_per_mm *= 1.005;
            }
            assert_eq!(
                view.review_cache.builds, 1,
                "pan/zoom must reuse {pcb} artwork"
            );
            assert_eq!(
                view.review_cache
                    .visible_changes
                    .iter()
                    .filter(|(_, bounds)| !bounds.empty)
                    .count(),
                5000
            );
            assert!(!view.review_cache.meshes.is_empty());
            frame(&mut view, [true, true, true, false]);
            assert_eq!(view.review_cache.builds, 2);
            assert!(view.review_cache.visible_changes.is_empty());
            view.camera.mirrored_x = true;
            frame(&mut view, [true; 4]);
            assert_eq!(view.review_cache.builds, 3);
            view.camera.pixels_per_mm *= 2.;
            frame(&mut view, [true; 4]);
            assert_eq!(
                view.review_cache.builds, 4,
                "large zoom refines text/strokes"
            );
        }
    }
    #[test]
    fn pcb_overlay_respects_hidden_layers_and_keeps_view_configuration() {
        let mut before = Document::new(DocumentKind::Board, "Layers");
        before.pcb = Some(eda_core::pcb::Board::default());
        let mut after = before.clone();
        after
            .pcb
            .as_mut()
            .unwrap()
            .items
            .push(eda_core::pcb::Item::new(
                eda_core::pcb::TOP,
                None,
                eda_core::pcb::Shape::Track {
                    start: Point::mm(0, 0),
                    end: Point::mm(10, 0),
                    width: 1_000_000,
                },
            ));
        let diff = eda_core::document_diff::compare(Some(&before), Some(&after));
        let ctx = egui::Context::default();
        let mut view = View::default();
        view.pcb.initialize(after.pcb.as_ref().unwrap());
        for hidden in [false, true] {
            if hidden {
                view.pcb.visible.remove(&eda_core::pcb::TOP);
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(
                            ui,
                            Some(&before),
                            Some(&after),
                            &diff,
                            &mut view,
                            Mode::Overlay,
                            [true; 4],
                            None,
                            true,
                        );
                    });
                },
            );
            let green = ctx.tessellate(output.shapes, 1.).iter().any(|p| matches!(&p.primitive,
                egui::epaint::Primitive::Mesh(m) if m.vertices.iter().any(|v| v.color == color(Kind::Added))));
            assert_eq!(green, !hidden);
            assert_eq!(view.pcb.visible.contains(&eda_core::pcb::TOP), !hidden);
        }
    }
    #[test]
    fn actual_schematic_geometry_tessellates_in_all_four_semantic_colors() {
        use eda_core::{schematic::SchPrimitiveKind as K, schematic_edit::primitive};
        let mut before = Document::new(DocumentKind::Schematic, "Review");
        for x in [20, 50, 80] {
            before
                .schematic
                .as_mut()
                .unwrap()
                .primitives
                .push(primitive(
                    K::Rectangle {
                        a: Point::mm(x, 30),
                        b: Point::mm(x + 10, 40),
                        radius: Point::default(),
                    },
                    None,
                ));
        }
        let mut after = before.clone();
        let s = after.schematic.as_mut().unwrap();
        if let K::Rectangle { a, b, .. } = &mut s.primitives[0].kind {
            a.y += 5_000_000;
            b.y += 5_000_000;
        }
        s.primitives[1].width_nm = 800_000;
        s.primitives.remove(2);
        s.primitives.push(primitive(
            K::Rectangle {
                a: Point::mm(95, 25),
                b: Point::mm(105, 35),
                radius: Point::default(),
            },
            None,
        ));
        let original = before.clone();
        let original_after = after.clone();
        let diff = eda_core::document_diff::compare(Some(&before), Some(&after));
        let mut view = View::default();
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            egui::FontFamily::Name("sch-sans".into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 650.))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    canvas(
                        ui,
                        Some(&before),
                        Some(&after),
                        &diff,
                        &mut view,
                        Mode::Overlay,
                        [true; 4],
                        None,
                        true,
                    );
                });
            },
        );
        let meshes = ctx.tessellate(output.shapes, 1.);
        for kind in [Kind::Removed, Kind::Moved, Kind::Modified, Kind::Added] {
            assert!(meshes.iter().any(|p|matches!(&p.primitive,egui::epaint::Primitive::Mesh(m) if m.vertices.iter().any(|v|v.color==color(kind)))),"Missing {kind:?} geometry");
        }
        assert_eq!(before, original);
        assert_eq!(after, original_after);
    }
}
