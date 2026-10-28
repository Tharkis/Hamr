// Copies the offscreen 3D image into the egui viewport rectangle.

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Out {
    // One triangle that covers the whole viewport.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Out;
    out.clip = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

// The offscreen image is sRGB, so sampling returns linear colour.
@fragment
fn fs_linear(in: Out) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, in.uv);
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

// Used when the window surface is not sRGB, so we encode the colour ourselves.
@fragment
fn fs_encode_srgb(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(image, image_sampler, in.uv);
    return vec4<f32>(to_srgb(c.rgb), c.a);
}
