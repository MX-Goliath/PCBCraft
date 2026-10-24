struct Camera {
    centre: vec4<f32>, // xyz, board thickness
    rotation: vec4<f32>, // sin yaw, cos yaw, sin pitch, cos pitch
    viewport: vec4<f32>, // logical width/height, scale, perspective distance
    view: vec4<f32>, // pan xy, thickness scale, perspective enabled
    depth: vec4<f32>,
    background0: vec4<f32>,
    background1: vec4<f32>,
    reserved: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<storage, read> colors: array<u32>;
fn scale_z(z: f32) -> f32 { return z + clamp(z, -camera.centre.w, 0.) * (camera.view.z - 1.); }
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec3<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
};
@vertex fn vertex(@location(0) position: vec3<f32>, @location(1) style: u32) -> Vertex {
    let v = vec3<f32>(position.xy - camera.centre.xy, scale_z(position.z) - scale_z(camera.centre.z));
    let r = camera.rotation;
    let point = vec3<f32>(v.x*r.y - v.y*r.x,
        (v.x*r.x + v.y*r.y)*r.z + v.z*r.w,
        -(v.x*r.x + v.y*r.y)*r.w + v.z*r.z);
    var distance = 1.;
    var perspective = 1.;
    var depth = 0.5 + point.z / camera.depth.x;
    if camera.view.w > 0. {
        distance = max(camera.viewport.w - point.z, camera.viewport.w * 0.1);
        perspective = camera.viewport.w / distance;
        depth = camera.viewport.w * 0.1 / distance;
    }
    let p = point.xy * camera.viewport.z * perspective;
    let ndc = vec2<f32>(2.*(p.x + camera.view.x)/camera.viewport.x,
        2.*(p.y - camera.view.y)/camera.viewport.y);
    let packed = colors[style];
    var out: Vertex;
    out.position = vec4<f32>(ndc * distance, depth * distance, distance);
    out.point = point;
    out.color = vec4<f32>(f32(packed & 255u), f32((packed >> 8u) & 255u),
        f32((packed >> 16u) & 255u), f32((packed >> 24u) & 255u)) / 255.;
    return out;
}
@fragment fn fragment(in: Vertex) -> @location(0) vec4<f32> {
    let n = cross(dpdx(in.point), dpdy(in.point));
    let light = clamp(0.35 + 0.65 * abs(dot(n, vec3<f32>(0.3, -0.45, 0.84))) / max(length(n), 1e-8), 0.25, 1.);
    return vec4<f32>(round(in.color.rgb * light * 255.) / 255., in.color.a);
}
struct Quad { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn quad(@builtin(vertex_index) index: u32) -> Quad {
    var out: Quad;
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    out.position = vec4<f32>(uv.x * 2. - 1., 1. - uv.y * 2., 0., 1.);
    out.uv = uv;
    return out;
}
@fragment fn background(in: Quad) -> @location(0) vec4<f32> {
    return vec4<f32>(round(mix(camera.background0.rgb, camera.background1.rgb, in.uv.y) * 255.) / 255., 1.);
}
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;
@fragment fn gamma_image(in: Quad) -> @location(0) vec4<f32> { return textureSample(image, image_sampler, in.uv); }
@fragment fn linear_image(in: Quad) -> @location(0) vec4<f32> {
    let c = textureSample(image, image_sampler, in.uv);
    return vec4<f32>(select(pow((c.rgb+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)), c.rgb/12.92, c.rgb<vec3<f32>(0.04045)),c.a);
}
