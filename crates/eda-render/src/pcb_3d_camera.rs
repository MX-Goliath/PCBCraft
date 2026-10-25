//! Transfer the visible board-plane centre and physical scale between 2D/3D.
use super::*;
use crate::Camera;
#[cfg(test)]
#[path = "pcb_3d_camera_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq)]
struct PlaneView {
    centre: [f64; 2],
    scale: f64,
}
impl PlaneView {
    fn from_camera(camera: &Camera) -> Option<Self> {
        (camera.fitted
            && camera.centre_mm.iter().all(|v| v.is_finite())
            && camera.pixels_per_mm.is_finite()
            && camera.pixels_per_mm > 0.)
            .then_some(Self {
                centre: camera.centre_mm,
                scale: camera.pixels_per_mm,
            })
    }
    fn apply(self, camera: &mut Camera) {
        camera.centre_mm = self.centre;
        camera.pixels_per_mm = self.scale;
        camera.fitted = true;
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    angles: [u32; 2],
    zoom: u32,
    pan: [u32; 2],
    focus: Option<[u32; 3]>,
    viewport: [u32; 2],
    radius: u32,
    perspective: bool,
    thickness: u32,
}
#[derive(Clone, Copy)]
struct Transfer {
    view: PlaneView,
    resume_scale: Option<f32>,
}
#[derive(Clone, Default)]
pub(super) struct Link {
    pending: Option<Transfer>,
    viewport: Option<Vec2>,
    radius: Option<f32>,
    checkpoint: Option<(PlaneView, Pose)>,
}

impl State {
    fn pose(&self, viewport: Vec2) -> Pose {
        Pose {
            angles: [self.yaw.to_bits(), self.pitch.to_bits()],
            zoom: self.zoom.to_bits(),
            pan: [self.pan.x.to_bits(), self.pan.y.to_bits()],
            focus: self.focus.map(|p| p.map(f32::to_bits)),
            viewport: [viewport.x.to_bits(), viewport.y.to_bits()],
            radius: self.scene.radius.to_bits(),
            perspective: self.presentation.perspective,
            thickness: self.presentation.thickness_scale.to_bits(),
        }
    }
    pub(super) fn pixels_per_mm(&self, viewport: Vec2) -> f32 {
        viewport.min_elem() * 0.43 / self.scene.radius.max(0.1) * self.zoom
    }
    pub(super) fn zoom_for_scale(&self, viewport: Vec2, scale: f32) -> f32 {
        scale * self.scene.radius.max(0.1) / (viewport.min_elem() * 0.43)
    }
    /// Defer the transfer until the real 3D viewport and scene radius are known.
    pub fn enter_from_2d(&mut self, camera: &Camera) {
        let Some(view) = PlaneView::from_camera(camera) else {
            return;
        };
        let resume_scale = self.camera_link.viewport.and_then(|viewport| {
            self.camera_link
                .checkpoint
                .filter(|(previous, pose)| *previous == view && *pose == self.pose(viewport))
                .map(|_| self.pixels_per_mm(viewport))
        });
        self.camera_link.pending = Some(Transfer { view, resume_scale });
    }
    /// Returning to 2D while 3D is still preparing must keep the incoming view.
    pub fn leave_to_2d(&mut self, camera: &mut Camera) {
        if let Some(transfer) = self.camera_link.pending.take() {
            transfer.view.apply(camera);
            return;
        }
        let Some(viewport) = self.camera_link.viewport else {
            return;
        };
        let pose = self.pose(viewport);
        let view = self
            .camera_link
            .checkpoint
            .filter(|(_, previous)| *previous == pose)
            .map(|(view, _)| view)
            .unwrap_or_else(|| self.board_plane_view(viewport));
        view.apply(camera);
        self.camera_link.checkpoint = Some((view, pose));
    }
    pub(super) fn sync_camera_viewport(&mut self, viewport: Vec2) {
        if !viewport.is_finite() || viewport.min_elem() <= 0. {
            return;
        }
        if let Some(transfer) = self.camera_link.pending.take() {
            if let Some(scale) = transfer.resume_scale {
                if self.camera_link.viewport != Some(viewport)
                    || self.camera_link.radius != Some(self.scene.radius)
                {
                    self.zoom = self.zoom_for_scale(viewport, scale);
                }
            } else {
                self.focus = Some([
                    transfer.view.centre[0] as f32,
                    transfer.view.centre[1] as f32,
                    0.,
                ]);
                self.pan = Vec2::ZERO;
                self.zoom = self.zoom_for_scale(viewport, transfer.view.scale as f32);
            }
            self.camera_link.checkpoint = Some((transfer.view, self.pose(viewport)));
        } else if let (Some(previous), Some(radius)) =
            (self.camera_link.viewport, self.camera_link.radius)
        {
            // Dock/toolbar size changes and scene rebuilds must not implicitly
            // zoom the view; the scale is measured in logical pixels per mm.
            self.zoom *= previous.min_elem() / viewport.min_elem() * self.scene.radius.max(0.1)
                / radius.max(0.1);
        }
        self.camera_link.viewport = Some(viewport);
        self.camera_link.radius = Some(self.scene.radius);
    }
    fn board_plane_view(&self, viewport: Vec2) -> PlaneView {
        let focus = self.focus.unwrap_or(self.scene.centre);
        let scale = self.pixels_per_mm(viewport);
        let fallback = PlaneView {
            centre: [focus[0] as f64, focus[1] as f64],
            scale: scale as f64,
        };
        // At an edge-on angle the centre ray may never meet the board plane.
        // Retain its board anchor rather than exporting a distant horizon.
        if self.pitch.sin().abs() < 0.001 {
            return fallback;
        }
        let x = -self.pan.x / scale;
        let y = self.pan.y / scale;
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let inverse = |p: [f32; 3]| {
            [
                cy * p[0] + sy * (sp * p[1] - cp * p[2]),
                -sy * p[0] + cy * (sp * p[1] - cp * p[2]),
                cp * p[1] + sp * p[2],
            ]
        };
        let d = self.scene.radius.max(0.1) * 4.;
        let (origin, direction) = if self.presentation.perspective {
            (inverse([0., 0., d]), inverse([x / d, y / d, -1.]))
        } else {
            (inverse([x, y, d]), inverse([0., 0., -1.]))
        };
        let scaled_focus_z = focus[2]
            + focus[2].clamp(-self.scene.thickness, 0.) * (self.presentation.thickness_scale - 1.);
        let origin = [
            origin[0] + focus[0],
            origin[1] + focus[1],
            origin[2] + scaled_focus_z,
        ];
        let t = -origin[2] / direction[2];
        let point = std::array::from_fn(|k| origin[k] + t * direction[k]);
        let projected = camera_point(point, self);
        let perspective = if self.presentation.perspective {
            d / (d - projected[2])
        } else {
            1.
        };
        let scale = scale * perspective;
        if t.is_finite()
            && t >= 0.
            && point.iter().all(|v| v.is_finite())
            && scale.is_finite()
            && scale > 0.
        {
            PlaneView {
                centre: [point[0] as f64, point[1] as f64],
                scale: (scale as f64).clamp(0.001, 100_000.),
            }
        } else {
            fallback
        }
    }
}
