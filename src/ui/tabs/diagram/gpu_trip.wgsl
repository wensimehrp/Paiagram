struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexIn {
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
    @location(4) curve: u32,
};

struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn, @builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var out: VertexOut;
    let dx = input.b.x - input.a.x;
    let dy = input.b.y - input.a.y;

    let seg_index = vertex_index / 6u;
    let local = vertex_index % 6u;
    let is_curve = input.curve != 0u;
    var seg_a = input.a;
    var seg_b = input.b;
    if (is_curve) {
        let mid = (input.a + input.b) * 0.5;
        let min_y = min(input.a.y, input.b.y);
        let curve_height = max(8.0, abs(dx) * 0.15 + 6.0);
        let control = vec2<f32>(mid.x, min_y - curve_height);

        let seg_count: u32 = 8u;
        let t0 = f32(seg_index) / f32(seg_count);
        let t1 = f32(seg_index + 1u) / f32(seg_count);

        let omt0 = 1.0 - t0;
        let omt1 = 1.0 - t1;
        let curve_p0 = omt0 * omt0 * input.a + 2.0 * omt0 * t0 * control + t0 * t0 * input.b;
        let curve_p1 = omt1 * omt1 * input.a + 2.0 * omt1 * t1 * control + t1 * t1 * input.b;

        seg_a = select(curve_p0, input.a, seg_index >= seg_count);
        seg_b = select(curve_p1, input.a, seg_index >= seg_count);
    }

    let sdx = seg_b.x - seg_a.x;
    let sdy = seg_b.y - seg_a.y;
    let len = max(sqrt(sdx * sdx + sdy * sdy), 1e-6);
    let nx = -sdy / len;
    let ny = sdx / len;
    let half = input.width * 0.5;
    let offset = vec2<f32>(nx * half, ny * half);

    let a1 = seg_a + offset;
    let a2 = seg_a - offset;
    let b1 = seg_b + offset;
    let b2 = seg_b - offset;

    var pos: vec2<f32>;
    switch (local) {
        case 0u: { pos = a1; }
        case 1u: { pos = a2; }
        case 2u: { pos = b1; }
        case 3u: { pos = a2; }
        case 4u: { pos = b2; }
        default: { pos = b1; }
    }

    let x = pos.x / uniforms.screen_size.x * 2.0 - 1.0;
    let y = 1.0 - pos.y / uniforms.screen_size.y * 2.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
