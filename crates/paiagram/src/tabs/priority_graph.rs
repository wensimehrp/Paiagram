use egui::{Align2, Margin, Painter, Pos2, Rect, Sense, Stroke, Vec2, Visuals, WidgetText, pos2};
use egui_i18n::tr;
use paiagram_core::colors::DisplayedColor;
use paiagram_core::units::time::TimetableTime;
use serde::{Deserialize, Serialize};

use crate::App;
use super::Navigatable;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PriorityGraphTab {
    route: paiagram_core::RouteKey,
    navi: PriorityTabNavigation,
    #[serde(skip, default)]
    downward_priorities: Option<Vec<Vec<(paiagram_core::TripKey, TimetableTime)>>>,
}

impl PriorityGraphTab {
    pub(crate) fn new(route: paiagram_core::RouteKey) -> Self {
        Self {
            route,
            navi: PriorityTabNavigation::default(),
            downward_priorities: None,
        }
    }
}

impl PartialEq for PriorityGraphTab {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct PriorityTabNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: Vec2,
    visible_rect: Rect,
}

impl Default for PriorityTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: Vec2::splat(1.0),
            visible_rect: Rect::ZERO,
        }
    }
}

impl super::Navigatable for PriorityTabNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = offset_x;
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom.x = zoom_x;
        self.zoom.y = zoom_y;
    }
}

impl super::Tab for PriorityGraphTab {
    const NAME: &'static str = "Priority Graph";
    fn title(&self) -> WidgetText {
        tr!("tab-priority-graph").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        calculate_priority(self, app);
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| main_display(self, app, ui));
    }
}

fn calculate_priority(tab: &mut PriorityGraphTab, app: &mut App) {
    let Some(route_handle) = app.routes.get_handle(tab.route) else {
        return;
    };
    let route = app.routes.get_view(tab.route).unwrap();
    if let Some(ref priorities) = tab.downward_priorities {
        // Check if route changed (simplified: always recalc for now)
    }
    let priorities = tab.downward_priorities.get_or_insert_with(Vec::new);
    priorities.clear();

    let stops = &route.stations;
    for stop in stops.iter().copied() {
        let mut times = Vec::new();
        // Find all trips that visit this station
        for trip_key in app.trips.keys() {
            if let Some(schedule) = app.trips.query(*trip_key, |b| b.schedule.clone()) {
                for entry in schedule.entries() {
                    if let paiagram_core::trip::TEntry::Pinned { stn, dep, .. } = entry {
                        if *stn == stop {
                            if let paiagram_core::trip::TravelMode::At(t) = dep {
                                times.push((*trip_key, *t));
                            }
                        }
                    }
                }
            }
        }
        times.sort_unstable_by_key(|(_, t)| *t);
        priorities.push(times);
    }
}

const STATION_SPACING: f64 = 10.0;

fn main_display(tab: &mut PriorityGraphTab, app: &mut App, ui: &mut egui::Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible_rect = response.rect;
    tab.navi.handle_navigation(ui, &response);

    draw_station_lines(&mut painter, tab, app, ui.visuals());
    draw_priority_lines(&mut painter, tab);
}

fn draw_station_lines(
    painter: &mut Painter,
    tab: &PriorityGraphTab,
    app: &App,
    visuals: &Visuals,
) {
    let Some(route_handle) = app.routes.get_handle(tab.route) else {
        return;
    };
    let route = app.routes.get_view(tab.route).unwrap();
    let stroke = Stroke {
        width: 0.6,
        color: visuals.window_stroke().color,
    };
    let text_color = visuals.text_color();
    for (idx, station_key) in route.stations.iter().copied().enumerate() {
        let name = app
            .stations
            .get_view(station_key)
            .map(|v| v.name.to_string())
            .unwrap_or_else(|| "<Unknown>".to_string());
        let pos = idx as f64 * STATION_SPACING;
        let pos = tab.navi.xy_to_screen_pos(pos, 0.0).x;
        painter.vline(pos, tab.navi.visible_rect.y_range(), stroke);
        let galley = painter.layout_no_wrap(name, egui::FontId::proportional(13.0), text_color);
        let text_shape = egui::epaint::TextShape::new(
            pos2(pos, tab.navi.visible_rect.top()),
            galley,
            text_color,
        )
        .with_angle_and_anchor(std::f32::consts::FRAC_PI_4, Align2::LEFT_BOTTOM);
        painter.add(text_shape);
    }
}

fn draw_priority_lines(painter: &mut Painter, tab: &PriorityGraphTab) {
    let stroke = Stroke {
        width: 2.0,
        color: DisplayedColor::Predefined(paiagram_core::colors::PredefinedColor::Amber).into_color32(false),
    };
    let mut line_map: std::collections::HashMap<paiagram_core::TripKey, Vec<Pos2>> =
        std::collections::HashMap::new();
    if let Some(ref maps) = tab.downward_priorities {
        for (station_idx, map) in maps.iter().enumerate() {
            let x = station_idx as f64 * STATION_SPACING;
            for (priority, (trip_key, _)) in map.iter().enumerate() {
                let pos = tab.navi.xy_to_screen_pos(x, priority as f64 * STATION_SPACING);
                line_map
                    .entry(*trip_key)
                    .or_insert_with(Vec::new)
                    .push(pos);
            }
        }
    }
    for points in line_map.values() {
        painter.line(points.clone(), stroke);
    }
}
