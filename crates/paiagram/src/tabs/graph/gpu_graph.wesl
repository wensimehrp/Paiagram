// SPDX-License-Identifier: MPL-2.0

import package::shaders::draw_lines::{
    SEGMENT_QUAD_CORNERS,
    SEGMENT_QUAD_LENGTH,
    VertexOut,
    line_alpha,
    line_feather,
    segment_quad_pos
};

struct Uniforms {
    screen_size: vec2<f32>,
    x_proj_min: f32,
    y_proj_min: f32,
    // x_screen_per_x_proj and y_screen_per_y_proj
    // same as zoom from OSM
    zoom: f32,
    pixels_per_point: f32,
}

/// NOT Wgs84LonLat
struct LonLat {
    lon: i32,
    lat: i32,
}

struct IntervalPoint {
    coordinate: LonLat,
    connects_to_next: u32,
}

/// Uniform buffers for general info
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
/// Storage for all points
@group(0) @binding(1)
var<storage, read> interval_points: array<IntervalPoint>;

// see coordinates.rs
const EARTH_RADIUS_METERS: f32 = 6378137.0;
const WEB_MERCATOR_MAX_LAT: f32 = 85.05112877980659;
const FRAC_PI_4: f32 = 0.7853981633974483;
const LONLAT_SCALE: f32 = 10000000.0;

fn lonlat_to_xy_proj_pos(ll: LonLat) -> vec2<f32> {
    let lon_deg = clamp(f32(ll.lon) / LONLAT_SCALE, -180.0, 180.0);
    let lat_deg = clamp(f32(ll.lat) / LONLAT_SCALE, -90.0, 90.0);
    let lon_rad = radians(lon_deg);
    let lat_rad = radians(clamp(lat_deg, -WEB_MERCATOR_MAX_LAT, WEB_MERCATOR_MAX_LAT));
    let x = EARTH_RADIUS_METERS * lon_rad;
    let y = -EARTH_RADIUS_METERS * log(tan(FRAC_PI_4 + lat_rad * 0.5));
    return vec2<f32>(x, y);
}

fn xy_proj_pos_to_screen_pos(xy_proj_pos: vec2<f32>) -> vec2<f32> {
    let proj_min = vec2<f32>(uniforms.x_proj_min, uniforms.y_proj_min);
    return (xy_proj_pos - proj_min) * uniforms.zoom;
}

fn screen_pos_to_clip(screen_pos: vec2<f32>) -> vec2<f32> {
    let clip_x = (screen_pos.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (screen_pos.y / uniforms.screen_size.y) * 2.0;
    return vec2<f32>(clip_x, clip_y);
}

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

    let start = xy_proj_pos_to_screen_pos(lonlat_to_xy_proj_pos(point.coordinate));

    let corner = SEGMENT_QUAD_CORNERS[vertex_index];
    let offset = corner.y * half_extent;
    // A point only draws its outgoing segment when it connects to the next one; otherwise every
    // vertex collapses onto `start`, leaving a zero-area (invisible) quad.
    var screen_pos = start;
    if point.connects_to_next != 0u {
        let end = xy_proj_pos_to_screen_pos(
                lonlat_to_xy_proj_pos(interval_points[instance_index + 1u].coordinate),
            );
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
