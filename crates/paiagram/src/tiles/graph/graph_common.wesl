// SPDX-License-Identifier: MPL-2.0
///
/// Types and helpers shared by the graph shaders.
///
/// This module deliberately has no entry points, so both `graph_intervals` and
/// `graph_nodes` can import from it without pulling in each other's `vs_main`/`fs_main`.

// numbers on the corner are index to SQUARE_CORNERS
//
// y-axis 0 ----- 1
//        |  0.0  |
//        2 ----- 3
//                x axis
const SQUARE_CORNERS: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );

const SQUARE_MESH_LENGTH: u32 = 4;

struct Uniforms {
    /// Size of the callback's viewport in egui points.
    screen_size: vec2<f32>,
    /// Device pixels per egui point, used to size the anti-aliasing feather band.
    pixels_per_point: f32,
}

/// The uniform buffer shared by the graph passes.
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

fn screen_pos_to_clip(screen_pos: vec2<f32>) -> vec2<f32> {
    let clip_x = (screen_pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (screen_pos.y / uniforms.screen_size.y) * 2.0;
    return vec2<f32>(clip_x, clip_y);
}
