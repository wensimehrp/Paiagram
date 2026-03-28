use egui::{Color32, Id, Painter, Rect, Shadow};

/// Display the time indicator's indicator given the SCREEN coordinates
/// of the clip rect and the time indicator.
pub fn display_time_indicator_indicator_vertical(
    id: Id,
    rect: Rect,
    time_indicator_y: f32,
    color: Color32,
    painter: &Painter,
) {
    let y_range = rect.y_range();
    let in_viewport = y_range.contains(time_indicator_y);
    let view_strength = painter.ctx().animate_bool(id, !in_viewport);
    let rect = if time_indicator_y < (y_range.min + y_range.max) / 2.0 {
        Rect::from_two_pos(rect.left_top(), rect.right_top())
    } else {
        Rect::from_two_pos(rect.left_bottom(), rect.right_bottom())
    };
    let shape = Shadow {
        offset: [0, 0],
        blur: 255,
        spread: (8.0 * view_strength) as u8,
        color: color.gamma_multiply(0.2).gamma_multiply(view_strength),
    }
    .as_shape(rect, 0);
    painter.add(shape);
}

/// Display the time indicator's indicator given the SCREEN coordinates
/// of the clip rect and the time indicator.
pub fn display_time_indicator_indicator_horizontal(
    id: Id,
    rect: Rect,
    time_indicator_x: f32,
    color: Color32,
    painter: &Painter,
) {
    let x_range = rect.x_range();
    let in_viewport = x_range.contains(time_indicator_x);
    let view_strength = painter.ctx().animate_bool(id, !in_viewport);
    let rect = if time_indicator_x < (x_range.min + x_range.max) / 2.0 {
        Rect::from_two_pos(rect.left_top(), rect.left_bottom())
    } else {
        Rect::from_two_pos(rect.right_top(), rect.right_bottom())
    };
    let shape = Shadow {
        offset: [0, 0],
        blur: 255,
        spread: (8.0 * view_strength) as u8,
        color: color.gamma_multiply(0.2).gamma_multiply(view_strength),
    }
    .as_shape(rect, 0);
    painter.add(shape);
}
