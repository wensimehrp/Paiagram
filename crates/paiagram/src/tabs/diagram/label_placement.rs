use egui::{Align, Align2, Pos2, Vec2};

/// Placement for a label drawn at the corner `p1`–`p2`–`p3`, where `p2` is the corner vertex.
pub fn label_placement(
    Pos2 { x: x1, y: y1 }: Pos2,
    Pos2 { x: x2, y: y2 }: Pos2,
    Pos2 { x: x3, y: y3 }: Pos2,
) -> (Align2, Vec2) {
    let edge_a = Vec2::new(x1 - x2, y1 - y2).normalized();
    let edge_b = Vec2::new(x3 - x2, y3 - y2).normalized();
    let bisector = if edge_a == Vec2::ZERO {
        let dir = Vec2::new(x3 - x2, y3 - y2);
        Vec2::new(-dir.y, dir.x)
    } else if edge_b == Vec2::ZERO {
        let dir = Vec2::new(x2 - x1, y2 - y1);
        Vec2::new(-dir.y, dir.x)
    } else {
        edge_a + edge_b
    };

    const EPS: f32 = 1e-4;
    let align_x = if bisector.x > EPS {
        Align::RIGHT
    } else if bisector.x < -EPS {
        Align::LEFT
    } else {
        Align::Center
    };
    let align_y = if bisector.y > EPS {
        Align::BOTTOM
    } else if bisector.y < -EPS {
        Align::TOP
    } else {
        Align::Center
    };
    (
        Align2::new(align_x, align_y),
        bisector.normalized().rot90().rot90(),
    )
}
