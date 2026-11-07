// SPDX-License-Identifier: MPL-2.0
import package::map::graph_common::{
    SQUARE_CORNERS,
    SQUARE_MESH_LENGTH,
    screen_pos_to_clip,
    uniforms
};

struct NodePoint {
    screen_pos: vec2<f32>,
    fill: u32,
}

/// Nodes of the graph.
@group(0) @binding(1)
var<storage, read> node_points: array<NodePoint>;

/// The vertex shader output of a node square.
struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
}

/// Half the side length of a node square, in egui points.
const NODE_HALF_SIZE: f32 = 4.0;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let node = node_points[instance_index];
    let corner = SQUARE_CORNERS[vertex_index];
    let screen_pos = node.screen_pos + corner * NODE_HALF_SIZE;
    let clip = screen_pos_to_clip(screen_pos);

    var color = unpack4x8unorm(node.fill);
    // if node.is_platform != 0u {
    //     color = PLATFORM_COLOR;
    // }
    return VertexOut(color, vec4<f32>(clip, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
