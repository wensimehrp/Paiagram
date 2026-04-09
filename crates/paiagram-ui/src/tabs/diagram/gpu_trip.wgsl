struct Uniforms {
    ticks_min: i32,
    y_min: f32,
    screen_size: vec2<f32>,
    x_per_unit: f32,
    y_per_unit: f32,
    screen_origin: vec2<f32>,
    repeat_interval_ticks: i32,
    repeat_from: i32,
    repeat_count: u32,
    source_instance_count: u32,
    style_count: u32,
    feathering_radius: f32,
    _uniform_pad0: vec2<u32>,
    styles: array<vec4<u32>, 256>,
};

/// field0: .......A AAAAAAAA AAAAAAAA AAAAAAAA
/// field1: ......ND DDDDDDDD DDDDDDDD DDDDDDDD
/// field2: SSSSSSSS SSSSSSSS RRRRRRRR RRRRRRRR
/// field3: ........ ........ ........ IIIIIIII
///
/// A: arrival seconds (signed). 2^25 ~= 388 days (194 days on each side)
/// D: departure seconds (signed). Same as arrival seconds.
/// S: station index
/// R: track index
/// I: style table index (8-bit, 0..=255).
///    style data (width + colour) is stored in uniform buffer.
/// N: whether the current entry connects to the next entry
///    when this bit is set it connects to the next entry.
struct Entry {
    field0: u32,
    field1: u32,
    field2: u32,
    field3: u32,
}

struct SegmentMeshVertex {
    along: f32,
    side: f32,
    outer: f32,
};

struct SegmentOut {
    p0: vec2<f32>,
    p1: vec2<f32>,
    half_width: f32,
    nx: f32,
    ny: f32,
    _pad0: f32,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> entries: array<Entry>;
@group(0) @binding(2) var<storage, read> stations: array<f32>;
@group(0) @binding(3) var<storage, read_write> segments: array<SegmentOut>;
@group(0) @binding(4) var<storage, read> render_segments: array<SegmentOut>;

const SEGMENT_MESH_VERTICES: array<SegmentMeshVertex, 8> = array<SegmentMeshVertex, 8>(
    SegmentMeshVertex(0.0, 1.0, 0.0),
    SegmentMeshVertex(0.0, -1.0, 0.0),
    SegmentMeshVertex(1.0, 1.0, 0.0),
    SegmentMeshVertex(1.0, -1.0, 0.0),
    SegmentMeshVertex(0.0, 1.0, 1.0),
    SegmentMeshVertex(1.0, 1.0, 1.0),
    SegmentMeshVertex(0.0, -1.0, 1.0),
    SegmentMeshVertex(1.0, -1.0, 1.0),
);

const SEGMENT_MESH_INDICES: array<u32, 18> = array<u32, 18>(
    0u, 1u, 2u, 1u, 3u, 2u,
    4u, 0u, 5u, 0u, 2u, 5u,
    1u, 6u, 3u, 6u, 7u, 3u,
);

const TICKS_PER_SECOND: i32 = 100;
const COMPUTE_WORKGROUP_SIZE: u32 = 64u;

fn signed_25_to_i32(bits: u32) -> i32 {
    // Sign bit is at bit 24, so shift left then arithmetic-shift right.
    return bitcast<i32>(bits << 7u) >> 7;
}

fn seconds_to_screen_x(secs: i32, repeat: i32) -> f32 {
    let repeat_offset_ticks = repeat * uniforms.repeat_interval_ticks;
    let ticks = secs * TICKS_PER_SECOND + repeat_offset_ticks;
    return f32(ticks - uniforms.ticks_min) / uniforms.x_per_unit;
}

fn height_to_screen_y(height: f32) -> f32 {
    return (height - uniforms.y_min) / uniforms.y_per_unit;
}

fn make_segment(entry: Entry, seg_a: vec2<f32>, seg_b: vec2<f32>) -> SegmentOut {
    let style_index = entry.field3 & 0xFFu;
    let style = uniforms.styles[style_index].x;
    let width_steps = (style >> 24u) & 0xFFu;
    let width_px = max(f32(width_steps) * 0.25, 1.0);

    let packed_color = style & 0x00ffffffu;
    let color = vec4<f32>(
        f32((packed_color >> 0u) & 0xFFu) / 255.0,
        f32((packed_color >> 8u) & 0xFFu) / 255.0,
        f32((packed_color >> 16u) & 0xFFu) / 255.0,
        1.0,
    );

    // Cursed linear algebra which I don't understand
    // I only know trigs!
    let dx = seg_b.x - seg_a.x;
    let dy = seg_b.y - seg_a.y;
    let inv_len = inverseSqrt(max(dx * dx + dy * dy, 1e-12));
    let nx = -dy * inv_len;
    let ny = dx * inv_len;

    return SegmentOut(
        seg_a,
        seg_b,
        width_px * 0.5,
        nx,
        ny,
        0.0,
        color,
    );
}

fn invalid_segment() -> SegmentOut {
    return SegmentOut(
        vec2<f32>(1.0e9, 1.0e9),
        vec2<f32>(1.0e9, 1.0e9),
        1.0,
        0.0,
        0.0,
        0.0,
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
    );
}

@compute @workgroup_size(COMPUTE_WORKGROUP_SIZE)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let entry_index = global_id.x;
    let source_count = uniforms.source_instance_count;
    let pair_index = entry_index * 2;
    if entry_index >= source_count {
        return;
    }

    let entry = entries[entry_index];
    let connects_to_next = ((entry.field1 >> 25u) & 1u) != 0u;
    let can_connect = connects_to_next && (entry_index + 1u < source_count);

    let arr_secs = signed_25_to_i32(entry.field0 & 0x01ffffffu);
    let dep_secs = signed_25_to_i32(entry.field1 & 0x01ffffffu);
    let station_index = (entry.field2 >> 16u) & 0xFFFFu;
    // currently no track index
    let y = height_to_screen_y(stations[station_index]);

    let seg_0 = vec2<f32>(seconds_to_screen_x(arr_secs, 0), y);
    let seg_1 = vec2<f32>(seconds_to_screen_x(dep_secs, 0), y);
    segments[pair_index] = make_segment(entry, seg_0, seg_1);

    if !can_connect {
        segments[pair_index + 1] = invalid_segment();
        return;
    }

    let next_entry = entries[entry_index + 1u];
    let next_station_index = (next_entry.field2 >> 16u) & 0xFFFFu;
    let next_arr_secs = signed_25_to_i32(next_entry.field0 & 0x01ffffffu);
    let next_y = height_to_screen_y(stations[next_station_index]);

    let seg_2 = vec2<f32>(seconds_to_screen_x(next_arr_secs, 0), next_y);
    segments[pair_index + 1] = make_segment(entry, seg_1, seg_2);
}

struct VertexOut {
    @location(0) color: vec4<f32>,
    @location(1) feather_alpha: f32,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOut {
    let source_count = uniforms.source_instance_count;
    let base_segment_count = source_count * 2u;
    let repeat_index = instance_index / base_segment_count;
    let segment_index = instance_index % base_segment_count;
    let repeat = uniforms.repeat_from + i32(repeat_index);
    let repeat_offset_x = f32(repeat * uniforms.repeat_interval_ticks) / uniforms.x_per_unit;

    let segment = render_segments[segment_index];
    let seg_a = segment.p0 + vec2<f32>(repeat_offset_x, 0.0);
    let seg_b = segment.p1 + vec2<f32>(repeat_offset_x, 0.0);

    let mesh_index = SEGMENT_MESH_INDICES[vertex_index];
    let mesh = SEGMENT_MESH_VERTICES[mesh_index];

    let half = max(segment.half_width - uniforms.feathering_radius * 0.5, 0.01);
    // Expand the segment on both sides so alpha can smoothly fade at the edges.
    let dist = half + mesh.outer * uniforms.feathering_radius;
    let normal_offset = vec2<f32>(segment.nx, segment.ny) * (mesh.side * dist);
    let base_pos = seg_a + (seg_b - seg_a) * mesh.along;
    let pos = base_pos + normal_offset;
    let feather_alpha = 1.0 - mesh.outer;

    let screen_pos = pos + uniforms.screen_origin;
    let x = screen_pos.x / uniforms.screen_size.x * 2.0 - 1.0;
    let y = 1.0 - screen_pos.y / uniforms.screen_size.y * 2.0;
    return VertexOut(segment.color, feather_alpha, vec4<f32>(x, y, 0.0, 1.0));
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // maybe use smoothstep in this case?
    // let feather = smoothstep(0.0, 1.0, input.feather_alpha);
    return vec4<f32>(input.color.rgb, input.feather_alpha);
}
