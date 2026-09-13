//! Geographic network editor backed exclusively by Source commands and spatial caches.
use std::sync::Arc;

use egui::{Color32, Frame, Pos2, Rect, Sense, Stroke, Ui, WidgetText};
use paiagram_core::spatial::project;
use paiagram_core::*;
use serde::{Deserialize, Serialize};

use super::{Navigatable, Tab};
use crate::App;
mod inspector;
mod underlay;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub(crate) struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip)]
    underlay: Arc<egui::mutex::Mutex<underlay::UnderlayPainter>>,
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
        let mut is_open = self.panel_is_open || ui.memory(|mem| mem.everything_is_visible());
        self.panel_is_open = is_open;
        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(0))
            .show(ui, |ui| self.map(app, ui));
    }
}

impl GraphTab {
    fn fit(&mut self, world: &WorldSnapshot) {
        let mut points = world
            .stations
            .iter()
            .map(|s| project(*s.pos))
            .chain(world.nodes.iter().map(|n| project(*n.pos)))
            .chain(
                world
                    .intervals
                    .iter()
                    .flat_map(|(_, interval)| interval.nodes.iter().copied().map(project)),
            );
        let Some(first) = points.next() else {
            return;
        };
        let (mut min, mut max) = (first, first);
        for point in points {
            for i in 0..2 {
                min[i] = min[i].min(point[i]);
                max[i] = max[i].max(point[i]);
            }
        }
        let width = self.navi.visible.width().max(100.0) as f64;
        let height = self.navi.visible.height().max(100.0) as f64;
        self.navi.zoom = ((width - 70.0) / (max[0] - min[0]).max(500.0))
            .min((height - 70.0) / (max[1] - min[1]).max(500.0))
            .clamp(0.00001, 20.0) as f32;
        self.navi.x_offset = (min[0] + max[0]) / 2.0 - width / (2.0 * self.navi.zoom as f64);
        self.navi.y_offset = (min[1] + max[1]) / 2.0 - height / (2.0 * self.navi.zoom as f64);
    }

    fn map(&mut self, app: &mut App, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Fit network").clicked() {
                self.fit(app.snap());
            }
        });
        let (response, mut painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        self.navi.visible = response.rect;
        self.navi.handle_navigation(ui, &response);
        let attribution = {
            let mut underlay = self.underlay.lock();
            underlay.update_tile_type(Some(self.underlay_tile_type));
            underlay.draw_underlay(&mut painter, &self.navi, ui)
        };
        let pad = 20.0 / self.navi.zoom as f64;
        let xr = self.navi.visible_x();
        let yr = self.navi.visible_y();
        let min = [xr.start - pad, yr.start - pad];
        let max = [xr.end + pad, yr.end + pad];
        let (geo_min, geo_max) = paiagram_core::spatial::geographic_bounds(min, max);
        let (xy_min, xy_max) = paiagram_core::spatial::projected_bounds(min, max);
        for edge in app.source.graph_cache().intervals(geo_min, geo_max) {
            painter.line(
                edge.projected
                    .iter()
                    .map(|(_, pos)| {
                        let XyPosF64 { x, y } = (*pos).into();
                        self.navi.xy_to_screen_pos(x, y)
                    })
                    .collect(),
                Stroke::new(1.0, Color32::BLACK),
            );
        }
        for station in app.source.graph_cache().stations(geo_min, geo_max) {
            let [x, y] = project(station.point);
            let pos = self.navi.xy_to_screen_pos(x, y);
            painter.circle_filled(pos, 1.0, Color32::BLACK);
        }
        for node in app.source.graph_cache().nodes(geo_min, geo_max) {}
        for (sample, time) in app.source.graph_cache().trips(
            xy_min,
            xy_max,
            app.timer.ticks().as_seconds_f64(),
            app.settings.repeat_frequency.0 as f64,
        ) {}
    }
}
