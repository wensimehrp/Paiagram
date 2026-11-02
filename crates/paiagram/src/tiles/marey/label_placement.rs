use egui::{Align2, Pos2, Vec2};

pub fn label_placement(p0: Pos2, p1: Pos2, p2: Pos2) -> (Align2, Vec2) {
    let dir0 = p0 - p1;
    let dir1 = p2 - p1;

    let len_sq0 = dir0.length_sq();
    let len_sq1 = dir1.length_sq();

    let offset_dir = if len_sq0 < 1e-6 && len_sq1 < 1e-6 {
        Vec2::LEFT
    } else if len_sq0 < 1e-6 {
        -dir1.normalized()
    } else if len_sq1 < 1e-6 {
        -dir0.normalized()
    } else {
        let v0 = dir0 / len_sq0.sqrt();
        let v1 = dir1 / len_sq1.sqrt();
        let sum = v0 + v1;

        let raw_offset = if sum.length_sq() < 1e-6 {
            v0.rot90()
        } else {
            -sum.normalized()
        };

        let raw_travel = p2 - p0;
        let travel_dir = if raw_travel.y > 0.0 || (raw_travel.y == 0.0 && raw_travel.x < 0.0) {
            -raw_travel
        } else {
            raw_travel
        };

        let cross = travel_dir.x * raw_offset.y - travel_dir.y * raw_offset.x;
        if cross >= f32::EPSILON {
            -raw_offset
        } else {
            raw_offset
        }
    };

    let deg = offset_dir.angle().to_degrees().rem_euclid(360.0).round() as u16;

    let align = match deg {
        0..=5 | 355..=359 => Align2::LEFT_CENTER, // Right (+x)
        6..=84 => Align2::LEFT_TOP,               // Down-Right (+x, +y)
        85..=95 => Align2::CENTER_TOP,            // Down (+y)
        96..=174 => Align2::RIGHT_TOP,            // Down-Left (-x, +y)
        175..=185 => Align2::RIGHT_CENTER,        // Left (-x)
        186..=264 => Align2::RIGHT_BOTTOM,        // Up-Left (-x, -y)
        265..=275 => Align2::CENTER_BOTTOM,       // Up (-y)
        _ => Align2::LEFT_BOTTOM,                 // Up-Right (+x, -y)
    };

    (align, offset_dir)
}
