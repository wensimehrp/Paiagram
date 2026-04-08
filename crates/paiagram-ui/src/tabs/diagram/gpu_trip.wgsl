struct Uniforms {
    ticks_min: i32,
    y_min: f32,
    screen_size: vec2<f32>,
    x_per_unit: f32,
    y_per_unit: f32,
    screen_origin: vec2<f32>,
    repeat_interval_ticks: i32,
    repeat_from: i32,
    repeat_to: i32,
    source_instance_count: u32,
    style_count: u32,
    feathering_radius: f32,
    _uniform_pad0: vec2<u32>,
    styles: array<vec4<u32>, 256>,
};

/// field0: 0000000A AAAAAAAA AAAAAAAA AAAAAAAA
/// field1: 000000ND DDDDDDDD DDDDDDDD DDDDDDDD
/// field2: SSSSSSSS SSSSSSSS RRRRRRRR RRRRRRRR
/// field3: 00000000 00000000 IIIIIIII IIIIIIII
///
/// A: arrival seconds (signed). 2^25 ~= 388 days (194 days on each side)
/// D: departure seconds (signed). Same as arrival seconds.
/// S: station index
/// R: track index
/// I: style table index.
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

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> entries: array<Entry>;
@group(0) @binding(2) var<storage, read> stations: array<f32>;

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

struct VertexOut {
    @location(0) color: vec4<f32>,
    @location(1) feather_alpha: f32,
    @builtin(position) position: vec4<f32>,
};

fn hidden_out() -> VertexOut {
    return VertexOut(vec4<f32>(0.0), 0.0, vec4<f32>(2.0, 2.0, 0.0, 1.0));
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOut {
    let source_count = uniforms.source_instance_count;
    if source_count == 0u {
        return hidden_out();
    }

    let repeat_count_i32 = max(uniforms.repeat_to - uniforms.repeat_from + 1, 1);
    let repeat_count = u32(repeat_count_i32);
    let total_instance_count = source_count * 2u * repeat_count;
    if instance_index >= total_instance_count {
        return hidden_out();
    }

    let pair_instance_index = instance_index / 2u;
    let draw_kind = instance_index & 1u;
    let entry_index = pair_instance_index % source_count;
    let repeat_index = i32(pair_instance_index / source_count);
    let repeat = uniforms.repeat_from + repeat_index;

    let entry = entries[entry_index];
    let station_index = (entry.field2 >> 16u) & 0xFFFFu;
    if station_index >= arrayLength(&stations) {
        return hidden_out();
    }

    let arr_secs = signed_25_to_i32(entry.field0 & 0x01ffffffu);
    let dep_secs = signed_25_to_i32(entry.field1 & 0x01ffffffu);
    let connects_to_next = ((entry.field1 >> 25u) & 1u) != 0u;

    let track_index = entry.field2 & 0xFFFFu;
    let station_y = height_to_screen_y(stations[station_index] + f32(track_index));

    let style_index = entry.field3 & 0xFFFFu;
    if style_index >= uniforms.style_count {
        return hidden_out();
    }
    let style = uniforms.styles[style_index].x;
    let width_steps = (style >> 24u) & 0xFFu;
    let width_px = max(f32(width_steps) * 0.25, 1.0);
    let half_width = width_px * 0.5;

    let packed_color = style & 0x00ffffffu;
    let color = vec3<f32>(
        f32((packed_color >> 0u) & 0xFFu) / 255.0,
        f32((packed_color >> 8u) & 0xFFu) / 255.0,
        f32((packed_color >> 16u) & 0xFFu) / 255.0,
    );

    var seg_a = vec2<f32>(0.0, 0.0);
    var seg_b = vec2<f32>(0.0, 0.0);
    if draw_kind == 0u {
        if !connects_to_next || entry_index + 1u >= source_count {
            return hidden_out();
        }

        let next_entry = entries[entry_index + 1u];
        let next_station_index = (next_entry.field2 >> 16u) & 0xFFFFu;
        if next_station_index >= arrayLength(&stations) {
            return hidden_out();
        }

        let next_arr_secs = signed_25_to_i32(next_entry.field0 & 0x01ffffffu);
        let next_track_index = next_entry.field2 & 0xFFFFu;
        let next_y = height_to_screen_y(stations[next_station_index] + f32(next_track_index));
        seg_a = vec2<f32>(seconds_to_screen_x(dep_secs, repeat), station_y);
        seg_b = vec2<f32>(seconds_to_screen_x(next_arr_secs, repeat), next_y);
    } else {
        seg_a = vec2<f32>(seconds_to_screen_x(arr_secs, repeat), station_y);
        seg_b = vec2<f32>(seconds_to_screen_x(dep_secs, repeat), station_y);
    }

    let mesh_index = SEGMENT_MESH_INDICES[vertex_index];
    let mesh = SEGMENT_MESH_VERTICES[mesh_index];

    // Cursed linear algebra magic which I don't understand
    // I only know trigs!
    let sdx = seg_b.x - seg_a.x;
    let sdy = seg_b.y - seg_a.y;
    let len = max(sqrt(sdx * sdx + sdy * sdy), 1e-6);
    let nx = -sdy / len;
    let ny = sdx / len;

    let half = max(half_width - uniforms.feathering_radius * 0.5, 0.01);
    // Expand the segment on both sides so alpha can smoothly fade at the edges.
    let dist = half + mesh.outer * uniforms.feathering_radius;
    let normal_offset = vec2<f32>(nx, ny) * (mesh.side * dist);
    let base_pos = seg_a + (seg_b - seg_a) * mesh.along;
    let pos = base_pos + normal_offset;
    let feather_alpha = 1.0 - mesh.outer;

    let screen_pos = pos + uniforms.screen_origin;
    let x = screen_pos.x / uniforms.screen_size.x * 2.0 - 1.0;
    let y = 1.0 - screen_pos.y / uniforms.screen_size.y * 2.0;
    return VertexOut(vec4<f32>(color, 1.0), feather_alpha, vec4<f32>(x, y, 0.0, 1.0));
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let feather = smoothstep(0.0, 1.0, input.feather_alpha);
    return vec4<f32>(input.color.rgb, input.color.a * feather);
}
