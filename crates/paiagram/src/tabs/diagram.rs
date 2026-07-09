use std::sync::Arc;

use egui::{
    Align2, Button, Color32, FontId, Id, Margin, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    WidgetText,
};
use egui_i18n::tr;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Tick, TimetableTime};
use paiagram_core::{RouteKey, TripKey};
use serde::{Deserialize, Serialize};

use super::{AppState, Navigatable, Tab};
use crate::widgets::TimeDragValue;

mod draw_lines;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct DiagramTabNavigation {
    pub x_offset: Tick,
    pub y_offset: f64,
    pub zoom: Vec2,
    pub visible_rect: Rect,
    pub max_height: f32,
}

impl Default for DiagramTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: Tick(0),
            y_offset: 0.0,
            zoom: Vec2::new(0.005, 10.0),
            visible_rect: Rect::NOTHING,
            max_height: 0.0,
        }
    }
}

impl Navigatable for DiagramTabNavigation {
    type XOffset = Tick;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 { self.zoom.x }
    fn zoom_y(&self) -> f32 { self.zoom.y }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) { self.zoom = Vec2::new(zoom_x, zoom_y); }
    fn offset_x(&self) -> f64 { self.x_offset.0 as f64 }
    fn offset_y(&self) -> f64 { self.y_offset }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = Tick(offset_x.round() as i64);
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect { self.visible_rect }
    fn x_per_screen_unit(&self) -> Self::XOffset {
        Tick((1.0 / self.zoom_x().max(f32::EPSILON) as f64) as i64)
    }
    fn visible_x(&self) -> std::ops::Range<Self::XOffset> {
        let width = self.visible_rect().width() as f64;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let start = self.x_offset;
        let end = Tick(start.0 + (width * ticks_per_screen_unit).ceil() as i64);
        start..end
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let height = self.visible_rect.height() as f64;
        let start = self.offset_y();
        let end = start + height / self.zoom_y().max(f32::EPSILON) as f64;
        start..end
    }
    fn y_per_screen_unit(&self) -> Self::YOffset { 1.0 / self.zoom_y().max(f32::EPSILON) as f64 }
    fn allow_axis_zoom(&self) -> bool { true }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x.clamp(0.00005, 0.4), zoom_y.clamp(0.1, 2048.0))
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        let max_tick = Tick::from_timetable_time(TimetableTime(366 * 86400)).0;
        self.x_offset = Tick(self.x_offset.0.clamp(
            -max_tick,
            max_tick - (response.rect.width() as f64 / self.zoom.x as f64) as i64,
        ));
        const TOP_BOTTOM_PADDING: f32 = 30.0;
        self.y_offset = if response.rect.height() / self.zoom.y
            > (self.max_height + TOP_BOTTOM_PADDING * 2.0 / self.zoom.y)
        {
            ((-response.rect.height() / self.zoom.y + self.max_height) / 2.0) as f64
        } else {
            self.y_offset.clamp(
                (-TOP_BOTTOM_PADDING / self.zoom.y) as f64,
                (self.max_height - response.rect.height() / self.zoom.y
                    + TOP_BOTTOM_PADDING / self.zoom.y) as f64,
            )
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DiagramTab {
    navi: DiagramTabNavigation,
    route_key: RouteKey,
    #[serde(skip)]
    use_global_timer: bool,
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_key == other.route_key
    }
}

impl DiagramTab {
    pub(crate) fn new(route_key: RouteKey) -> Self {
        Self {
            navi: DiagramTabNavigation::default(),
            route_key,
            use_global_timer: false,
        }
    }
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn title(&self) -> WidgetText { tr!("tab-diagram").into() }
    fn id(&self) -> Id { Id::new(self.route_key) }
    fn scroll_bars(&self) -> [bool; 2] { [false; 2] }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                main_display(self, app, ui)
            });
    }
    fn edit_display(&mut self, _app: &mut AppState, ui: &mut Ui) {
        ui.checkbox(&mut self.use_global_timer, tr!("diagram-use-global-timer"));
        ui.label(tr!("side-panel-edit-fallback-1"));
        ui.label(tr!("side-panel-edit-fallback-2"));
    }
}

fn main_display(tab: &mut DiagramTab, app: &mut AppState, ui: &mut egui::Ui) {
    let route_view = app.source.routes.get_view(tab.route_key);
    let Some(ref route) = route_view else {
        ui.label("Route not found");
        return;
    };

    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

    tab.navi.visible_rect = response.rect;
    if tab.use_global_timer {
        tab.navi.x_offset = app.timer.read_ticks();
    }
    let moved = tab.navi.handle_navigation(ui, &response);
    if tab.use_global_timer && moved {
        app.timer.write_ticks(tab.navi.x_offset);
        app.timer.lock();
    } else {
        app.timer.unlock();
    }

    // Build station list with heights
    let mut station_heights: Vec<(paiagram_core::StationKey, f32)> = Vec::new();
    for (i, sk) in route.stations.iter().enumerate() {
        station_heights.push((*sk, i as f32 * 50.0));
    }
    if station_heights.is_empty() { return; }
    tab.navi.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);

    // Draw station lines
    draw_lines::draw_station_lines(&mut painter, &tab.navi, station_heights.iter().copied(), ui.visuals(), app);
    draw_lines::draw_time_lines(&mut painter, &tab.navi);

    // Draw trip lines
    let repeat_interval_ticks = app.project_settings.repeat_frequency.to_ticks();
    for tk in app.source.trips.keys() {
        if let Some(view) = app.source.trips.get_view(*tk) {
            draw_trip_on_diagram(&painter, &tab.navi, app, *tk, &view, &station_heights, repeat_interval_ticks);
        }
    }

    // Draw time indicator
    let ticks = app.timer.read_ticks();
    let mut time_indicator_x = tab.navi.logical_x_to_screen_x(ticks);
    let time_indicator_stroke = Stroke::new(1.5, Color32::RED);
    time_indicator_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut time_indicator_x);
    crate::widgets::indicators::display_time_indicator_indicator_horizontal(
        ui.id().with("time indicator"),
        ui.clip_rect(),
        time_indicator_x,
        time_indicator_stroke.color,
        &painter,
    );
    painter.vline(time_indicator_x, response.rect.top()..=response.rect.bottom(), time_indicator_stroke);
}

fn draw_trip_on_diagram(
    painter: &egui::Painter,
    navi: &DiagramTabNavigation,
    app: &AppState,
    _tk: TripKey,
    view: &paiagram_core::TripView,
    station_heights: &[(paiagram_core::StationKey, f32)],
    repeat_interval_ticks: Tick,
) {
    let class_stroke = view.class
        .and_then(|ck| app.source.classes.get_view(ck))
        .map(|cv| egui::Stroke::new(cv.style.width as f32, cv.style.color))
        .unwrap_or(egui::Stroke::new(1.0, Color32::GRAY));

    let station_map: std::collections::HashMap<paiagram_core::StationKey, f32> =
        station_heights.iter().copied().collect();

    // Draw each entry segment
    let entries = view.schedule.entries();
    for (i, entry) in entries.iter().enumerate() {
        let (sk, arr_time, dep_time) = match entry {
            TEntry::Pinned { stn, arr: TravelMode::At(a), dep: TravelMode::At(d), .. } => (*stn, Some(*a), *d),
            TEntry::Pinned { stn, dep: TravelMode::At(d), .. } => (*stn, None, *d),
            TEntry::PinnedNonStop { stn, pass: TravelMode::At(t), .. } => (*stn, None, *t),
            TEntry::Derived(s) => (*s, None, TimetableTime(0)),
            _ => continue,
        };

        let Some(&station_y) = station_map.get(&sk) else { continue; };

        // Get times from next entry as well
        let next_arr = entries.get(i + 1).and_then(|next| match next {
            TEntry::Pinned { arr: TravelMode::At(a), .. } => Some(*a),
            TEntry::Pinned { dep: TravelMode::At(d), .. } => Some(*d),
            _ => None,
        });

        if let (Some(arr), Some(next_a)) = (arr_time, next_arr) {
            let visible_x = navi.visible_x();
            let norm_arr = Tick::from_timetable_time(arr).normalized_with(repeat_interval_ticks);
            let norm_next = Tick::from_timetable_time(next_a).normalized_with(repeat_interval_ticks);
            let y1 = navi.logical_y_to_screen_y(station_y as f64);

            // Find next station
            let next_station_y = station_heights.get(i + 1).map(|(_, y)| *y).unwrap_or(station_y);
            let y2 = navi.logical_y_to_screen_y(next_station_y as f64);

            let x1 = navi.logical_x_to_screen_x(norm_arr);
            let x2 = navi.logical_x_to_screen_x(norm_next);

            let mut c1 = Pos2::new(x1, y1);
            let mut c2 = Pos2::new(x2, y2);
            class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.x);
            class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.y);
            class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.x);
            class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.y);
            painter.line_segment([c1, c2], class_stroke);

            // Draw departure handle
            let dep_x = navi.logical_x_to_screen_x(norm_arr);
            let dep_pos = Pos2::new(dep_x, y1);
            painter.circle_filled(dep_pos, 3.0, class_stroke.color);
        }
    }
}