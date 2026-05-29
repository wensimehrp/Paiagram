struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;

    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let pos = corners[vi];
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    // Vulkan/wgpu NDC: Y is up for pos, but Y is down for texture coordinates.
    // pos.y = 1.0 (top) -> tex_coords.y = 0.0
    // pos.y = -1.0 (bottom) -> tex_coords.y = 1.0
    out.tex_coords = vec2<f32>(pos.x * 0.5 + 0.5, -pos.y * 0.5 + 0.5);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
