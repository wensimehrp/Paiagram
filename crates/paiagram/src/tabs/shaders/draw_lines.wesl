// SPDX-License-Identifier: MPL-2.0
///
/// Line drawing module.
///
/// A segment is rendered as a quad offset from the centre line by
/// `half_width + feather/2`, so that the 50%-coverage contour sits on the nominal
/// edge. Vertices that two segments share are mitered together, so the lines bevel
/// into each other instead of overlapping or leaving a notch.

/// Width of the anti-aliasing feather band, in hardware pixels.
const FEATHER_WIDTH_PX: f32 = 2.0;

/// A corner of the unit quad that is expanded into a line segment.
struct SegmentMeshVertex {
    /// Position along the segment (0 = start, 1 = end).
    along: f32,
    /// Offset along the segment normal (-1 = left edge, +1 = right edge).
    side: f32,
}

const SEGMENT_MESH_VERTICES: array<SegmentMeshVertex, 4> =
    array<SegmentMeshVertex, 4>(
        SegmentMeshVertex(0.0, 1.0), // 0, Start-Left
        SegmentMeshVertex(0.0, -1.0), // 1, Start-Right
        SegmentMeshVertex(1.0, 1.0), // 2, End-Left
        SegmentMeshVertex(1.0, -1.0), // 3, End-Right
    );

/// Number of indices per instance: the segment's own quad plus the quad connecting it to the
/// next one.
const SEGMENT_MESH_LENGTH: u32 = 12;

const SEGMENT_MESH_INDICES: array<u32, 12> = array<u32, 12>(
        // the entry's own (dwell) segment
        0u,
        1u,
        2u,
        1u,
        3u,
        2u,
        // the segment connecting to the next entry
        0u,
        1u,
        2u,
        1u,
        3u,
        2u,
    );

/// The vertex shader output of a line segment.
struct VertexOut {
    @location(0) color: vec4<f32>,
    /// Signed distance in screen points from the centre line.
    @location(1) offset: f32,
    /// Half the line thickness in screen points.
    @location(2) half_width: f32,
    @builtin(position) position: vec4<f32>,
}

/// The feather band width in screen points, converted from hardware pixels.
fn line_feather(pixels_per_point: f32) -> f32 {
    return FEATHER_WIDTH_PX / max(pixels_per_point, 1e-6);
}

fn normalize_or(v: vec2<f32>, fallback: vec2<f32>) -> vec2<f32> {
    let len = length(v);
    if len < 1e-5 {
        return fallback;
    }
    return v / len;
}

// Implementation detail of `joint_vertex`, not used outside this module.
fn left_normal(dir: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(-dir.y, dir.x);
}

/// Vertex of a joint: the intersection of the offset lines of the two segments meeting at `p`,
/// so the two lines bevel into each other instead of overlapping or leaving a notch.
fn joint_vertex(
    p: vec2<f32>,
    dir_in: vec2<f32>,
    dir_out: vec2<f32>,
    side: f32,
    half_width: f32,
) -> vec2<f32> {
    let n_in = left_normal(dir_in) * side;
    let n_out = left_normal(dir_out) * side;
    // The miter length grows without bound as the turn approaches a reversal.
    let denom = max(1.0 + dot(dir_in, dir_out), 0.25);
    return p + half_width * (n_in + n_out) / denom;
}

/// Anti-aliased coverage of a line at signed perpendicular distance `offset` from its centre.
fn line_alpha(offset: f32, half_width: f32, feather: f32) -> f32 {
    return 1.0 - smoothstep(half_width - feather * 0.5, half_width + feather * 0.5, abs(offset));
}

/// A corner of the quad a single straight segment (two independent endpoints, no joints) is
/// expanded from. `x` is the position along the segment (-1 = start, +1 = end) and `y` is the
/// offset along the segment normal (-1 = left edge, +1 = right edge).
const SEGMENT_QUAD_CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

const SEGMENT_QUAD_LENGTH: u32 = 6;

/// Expands one corner of a straight segment from `p0` to `p1`, offset from the centre line by
/// `half_extent`. Pass `half_width + feather / 2` as `half_extent` so the 50%-coverage contour
/// sits exactly on the nominal edge.
fn segment_quad_pos(
    p0: vec2<f32>,
    p1: vec2<f32>,
    corner: vec2<f32>,
    half_extent: f32,
) -> vec2<f32> {
    let dir = normalize_or(p1 - p0, vec2<f32>(1.0, 0.0));
    let normal = left_normal(dir);
    let center = (p0 + p1) * 0.5;
    let half_len = length(p1 - p0) * 0.5;
    return center + dir * (corner.x * half_len) + normal * (corner.y * half_extent);
}
