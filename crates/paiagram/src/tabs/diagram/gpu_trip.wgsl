// SPDX-License-Identifier: MPL-2.0

struct Uniforms {
    screen_size: vec2<f32>,
    ticks_min: i32,
    canvas_length_min: f32,
    x_per_tick: f32,
    entry_segment_count: u32,
    y_per_canvas_length: f32,
    pixels_per_point: f32,
};

struct Tick {
    value: i32,
}

struct CanvasLength {
    value: f32,
}

struct EntrySegment {
    curr_time_seconds: i32,
    curr_index: u32,
    curr_progress: f32,
    next_time_seconds: i32,
    next_index: u32,
    next_progress: f32,
    style: Style,
}

struct Style {
    thickness: f32,
    fill_rgba: u32,
}

/// Uniform buffers for general info
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
/// Storage for all entries
@group(0) @binding(1) var<storage, read> entry_segments: array<EntrySegment>;
/// Storage for stations
@group(0) @binding(2) var<storage, read> stations: array<CanvasLength>;

struct SegmentMeshVertex {
    /// Position along line length
    along: f32,
    /// Offset along line normal.
    /// -1.0 = Left edge, +1.0 = Right edge
    side: f32,
};

const SEGMENT_MESH_VERTICES: array<SegmentMeshVertex, 4> = array<SegmentMeshVertex, 4>(
    SegmentMeshVertex(0.0, 1.0),  // 0, Start-Left
    SegmentMeshVertex(0.0, -1.0), // 1, Start-Right
    SegmentMeshVertex(1.0, 1.0),  // 2, End-Left
    SegmentMeshVertex(1.0, -1.0), // 3, End-Right
);

const SEGMENT_MESH_LENGTH: u32 = 6;

const SEGMENT_MESH_INDICES: array<u32, 6> = array<u32, 6>(
    0u, 1u, 2u,
    1u, 3u, 2u,
);

const FEATHER_WIDTH_PX: f32 = 2.0;

const TICKS_PER_SECOND: i32 = 100;

fn seconds_to_screen_x(secs: i32) -> f32 {
    // let repeat_offset_ticks = repeat * uniforms.repeat_interval_ticks;
    let ticks = secs * TICKS_PER_SECOND;
    return f32(ticks - uniforms.ticks_min) / uniforms.x_per_tick;
}

fn height_to_screen_y(height: CanvasLength) -> f32 {
    return (height.value - uniforms.canvas_length_min) / uniforms.y_per_canvas_length;
}

struct VertexOut {
    @location(0) color: vec4<f32>,
    /// Signed distance in screen points from the centre line
    @location(1) offset: f32,
    /// Half the line thickness in screen points
    @location(2) half_width: f32,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOut {
    let entry = entry_segments[instance_index];

    let y0_source = height_to_screen_y(stations[entry.curr_index]);
    let y0_target = height_to_screen_y(stations[entry.curr_index + 1]);
    let y0 = mix(y0_source, y0_target, entry.curr_progress);
    let y1_source = height_to_screen_y(stations[entry.next_index]);
    let y1_target = height_to_screen_y(stations[entry.next_index]);
    let y1 = mix(y1_source, y1_target, entry.next_progress);
    let x0 = seconds_to_screen_x(entry.curr_time_seconds);
    let x1 = seconds_to_screen_x(entry.next_time_seconds);

    let p0 = vec2<f32>(x0, y0);
    let p1 = vec2<f32>(x1, y1);

    // Compute normal
    let dir = p1 - p0;
    let inv_len = inverseSqrt(max(dot(dir, dir), 1e-12));
    let normal = vec2<f32>(-dir.y * inv_len, dir.x * inv_len);

    // Expand mesh quad vertex
    let mesh_index = SEGMENT_MESH_INDICES[vertex_index];
    let mesh = SEGMENT_MESH_VERTICES[mesh_index];

    // `thickness` is in egui points; the feather band is a fixed number of hardware pixels, so
    // convert it to points using the device's pixels-per-point.
    let feather = FEATHER_WIDTH_PX / max(uniforms.pixels_per_point, 1e-6);
    let half_width = entry.style.thickness * 0.5;
    // Expand the quad by half the feather so the 50%-coverage contour sits exactly on the
    // nominal edge, i.e. the line keeps its requested width.
    let offset = mesh.side * (half_width + feather * 0.5);
    let base_pos = mix(p0, p1, mesh.along);
    let world_pos = base_pos + normal * offset;

    let clip_x = (world_pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (world_pos.y / uniforms.screen_size.y) * 2.0;

    let rgba = entry.style.fill_rgba;
    let color = vec4<f32>(
        f32((rgba >> 24u) & 0xFFu) / 255.0,
        f32((rgba >> 16u) & 0xFFu) / 255.0,
        f32((rgba >> 8u) & 0xFFu) / 255.0,
        f32(rgba & 0xFFu) / 255.0
    );

    return VertexOut(color, offset, half_width, vec4<f32>(clip_x, clip_y, 0.0, 1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let feather = FEATHER_WIDTH_PX / max(uniforms.pixels_per_point, 1e-6);
    let alpha = 1.0 - smoothstep(
        in.half_width - feather * 0.5,
        in.half_width + feather * 0.5,
        abs(in.offset),
    );
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
