use bevy::math::ops::sqrt;
use egui::{Color32, Frame, Painter, Pos2, Response, Shape, Stroke, Style, Ui};

pub fn circle_button_shape(
    painter: &mut Painter,
    center: Pos2,
    diameter: f32,
    stroke: Stroke,
    fill_color: Color32,
) {
    painter.circle(center, diameter / 2.0, fill_color, stroke);
}

pub fn triangle_button_shape(
    painter: &mut Painter,
    center: Pos2,
    base: f32,
    stroke: Stroke,
    fill_color: Color32,
) {
    let dx = base / 4.0 * sqrt(3.0);
    let a = Pos2 {
        x: center.x - dx,
        y: center.y - base / 2.0,
    };
    let b = Pos2 {
        x: center.x + dx,
        y: center.y,
    };
    let c = Pos2 {
        x: a.x,
        y: center.y + base / 2.0,
    };
    painter.add(Shape::convex_polygon(vec![a, b, c], fill_color, stroke));
}

pub fn dash_button_shape(
    painter: &mut Painter,
    center: Pos2,
    base: f32,
    stroke: Stroke,
    fill_color: Color32,
) {
    let a = Pos2 {
        x: center.x - base / 2.0 - base / 2.0 / sqrt(3.0),
        y: center.y - base / 2.0,
    };
    let b = Pos2 {
        x: a.x + base,
        y: a.y,
    };
    let c = Pos2 {
        x: center.x + base / 2.0,
        y: center.y,
    };
    let d = Pos2 {
        x: b.x,
        y: center.y + base / 2.0,
    };
    let e = Pos2 { x: a.x, y: d.y };
    let f = Pos2 {
        x: center.x - base / 2.0,
        y: center.y,
    };
    painter.add(Shape::convex_polygon(
        vec![a, b, c, f],
        fill_color,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![f, c, d, e],
        fill_color,
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(vec![a, b, c, d, e, f], stroke));
}
