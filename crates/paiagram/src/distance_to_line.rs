use egui::Pos2;

pub fn distance_sq_to_line((p1, p2): (Pos2, Pos2), p: Pos2) -> f32 {
    let diff = p2 - p1;
    let len_sq = diff.length_sq();
    let t = if len_sq > 0.0 { ((p - p1).dot(diff) / len_sq).clamp(0.0, 1.0) } else { 0.0 };
    (p1 + t * diff - p).length_sq()
}
