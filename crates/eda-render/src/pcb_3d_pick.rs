//! Revision-bound triangle BVH; pointer picking never rasterizes an image.
use super::*;

#[derive(Clone, Copy)]
struct Bounds3 {
    min: [f32; 3],
    max: [f32; 3],
}
impl Bounds3 {
    fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }
    fn include(&mut self, p: [f32; 3]) {
        for (k, v) in p.into_iter().enumerate() {
            self.min[k] = self.min[k].min(v);
            self.max[k] = self.max[k].max(v);
        }
    }
    fn intersects(
        self,
        origin: [f32; 3],
        direction: [f32; 3],
        thickness: f32,
        factor: f32,
        best: f32,
    ) -> bool {
        let mut min = self.min;
        let mut max = self.max;
        min[2] = scaled_z(min[2], thickness, factor);
        max[2] = scaled_z(max[2], thickness, factor);
        let mut near: f32 = 0.;
        let mut far = best;
        for k in 0..3 {
            if direction[k].abs() < 1e-12 {
                if origin[k] < min[k] || origin[k] > max[k] {
                    return false;
                }
            } else {
                let a = (min[k] - origin[k]) / direction[k];
                let b = (max[k] - origin[k]) / direction[k];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if far < near {
                    return false;
                }
            }
        }
        true
    }
}
struct Node {
    bounds: Bounds3,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}
#[derive(Default)]
pub(super) struct Index {
    indices: Vec<usize>,
    nodes: Vec<Node>,
}
pub(super) fn scaled_z(z: f32, thickness: f32, factor: f32) -> f32 {
    z + z.clamp(-thickness, 0.) * (factor - 1.)
}
impl Index {
    pub fn new(faces: &[Face]) -> Self {
        let mut result = Self {
            indices: (0..faces.len()).collect(),
            nodes: vec![],
        };
        if !faces.is_empty() {
            // Triangle bounds and sort keys are invariant throughout the BVH
            // build. Reading all nine vertex coordinates at every tree level
            // and partition comparison dominates large-scene preparation.
            let geometry: Vec<_> = faces
                .iter()
                .map(|face| {
                    let mut bounds = Bounds3::empty();
                    for p in face.triangle.v {
                        bounds.include(p);
                    }
                    let centre: [f32; 3] =
                        std::array::from_fn(|axis| face.triangle.v.iter().map(|p| p[axis]).sum());
                    (bounds, centre)
                })
                .collect();
            result.build(&geometry, 0..faces.len());
        }
        result
    }
    fn build(&mut self, geometry: &[(Bounds3, [f32; 3])], range: std::ops::Range<usize>) -> usize {
        let mut bounds = Bounds3::empty();
        for &index in &self.indices[range.clone()] {
            let b = geometry[index].0;
            bounds.include(b.min);
            bounds.include(b.max);
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 12 {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.max[a] - bounds.min[a]).total_cmp(&(bounds.max[b] - bounds.min[b]))
                })
                .unwrap();
            let middle = range.len() / 2;
            self.indices[range.clone()].select_nth_unstable_by(middle, |&a, &b| {
                geometry[a].1[axis].total_cmp(&geometry[b].1[axis])
            });
            let left = self.build(geometry, range.start..range.start + middle);
            let right = self.build(geometry, range.start + middle..range.end);
            self.nodes[index].children = Some((left, right));
        }
        index
    }
    pub fn hit(&self, state: &State, point: Vec2, logical: Vec2) -> Option<usize> {
        let root = self.nodes.first()?;
        let factor = state.presentation.thickness_scale;
        let thickness = state.scene.thickness;
        let scale = logical.min_elem() * 0.43 / state.scene.radius.max(0.1) * state.zoom;
        let x = (point.x - logical.x / 2. - state.pan.x) / scale;
        let y = -(point.y - logical.y / 2. - state.pan.y) / scale;
        let centre = state.focus.unwrap_or(state.scene.centre);
        let (sy, cy) = state.yaw.sin_cos();
        let (sp, cp) = state.pitch.sin_cos();
        let inverse = |p: [f32; 3]| {
            [
                cy * p[0] + sy * (sp * p[1] - cp * p[2]),
                -sy * p[0] + cy * (sp * p[1] - cp * p[2]),
                cp * p[1] + sp * p[2],
            ]
        };
        let (camera_origin, camera_direction) = if state.presentation.perspective {
            let d = state.scene.radius.max(0.1) * 4.;
            ([0., 0., d], [x / d, y / d, -1.])
        } else {
            let mut far = f32::NEG_INFINITY;
            for i in 0..8 {
                let p = std::array::from_fn(|k| {
                    if i & (1 << k) == 0 {
                        root.bounds.min[k]
                    } else {
                        root.bounds.max[k]
                    }
                });
                far = far.max(camera_point(p, state)[2]);
            }
            ([x, y, far + 1.], [0., 0., -1.])
        };
        let mut origin = inverse(camera_origin);
        let direction = inverse(camera_direction);
        origin[0] += centre[0];
        origin[1] += centre[1];
        origin[2] += scaled_z(centre[2], thickness, factor);
        let mut stack = vec![0];
        let mut best = f32::INFINITY;
        let mut hit = None;
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            if !node
                .bounds
                .intersects(origin, direction, thickness, factor, best)
            {
                continue;
            }
            if let Some((left, right)) = node.children {
                stack.extend([left, right]);
                continue;
            }
            for &index in &self.indices[node.range.clone()] {
                let face = &state.scene.faces[index];
                let vertices = face.triangle.v.map(|mut p| {
                    p[2] = scaled_z(p[2], thickness, factor);
                    p
                });
                if let Some(t) = intersect(origin, direction, vertices)
                    && (t < best || (t == best && hit.is_some_and(|old| index < old)))
                    && material_color(face, state).a() > 0
                {
                    best = t;
                    hit = Some(index);
                }
            }
        }
        hit
    }
}

fn intersect(origin: [f32; 3], direction: [f32; 3], v: [[f32; 3]; 3]) -> Option<f32> {
    let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn::<_, 3, _>(|k| a[k] - b[k]);
    let dot = |a: [f32; 3], b: [f32; 3]| a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let e1 = sub(v[1], v[0]);
    let e2 = sub(v[2], v[0]);
    let p = cross(direction, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-10 {
        return None;
    }
    let inv = 1. / det;
    let s = sub(origin, v[0]);
    let u = inv * dot(s, p);
    if !(0. ..=1.).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let w = inv * dot(direction, q);
    if w < 0. || u + w > 1. {
        return None;
    }
    let t = inv * dot(e2, q);
    (t >= 0.).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bvh_matches_raster_depth_visibility_perspective_flip_and_thickness() {
        let face = |z, owner, material| Face {
            triangle: Triangle {
                v: [[-1., -1., z], [1., -1., z], [0., 1., z]],
                color: Color32::GRAY,
            },
            owner,
            material,
            layer: TOP,
        };
        let front = Uuid::new_v4();
        let back = Uuid::new_v4();
        let mut scene = Scene {
            faces: vec![face(-1., Some(back), 8), face(0., Some(front), 0)],
            radius: 2.,
            thickness: 1.,
            ..Default::default()
        };
        // Include enough distant triangles to exercise internal BVH nodes.
        for n in 0..100 {
            let mut f = face(0., None, 8);
            for p in &mut f.triangle.v {
                p[0] += 10. + n as f32;
            }
            scene.faces.push(f);
        }
        scene.picking = Index::new(&scene.faces);
        let mut state = State {
            scene: Arc::new(scene),
            ..Default::default()
        };
        for perspective in [false, true] {
            state.presentation.perspective = perspective;
            for bottom in [false, true] {
                if bottom {
                    state.bottom();
                } else {
                    state.top();
                }
                state.yaw += 0.3;
                state.pitch *= 0.8;
                state.pan = Vec2::new(1.5, -2.5);
                state.zoom = 1.1;
                for factor in [1., 4.] {
                    state.presentation.thickness_scale = factor;
                    for opacity in [0, 128, 255] {
                        state.presentation.opacity[0] = opacity;
                        let (_, ids) = raster(&state, &[], Vec2::splat(64.), [64, 64]);
                        for y in (22..42).step_by(3) {
                            for x in (26..38).step_by(3) {
                                let expected = ids[y * 64 + x].checked_sub(1).map(|n| n as usize);
                                let actual = state.scene.picking.hit(
                                    &state,
                                    Vec2::new(x as f32 + 0.5, y as f32 + 0.5),
                                    Vec2::splat(64.),
                                );
                                assert_eq!(
                                    actual, expected,
                                    "perspective={perspective} bottom={bottom} scale={factor} opacity={opacity} x={x} y={y}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
