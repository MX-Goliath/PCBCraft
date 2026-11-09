use super::*;

#[test]
fn pending_graphics_requests_are_bounded_and_stale_results_are_rejected() {
    let ctx = egui::Context::default();
    let mut cache = Cache::default();
    let (sender, receiver) = mpsc::sync_channel(1);
    cache.preparation.lock().unwrap().job = Some(Job { key: 1, receiver });
    let doc = Arc::new(Document::new(DocumentKind::Board, "Review"));
    for key in 2..20 {
        start(
            &mut cache,
            Sources {
                before: None,
                after: Some(doc.clone()),
                diff: Arc::new(Diff::default()),
            },
            &View::default(),
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.)),
            [true; 4],
            key,
            &ctx,
        );
        assert_eq!(
            cache.preparation.lock().unwrap().job.as_ref().unwrap().key,
            1
        );
    }
    sender
        .send(Completed {
            cache: Cache {
                key: Some(1),
                ..Default::default()
            },
            textures: vec![],
        })
        .unwrap_or_else(|_| panic!("receiver lost"));
    poll(&mut cache, 19, &ctx);
    assert!(
        cache.key.is_none(),
        "Stale content must never become visible"
    );
    assert!(!pending(&cache));
}

#[test]
fn cpu_atlas_and_sheet_images_get_ui_owned_textures_without_changing_pixels() {
    let ctx = egui::Context::default();
    let mut completed = Completed {
        cache: Cache::default(),
        textures: vec![],
    };
    for (n, color) in [(8, Color32::RED), (9, Color32::GREEN)] {
        let id = egui::TextureId::Managed(n);
        let mut mesh = egui::Mesh::with_texture(id);
        mesh.add_rect_with_uv(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        completed.cache.meshes.push(Arc::new(mesh));
        completed.cache.gpu_slots.push(None);
        completed.textures.push((
            id,
            egui::epaint::ImageDelta::full(
                egui::ColorImage::new([2, 2], vec![color; 4]),
                egui::TextureOptions::LINEAR,
            ),
        ));
    }
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        if completed.cache.textures.is_empty() {
            publish_textures(&mut completed, ctx).unwrap();
        }
    });
    assert_eq!(completed.cache.textures.len(), 2);
    for (slot, color) in [Color32::RED, Color32::GREEN].into_iter().enumerate() {
        let mesh = &completed.cache.meshes[slot];
        assert_eq!(mesh.texture_id, completed.cache.textures[slot].id());
        assert!(output.textures_delta.set.iter().any(|(id, delta)| {
            let egui::ImageData::Color(image) = &delta.image;
            *id == mesh.texture_id && image.pixels == vec![color; 4]
        }));
    }
}

#[test]
fn shared_schematic_and_board_prepare_complete_offscreen_artwork_then_reuse_it_for_pan() {
    for pcb in [false, true] {
        let mut doc = Document::new(
            if pcb {
                DocumentKind::Board
            } else {
                DocumentKind::Schematic
            },
            "Shared review",
        );
        for x in [10, 2000] {
            if pcb {
                doc.pcb
                    .get_or_insert_with(Default::default)
                    .items
                    .push(eda_core::pcb::Item::new(
                        eda_core::pcb::TOP,
                        None,
                        eda_core::pcb::Shape::Track {
                            start: Point::mm(x, 10),
                            end: Point::mm(x + 2, 10),
                            width: 300_000,
                        },
                    ));
            } else {
                let s = doc.schematic.as_mut().unwrap();
                s.sheet.border = false;
                s.sheet.grid_visible = false;
                s.primitives.push(eda_core::schematic_edit::primitive(
                    eda_core::schematic::SchPrimitiveKind::Rectangle {
                        a: Point::mm(x, 10),
                        b: Point::mm(x + 2, 12),
                        radius: Point::default(),
                    },
                    None,
                ));
            }
        }
        let diff = Arc::new(eda_core::document_diff::compare(None, Some(&doc)));
        let doc = Arc::new(doc);
        let original = doc.as_ref().clone();
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            egui::FontFamily::Name("sch-sans".into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        view.camera.centre_mm = [10., 10.];
        let frame = |view: &mut View| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600., 400.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        super::super::canvas_shared(
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
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while view.review_cache.key.is_none() && std::time::Instant::now() < deadline {
            frame(&mut view);
            std::thread::yield_now();
        }
        assert!(view.review_cache.key.is_some(), "Graphics did not complete");
        assert_eq!(view.review_cache.builds, 1);
        assert!(
            view.review_cache
                .meshes
                .iter()
                .any(|m| m.vertices.iter().any(|v| v.pos.x > 10000.)),
            "Worker culled offscreen artwork"
        );
        view.camera.centre_mm = [2000., 10.];
        frame(&mut view);
        assert_eq!(view.review_cache.builds, 1);
        assert_eq!(*doc, original);
    }
}
