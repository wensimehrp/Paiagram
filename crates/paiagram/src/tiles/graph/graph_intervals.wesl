// SPDX-License-Identifier: MPL-2.0

import package::shaders::draw_lines::{VertexOut, line_alpha, line_feather, segment_quad_pos};
import package::graph::graph_common::{
    SQUARE_CORNERS,
    SQUARE_MESH_LENGTH,
    screen_pos_to_clip,
    uniforms
};

struct IntervalPoint {
    /// Position relative to the callback's viewport, in egui points.
    screen_pos: vec2<f32>,
    connects_to_next: u32,
}

/// Storage for all points
@group(0) @binding(1)
var<storage, read> interval_points: array<IntervalPoint>;

// TODO: take this from the interval
const LINE_THICKNESS: f32 = 2.0;
const LINE_COLOR: vec4<f32> = vec4<f32>(0.23, 0.55, 0.95, 1.0);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let point = interval_points[instance_index];
    let half_width = LINE_THICKNESS * 0.5;
    // `LINE_THICKNESS` is in egui points; the feather band is a fixed number of hardware pixels,
    // so convert it to points using the device's pixels-per-point.
    let feather = line_feather(uniforms.pixels_per_point);
    let half_extent = half_width + feather * 0.5;

    let start = point.screen_pos;

    let corner = SQUARE_CORNERS[vertex_index];
    let offset = corner.y * half_extent;
    // A point only draws its outgoing segment when it connects to the next one; otherwise every
    // vertex collapses onto `start`, leaving a zero-area (invisible) quad.
    var screen_pos = start;
    if point.connects_to_next != 0u {
        let end = interval_points[instance_index + 1u].screen_pos;
        screen_pos = segment_quad_pos(start, end, corner, half_extent);
    }

    let clip = screen_pos_to_clip(screen_pos);
    return VertexOut(LINE_COLOR, offset, half_width, vec4<f32>(clip, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let feather = line_feather(uniforms.pixels_per_point);
    let alpha = line_alpha(in.offset, in.half_width, feather);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
