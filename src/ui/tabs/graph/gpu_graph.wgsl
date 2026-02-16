struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) @interpolate(flat) kind: u32,
};

struct ShapeInstance {
  a: vec2<f32>,
  b: vec2<f32>,
  size: f32,
  color: vec4<f32>,
  kind: u32,
};

struct Screen {
  size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u_screen: Screen;

fn to_ndc(p: vec2<f32>) -> vec4<f32> {
  let x = (p.x / u_screen.size.x) * 2.0 - 1.0;
  let y = 1.0 - (p.y / u_screen.size.y) * 2.0;
  return vec4<f32>(x, y, 0.0, 1.0);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @location(0) a: vec2<f32>, @location(1) b: vec2<f32>, @location(2) size: f32, @location(3) color: vec4<f32>, @location(4) kind: u32) -> VsOut {
  var out: VsOut;

  var corners = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
  );

  let local = corners[vi];
  var world: vec2<f32>;

  if kind == 0u {
    let d = b - a;
    let len = max(length(d), 0.0001);
    let dir = d / len;
    let n = vec2<f32>(-dir.y, dir.x);
    let center = (a + b) * 0.5;
    let half_len = len * 0.5;
    let half_w = max(size * 0.5, 0.5);
    world = center + dir * local.x * half_len + n * local.y * half_w;
  } else {
    let r = max(size, 0.5);
    world = a + local * r;
  }

  out.pos = to_ndc(world);
  out.local = local;
  out.color = color;
  out.kind = kind;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  if in.kind == 1u {
    if dot(in.local, in.local) > 1.0 {
      discard;
    }
  }
  return in.color;
}
