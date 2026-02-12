use bevy::prelude::*;
use egui::{Color32, Margin, Painter, Rect, Sense, Stroke, Vec2};
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

use crate::ui::tabs::Navigatable;

#[derive(Serialize, Deserialize, Clone, MapEntities, Default)]
pub struct GraphTab {
    navi: GraphNavigation,
}

impl PartialEq for GraphTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GraphNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: f32,
    visible: egui::Rect,
}

impl Default for GraphNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: 1.0,
            visible: egui::Rect::NOTHING,
        }
    }
}

impl super::Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 {
        self.zoom
    }
    fn zoom_y(&self) -> f32 {
        self.zoom
    }
    fn set_zoom(&mut self, zoom_x: f32, _zoom_y: f32) {
        self.zoom = zoom_x;
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f32 {
        self.y_offset as f32
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f32) {
        self.x_offset = offset_x;
        self.y_offset = offset_y as f64
    }
    fn x_from_f64(&self, value: f64) -> Self::XOffset {
        value
    }
    fn x_to_f64(&self, value: Self::XOffset) -> f64 {
        value
    }
    fn y_from_f32(&self, value: f32) -> Self::YOffset {
        value as f64
    }
    fn y_to_f32(&self, value: Self::YOffset) -> f32 {
        value as f32
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible
    }
}

impl super::Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| display(self, world, ui));
    }
}

fn display(tab: &mut GraphTab, world: &mut World, ui: &mut egui::Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);
    draw_world_grid(
        &painter,
        tab.navi.visible,
        Vec2 {
            x: tab.navi.x_offset as f32,
            y: tab.navi.y_offset as f32,
        },
        tab.navi.zoom,
    );
}

fn draw_world_grid(painter: &Painter, viewport: Rect, offset: Vec2, zoom: f32) {
    if zoom <= 0.0 {
        return;
    }

    // Transitions like diagram.rs: Linear fade between MIN and MAX screen spacing
    const MIN_WIDTH: f32 = 32.0;
    const MAX_WIDTH: f32 = 120.0;

    // Use a neutral gray without querying visuals
    let base_color = Color32::from_gray(160);

    for p in ((-5)..=5).rev() {
        let spacing = 10.0f32.powi(p);
        let screen_spacing = spacing * zoom;

        // Strength calculation identical to diagram.rs (1.5 scaling factor)
        let strength =
            ((screen_spacing * 1.5 - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)).clamp(0.0, 1.0);
        if strength <= 0.0 {
            continue;
        }

        let stroke = Stroke::new(0.6, base_color.gamma_multiply(strength));

        // Vertical lines
        let mut n = (offset.x / spacing).floor();
        loop {
            let world_x = n * spacing;
            let screen_x_rel = (world_x - offset.x) * zoom;
            if screen_x_rel > viewport.width() {
                break;
            }
            if screen_x_rel >= 0.0 {
                painter.vline(viewport.left() + screen_x_rel, viewport.y_range(), stroke);
            }
            n += 1.0;
        }

        // Horizontal lines
        let mut m = (offset.y / spacing).floor();
        loop {
            let world_y = m * spacing;
            let screen_y_rel = (world_y - offset.y) * zoom;
            if screen_y_rel > viewport.height() {
                break;
            }
            if screen_y_rel >= 0.0 {
                painter.hline(viewport.x_range(), viewport.top() + screen_y_rel, stroke);
            }
            m += 1.0;
        }
    }
}
