use std::sync::Arc;

use egui::{
    Button, Color32, Id, Margin, NumExt, Painter, Pos2, Rect, RectAlign, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2, WidgetText, vec2,
};
use egui_i18n::tr;
use itertools::Itertools;
use paiagram_core::trip::TravelMode;
use paiagram_core::units::time::{Duration, Tick, TimetableTime};
use paiagram_core::{Command, Key, RouteHandle, RouteKey, StationKey, TripKey, TripKeyHashMap};
use paiagram_raptor::Journey;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use vec1::Vec1;

use super::{Navigatable, Tab};
use crate::tabs::station::StationTab;
use crate::widgets::indicators::display_time_indicator_indicator_horizontal;
use crate::widgets::timetable_popup::{POPUP_WIDTH, arrival_popup, departure_popup};
use crate::widgets::{TimeDragValue, buttons};
use crate::{
    App, ModifySelectedItems, SelectedItem, SelectedItems,
    StationPairSelection, StationSelection, TripSelection,
};
mod draw_lines;
mod gpu_draw;
pub(crate) mod prep_segments;

/// The state of the canvas
#[derive(Default)]
#[non_exhaustive]
pub(crate) enum CanvasState {
    /// User is doing nothing
    #[default]
    Idle,
    /// User is doing something in another panel.
    IdleNoInterrupt,
    /// User is selecting some trips
    SelectingTrips,
    /// User is selecting some intervals
    SelectingStationPairs,
    /// User is selecting some stations
    SelectingStations,
    /// User is extending a trip
    ExtendingTrip,
}

type TripCache = TripKeyHashMap<SmallVec<[Vec1<TripPoint>; 1]>>;

/// The diagram tab.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DiagramTab {
    navi: DiagramTabNavigation,
    #[serde(skip, default)]
    last_secondary_click_position: Option<(Tick, f64)>,
    route: RouteKey,
    use_global_timer: bool,
    #[serde(skip, default)]
    cached_trips: Option<TripCache>,
    #[serde(skip, default)]
    raptor_params: RaptorParams,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuTripRendererState>>,
}

#[derive(Clone, Default)]
pub(crate) struct RaptorParams {
    departure_time: TimetableTime,
    start_stop: Option<StationKey>,
    end_stop: Option<StationKey>,
    result: Vec<Journey<TripKey, StationKey>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DiagramTabNavigation {
    pub(crate) x_offset: Tick,
    pub(crate) y_offset: f64,
    pub(crate) zoom: Vec2,
    #[serde(skip, default = "default_visible_rect")]
    pub(crate) visible_rect: Rect,
    pub(crate) max_height: f32,
}

impl Default for DiagramTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: Tick(0),
            y_offset: 0.0,
            zoom: vec2(0.005, 10.0),
            visible_rect: Rect::NOTHING,
            max_height: 0.0,
        }
    }
}

fn default_visible_rect() -> Rect {
    Rect::NOTHING
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl DiagramTab {
    pub(crate) fn new(route: RouteKey) -> Self {
        Self {
            navi: DiagramTabNavigation::default(),
            last_secondary_click_position: None,
            route,
            use_global_timer: false,
            cached_trips: None,
            raptor_params: RaptorParams::default(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuTripRendererState::default(),
            )),
        }
    }
}

impl Navigatable for DiagramTabNavigation {
    type XOffset = paiagram_core::units::time::Tick;
    type YOffset = f64;

    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom = Vec2::new(zoom_x, zoom_y);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset.0 as f64
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = Tick(offset_x.round() as i64);
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
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
    fn y_per_screen_unit(&self) -> Self::YOffset {
        1.0 / self.zoom_y().max(f32::EPSILON) as f64
    }
    fn allow_axis_zoom(&self) -> bool {
        true
    }
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct TripPoint {
    arr: TimetableTime,
    dep: TimetableTime,
    station_index: usize,
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn title(&self) -> WidgetText {
        tr!("tab-diagram").into()
    }
    fn id(&self) -> Id {
        Id::new(self.route.to_bits())
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        let Some(handle) = app.routes.get_handle(self.route) else {
            ui.label("No route?");
            return;
        };
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| main_display(self, app, ui, handle));
    }
}

fn main_display(tab: &mut DiagramTab, app: &mut App, ui: &mut egui::Ui, handle: RouteHandle) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

    // Timer shifting logic
    tab.navi.visible_rect = response.rect;
    if tab.use_global_timer {
        tab.navi.x_offset = app.timer.read_ticks();
    }
    let moved = tab.navi.handle_navigation(ui, &response);
    if tab.use_global_timer {
        app.timer.write_ticks(tab.navi.x_offset);
    }
    if moved && tab.use_global_timer {
        app.timer.try_lock(tab.route.to_bits());
    } else {
        app.timer.try_unlock(tab.route.to_bits());
    }

    // Prepare the station info
    let stations = app.routes.get_stations(handle);
    let station_heights: Vec<(StationKey, f32)> = stations
        .iter()
        .copied()
        .enumerate()
        .map(|(i, sk)| (sk, i as f32 * 20.0))
        .collect();
    if station_heights.is_empty() {
        return;
    }
    tab.navi.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);

    // Get station names for drawing
    let station_names: Vec<String> = station_heights
        .iter()
        .map(|(sk, _)| {
            app.stations
                .get_handle(*sk)
                .map(|h| app.stations.get_name(h).to_string())
                .unwrap_or_else(|| "<Unknown>".to_string())
        })
        .collect();

    // Draw the horizontal station lines
    draw_lines::draw_station_lines(
        &mut painter,
        &tab.navi,
        station_names.iter().cloned().zip(station_heights.iter().map(|(_, h)| *h)),
        ui.visuals(),
    );

    // Draw the vertical time lines
    draw_lines::draw_time_lines(&mut painter, &tab.navi);

    // Calculate the visible trains
    let cached_trips_are_changed = prep_segments::calc(
        tab.route,
        &station_heights,
        &mut tab.cached_trips,
        app,
    );

    // Prepare GPU drawing
    let mut state = tab.gpu_state.lock();
    if let Some(target_format) = ui.ctx().data(|data| {
        data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(Id::new("wgpu_target_format"))
    }) {
        state.target_format = Some(target_format);
    }
    if let Some(msaa_samples) = ui
        .ctx()
        .data(|data| data.get_temp::<u32>(Id::new("wgpu_msaa_samples")))
    {
        state.msaa_samples = msaa_samples;
    }

    state.antialiasing_mode = app.preferences.antialiasing_mode;
    let repeat_frequency = app.project_settings.repeat_frequency;
    let visible_x = tab.navi.visible_x();
    let visible_span_seconds =
        (visible_x.end.to_timetable_time() - visible_x.start.to_timetable_time()).0;
    state.level_of_detail_mode = match app.preferences.level_of_detail_mode {
        paiagram_core::settings::LevelOfDetailMode::Off => {
            paiagram_core::settings::LevelOfDetailMode::Off
        }
        paiagram_core::settings::LevelOfDetailMode::Lod2 => {
            if visible_span_seconds >= 86400 / 2 {
                paiagram_core::settings::LevelOfDetailMode::Lod2
            } else {
                paiagram_core::settings::LevelOfDetailMode::Off
            }
        }
        paiagram_core::settings::LevelOfDetailMode::Lod4 => {
            if visible_span_seconds >= 86400 {
                paiagram_core::settings::LevelOfDetailMode::Lod4
            } else if visible_span_seconds >= 86400 / 2 {
                paiagram_core::settings::LevelOfDetailMode::Lod2
            } else {
                paiagram_core::settings::LevelOfDetailMode::Off
            }
        }
    };

    let cached_trips = tab.cached_trips.as_ref().unwrap();

    gpu_draw::upload_trip_strokes(
        cached_trips.iter().filter_map(|(trip_key, _)| {
            let trip_view = app.trips.get_view(*trip_key)?;
            let class_key = trip_view.class?;
            let class_view = app.classes.get_view(class_key)?;
            let color = class_view.style.color;
            let [r, g, b, _] = color.to_array();
            Some((class_key, class_view.style.width as f32, [r, g, b]))
        }),
        &mut state,
    );

    if cached_trips_are_changed {
        gpu_draw::rewrite_trip_cache(
            cached_trips,
            station_heights.iter().map(|(_, y)| *y),
            &|trip_key: TripKey| {
                let trip_view = app.trips.get_view(trip_key)?;
                let class_key = trip_view.class?;
                let class_view = app.classes.get_view(class_key)?;
                let color = class_view.style.color;
                let [r, g, b, _] = color.to_array();
                Some((class_key, class_view.style.width as f32, [r, g, b]))
            },
            &mut state,
        );
    }

    state.uniforms = gpu_draw::Uniforms {
        ticks_min: tab.navi.x_offset.0,
        y_min: tab.navi.y_offset,
        screen_size: [0.0, 0.0],
        x_per_unit: tab.navi.x_per_screen_unit_f64() as f32,
        y_per_unit: tab.navi.y_per_screen_unit_f64() as f32,
        screen_origin: [tab.navi.visible_rect.left(), tab.navi.visible_rect.top()],
        repeat_interval_ticks: Tick::from_timetable_time(TimetableTime(repeat_frequency.0))
            .0
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
    };

    let r = tab.navi.visible_x();
    state.visible_secs_min = r
        .start
        .normalized_with(repeat_frequency.to_ticks())
        .to_timetable_time();
    state.visible_secs_max = r
        .end
        .normalized_with(repeat_frequency.to_ticks())
        .to_timetable_time();

    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);

    // Clone selection data to avoid borrow conflicts with app access
    let trip_selections: Vec<TripSelection> = match &app.selected_items {
        SelectedItems::Trips(s) => s.to_vec(),
        _ => vec![],
    };
    let station_selections: Vec<StationSelection> = match &app.selected_items {
        SelectedItems::Stations(s) => s.to_vec(),
        _ => vec![],
    };
    let is_idle_no_interrupt = matches!(app.selected_items, SelectedItems::ExtendingRoute(_));
    let is_extending_trip = matches!(app.selected_items, SelectedItems::ExtendingTrip(_));

    // Determine current selection state (non-borrowing)
    let canvas_state = match &app.selected_items {
        SelectedItems::None | SelectedItems::Coordinate(_) => CanvasState::Idle,
        SelectedItems::Trips(_) => CanvasState::SelectingTrips,
        SelectedItems::StationPairs(_) => CanvasState::SelectingStationPairs,
        SelectedItems::Stations(_) => CanvasState::SelectingStations,
        SelectedItems::ExtendingRoute(_) => CanvasState::IdleNoInterrupt,
        SelectedItems::ExtendingTrip(_) => CanvasState::ExtendingTrip,
    };

    let selection_strength = ui.ctx().animate_bool(
        ui.id().with("selection"),
        match &canvas_state {
            CanvasState::Idle => false,
            CanvasState::ExtendingTrip => false,
            CanvasState::SelectingTrips => {
                trip_selections.iter().any(|it| cached_trips.contains_key(&it.trip))
            }
            _ => true,
        },
    );

    let s = (selection_strength * 0.5 * u8::MAX as f32) as u8;
    painter.rect_filled(
        response.rect,
        0,
        if ui.visuals().dark_mode {
            Color32::from_black_alpha(s)
        } else {
            Color32::from_white_alpha(s)
        },
    );

    let interact_pos = response
        .clicked()
        .then(|| ui.input(|it| it.pointer.interact_pos()))
        .flatten();
    let repeat_interval_ticks = Tick::from_timetable_time(TimetableTime(repeat_frequency.0));

    let get_closest_station = |selected_y: f32| -> (StationKey, f32, usize) {
        let idx = station_heights.partition_point(|(_, y)| *y < selected_y);
        let (e, h) = if idx == 0 {
            station_heights.first().copied().unwrap()
        } else if idx >= station_heights.len() {
            station_heights.last().copied().unwrap()
        } else {
            let (prev_e, prev_y) = station_heights[idx - 1];
            let (curr_e, curr_y) = station_heights[idx];
            if selected_y > (prev_y + curr_y) / 2.0 {
                return (curr_e, curr_y, idx);
            } else {
                return (prev_e, prev_y, idx - 1);
            }
        };
        (e, h, idx)
    };

    // Use the closures in popups that need to know the idle-no-interrupt state
    let is_idle_no_interrupt_popup = is_idle_no_interrupt;
    let is_extending_trip_state = is_extending_trip;

    match canvas_state {
        CanvasState::Idle if let Some(pos) = interact_pos => {
            if let Some(selection) = select_trip(
                cached_trips,
                pos,
                &station_heights,
                &tab.navi,
                repeat_interval_ticks,
            ) {
                app.modify_selected_items(ModifySelectedItems::SetSingle(SelectedItem::Trip(
                    selection,
                )));
            }
            tab.last_secondary_click_position = None;
        }
        CanvasState::IdleNoInterrupt if interact_pos.is_some() => {
            tab.last_secondary_click_position = None;
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt
            if response.secondary_clicked()
                && let Some(pos) = ui.input(|it| it.pointer.interact_pos()) =>
        {
            tab.last_secondary_click_position = Some(tab.navi.screen_pos_to_xy(pos));
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt
            if let Some((x, y)) = tab.last_secondary_click_position =>
        {
            let (closest_station, station_y, _) = get_closest_station(y as f32);
            let station_y = tab.navi.logical_y_to_screen_y(station_y as f64);
            let screen_pos = tab.navi.xy_to_screen_pos(x, y);

            painter.line_segment(
                [screen_pos, Pos2::new(screen_pos.x, station_y)],
                Stroke::new(1.0, Color32::RED),
            );
            painter.circle_filled(screen_pos, 3.0, Color32::RED);

            let rect = Rect::from_pos(screen_pos).expand(6.0);
            let res = ui
                .allocate_rect(rect, Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if res.dragged() {
                ui.set_cursor_icon(egui::CursorIcon::Grabbing);
                let new_pos = screen_pos + res.drag_delta();
                tab.last_secondary_click_position = Some(tab.navi.screen_pos_to_xy(new_pos));
            }

            egui::Popup::menu(&res).open(true).show(|ui| {
                ui.set_min_width(POPUP_WIDTH);
                let station_name = app
                    .stations
                    .get_handle(closest_station)
                    .map(|h| app.stations.get_name(h).to_string())
                    .unwrap_or_else(|| "<Unknown>".to_string());
                if ui.add(Button::new(station_name).truncate()).clicked() {
                    tab.last_secondary_click_position = None;
                    app.open_or_focus(crate::MainTab::Station(StationTab::new(
                        closest_station,
                    )));
                }

                let mut new_time = x.to_timetable_time();
                if ui.add(TimeDragValue(&mut new_time)).changed() {
                    tab.last_secondary_click_position =
                        Some((Tick::from_timetable_time(new_time), y))
                }

                if is_idle_no_interrupt_popup {
                    ui.label(tr!("diagram-already-editing"));
                } else if ui.button(tr!("menu-new-trip")).clicked() {
                    tab.last_secondary_click_position = None;
                    // TODO: create a new trip at this position
                }
            });
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt => {}
        CanvasState::SelectingTrips => {
            let visible_ticks = tab.navi.visible_x();
            for selection_entry in &trip_selections {
                if let Some(segments) = cached_trips.get(&selection_entry.trip) {
                    for segment in segments.iter() {
                        let mut seg_min = i64::MAX;
                        let mut seg_max = i64::MIN;
                        for it in segment {
                            let arr_tick = it.arr.to_ticks().0;
                            let dep_tick = it.dep.to_ticks().0;
                            seg_min = seg_min.min(arr_tick.min(dep_tick));
                            seg_max = seg_max.max(arr_tick.max(dep_tick));
                        }

                        if seg_min > seg_max {
                            continue;
                        }

                        let (repeat_start, repeat_end) = if repeat_interval_ticks.0 > 0 {
                            (
                                (visible_ticks.start.0 - seg_max)
                                    .div_euclid(repeat_interval_ticks.0),
                                (visible_ticks.end.0 - seg_min)
                                    .div_euclid(repeat_interval_ticks.0),
                            )
                        } else {
                            (0, 0)
                        };

                        // Get class style for stroke
                        let stroke_info = app.trips.get_view(selection_entry.trip).and_then(|tv| {
                            tv.class.and_then(|ck| {
                                app.classes.get_view(ck).map(|cv| {
                                    let stroke_color = cv.style.color;
                                    let width = cv.style.width as f32;
                                    (stroke_color, width)
                                })
                            })
                        });
                        let (stroke_color, stroke_width) =
                            stroke_info.unwrap_or((Color32::GRAY, 2.0));
                        let mut stroke = Stroke::new(
                            stroke_width + stroke_width * 3.0 * selection_strength,
                            stroke_color,
                        );

                        let mut base_points = Vec::with_capacity(segment.len() * 4);
                        for it in segment {
                            let y = tab.navi.logical_y_to_screen_y(
                                station_heights[it.station_index].1 as f64,
                            );
                            let arr_x = tab.navi.logical_x_to_screen_x(it.arr.to_ticks());
                            let dep_x = tab.navi.logical_x_to_screen_x(it.dep.to_ticks());
                            base_points.push([
                                Pos2::new(arr_x, y),
                                Pos2::new(arr_x, y),
                                Pos2::new(dep_x, y),
                                Pos2::new(dep_x, y),
                            ]);
                        }

                        for repeat in repeat_start..=repeat_end {
                            let repeat_offset = repeat * repeat_interval_ticks.0;
                            let offset_x = tab.navi.logical_x_to_screen_x(Tick(repeat_offset))
                                - tab.navi.logical_x_to_screen_x(Tick::ZERO);
                            let offset = Vec2::new(offset_x, 0.0);
                            let points: Vec<[Pos2; 4]> = base_points
                                .iter()
                                .map(|p| {
                                    [
                                        p[0] + offset,
                                        p[1] + offset,
                                        p[2] + offset,
                                        p[3] + offset,
                                    ]
                                })
                                .collect();
                            painter.line(points.iter().flat_map(|it| *it).collect(), stroke);
                        }

                        // Draw entry handles
                        for (seg_idx, segment_entry) in segment.iter().enumerate() {
                            let y = tab.navi.logical_y_to_screen_y(
                                station_heights[segment_entry.station_index].1 as f64,
                            );
                            let arr_x =
                                tab.navi.logical_x_to_screen_x(segment_entry.arr.to_ticks());
                            let dep_x =
                                tab.navi.logical_x_to_screen_x(segment_entry.dep.to_ticks());
                            let pos_arr = Pos2::new(arr_x, y);
                            let pos_dep = Pos2::new(dep_x, y);
                            let curr = [pos_arr, pos_arr, pos_dep, pos_dep];

                            draw_handles(
                                &curr,
                                selection_entry.trip,
                                seg_idx,
                                &tab.navi,
                                &mut painter,
                                ui,
                                app,
                            );
                        }
                    }
                }
            }

            if let Some(pos) = interact_pos {
                match select_trip(
                    cached_trips,
                    pos,
                    &station_heights,
                    &tab.navi,
                    repeat_interval_ticks,
                ) {
                    Some(s) if ui.input(|r| r.modifiers.command) => {
                        app.modify_selected_items(ModifySelectedItems::Toggle(
                            SelectedItem::Trip(s),
                        ));
                    }
                    None if ui.input(|r| r.modifiers.command) => {}
                    _ => {
                        app.modify_selected_items(ModifySelectedItems::Clear);
                    }
                }
            }
        }
        CanvasState::SelectingStationPairs => {}
        CanvasState::SelectingStations => {
            for station in &station_selections {
                for (sk, height) in station_heights
                    .iter()
                    .copied()
                    .filter(|(e, _)| *e == station.station)
                {
                    let y = tab.navi.logical_y_to_screen_y(height as f64);
                    painter.rect(
                        Rect::from_x_y_ranges(response.rect.x_range(), (y - 5.0)..=(y + 5.0)),
                        0,
                        Color32::RED.gamma_multiply(0.5),
                        Stroke::new(1.0, Color32::RED),
                        StrokeKind::Inside,
                    );
                }
            }
        }
        CanvasState::ExtendingTrip
            if response.contains_pointer()
                && let Some(hover_pos) = ui.input(|r| r.pointer.hover_pos()) =>
        {
            let (_cand_stn, cand_h, cand_idx) =
                get_closest_station(tab.navi.screen_y_to_logical_y(hover_pos.y) as f32);
            let dt = ui.input(|input| input.stable_dt).at_most(0.1);
            let new_y = tab.navi.logical_y_to_screen_y(cand_h as f64);
            let cand_t = tab
                .navi
                .screen_x_to_logical_x(hover_pos.x)
                .to_timetable_time();
            let curr_y = ui.data_mut(|r| {
                let smoothed = r.get_temp_mut_or(ui.id().with("selection"), new_y);
                let t = egui::emath::exponential_smooth_factor(0.9, 0.03, dt);
                *smoothed = egui::emath::lerp((*smoothed)..=new_y, t);
                *smoothed
            });
            if (curr_y - new_y).abs() >= 0.05 {
                ui.request_repaint();
            }
            painter.hline(
                response.rect.x_range(),
                curr_y,
                Stroke::new(1.0, Color32::RED),
            );
            painter.vline(
                hover_pos.x,
                response.rect.y_range(),
                Stroke::new(1.0, Color32::RED),
            );
            // Read previous_pos from app.selected_items directly
            let previous_pos: Option<(TimetableTime, usize)> = match &app.selected_items {
                SelectedItems::ExtendingTrip(s) => s.previous_pos,
                _ => None,
            };
            if let Some((previous_time, previous_station_index)) = previous_pos
                && let Some((_, prev_h)) = station_heights.get(previous_station_index).copied()
            {
                let t = previous_time.to_ticks();
                let pos = tab.navi.xy_to_screen_pos(t, prev_h as f64);
                painter.line_segment(
                    [pos, Pos2::new(hover_pos.x, curr_y)],
                    Stroke::new(1.0, Color32::RED),
                );
            }

            if response.clicked() {
                if let SelectedItems::ExtendingTrip(sel) = &mut app.selected_items {
                    sel.previous_pos = Some((cand_t, cand_idx));
                }
                // TODO: add entry to trip via command
            }
        }
        CanvasState::ExtendingTrip => {}
    }

    // Draw time indicator
    let ticks = app.timer.read_ticks();
    let time_indicator_stroke = Stroke::new(1.5, Color32::RED);
    let mut time_indicator_x = tab.navi.logical_x_to_screen_x(ticks);
    time_indicator_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut time_indicator_x);

    display_time_indicator_indicator_horizontal(
        ui.id().with("time indicator"),
        ui.clip_rect(),
        time_indicator_x,
        time_indicator_stroke.color,
        &painter,
    );
    painter.vline(
        time_indicator_x,
        response.rect.top()..=response.rect.bottom(),
        time_indicator_stroke,
    );
}

fn select_trip(
    cache: &TripCache,
    pos: Pos2,
    station_heights: &[(StationKey, f32)],
    navi: &DiagramTabNavigation,
    normalize_cycle: Tick,
) -> Option<TripSelection> {
    cache.iter().find_map(|(trip_entity, segments)| {
        let entry = select_trip_inner(segments, pos, station_heights, navi, normalize_cycle)?;
        Some(TripSelection { trip: *trip_entity })
    })
}

fn select_trip_inner(
    segments: &[Vec1<TripPoint>],
    mut pos: Pos2,
    station_heights: &[(StationKey, f32)],
    navi: &DiagramTabNavigation,
    normalize_cycle: Tick,
) -> Option<()> {
    pos.x = navi.logical_x_to_screen_x(
        navi.screen_x_to_logical_x(pos.x)
            .normalized_with(normalize_cycle),
    );

    const TRIP_SELECTION_RADIUS: f32 = 7.0;
    for segment in segments {
        let points_iter = segment.iter().map(|it| {
            let station_y =
                navi.logical_y_to_screen_y(station_heights[it.station_index].1 as f64);
            let arr_x =
                navi.logical_x_to_screen_x(it.arr.to_ticks().normalized_with(normalize_cycle));
            let dep_x =
                navi.logical_x_to_screen_x(it.dep.to_ticks().normalized_with(normalize_cycle));
            [
                Pos2::new(arr_x, station_y),
                Pos2::new(arr_x, station_y),
                Pos2::new(dep_x, station_y),
                Pos2::new(dep_x, station_y),
            ]
        });
        let last = points_iter.clone().last().into_iter().flat_map(|it| {
            let [a, b, c, d] = it;
            [[a, b], [b, c], [c, d]]
        });
        for [curr, next] in points_iter
            .tuple_windows()
            .flat_map(|([a1, a2, a3, a4], [b, ..])| {
                let mid = a4.lerp(b, 0.5);
                [[a1, a2], [a2, a3], [a3, a4], [a4, mid], [mid, b]]
            })
            .chain(last)
        {
            let a = pos.x - curr.x;
            let b = pos.y - curr.y;
            let c = next.x - curr.x;
            let d = next.y - curr.y;
            let dot = a * c + b * d;
            let len_sq = c * c + d * d;
            if len_sq == 0.0 {
                continue;
            }
            let t = (dot / len_sq).clamp(0.0, 1.0);
            let px = curr.x + t * c;
            let py = curr.y + t * d;
            let dx = pos.x - px;
            let dy = pos.y - py;

            if dx * dx + dy * dy < TRIP_SELECTION_RADIUS.powi(2) {
                return Some(());
            }
        }
    }
    None
}

fn draw_handles(
    p: &[Pos2; 4],
    trip_key: TripKey,
    entry_idx: usize,
    navi: &DiagramTabNavigation,
    painter: &mut Painter,
    ui: &mut Ui,
    app: &mut App,
) {
    let Some(trip_view) = app.trips.get_view(trip_key) else {
        return;
    };
    let entries = trip_view.schedule.entries();
    let Some(entry) = entries.get(entry_idx) else {
        return;
    };

    let strength = 1.0;

    const HANDLE_SIZE: f32 = 15.0;
    const CIRCLE_HANDLE_SIZE: f32 = 7.0 / 12.0 * HANDLE_SIZE;
    const TRIANGLE_HANDLE_SIZE: f32 = 10.0 / 12.0 * HANDLE_SIZE;
    const DASH_HANDLE_SIZE: f32 = 9.0 / 12.0 * HANDLE_SIZE;

    let mut arrival_pos = p[1];
    let departure_pos: Pos2;
    if (p[1].x - p[2].x).abs() < HANDLE_SIZE {
        let midpoint_x = (p[1].x + p[2].x) / 2.0;
        arrival_pos.x = midpoint_x - HANDLE_SIZE / 2.0;
        let mut pos = p[2];
        pos.x = midpoint_x + HANDLE_SIZE / 2.0;
        departure_pos = pos;
    } else {
        departure_pos = p[2];
    }

    let handle_stroke = egui::Stroke {
        width: 2.5,
        color: Color32::BLACK.linear_multiply(strength),
    };

    let arrival_rect = Rect::from_center_size(arrival_pos, Vec2::splat(HANDLE_SIZE));
    let arrival_id = ui.id().with((trip_key, "arr", entry_idx));
    let arrival_response = ui.interact(arrival_rect, arrival_id, Sense::click_and_drag());

    let popup_alignment = RectAlign::BOTTOM_START;

    // Show departure popup on the arrival handle too (traditional behavior)
    match entry {
        paiagram_core::trip::TEntry::Pinned { arr, dep, .. } => {
            // Arrival side
            let arrival_fill = if arrival_response.hovered() {
                Color32::GRAY
            } else {
                Color32::WHITE
            }
            .linear_multiply(strength);
            match arr {
                TravelMode::At(_) => buttons::circle_button_shape(
                    painter,
                    arrival_pos,
                    CIRCLE_HANDLE_SIZE,
                    handle_stroke,
                    arrival_fill,
                ),
                TravelMode::For(_) => buttons::dash_button_shape(
                    painter,
                    arrival_pos,
                    DASH_HANDLE_SIZE,
                    handle_stroke,
                    arrival_fill,
                ),
                TravelMode::Flexible => buttons::triangle_button_shape(
                    painter,
                    arrival_pos,
                    TRIANGLE_HANDLE_SIZE,
                    handle_stroke,
                    arrival_fill,
                ),
            }

            if arrival_response.drag_started() {
                // TODO: handle drag for time adjustment
            }
            if let Some(total_drag_delta) = arrival_response.total_drag_delta() {
                if navi.zoom_x() > f32::EPSILON {
                    let delta_ticks = Tick(
                        (total_drag_delta.x as f64 / navi.zoom_x() as f64) as i64,
                    );
                    let duration = Duration(delta_ticks.to_timetable_time().0);
                    if duration != Duration(0) {
                        // Apply arrival time shift via command
                        let schedule = trip_view.schedule.clone();
                        let mut new_entries: Vec<_> = schedule.entries().to_vec();
                        if let paiagram_core::trip::TEntry::Pinned {
                            arr, ..
                        } = &mut new_entries[entry_idx]
                        {
                            if let TravelMode::At(t) = arr {
                                let new_t =
                                    TimetableTime((t.0 + duration.0).max(0));
                                *arr = TravelMode::At(new_t);
                            }
                        }
                        app.source.apply_command(Command::ChangeTripEntries {
                            key: trip_key,
                            entries: new_entries.into(),
                        });
                    }
                }
            }

            // Departure side
            let dep_sense = match dep {
                TravelMode::Flexible => Sense::click(),
                _ => Sense::click_and_drag(),
            };
            let departure_rect = Rect::from_center_size(departure_pos, Vec2::splat(HANDLE_SIZE));
            let departure_id = ui.id().with((trip_key, "dep", entry_idx));
            let departure_response = ui.interact(departure_rect, departure_id, dep_sense);
            let departure_fill = if departure_response.hovered() {
                Color32::GRAY
            } else {
                Color32::WHITE
            }
            .linear_multiply(strength);
            match dep {
                TravelMode::At(_) => buttons::circle_button_shape(
                    painter,
                    departure_pos,
                    CIRCLE_HANDLE_SIZE,
                    handle_stroke,
                    departure_fill,
                ),
                TravelMode::For(_) => buttons::dash_button_shape(
                    painter,
                    departure_pos,
                    DASH_HANDLE_SIZE,
                    handle_stroke,
                    departure_fill,
                ),
                TravelMode::Flexible => buttons::triangle_button_shape(
                    painter,
                    departure_pos,
                    TRIANGLE_HANDLE_SIZE,
                    handle_stroke,
                    departure_fill,
                ),
            }

            if departure_response.drag_started() {
                // TODO: handle departure drag
            }
            if let Some(total_drag_delta) = departure_response.total_drag_delta() {
                if navi.zoom_x() > f32::EPSILON {
                    let delta_ticks = Tick(
                        (total_drag_delta.x as f64 / navi.zoom_x() as f64) as i64,
                    );
                    let duration = Duration(delta_ticks.to_timetable_time().0);
                    if duration != Duration(0) {
                        let schedule = trip_view.schedule.clone();
                        let mut new_entries: Vec<_> = schedule.entries().to_vec();
                        if let paiagram_core::trip::TEntry::Pinned {
                            dep, ..
                        } = &mut new_entries[entry_idx]
                        {
                            if let TravelMode::At(t) = dep {
                                let new_t =
                                    TimetableTime((t.0 + duration.0).max(0));
                                *dep = TravelMode::At(new_t);
                            }
                        }
                        app.source.apply_command(Command::ChangeTripEntries {
                            key: trip_key,
                            entries: new_entries.into(),
                        });
                    }
                }
            }

            // Show popups via new timetable_popup API
            arrival_popup(
                app,
                &arrival_response,
                trip_key,
                entry_idx,
                popup_alignment,
            );
            departure_popup(
                app,
                &departure_response,
                trip_key,
                entry_idx,
                popup_alignment,
            );
        }
        _ => {}
    }
}
