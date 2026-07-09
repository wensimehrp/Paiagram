use egui::{Painter, Rect, Stroke};
use serde::{Deserialize, Serialize};

use super::GraphNavigation;

#[derive(Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) enum UnderlayTileType {
    None,
    #[default]
    OpenStreetMap,
    EsriWorldImagery,
}

/// Simplified underlay: draw world grid.
pub(crate) fn draw_underlay(
    painter: &mut Painter,
    navi: &GraphNavigation,
    _ui: &egui::Ui,
    _new_type: Option<UnderlayTileType>,
) -> Option<(String, String)> {
    use crate::tabs::Navigatable;
    draw_world_grid(
        painter,
        navi.visible,
        navi.offset_x() as f32,
        navi.offset_y() as f32,
        navi.zoom_x(),
    );
    None
}

fn draw_world_grid(painter: &Painter, viewport: Rect, offset_x: f32, offset_y: f32, zoom: f32) {
    if zoom <= 0.0 { return; }
    const MIN_WIDTH: f32 = 32.0;
    const MAX_WIDTH: f32 = 120.0;
    let base_color = egui::Color32::from_gray(160);

    for p in ((-5)..=5).rev() {
        let spacing = 10.0f32.powi(p);
        let screen_spacing = spacing * zoom;
        let strength = ((screen_spacing * 1.5 - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)).clamp(0.0, 1.0);
        if strength <= 0.0 { continue; }
        let stroke = Stroke::new(0.6, base_color.gamma_multiply(strength));

        let mut n = (offset_x / spacing).floor();
        loop {
            let world_x = n * spacing;
            let screen_x_rel = (world_x - offset_x) * zoom;
            if screen_x_rel > viewport.width() { break; }
            if screen_x_rel >= 0.0 {
                painter.vline(viewport.left() + screen_x_rel, viewport.y_range(), stroke);
            }
            n += 1.0;
        }

        let mut m = (offset_y / spacing).floor();
        loop {
            let world_y = m * spacing;
            let screen_y_rel = (world_y - offset_y) * zoom;
            if screen_y_rel > viewport.height() { break; }
            if screen_y_rel >= 0.0 {
                painter.hline(viewport.x_range(), viewport.top() + screen_y_rel, stroke);
            }
            m += 1.0;
        }
    }
}
