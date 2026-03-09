use std::sync::Arc;

use super::DrawnTrip;
use bevy::prelude::*;
use egui::{Color32, Painter, Pos2, TextureId, Vec2, epaint};

const CURVE_CAP: u32 = 1;
const CURVE_CUP: u32 = 2;
const CURVE_CAP_CUP: u32 = 3;
const CURVE_CUP_CAP: u32 = 4;
const CURVE_SEGMENTS: usize = 8;

pub fn draw(
    (InRef(trips), InMut(painter)): (InRef<[DrawnTrip]>, InMut<Painter>),
    mut mesh: Local<Arc<egui::epaint::Mesh>>,
) {
    let is_dark = painter.ctx().style().visuals.dark_mode;
    let mesh_mut = Arc::make_mut(&mut mesh);
    mesh_mut.clear();
    mesh_mut.texture_id = TextureId::default();

    for DrawnTrip {
        entity: _,
        stroke,
        points,
        entries: _,
    } in trips
    {
        let width = stroke.width;
        let color = stroke.color.get(is_dark);
        for point_group in points {
            let flattened = point_group.as_flattened();
            for (idx, window) in flattened
                .windows(2)
                .enumerate()
                .filter(|(_, window)| window[0] != window[1])
            {
                let u = idx.checked_sub(2).map(|j| flattened[j]);
                let a = window[0];
                let b = window[1];
                let v = flattened.get(idx + 3).copied();
                if idx % 4 == 1 {
                    let curve_type = classify_curve_type(u, a, b, v);
                    push_curve(mesh_mut, a, b, width, color, curve_type);
                } else {
                    push_segment_quad(mesh_mut, a, b, width, color);
                }
            }
        }
    }

    painter.add(epaint::Shape::mesh(Arc::clone(&mesh)));
}

fn classify_curve_type(u: Option<Pos2>, a: Pos2, b: Pos2, v: Option<Pos2>) -> u32 {
    match (u, v) {
        (Some(u), Some(v)) => {
            let incoming_up = u.y > a.y;
            let incoming_down = u.y < a.y;
            let outgoing_up = v.y > b.y;
            let outgoing_down = v.y < b.y;

            if incoming_up && outgoing_down {
                CURVE_CAP_CUP
            } else if incoming_down && outgoing_up {
                CURVE_CUP_CAP
            } else if incoming_up || outgoing_up {
                CURVE_CAP
            } else if incoming_down || outgoing_down {
                CURVE_CUP
            } else {
                CURVE_CAP_CUP
            }
        }
        (None, None) => CURVE_CAP_CUP,
        (Some(u), None) => {
            let incoming_up = u.y > a.y;
            let incoming_down = u.y < a.y;
            if incoming_up {
                CURVE_CAP
            } else if incoming_down {
                CURVE_CUP
            } else {
                CURVE_CAP_CUP
            }
        }
        (None, Some(v)) => {
            let outgoing_up = v.y > b.y;
            let outgoing_down = v.y < b.y;
            if outgoing_up {
                CURVE_CAP
            } else if outgoing_down {
                CURVE_CUP
            } else {
                CURVE_CUP_CAP
            }
        }
    }
}

fn push_curve(mesh: &mut epaint::Mesh, a: Pos2, b: Pos2, width: f32, color: Color32, curve_type: u32) {
    let dx = b.x - a.x;
    let curve_height = 8.0_f32.max(dx.abs() * 0.15 + 6.0);
    for seg_index in 0..CURVE_SEGMENTS {
        let t0 = seg_index as f32 / CURVE_SEGMENTS as f32;
        let t1 = (seg_index + 1) as f32 / CURVE_SEGMENTS as f32;
        let (seg_a, seg_b) = curve_points(a, b, curve_type, curve_height, t0, t1);
        push_segment_quad(mesh, seg_a, seg_b, width, color);
    }
}

fn curve_points(
    a: Pos2,
    b: Pos2,
    curve_type: u32,
    curve_height: f32,
    t0: f32,
    t1: f32,
) -> (Pos2, Pos2) {
    let mut p0 = lerp_pos(a, b, t0);
    let mut p1 = lerp_pos(a, b, t1);
    if curve_type == CURVE_CAP || curve_type == CURVE_CUP {
        let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        let min_y = a.y.min(b.y);
        let max_y = a.y.max(b.y);
        let control = if curve_type == CURVE_CAP {
            Pos2::new(mid.x, min_y - curve_height)
        } else {
            Pos2::new(mid.x, max_y + curve_height)
        };
        p0 = quadratic_bezier(a, control, b, t0);
        p1 = quadratic_bezier(a, control, b, t1);
    } else if curve_type == CURVE_CAP_CUP || curve_type == CURVE_CUP_CAP {
        let tau = std::f32::consts::TAU;
        let amp = curve_height * 0.2;
        let dir = if curve_type == CURVE_CAP_CUP {
            -1.0
        } else {
            1.0
        };
        p0.y += dir * amp * (t0 * tau).sin();
        p1.y += dir * amp * (t1 * tau).sin();
    }
    (p0, p1)
}

fn quadratic_bezier(a: Pos2, control: Pos2, b: Pos2, t: f32) -> Pos2 {
    let omt = 1.0 - t;
    let x = omt * omt * a.x + 2.0 * omt * t * control.x + t * t * b.x;
    let y = omt * omt * a.y + 2.0 * omt * t * control.y + t * t * b.y;
    Pos2::new(x, y)
}

fn lerp_pos(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn push_segment_quad(mesh: &mut epaint::Mesh, a: Pos2, b: Pos2, width: f32, color: Color32) {
    let delta = b - a;
    let len_sq = delta.length_sq();
    if len_sq <= f32::EPSILON {
        return;
    }
    let len = len_sq.sqrt();
    let normal = Vec2::new(-delta.y / len, delta.x / len);
    let offset = normal * (width * 0.5);

    let a1 = a + offset;
    let a2 = a - offset;
    let b1 = b + offset;
    let b2 = b - offset;

    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(epaint::Vertex {
        pos: a1,
        uv: epaint::WHITE_UV,
        color,
    });
    mesh.vertices.push(epaint::Vertex {
        pos: a2,
        uv: epaint::WHITE_UV,
        color,
    });
    mesh.vertices.push(epaint::Vertex {
        pos: b1,
        uv: epaint::WHITE_UV,
        color,
    });
    mesh.vertices.push(epaint::Vertex {
        pos: b2,
        uv: epaint::WHITE_UV,
        color,
    });

    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
}
