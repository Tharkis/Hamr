struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: direction the light travels towards, in world space.
    light_dir: vec4<f32>,
    // xyz: centre of the grid fade, w: radius where lines become invisible.
    grid_fade: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_line(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> LineOut {
    var out: LineOut;
    out.clip = globals.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    out.world = position;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    let radius = globals.grid_fade.w;
    let d = distance(in.world.xy, globals.grid_fade.xy);
    let fade = 1.0 - smoothstep(radius * 0.4, radius, d);
    return vec4<f32>(in.color.rgb, in.color.a * fade);
}

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_mesh(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
) -> MeshOut {
    var out: MeshOut;
    out.clip = globals.view_proj * vec4<f32>(position, 1.0);
    out.normal = normal;
    out.color = color;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let diffuse = max(dot(n, -globals.light_dir.xyz), 0.0);
    // A little light from below keeps undersides readable.
    let fill = max(dot(n, globals.light_dir.xyz), 0.0) * 0.15;
    let shade = 0.3 + 0.7 * diffuse + fill;
    return vec4<f32>(in.color.rgb * shade, in.color.a);
}
