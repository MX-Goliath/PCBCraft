use super::*;

fn scene(radius: f32) -> Arc<Scene> {
    Arc::new(Scene {
        radius,
        centre: [82., 83., -0.8],
        thickness: 1.6,
        ..Default::default()
    })
}
fn camera() -> Camera {
    Camera {
        centre_mm: [95.707197, 63.233325],
        pixels_per_mm: 867.125,
        fitted: true,
        mirrored_x: false,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 0.0001 * b.abs().max(1.), "{a} != {b}");
}

#[test]
fn cold_entry_uses_the_ready_scene_and_actual_viewport_without_losing_high_zoom() {
    let input = camera();
    let mut state = State {
        yaw: 0.35,
        pitch: 0.7,
        ..Default::default()
    };
    state.enter_from_2d(&input);
    state.scene = scene(60.);
    let viewport = Vec2::new(540., 720.);
    state.sync_camera_viewport(viewport);
    assert!(state.zoom > 40.);
    close(state.pixels_per_mm(viewport) as f64, input.pixels_per_mm);
    assert_eq!([state.yaw, state.pitch], [0.35, 0.7]);
    let focus = state.focus.unwrap();
    close(focus[0] as f64, input.centre_mm[0]);
    close(focus[1] as f64, input.centre_mm[1]);
    let centre = project(focus, &state);
    assert_eq!([centre[0], centre[1]], [0., 0.]);
    let along_board = [focus[0] + state.yaw.cos(), focus[1] - state.yaw.sin(), 0.];
    let point = project(along_board, &state);
    close(
        (point[0] * state.pixels_per_mm(viewport)) as f64,
        input.pixels_per_mm,
    );
}

#[test]
fn repeated_switches_preserve_exact_area_scale_and_the_oblique_3d_pose() {
    let mut input = camera();
    input.mirrored_x = true;
    let original = input.clone();
    let viewport = Vec2::new(600., 780.);
    let mut state = State {
        scene: scene(60.),
        yaw: 1.3,
        pitch: -0.8,
        ..Default::default()
    };
    state.enter_from_2d(&input);
    state.sync_camera_viewport(viewport);
    state.pan = Vec2::new(31., -46.);
    state.zoom *= 1.25;
    state.leave_to_2d(&mut input);
    let centre = input.centre_mm;
    let scale = input.pixels_per_mm;
    let pose = state.pose(viewport);
    for _ in 0..100 {
        state.enter_from_2d(&input);
        state.sync_camera_viewport(viewport);
        assert_eq!(state.pose(viewport), pose);
        state.leave_to_2d(&mut input);
        assert_eq!(input.centre_mm, centre);
        assert_eq!(input.pixels_per_mm, scale);
        assert_eq!(input.mirrored_x, original.mirrored_x);
    }
}

#[test]
fn panning_and_zooming_in_3d_export_the_board_point_at_the_viewport_centre() {
    for perspective in [false, true] {
        for (yaw, pitch) in [(0., std::f32::consts::FRAC_PI_2), (0.7, 0.6), (2.4, -0.75)] {
            let viewport = Vec2::new(800., 520.);
            let mut input = camera();
            input.pixels_per_mm = 12.;
            let mut state = State {
                scene: scene(50.),
                yaw,
                pitch,
                ..Default::default()
            };
            state.presentation.perspective = perspective;
            state.enter_from_2d(&input);
            state.sync_camera_viewport(viewport);
            state.pan = Vec2::new(70., -40.);
            state.zoom *= 1.4;
            state.leave_to_2d(&mut input);
            let p = [input.centre_mm[0] as f32, input.centre_mm[1] as f32, 0.];
            let projected = project(p, &state);
            close(
                (projected[0] * state.pixels_per_mm(viewport) + state.pan.x) as f64,
                0.,
            );
            close(
                (-projected[1] * state.pixels_per_mm(viewport) + state.pan.y) as f64,
                0.,
            );
            let along_board = [p[0] + yaw.cos(), p[1] - yaw.sin(), 0.];
            let other = project(along_board, &state);
            close(
                ((other[0] - projected[0]) * state.pixels_per_mm(viewport)) as f64,
                input.pixels_per_mm,
            );
        }
    }
}

#[test]
fn navigation_in_2d_retargets_3d_but_keeps_its_viewing_direction() {
    let mut input = camera();
    let viewport = Vec2::new(800., 520.);
    let mut state = State {
        scene: scene(50.),
        yaw: 0.7,
        pitch: 0.6,
        ..Default::default()
    };
    state.enter_from_2d(&input);
    state.sync_camera_viewport(viewport);
    state.leave_to_2d(&mut input);
    input.centre_mm = [43.25, 81.75];
    input.pixels_per_mm = 47.;
    state.enter_from_2d(&input);
    state.sync_camera_viewport(viewport);
    assert_eq!([state.yaw, state.pitch], [0.7, 0.6]);
    assert_eq!(state.focus.unwrap(), [43.25, 81.75, 0.]);
    assert_eq!(state.pan, Vec2::ZERO);
    close(state.pixels_per_mm(viewport) as f64, 47.);
}

#[test]
fn immediate_return_before_scene_preparation_keeps_the_exact_2d_camera() {
    let mut input = camera();
    let original = input.clone();
    let mut state = State::default();
    for _ in 0..20 {
        state.enter_from_2d(&input);
        state.leave_to_2d(&mut input);
        assert_eq!(input.centre_mm, original.centre_mm);
        assert_eq!(input.pixels_per_mm, original.pixels_per_mm);
    }
    state.scene = scene(300.);
    state.enter_from_2d(&input);
    state.sync_camera_viewport(Vec2::new(400., 500.));
    close(
        state.pixels_per_mm(Vec2::new(400., 500.)) as f64,
        input.pixels_per_mm,
    );
}

#[test]
fn dock_resize_and_a_changed_scene_radius_keep_the_physical_scale() {
    let input = camera();
    let mut state = State {
        scene: scene(50.),
        ..Default::default()
    };
    state.enter_from_2d(&input);
    state.sync_camera_viewport(Vec2::new(800., 520.));
    let focus = state.focus;
    state.scene = scene(110.);
    let viewport = Vec2::new(420., 760.);
    state.sync_camera_viewport(viewport);
    close(state.pixels_per_mm(viewport) as f64, input.pixels_per_mm);
    assert_eq!(state.focus, focus);
    for _ in 0..100 {
        state.sync_camera_viewport(viewport);
    }
    close(state.pixels_per_mm(viewport) as f64, input.pixels_per_mm);
}

#[test]
fn an_edge_on_view_returns_to_its_board_anchor_without_a_distant_jump() {
    let mut input = camera();
    let mut state = State {
        scene: scene(50.),
        ..Default::default()
    };
    let viewport = Vec2::new(800., 520.);
    state.enter_from_2d(&input);
    state.sync_camera_viewport(viewport);
    state.pitch = 0.;
    state.pan = Vec2::new(30., 50.);
    state.leave_to_2d(&mut input);
    close(input.centre_mm[0], camera().centre_mm[0]);
    close(input.centre_mm[1], camera().centre_mm[1]);
    close(input.pixels_per_mm, camera().pixels_per_mm);
}
