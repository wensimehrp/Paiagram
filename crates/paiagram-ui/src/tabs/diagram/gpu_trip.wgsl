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
    feathering_radius: f32,
    pixels_per_point: f32,
};

struct Entry {
    arr_secs: i32,
    dep_secs: i32,
    station_index: u32,
    _track_index: u32,
};

struct Trip {
    color: u32,
    width: f32,
    len: u32,
    start_idx: u32,
};

struct InstanceMapEntry {
    trip_index: u32,
    local_segment: u32,
};

struct VisibleSegment {
    p0: vec2<f32>,
    p1: vec2<f32>,
    half_width: f32,
    curve_a_or_nan: f32,
    curve_b: f32,
    color: u32,
};

struct SegmentMeshVertex {
    along: f32,
    side: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> entries: array<Entry>;
@group(0) @binding(2) var<storage, read> trips: array<Trip>;
@group(0) @binding(3) var<storage, read> stations: array<f32>;
@group(0) @binding(4) var<storage, read> source_instance_map: array<InstanceMapEntry>;
@group(0) @binding(5) var<storage, read_write> visible_segments_rw: array<VisibleSegment>;
@group(0) @binding(6) var<storage, read> visible_segments: array<VisibleSegment>;

const SEGMENT_MESH_VERTICES: array<SegmentMeshVertex, 4> = array<SegmentMeshVertex, 4>(
    SegmentMeshVertex(0.0, 1.0),
    SegmentMeshVertex(0.0, -1.0),
    SegmentMeshVertex(1.0, 1.0),
    SegmentMeshVertex(1.0, -1.0),
);

const SEGMENT_MESH_INDICES: array<u32, 6> = array<u32, 6>(
    0u, 1u, 2u, 1u, 3u, 2u,
);

const TICKS_PER_SECOND: i32 = 100;
const LANE_COUNT: u32 = 2u;
const LINE_MARKER_BITS: u32 = 0xDEADBEEFu;

fn ticks_to_screen_x(ticks: i32) -> f32 {
    return f32(ticks - uniforms.ticks_min) / uniforms.x_per_unit;
}

fn station_to_screen_y(station_index: u32) -> f32 {
    return (stations[station_index] - uniforms.y_min) / uniforms.y_per_unit;
}

fn repeat_ticks_for_slot(repeat_slot: u32) -> i32 {
    let repeat_offset = uniforms.repeat_from + i32(repeat_slot);
    return repeat_offset * uniforms.repeat_interval_ticks;
}

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

    let repeat_ticks = repeat_ticks_for_slot(repeat_slot);
    let dep_ticks = entry0.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let arr_ticks = entry1.arr_secs * TICKS_PER_SECOND + repeat_ticks;

    let x0 = ticks_to_screen_x(dep_ticks) + uniforms.screen_origin.x;
    let x1 = ticks_to_screen_x(arr_ticks) + uniforms.screen_origin.x;
    let y0 = station_to_screen_y(s0) + uniforms.screen_origin.y;
    let y1 = station_to_screen_y(s1) + uniforms.screen_origin.y;

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

fn write_visible_segment(source_index: u32, trip: Trip, entry0: Entry, entry1: Entry, repeat_slot: u32) {
    let base = uniforms.source_instance_count * repeat_slot;
    let idx = base + source_index * LANE_COUNT;
    if idx >= arrayLength(&visible_segments_rw) {
        return;
    }

    let s0 = u32(entry0.station_index);
    let s1 = u32(entry1.station_index);

    let repeat_ticks = repeat_ticks_for_slot(repeat_slot);
    let dep_ticks = entry0.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let arr_ticks = entry1.arr_secs * TICKS_PER_SECOND + repeat_ticks;

    let x0 = ticks_to_screen_x(dep_ticks);
    let x1 = ticks_to_screen_x(arr_ticks);
    let y0 = station_to_screen_y(s0);
    let y1 = station_to_screen_y(s1);

    visible_segments_rw[idx] = VisibleSegment(
        vec2<f32>(x0, y0),
        vec2<f32>(x1, y1),
        trip.width / 2.0,
        bitcast<f32>(LINE_MARKER_BITS),
        0.0,
        trip.color,
    );
}

fn segment_slope(entry0: Entry, entry1: Entry, repeat_slot: u32) -> f32 {
    let station_count = arrayLength(&stations);
    if entry0.station_index >= station_count || entry1.station_index >= station_count {
        return 0.0;
    }

    let repeat_ticks = repeat_ticks_for_slot(repeat_slot);
    let dep_ticks = entry0.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let arr_ticks = entry1.arr_secs * TICKS_PER_SECOND + repeat_ticks;
    let x0 = ticks_to_screen_x(dep_ticks);
    let x1 = ticks_to_screen_x(arr_ticks);

    let dx = x1 - x0;
    if abs(dx) < 1e-6 {
        return 0.0;
    }

    let y0 = station_to_screen_y(entry0.station_index);
    let y1 = station_to_screen_y(entry1.station_index);
    return (y1 - y0) / dx;
}

fn write_stop_segment(
    source_index: u32,
    trip: Trip,
    is_first: bool,
    is_last: bool,
    prev_entry: Entry,
    curr_entry: Entry,
    next_entry: Entry,
    repeat_slot: u32,
) {
    if curr_entry.arr_secs == curr_entry.dep_secs {
        return;
    }

    let base = uniforms.source_instance_count * repeat_slot;
    let idx = base + source_index * LANE_COUNT + 1u;
    if idx >= arrayLength(&visible_segments_rw) {
        return;
    }

    let station_count = arrayLength(&stations);
    if curr_entry.station_index >= station_count {
        return;
    }

    var a = segment_slope(prev_entry, curr_entry, repeat_slot);
    var b = segment_slope(curr_entry, next_entry, repeat_slot);
    if is_first {
        a = -b;
    }
    if is_last {
        b = -a;
    }

    let repeat_ticks = repeat_ticks_for_slot(repeat_slot);
    let arr_ticks = curr_entry.arr_secs * TICKS_PER_SECOND + repeat_ticks;
    let dep_ticks = curr_entry.dep_secs * TICKS_PER_SECOND + repeat_ticks;
    let x0 = ticks_to_screen_x(arr_ticks);
    let x1 = ticks_to_screen_x(dep_ticks);
    let y = station_to_screen_y(curr_entry.station_index);

    visible_segments_rw[idx] = VisibleSegment(
        vec2<f32>(x0, y),
        vec2<f32>(x1, y),
        trip.width / 2.0,
        a,
        b,
        trip.color,
    );
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let source_index = gid.x / LANE_COUNT;
    let lane = gid.x % LANE_COUNT;
    let repeat_slot = gid.y;
    let source_count = uniforms.source_instance_count / LANE_COUNT;
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

    let curr = entries[start_idx];
    let next = entries[end_idx];
    let is_first = src.local_segment == 0u;
    var last_real_segment = 0u;
    if trip.len > 1u {
        last_real_segment = trip.len - 2u;
    }
    let is_last = src.local_segment == last_real_segment;
    var prev = curr;
    if !is_first {
        prev = entries[start_idx - 1u];
    }

    var repeat_count = 1u;
    if uniforms.repeat_interval_ticks > 0 {
        let repeat_span = uniforms.repeat_to - uniforms.repeat_from + 1;
        if repeat_span <= 0 {
            return;
        }
        repeat_count = u32(repeat_span);
    }

    if repeat_slot >= repeat_count {
        return;
    }

    if lane == 0u {
        if segment_visible(curr, next, repeat_slot) {
            write_visible_segment(source_index, trip, curr, next, repeat_slot);
        }
    } else {
        write_stop_segment(
            source_index,
            trip,
            is_first,
            is_last,
            prev,
            curr,
            next,
            repeat_slot,
        );
    }
}

struct VertexOut {
    @location(0) color: vec4<f32>,
    @location(1) world_pos: vec2<f32>,
    @location(2) seg_p0: vec2<f32>,
    @location(3) seg_p1: vec2<f32>,
    @location(4) half_width: f32,
    @location(5) curve_a: f32,
    @location(6) curve_b: f32,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOut {
    if instance_index >= arrayLength(&visible_segments) {
        return VertexOut(
            vec4<f32>(0.0, 0.0, 0.0, 0.0),
            vec2<f32>(0.0, 0.0),
            vec2<f32>(0.0, 0.0),
            vec2<f32>(0.0, 0.0),
            0.0,
            0.0,
            0.0,
            vec4<f32>(2.0, 2.0, 0.0, 1.0),
        );
    }

    let mesh_index = SEGMENT_MESH_INDICES[vertex_index];
    let mesh = SEGMENT_MESH_VERTICES[mesh_index];
    let seg = visible_segments[instance_index];

    let marker_bits = bitcast<u32>(seg.curve_a_or_nan);
    let is_stop = marker_bits != LINE_MARKER_BITS;
    var seg_a = seg.p0;
    var seg_b = seg.p1;
    var tangent = vec2<f32>(1.0, 0.0);
    var normal = vec2<f32>(0.0, 1.0);
    let stop_r = max(0.5, 60.0 * max(uniforms.pixels_per_point, 1e-3));
    var dist = max(seg.half_width + uniforms.feathering_radius, 0.01);

    if !is_stop {
        let sdx = seg_b.x - seg_a.x;
        let sdy = seg_b.y - seg_a.y;
        let len = max(sqrt(sdx * sdx + sdy * sdy), 1e-6);
        tangent = vec2<f32>(sdx / len, sdy / len);
        normal = vec2<f32>(-tangent.y, tangent.x);
    } else {
        let x_min = min(seg.p0.x, seg.p1.x);
        let x_max = max(seg.p0.x, seg.p1.x);
        let cap_pad = seg.half_width + uniforms.feathering_radius;
        seg_a = vec2<f32>(x_min - cap_pad, seg.p0.y);
        seg_b = vec2<f32>(x_max + cap_pad, seg.p0.y);
        // Stop quads must cover curve amplitude + full line half-width + AA feather band.
        let stop_extent = stop_r + seg.half_width + uniforms.feathering_radius;
        dist = max(stop_extent, 0.01);
    }

    let normal_offset = normal * (mesh.side * dist);
    let base_pos = seg_a + (seg_b - seg_a) * mesh.along;
    let pos = base_pos + normal_offset;

    let screen_pos = pos + uniforms.screen_origin;
    let x = screen_pos.x / uniforms.screen_size.x * 2.0 - 1.0;
    let y = 1.0 - screen_pos.y / uniforms.screen_size.y * 2.0;

    let color = seg.color;
    let r = f32((color >> 0u) & 0xFFu) / 255.0;
    let g = f32((color >> 8u) & 0xFFu) / 255.0;
    let b = f32((color >> 16u) & 0xFFu) / 255.0;
    let a = f32((color >> 24u) & 0xFFu) / 255.0;

    return VertexOut(
        vec4<f32>(r, g, b, a),
        pos,
        seg.p0,
        seg.p1,
        seg.half_width,
        seg.curve_a_or_nan,
        seg.curve_b,
        vec4<f32>(x, y, 0.0, 1.0),
    );
}

fn distance_to_line_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let ab_len2 = dot(ab, ab);
    if ab_len2 < 1e-10 {
        return distance(p, a);
    }
    let t = clamp(dot(p - a, ab) / ab_len2, 0.0, 1.0);
    let q = a + ab * t;
    return distance(p, q);
}

fn stop_curve_y_raw(dx: f32, a: f32, f0: f32, f1: f32) -> f32 {
    return a * dx + f0 * dx * dx + f1 * dx * dx * dx;
}

fn stop_curve_dy_raw(dx: f32, a: f32, f0: f32, f1: f32) -> f32 {
    return a + 2.0 * f0 * dx + 3.0 * f1 * dx * dx;
}

fn distance_to_stop_curve(
    p: vec2<f32>,
    x0: f32,
    x1: f32,
    y_base: f32,
    a: f32,
    b: f32,
    r: f32,
) -> f32 {
    let h = x1 - x0;

    if abs(h) < 3.0 {
        return distance(p, vec2<f32>(x0, y_base));
    }

    let min_x = min(x0, x1);
    let max_x = max(x0, x1);

    let f0 = -(a * 2.0 + b) / h;
    let f1 = (a + b) / h / h;
    let r_safe = max(r, 1e-3);

    let clamped_x = clamp(p.x, min_x, max_x);
    let dx = clamped_x - x0;

    let y_raw = stop_curve_y_raw(dx, a, f0, f1);
    let dy_raw = stop_curve_dy_raw(dx, a, f0, f1);

    let arg = y_raw / r_safe;
    let t = arg / sqrt(1.0 + arg * arg);
    let sech2 = 1.0 / (1.0 + arg * arg);

    let curve_y = y_base + r_safe * t;

    // If the pixel is horizontally outside the curve segment, just use standard point distance to the cap
    if p.x < min_x || p.x > max_x {
        return distance(p, vec2<f32>(clamped_x, curve_y));
    }

    let curve_dy = sech2 * dy_raw;
    let vertical_dist = abs(p.y - curve_y);

    // Divide vertical distance by the magnitude of the normal vector
    return vertical_dist / sqrt(1.0 + curve_dy * curve_dy);
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    var dist = 0.0;
    let marker_bits = bitcast<u32>(input.curve_a);
    let is_stop = marker_bits != LINE_MARKER_BITS;
    if is_stop {
        let stop_r = max(0.5, 60.0 * max(uniforms.pixels_per_point, 1e-3));
        dist = distance_to_stop_curve(
            input.world_pos,
            input.seg_p0.x,
            input.seg_p1.x,
            input.seg_p0.y,
            input.curve_a,
            input.curve_b,
            stop_r,
        );
    } else {
        dist = distance_to_line_segment(input.world_pos, input.seg_p0, input.seg_p1);
    }

    let sdf = dist - input.half_width;
    let ppp = max(uniforms.pixels_per_point, 1e-3);
    let aa_min = 0.30 / ppp;
    let aa_max = max(input.half_width, aa_min);
    let aa = clamp(fwidth(sdf) * 1.1, aa_min, aa_max);
    let alpha = smoothstep(aa, -aa, sdf);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
