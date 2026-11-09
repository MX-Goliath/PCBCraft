use super::*;
use crate::{View, pcb};
use eda_core::{
    pcb::*,
    pcb_route::{Conflict, RouteSession},
};
use eda_geom::Point;

#[test]
fn routing_tail_matches_full_gpu_pixels_and_keeps_buffers_across_bends_vias_and_backtrack() {
    let instance = wgpu::Instance::default();
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    for (format, dither) in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ]
    .into_iter()
    .flat_map(|format| [false, true].map(|dither| (format, dither)))
    {
        let ctx = Context::default();
        enable_gpu(&ctx, format);
        // Both paths must use the same framebuffer dithering. Native eframe
        // enables it for egui as well as resident PCB meshes; otherwise moving
        // tracks use an undithered shader while the reference scene dithers them.
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("pcb-artwork-test-dither"), dither));
        let mut fonts = egui::FontDefinitions::default();
        for name in [
            "sch-sans",
            "sch-sans-bold",
            "sch-sans-italic",
            "sch-sans-bold-italic",
        ] {
            fonts.families.insert(
                egui::FontFamily::Name(name.into()),
                fonts.families[&egui::FontFamily::Proportional].clone(),
            );
        }
        ctx.set_fonts(fonts);
        let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, dither);
        let mut board = Board::default();
        board.nets.push(Net {
            index: 0,
            name: "SIGNAL".into(),
            properties: Fields::new(),
        });
        for layer in [TOP, BOTTOM] {
            board.items.push(Item::new(
                layer,
                Some(0),
                Shape::Track {
                    start: Point::mm(-3, 0),
                    end: Point::mm(3, 0),
                    width: 700_000,
                },
            ));
            board.items.push(Item::new(
                layer,
                None,
                Shape::Fill {
                    start: Point::mm(-2, -2),
                    end: Point::mm(2, 2),
                    rotation: 0.,
                },
            ));
            board.items.push(Item::new(
                layer,
                Some(0),
                Shape::Pad {
                    position: Point::mm(-1, -1),
                    size: Point::mm(2, 1),
                    rotation: 20.,
                    shape: PadShape::Round,
                    hole: 400_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            ));
        }
        board.polygon_pours.push(eda_core::pcb_polygon::Polygon {
            layer: TOP,
            net: Some(0),
            contour: [
                Point::mm(-3, -3),
                Point::mm(3, -3),
                Point::mm(3, 3),
                Point::mm(-3, 3),
            ]
            .map(|position| ContourVertex {
                position,
                arc: None,
            })
            .into(),
            ..Default::default()
        });
        let mut copper = Item::new(
            TOP,
            Some(0),
            Shape::Fill {
                start: Point::mm(-3, -3),
                end: Point::mm(3, 3),
                rotation: 0.,
            },
        );
        copper.polygon = Some(0);
        board.items.push(copper);
        let original = board.clone();
        let mut route =
            RouteSession::begin(&board, 0, Point::default(), TOP, Some(0), 500_000, 0).unwrap();
        route.conflict = Conflict::Ignore;
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 7.;
        view.grid = false;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        view.pcb.display.origin_visible = false;
        // The first old scene mesh belongs to Top copper. Once it goes stale,
        // only later meshes from that scene remain; their camera must update.
        view.pcb.display.board_visible = false;
        view.pcb.active_layer = BOTTOM;
        view.pcb.visible.extend([35, 36, 37, 38]);
        let capture = |view: &mut View, renderer: &mut egui_wgpu::Renderer| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(64.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pcb::canvas(ui, &board, view);
                    });
                },
            );
            for (id, delta) in output.textures_delta.set {
                renderer.update_texture(&device, &queue, id, &delta);
            }
            let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
            image(&device, &queue, renderer, &jobs, format, 1.)
        };
        // Exercise actual source-to-route reuse, including a stale-copper toggle.
        let _ = capture(&mut view, &mut renderer);
        let mut stale = false;
        let mut resident = None;
        let mut builds = 0;
        for n in 0..16 {
            if n == 1 {
                view.camera.centre_mm[0] += 0.23;
            }
            match n {
                7 => {
                    view.pcb.active_layer = 37;
                    resident = None;
                }
                8 => {
                    view.pcb.active_layer = BOTTOM;
                    view.pcb.single_layer = 2;
                    resident = None;
                }
                9 => {
                    view.pcb.active_layer = TOP;
                    view.pcb.single_layer = 0;
                    resident = None;
                }
                12 => {
                    view.camera.mirrored_x = true;
                    resident = None;
                }
                13 => {
                    view.pcb.highlight_net = Some(0);
                    resident = None;
                }
                14 => {
                    view.pcb.display.draft[4] = true;
                    resident = None;
                }
                _ => {}
            }
            if n == 5 {
                route.place(Point::mm(1, 1), None).unwrap();
            }
            if n == 6 {
                route
                    .via(Point::mm(2, 1), BOTTOM, 1_200_000, 500_000)
                    .unwrap();
            }
            if n == 9 {
                assert!(route.backtrack());
            }
            if n == 10 {
                assert!(route.backtrack());
            }
            let end = match n % 4 {
                0 => route.anchor,
                1 => Point::mm(2, 2),
                2 => Point::mm(2, 1),
                _ => Point::mm(-2, -1),
            };
            view.pcb.set_preview(None);
            let preview = route.preview_shared(end).unwrap();
            if stale != preview.board.pours_stale {
                resident = None;
                stale = preview.board.pours_stale;
            }
            view.pcb.set_route_preview(preview.clone());
            // Dithering's sub-LSB perturbation can accumulate across overlapping
            // highlighted track/cap passes when their mesh batching differs.
            {
                let actual = capture(&mut view, &mut renderer);
                if let Some(old) = &resident {
                    assert_eq!(
                        view.pcb.paint_cache.builds, builds,
                        "tail change rebuilt stationary artwork at {n}"
                    );
                    assert!(std::sync::Arc::ptr_eq(
                        old,
                        view.pcb.paint_cache.resident_scene().as_ref().unwrap()
                    ));
                } else {
                    resident = view.pcb.paint_cache.resident_scene();
                    builds = view.pcb.paint_cache.builds;
                }
                let mut fresh = view.clone();
                fresh.pcb.set_preview(Some((*preview.board).clone()));
                fresh.pcb.paint_cache = Default::default();
                let expected = capture(&mut fresh, &mut renderer);
                let maximum = actual
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                let tolerance = if dither { 3 } else { 1 };
                assert!(
                    maximum <= tolerance,
                    "format={format:?}, frame {n}, dither={dither}: GPU routing pixels differ by {maximum}"
                );
            }
        }
        assert_eq!(board, original);
    }
}
