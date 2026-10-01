// SPDX-License-Identifier: MPL-2.0

import package::shaders::draw_lines::{
    SEGMENT_MESH_INDICES,
    SEGMENT_MESH_VERTICES,
    VertexOut,
    joint_vertex,
    line_alpha,
    line_feather,
    normalize_or
};

struct Uniforms {
    screen_size: vec2<f32>,
    ticks_min: i32,
    canvas_length_min: f32,
    x_per_tick: f32,
    y_per_canvas_length: f32,
    pixels_per_point: f32,
}

struct Tick {
    value: i32,
}

struct CanvasLength {
    value: f32,
}

struct EntrySegment {
    arr_seconds: i32,
    dep_seconds: i32,
    curr_index: u32,
    curr_progress: f32,
    connects_to_prev: u32, // TODO: make this more compact
    connects_to_next: u32,
    style: Style,
}

struct Style {
    thickness: f32,
    fill_rgba: u32,
}

/// Uniform buffers for general info
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
/// Storage for all entries
@group(0) @binding(1)
var<storage, read> entry_segments: array<EntrySegment>;
/// Storage for stations
@group(0) @binding(2)
var<storage, read> stations: array<CanvasLength>;

const TICKS_PER_SECOND: i32 = 100;

fn seconds_to_screen_x(secs: i32) -> f32 {
    // let repeat_offset_ticks = repeat * uniforms.repeat_interval_ticks;
    let ticks = secs * TICKS_PER_SECOND;
    return f32(ticks - uniforms.ticks_min) / uniforms.x_per_tick;
}

fn height_to_screen_y(height: CanvasLength) -> f32 {
    return (height.value - uniforms.canvas_length_min) / uniforms.y_per_canvas_length;
}

/// Screen height of the entry's node on the centre line.
fn entry_height(entry: EntrySegment) -> f32 {
    let source_y = height_to_screen_y(stations[entry.curr_index]);
    let target_y = height_to_screen_y(stations[entry.curr_index + 1u]);
    return mix(source_y, target_y, entry.curr_progress);
}

fn entry_arrival(entry: EntrySegment) -> vec2<f32> {
    return vec2<f32>(seconds_to_screen_x(entry.arr_seconds), entry_height(entry));
}

fn entry_departure(entry: EntrySegment) -> vec2<f32> {
    return vec2<f32>(seconds_to_screen_x(entry.dep_seconds), entry_height(entry));
}

/// Direction in which the trip leaves an entry's arrival point. For a normal entry that is the
/// horizontal dwell; for a pass-through entry (`arrival == departure`) it is the outgoing
/// connection instead.
fn outgoing_from_arrival(index: u32) -> vec2<f32> {
    let entry = entry_segments[index];
    let arrival = entry_arrival(entry);
    let departure = entry_departure(entry);
    if entry.arr_seconds != entry.dep_seconds {
        return normalize_or(departure - arrival, vec2<f32>(1.0, 0.0));
    }
    if entry.connects_to_next != 0u {
        let next = entry_segments[index + 1u];
        return normalize_or(entry_arrival(next) - departure, vec2<f32>(1.0, 0.0));
    }
    return vec2<f32>(1.0, 0.0);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let entry = entry_segments[instance_index];
    let half_width = entry.style.thickness * 0.5;
    // `thickness` is in egui points; the feather band is a fixed number of hardware pixels, so
    // convert it to points using the device's pixels-per-point.
    let feather = line_feather(uniforms.pixels_per_point);

    let mesh = SEGMENT_MESH_VERTICES[SEGMENT_MESH_INDICES[vertex_index]];
    let is_end = mesh.along > 0.5;
    // Vertex indices `6..12` form the quad connecting this entry to the next one.
    let is_connect = vertex_index >= 6u;

    let p_arr = entry_arrival(entry);
    let p_dep = entry_departure(entry);
    // A pass-through entry has no horizontal part (`arrival == departure`).
    let is_dwell = entry.arr_seconds != entry.dep_seconds;
    let dir_dwell = normalize_or(p_dep - p_arr, vec2<f32>(1.0, 0.0));

    // Direction of the connection coming from the previous entry into this one.
    var dir_incoming = dir_dwell;
    if entry.connects_to_prev != 0u && instance_index > 0u {
        let prev = entry_segments[instance_index - 1u];
        dir_incoming = normalize_or(p_arr - entry_departure(prev), dir_dwell);
    }

    // Where this entry connects to, and along which direction.
    var p_connect = p_dep;
    var dir_connect = dir_dwell;
    var dir_outgoing = dir_dwell;
    if entry.connects_to_next != 0u {
        let next = entry_segments[instance_index + 1u];
        p_connect = entry_arrival(next);
        dir_connect = normalize_or(p_connect - p_dep, dir_dwell);
        // Use the *next* entry's leaving direction, which accounts for it being a pass-through.
        dir_outgoing = outgoing_from_arrival(instance_index + 1u);
    }

    // The joint this vertex belongs to, with its incoming and outgoing directions.
    var joint_p = p_arr;
    var dir_in = dir_incoming;
    var dir_out = dir_dwell;
    if is_connect {
        if is_end {
            joint_p = p_connect;
            dir_in = dir_connect;
            dir_out = dir_outgoing;
        } else {
            joint_p = p_dep;
            dir_out = dir_connect;
            // A pass-through entry connects directly to the incoming connection, or is a flat cap
            // when there is no incoming one.
            dir_in = dir_dwell;
            if !is_dwell {
                dir_in = dir_incoming;
                if entry.connects_to_prev == 0u {
                    dir_in = dir_connect;
                }
            }
        }
    } else if is_end {
        joint_p = p_dep;
        dir_in = dir_dwell;
        dir_out = dir_connect;
    }

    // Expand the mesh quad. The quad is offset from the centre line by `half_width + feather/2`
    // so the 50%-coverage contour sits exactly on the nominal edge, and its corners are mitered
    // against the neighbouring segments. Crucially the miter uses the *expanded* width too, so
    // the two quads that share a joint produce the exact same vertex and tile without gaps. The
    // `offset` passed to the fragment shader is the perpendicular distance at that vertex, which
    // is still `half_width + feather/2` because the vertex lies on the segment's offset line.
    let offset = mesh.side * (half_width + feather * 0.5);
    var world_pos = joint_vertex(joint_p, dir_in, dir_out, mesh.side, half_width + feather * 0.5);
    // Skip the horizontal quad entirely when there is no horizontal part to draw.
    if !is_connect && !is_dwell {
        world_pos = p_arr;
    }

    let clip_x = (world_pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (world_pos.y / uniforms.screen_size.y) * 2.0;

    let rgba = entry.style.fill_rgba;
    let color = vec4<f32>(
            f32((rgba >> 24u) & 0xFFu) / 255.0,
            f32((rgba >> 16u) & 0xFFu) / 255.0,
            f32((rgba >> 8u) & 0xFFu) / 255.0,
            f32(rgba & 0xFFu) / 255.0,
        );

    return VertexOut(color, offset, half_width, vec4<f32>(clip_x, clip_y, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let feather = line_feather(uniforms.pixels_per_point);
    let alpha = line_alpha(in.offset, in.half_width, feather);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
