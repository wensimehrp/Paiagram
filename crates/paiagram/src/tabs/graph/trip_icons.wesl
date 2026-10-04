// SPDX-License-Identifier: MPL-2.0
///
/// Renders a sprite (currently the vehicle icon) for each active trip leg.

import package::graph::graph_common::{
    SQUARE_CORNERS,
    SQUARE_MESH_LENGTH,
    screen_pos_to_clip,
    uniforms
};

const ICON_HALF_SIZE: f32 = 18.0;

// TODO: this should actaully be VehicleIcon but let's just take this for now...
struct TripIcon {
    texture_idx: u32,
    screen_pos: vec2<f32>,
}

@group(0) @binding(1)
var<storage, read> trip_icons: array<TripIcon>;
@group(0) @binding(2)
var texture_sampler: sampler;
@group(0) @binding(3)
var trip_icon_texture: texture_2d<f32>;

struct VertexOut {
    @location(0) texture_pos: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let icon = trip_icons[instance_index];
    // The four corners are drawn as a triangle strip.
    let corner = SQUARE_CORNERS[vertex_index];
    let screen_pos = icon.screen_pos + corner * ICON_HALF_SIZE;
    let clip = screen_pos_to_clip(screen_pos);

    // `SQUARE_CORNERS` is in `[-1, 1]`; both screen y and texture v grow downwards, so
    // remapping is a plain affine map with no vertical flip.
    let texture_pos = (corner + vec2<f32>(1.0, 1.0)) * 0.5;
    return VertexOut(texture_pos, vec4<f32>(clip, 0.0, 1.0));
}

fn sample_stable(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>) -> vec4<f32> {
    // How much UV space this screen pixel covers.
    let dx = dpdx(uv);
    let dy = dpdy(uv);

    // 2x2 rotated-grid-ish sampling within the pixel footprint.
    let a = textureSample(tex, samp, uv - dx * 0.25 - dy * 0.25);
    let b = textureSample(tex, samp, uv + dx * 0.25 - dy * 0.25);
    let c = textureSample(tex, samp, uv - dx * 0.25 + dy * 0.25);
    let d = textureSample(tex, samp, uv + dx * 0.25 + dy * 0.25);

    return (a + b + c + d) * 0.25;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return sample_stable(trip_icon_texture, texture_sampler, in.texture_pos);
}
