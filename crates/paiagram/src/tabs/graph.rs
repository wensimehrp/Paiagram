//! Geographic network editor backed exclusively by Source commands and spatial caches.
use std::sync::Arc;

use egui::*;
use paiagram_core::*;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::{Navigatable, Tab};
use crate::App;
mod gpu_draw;
mod gpu_graph;
mod underlay;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub(crate) struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip)]
    underlay: Arc<Mutex<underlay::UnderlayPainter>>,
    #[serde(skip)]
    panel_is_open: bool,
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            underlay_tile_type: underlay::UnderlayTileType::None,
            underlay: Default::default(),
            panel_is_open: true,
        }
    }
}

impl PartialEq for GraphTab {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GraphNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: f32,
    visible: Rect,
}

impl Default for GraphNavigation {
    fn default() -> Self {
        Self {
            x_offset: -500.0,
            y_offset: -500.0,
            zoom: 0.5,
            visible: Rect::NOTHING,
        }
    }
}

impl Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;

    fn zoom_x(&self) -> f32 {
        self.zoom
    }
    fn zoom_y(&self) -> f32 {
        self.zoom
    }
    fn set_zoom(&mut self, x: f32, _: f32) {
        self.zoom = x.clamp(0.00001, 20.0);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, x: f64, y: f64) {
        self.x_offset = x;
        self.y_offset = y;
    }
    fn visible_rect(&self) -> Rect {
        self.visible
    }
    fn clamp_zoom(&self, x: f32, _: f32) -> (f32, f32) {
        let z = x.clamp(0.00001, 20.0);
        (z, z)
    }
}

impl GraphNavigation {
    fn screen(&self, p: [f64; 2]) -> Pos2 {
        self.xy_to_screen_pos(p[0], p[1])
    }
    fn fixed_screen(&self, p: XyPos) -> Pos2 {
        let p = XyPosF64::from(p);
        self.screen([p.x, p.y])
    }
    fn coordinate(&self, p: Pos2) -> LonLat {
        let (x, y) = self.screen_pos_to_xy(p);
        Wgs84LonLat::from(XyPosF64::new(x, y)).into()
    }
}

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn title(&self) -> WidgetText {
        egui_i18n::tr!("tab-graph").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                main_display(self, app, ui);
            });
    }
}

fn main_display(tab: &mut GraphTab, app: &mut App, ui: &mut Ui) {}
