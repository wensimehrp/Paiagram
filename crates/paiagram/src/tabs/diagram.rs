use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use egui::{
    Align2, Button, Color32, FontId, Id, Margin, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    WidgetText,
};
use egui_i18n::tr;
use ecow::EcoVec;
use paiagram_core::trip::{TEntry, TravelMode, TripSchedule};
use paiagram_core::units::time::{Tick, TimetableTime};
use paiagram_core::{Command, RouteKey, TripKey};
use serde::{Deserialize, Serialize};

use super::{AppState, Navigatable, Tab};
use crate::widgets::TimeDragValue;
use crate::ExtendingTripSelection;

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
    fn edit_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        ui.checkbox(&mut self.use_global_timer, tr!("diagram-use-global-timer"));
        match app.selected_items.clone() {
            crate::SelectedItems::None | crate::SelectedItems::Coordinate(_) => {
                ui.strong(tr!("menu-new-trip"));
                ui.label(tr!("diagram-create-new-trip-scratch"));
                if ui.button(tr!("diagram-create-new-trip")).clicked() {
                    let key = TripKey::new();
                    app.source.apply_command(paiagram_core::Command::AddTrip {
                        key,
                        view: paiagram_core::TripView {
                            name: "New Trip".into(),
                            schedule: paiagram_core::TripSchedule::new(Default::default()),
                            class: None,
                        },
                    });
                    app.selected_items = crate::SelectedItems::ExtendingTrip(
                        crate::ExtendingTripSelection {
                            trip: key,
                            previous_pos: None,
                            current_entry: None,
                            last_time: None,
                        },
                    );
                }
            }
            crate::SelectedItems::ExtendingTrip(ref mut sel) => {
                if let Some(view) = app.source.trips.get_view(sel.trip) {
                    let mut name = view.name.to_string();
                    ui.text_edit_singleline(&mut name);
                    if name != view.name.as_str() {
                        app.source.apply_command(paiagram_core::Command::RenameTrip {
                            key: sel.trip,
                            name: name.into(),
                        });
                    }
                }
                if ui.button(tr!("diagram-complete")).clicked() {
                    app.selected_items = crate::SelectedItems::None;
                }
            }
            _ => {}
        }
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

    // Handle ExtendingTrip: mouse interaction to add entries
    if let crate::SelectedItems::ExtendingTrip(ref mut sel) = app.selected_items {
        if response.contains_pointer() {
            if let Some(hover_pos) = ui.input(|r| r.pointer.hover_pos()) {
                // Find closest station
                let cand_y = tab.navi.screen_y_to_logical_y(hover_pos.y) as f32;
                let idx = station_heights.partition_point(|(_, y)| *y < cand_y);
                let (cand_stn, cand_h, cand_idx) = if idx == 0 {
                    station_heights.first().map(|(e, h)| (*e, *h, 0)).unwrap()
                } else if idx >= station_heights.len() {
                    station_heights.last().map(|(e, h)| (*e, *h, station_heights.len() - 1)).unwrap()
                } else {
                    let (prev_e, prev_y) = station_heights[idx - 1];
                    let (curr_e, curr_y) = station_heights[idx];
                    if cand_y > (prev_y + curr_y) / 2.0 {
                        (curr_e, curr_y, idx)
                    } else {
                        (prev_e, prev_y, idx - 1)
                    }
                };
                let cand_t = tab.navi.screen_x_to_logical_x(hover_pos.x).to_timetable_time();
                let screen_stn_y = tab.navi.logical_y_to_screen_y(cand_h as f64);
                let display_pos = Pos2::new(hover_pos.x, screen_stn_y);

                // Draw crosshair
                let cross_stroke = Stroke::new(1.0, Color32::RED);
                painter.hline(response.rect.x_range(), display_pos.y, cross_stroke);
                painter.vline(display_pos.x, response.rect.y_range(), cross_stroke);

                // Draw station name
                let stn_name = app.source.stations.query(cand_stn, |b| b.name.clone()).unwrap_or_default();
                painter.text(display_pos, Align2::RIGHT_BOTTOM, stn_name.as_str(), FontId::default(), ui.visuals().text_color());
                painter.text(display_pos, Align2::RIGHT_TOP, cand_t.to_string(), FontId::default(), ui.visuals().text_color());

                // Draw line from previous entry to current
                if let Some((prev_tick, prev_idx)) = sel.previous_pos {
                    if let Some((_, prev_h)) = station_heights.get(prev_idx).copied() {
                        let prev_pos = tab.navi.xy_to_screen_pos(prev_tick, prev_h as f64);
                        painter.line_segment([prev_pos, display_pos], cross_stroke);
                        if prev_pos.distance(display_pos) > 50.0 {
                            let mid = prev_pos.lerp(display_pos, 0.5);
                            let prev_tt = prev_tick.to_timetable_time();
                            painter.text(mid, Align2::CENTER_BOTTOM, (cand_t - prev_tt).to_string(), FontId::default(), ui.visuals().text_color());
                        }
                        painter.circle_filled(prev_pos, 3.0, Color32::RED);
                    }
                }

                // On click: add entry
                if response.clicked() {
                    let click_tick = tab.navi.screen_x_to_logical_x(hover_pos.x);
                    sel.previous_pos = Some((click_tick, cand_idx));
                    sel.last_time = Some(cand_t);
                    static ENTRY_ID: AtomicU32 = AtomicU32::new(1);
                    let id = ENTRY_ID.fetch_add(1, Ordering::Relaxed);
                    if let Some(view) = app.source.trips.get_view(sel.trip) {
                        let mut entries: Vec<TEntry> = view.schedule.entries().to_vec();
                        entries.push(TEntry::Pinned {
                            stn: cand_stn,
                            trk: 0,
                            arr: TravelMode::At(cand_t),
                            dep: TravelMode::At(cand_t),
                            id,
                        });
                        app.source.apply_command(Command::ChangeTripEntries {
                            key: sel.trip,
                            entries: entries.into(),
                        });
                    }
                }
            }
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
    // Build list of (station_key, time, station_height) for all At entries
    struct SegPt { stn: paiagram_core::StationKey, t: TimetableTime, y: f32 }
    let points: Vec<SegPt> = view.schedule.entries().iter().filter_map(|entry| {
        let (sk, t) = match entry {
            TEntry::Pinned { stn, arr: TravelMode::At(a), .. } => (*stn, *a),
            TEntry::PinnedNonStop { stn, pass: TravelMode::At(t), .. } => (*stn, *t),
            _ => return None,
        };
        let y = station_map.get(&sk).copied()?;
        Some(SegPt { stn: sk, t, y })
    }).collect();

    // Draw segments between consecutive points
    for pair in points.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let ta = Tick::from_timetable_time(a.t).normalized_with(repeat_interval_ticks);
        let tb = Tick::from_timetable_time(b.t).normalized_with(repeat_interval_ticks);
        let ya = navi.logical_y_to_screen_y(a.y as f64);
        let yb = navi.logical_y_to_screen_y(b.y as f64);
        let xa = navi.logical_x_to_screen_x(ta);
        let xb = navi.logical_x_to_screen_x(tb);
        let mut c1 = Pos2::new(xa, ya);
        let mut c2 = Pos2::new(xb, yb);
        class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.x);
        class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.y);
        class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.x);
        class_stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.y);
        painter.line_segment([c1, c2], class_stroke);
        painter.circle_filled(c1, 3.0, class_stroke.color);
    }
    // Draw handle for the last point
    if let Some(last) = points.last() {
        let t = Tick::from_timetable_time(last.t).normalized_with(repeat_interval_ticks);
        let x = navi.logical_x_to_screen_x(t);
        let y = navi.logical_y_to_screen_y(last.y as f64);
        painter.circle_filled(Pos2::new(x, y), 3.0, class_stroke.color);
    }
}