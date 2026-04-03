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
    _pad0: i32,
};

struct Entry {
    arr_secs: i32,
    dep_secs: i32,
    station_index: i32,
    _track_index: i32,
};

struct Trip {
    color: vec4<f32>,
    width: f32,
    len: u32,
    start_idx: u32,
    _pad1: u32,
};

struct InstanceMapEntry {
    trip_index: u32,
    local_segment: u32,
};

struct VisibleSegment {
    p0: vec2<f32>,
    p1: vec2<f32>,
    half_width: f32,
    _pad0: f32,
    color: vec4<f32>,
};

struct VertexIn {
    @location(0) p0: vec2<f32>,
    @location(1) p1: vec2<f32>,
    @location(2) half_width: f32,
    @location(3) color: vec4<f32>,
};

struct InstanceCounter {
    count: atomic<u32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> entries: array<Entry>;
@group(0) @binding(2) var<storage, read> trips: array<Trip>;
@group(0) @binding(3) var<storage, read> stations: array<f32>;
@group(0) @binding(6) var<storage, read> source_instance_map: array<InstanceMapEntry>;
@group(0) @binding(7) var<storage, read_write> instance_counter: InstanceCounter;
@group(0) @binding(8) var<storage, read_write> visible_segments_rw: array<VisibleSegment>;

const TICKS_PER_SECOND: i32 = 100;

fn segment_visible(entry0: Entry, entry1: Entry, repeat_slot: u32) -> bool {
    if entry0.station_index < 0 || entry1.station_index < 0 {
        return false;
    }

    let s0 = u32(entry0.station_index);
    let s1 = u32(entry1.station_index);
    let station_count = arrayLength(&stations);
    if s0 >= station_count || s1 >= station_count {
        return false;
    }

    let repeat_offset = uniforms.repeat_from + i32(repeat_slot);
    let repeat_ticks = repeat_offset * uniforms.repeat_interval_ticks;
    let dep_ticks = entry0.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let arr_ticks = entry1.arr_secs * TICKS_PER_SECOND + repeat_ticks;

    let x0 = f32(dep_ticks - uniforms.ticks_min) / uniforms.x_per_unit + uniforms.screen_origin.x;
    let x1 = f32(arr_ticks - uniforms.ticks_min) / uniforms.x_per_unit + uniforms.screen_origin.x;
    let y0 = (stations[s0] - uniforms.y_min) / uniforms.y_per_unit + uniforms.screen_origin.y;
    let y1 = (stations[s1] - uniforms.y_min) / uniforms.y_per_unit + uniforms.screen_origin.y;

    let min_x = min(x0, x1);
    let max_x = max(x0, x1);
    let min_y = min(y0, y1);
    let max_y = max(y0, y1);

    if max_x < 0.0 || min_x > uniforms.screen_size.x {
        return false;
    }
    if max_y < 0.0 || min_y > uniforms.screen_size.y {
        return false;
    }

    return true;
}

fn write_visible_segment(trip: Trip, entry0: Entry, entry1: Entry, repeat_slot: u32) {
    let idx = atomicAdd(&instance_counter.count, 1u);
    if idx >= arrayLength(&visible_segments_rw) {
        return;
    }

    let s0 = u32(entry0.station_index);
    let s1 = u32(entry1.station_index);

    let repeat_offset = uniforms.repeat_from + i32(repeat_slot);
    let repeat_ticks = repeat_offset * uniforms.repeat_interval_ticks;
    let dep_ticks = entry0.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let arr_ticks = entry1.arr_secs * TICKS_PER_SECOND + repeat_ticks;

    let x0 = f32(dep_ticks - uniforms.ticks_min) / uniforms.x_per_unit;
    let x1 = f32(arr_ticks - uniforms.ticks_min) / uniforms.x_per_unit;
    let y0 = (stations[s0] - uniforms.y_min) / uniforms.y_per_unit;
    let y1 = (stations[s1] - uniforms.y_min) / uniforms.y_per_unit;

    visible_segments_rw[idx] = VisibleSegment(
        vec2<f32>(x0, y0),
        vec2<f32>(x1, y1),
        trip.width / 2.0,
        0.0,
        trip.color,
    );
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let source_index = gid.x;
    let source_count = arrayLength(&source_instance_map);
    if source_index >= source_count {
        return;
    }

    let src = source_instance_map[source_index];
    if src.trip_index >= arrayLength(&trips) {
        return;
    }

    let trip = trips[src.trip_index];

    let start_idx = trip.start_idx + u32(src.local_segment);

    let end_idx = start_idx + 1u;
    if end_idx >= arrayLength(&entries) {
        return;
    }

    let e0 = entries[start_idx];
    let e1 = entries[end_idx];

    if uniforms.repeat_interval_ticks <= 0 {
        if segment_visible(e0, e1, 0u) {
            write_visible_segment(trip, e0, e1, 0u);
        }
        return;
    }

    let repeat_span = uniforms.repeat_to - uniforms.repeat_from + 1;
    if repeat_span <= 0 {
        return;
    }

    let repeat_count = u32(repeat_span);
    var repeat_slot = 0u;

    loop {
        if repeat_slot >= repeat_count {
            break;
        }

        if segment_visible(e0, e1, repeat_slot) {
            write_visible_segment(trip, e0, e1, repeat_slot);
        }
        repeat_slot = repeat_slot + 1u;
    }
}

struct VertexOut {
    @location(0) color: vec4<f32>,
    @location(1) feather_alpha: f32,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, seg: VertexIn) -> VertexOut {
    var out: VertexOut;
    let local = vertex_index % 18u;

    let seg_a = seg.p0;
    let seg_b = seg.p1;

    // Cursed linear algebra magic which I don't understand
    // I only know trigs!
    let sdx = seg_b.x - seg_a.x;
    let sdy = seg_b.y - seg_a.y;
    let len = max(sqrt(sdx * sdx + sdy * sdy), 1e-6);
    let nx = -sdy / len;
    let ny = sdx / len;

    let half = max(seg.half_width, 0.5) - 0.2;
    let offset = vec2<f32>(nx * half, ny * half);
    // Expand the segment on both sides so alpha can smoothly fade at the edges.
    const FEATHERING_PIXELS = 0.5;
    let offset_feathering = vec2<f32>(nx * FEATHERING_PIXELS, ny * FEATHERING_PIXELS);
    let offset_outer = offset + offset_feathering;

    let a_pos_inner = seg_a + offset;
    let a_neg_inner = seg_a - offset;
    let b_pos_inner = seg_b + offset;
    let b_neg_inner = seg_b - offset;
    let a_pos_outer = seg_a + offset_outer;
    let a_neg_outer = seg_a - offset_outer;
    let b_pos_outer = seg_b + offset_outer;
    let b_neg_outer = seg_b - offset_outer;

    var pos: vec2<f32>;
    var feather_alpha = 1.0;
    switch local {
        // Core strip.
        case 0u: { pos = a_pos_inner; }
        case 1u: { pos = a_neg_inner; }
        case 2u: { pos = b_pos_inner; }
        case 3u: { pos = a_neg_inner; }
        case 4u: { pos = b_neg_inner; }
        case 5u: { pos = b_pos_inner; }

        // Positive-side feather strip.
        case 6u: {
            pos = a_pos_outer;
            feather_alpha = 0.0;
        }
        case 7u: { pos = a_pos_inner; }
        case 8u: {
            pos = b_pos_outer;
            feather_alpha = 0.0;
        }
        case 9u: { pos = a_pos_inner; }
        case 10u: { pos = b_pos_inner; }
        case 11u: {
            pos = b_pos_outer;
            feather_alpha = 0.0;
        }

        // Negative-side feather strip.
        case 12u: { pos = a_neg_inner; }
        case 13u: {
            pos = a_neg_outer;
            feather_alpha = 0.0;
        }
        case 14u: { pos = b_neg_inner; }
        case 15u: {
            pos = a_neg_outer;
            feather_alpha = 0.0;
        }
        case 16u: {
            pos = b_neg_outer;
            feather_alpha = 0.0;
        }
        default: { pos = b_neg_inner; }
    }

    let screen_pos = pos + uniforms.screen_origin;
    let x = screen_pos.x / uniforms.screen_size.x * 2.0 - 1.0;
    let y = 1.0 - screen_pos.y / uniforms.screen_size.y * 2.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);

    out.color = seg.color;
    out.feather_alpha = feather_alpha;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let feather = smoothstep(0.0, 1.0, input.feather_alpha);
    return vec4<f32>(input.color.rgb, input.color.a * feather);
}
