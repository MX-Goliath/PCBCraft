// Match egui's premultiplied gamma colors, atlas sampling and dithering.
struct Camera { size: vec2<f32>, shift: vec2<f32>, dither: f32, padding0: f32, padding1: vec2<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex fn vertex(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>,
    @location(2) packed: u32) -> Output {
    var out: Output;
    let p = (position + camera.shift) / camera.size;
    out.position = vec4<f32>(2. * p.x - 1., 1. - 2. * p.y, 0., 1.);
    out.uv = uv;
    out.color = vec4<f32>(f32(packed & 255u), f32((packed >> 8u) & 255u),
        f32((packed >> 16u) & 255u), f32((packed >> 24u) & 255u)) / 255.;
    return out;
}
fn color(in: Output) -> vec4<f32> {
    var c = in.color * textureSample(atlas, atlas_sampler, in.uv);
    if camera.dither > 0. {
        let f = 0.06711056 * in.position.x + 0.00583715 * in.position.y;
        let noise = (fract(52.9829189 * fract(f)) - 0.5) * 0.95;
        c = vec4<f32>(c.rgb + noise / 255., c.a);
    }
    return c;
}
@fragment fn gamma_fragment(in: Output) -> @location(0) vec4<f32> { return color(in); }
@fragment fn linear_fragment(in: Output) -> @location(0) vec4<f32> {
    let c = color(in);
    let lower = c.rgb / 12.92;
    let higher = pow((c.rgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return vec4<f32>(select(higher, lower, c.rgb < vec3<f32>(0.04045)), c.a);
}
