use std::sync::atomic::{AtomicU32, Ordering};

use egui::{
    Align2, Color32, FontId, Id, Margin, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2, WidgetText,
};
use egui_i18n::tr;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, Tick, TimetableTime};
use paiagram_core::{Command, RouteKey, StationKey, TripKey};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use vec1::Vec1;

use super::{AppState, Navigatable, Tab};
use crate::widgets::TimeDragValue;
use crate::{ExtendingTripSelection, SelectedItem, SelectedItems, TripSelection};

mod draw_lines;

#[derive(Clone, Copy, Debug)]
struct TripPoint {
    arr: TimetableTime,
    dep: TimetableTime,
    entry_idx: usize,
    station_idx: usize,
}

type TripCache = std::collections::HashMap<TripKey, SmallVec<[Vec1<TripPoint>; 1]>>;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct DiagramTabNavigation {
    pub(crate) x_offset: Tick,
    pub(crate) y_offset: f64,
    pub(crate) zoom: Vec2,
    #[serde(skip, default = "default_rect")]
    pub(crate) visible_rect: Rect,
    pub(crate) max_height: f32,
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
        let tsu = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let start = self.x_offset;
        let end = Tick(start.0 + (width * tsu).ceil() as i64);
        start..end
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let h = self.visible_rect.height() as f64;
        let s = self.offset_y();
        s..(s + h / self.zoom_y().max(f32::EPSILON) as f64)
    }
    fn y_per_screen_unit(&self) -> Self::YOffset { 1.0 / self.zoom_y().max(f32::EPSILON) as f64 }
    fn allow_axis_zoom(&self) -> bool { true }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x.clamp(0.00005, 0.4), zoom_y.clamp(0.1, 2048.0))
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        let mt = Tick::from_timetable_time(TimetableTime(366 * 86400)).0;
        self.x_offset = Tick(self.x_offset.0.clamp(-mt, mt - (response.rect.width() as f64 / self.zoom.x as f64) as i64));
        const TP: f32 = 30.0;
        self.y_offset = if response.rect.height() / self.zoom.y > (self.max_height + TP * 2.0 / self.zoom.y) {
            ((-response.rect.height() / self.zoom.y + self.max_height) / 2.0) as f64
        } else {
            self.y_offset.clamp((-TP / self.zoom.y) as f64, (self.max_height - response.rect.height() / self.zoom.y + TP / self.zoom.y) as f64)
        };
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DiagramTab {
    navi: DiagramTabNavigation,
    route_key: RouteKey,
    #[serde(skip)]
    use_global_timer: bool,
    #[serde(skip)]
    trip_cache: Option<TripCache>,
    #[serde(skip)]
    selected_entry: Option<(TripKey, usize)>,
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool { self.route_key == other.route_key }
}

impl DiagramTab {
    pub(crate) fn new(route_key: RouteKey) -> Self {
        Self {
            navi: DiagramTabNavigation::default(),
            route_key,
            use_global_timer: false,
            trip_cache: None,
            selected_entry: None,
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
            .show(ui, |ui| main_display(self, app, ui));
    }
    fn edit_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        ui.checkbox(&mut self.use_global_timer, tr!("diagram-use-global-timer"));
        match app.selected_items.clone() {
            SelectedItems::None | SelectedItems::Coordinate(_) => {
                ui.strong(tr!("menu-new-trip"));
                ui.label(tr!("diagram-create-new-trip-scratch"));
                if ui.button(tr!("diagram-create-new-trip")).clicked() {
                    let key = TripKey::new();
                    app.source.apply_command(Command::AddTrip {
                        key,
                        view: paiagram_core::TripView {
                            name: "New Trip".into(),
                            schedule: paiagram_core::TripSchedule::new(Default::default()),
                            class: None,
                        },
                    });
                    app.selected_items = SelectedItems::ExtendingTrip(ExtendingTripSelection {
                        trip: key, previous_pos: None, current_entry: None, last_time: None,
                    });
                }
            }
            SelectedItems::ExtendingTrip(ref mut sel) => {
                if let Some(view) = app.source.trips.get_view(sel.trip) {
                    let mut name = view.name.to_string();
                    ui.text_edit_singleline(&mut name);
                    if name != view.name.as_str() {
                        app.source.apply_command(Command::RenameTrip { key: sel.trip, name: name.into() });
                    }
                }
                if ui.button(tr!("diagram-complete")).clicked() {
                    app.selected_items = SelectedItems::None;
                }
            }
            _ => {}
        }
    }
}

fn default_rect() -> egui::Rect { egui::Rect::NOTHING }

// ── Build trip segments cache ──

fn build_cache(tab: &mut DiagramTab, app: &AppState, _station_list: &[(StationKey, f32)]) -> bool {
    let _ = _station_list;
    let route_view = app.source.routes.get_view(tab.route_key);
    let Some(route) = route_view else { return false; };

    // Map station key -> route index
    let stn_to_idx: std::collections::HashMap<_, _> = route.stations.iter().enumerate()
        .map(|(i, sk)| (*sk, i)).collect();

    let old_cache = tab.trip_cache.take();
    let mut cache: TripCache = std::collections::HashMap::new();

    for tk in app.source.trips.keys() {
        let Some(view) = app.source.trips.get_view(*tk) else { continue; };
        let entries = view.schedule.entries();
        if entries.len() < 2 { continue; }

        // Collect entry data with station indices and estimates
        let mut entry_data: Vec<(usize, TimetableTime, TimetableTime)> = Vec::new();
        for (ei, e) in entries.iter().enumerate() {
            let (sk, arr, dep) = match e {
                TEntry::Pinned { stn, arr: TravelMode::At(a), dep: TravelMode::At(d), .. } => (*stn, *a, *d),
                TEntry::Pinned { stn, dep: TravelMode::At(d), .. } => (*stn, *d, *d),
                _ => continue,
            };
            let Some(&si) = stn_to_idx.get(&sk) else { continue; };
            entry_data.push((si, arr, dep));
        }
        if entry_data.len() < 2 { continue; }

        // Build segments: for each consecutive entry pair, draw a segment
        // connecting their station positions.
        let mut segments: SmallVec<[Vec1<TripPoint>; 1]> = SmallVec::new();
        let mut current: Vec<TripPoint> = Vec::new();
        for w in entry_data.windows(2) {
            let (si_a, arr_a, dep_a) = w[0];
            let (si_b, arr_b, dep_b) = w[1];
            // Start new segment if gap
            if !current.is_empty() && current.last().unwrap().station_idx.abs_diff(si_a) > 1 {
                if current.len() >= 2 {
                    segments.push(Vec1::try_from_vec(std::mem::take(&mut current)).unwrap());
                } else {
                    current.clear();
                }
            }
            if current.is_empty() {
                current.push(TripPoint { arr: arr_a, dep: dep_a, entry_idx: 0, station_idx: si_a });
            }
            current.push(TripPoint { arr: arr_b, dep: dep_b, entry_idx: 0, station_idx: si_b });
        }
        if current.len() >= 2 {
            if let Ok(v) = Vec1::try_from_vec(current) { segments.push(v); }
        }
        if !segments.is_empty() {
            cache.insert(*tk, segments);
        }
    }
    tab.trip_cache = Some(cache);
    true
}

// ── Trip selection ──

fn select_trip(
    cache: &TripCache,
    pos: Pos2,
    station_heights: &[(StationKey, f32)],
    navi: &DiagramTabNavigation,
    norm_cycle: Tick,
) -> Option<TripSelection> {
    for (tk, segments) in cache {
        if let Some(entry_idx) = select_trip_in_segments(segments, pos, station_heights, navi, norm_cycle) {
            return Some(TripSelection { trip: *tk, entries: vec1::vec1![entry_idx] });
        }
    }
    None
}

fn select_trip_in_segments(
    segments: &[Vec1<TripPoint>],
    mut pos: Pos2,
    station_heights: &[(StationKey, f32)],
    navi: &DiagramTabNavigation,
    norm_cycle: Tick,
) -> Option<usize> {
    pos.x = navi.logical_x_to_screen_x(navi.screen_x_to_logical_x(pos.x).normalized_with(norm_cycle));
    const RAD: f32 = 7.0;
    for seg in segments {
        // Build line points between consecutive entries
        let seg_pts: Vec<(Pos2, Pos2, usize)> = seg.windows(2).map(|w| {
            let a = w[0]; let b = w[1];
            let ya = navi.logical_y_to_screen_y(station_heights[a.station_idx].1 as f64);
            let yb = navi.logical_y_to_screen_y(station_heights[b.station_idx].1 as f64);
            let xa = navi.logical_x_to_screen_x(a.arr.to_ticks().normalized_with(norm_cycle));
            let xb = navi.logical_x_to_screen_x(b.arr.to_ticks().normalized_with(norm_cycle));
            (Pos2::new(xa, ya), Pos2::new(xb, yb), a.entry_idx)
        }).collect();

        for (c1, c2, _entry_idx) in &seg_pts {
            let a = pos.x - c1.x; let b = pos.y - c1.y;
            let c = c2.x - c1.x; let d = c2.y - c1.y;
            let dot = a * c + b * d;
            let len_sq = c * c + d * d;
            if len_sq == 0.0 { continue; }
            let t = (dot / len_sq).clamp(0.0, 1.0);
            let px = c1.x + t * c; let py = c1.y + t * d;
            if (pos.x - px).powi(2) + (pos.y - py).powi(2) < RAD.powi(2) {
                return Some(0);
            }
        }
    }
    None
}

// ── Draw trip segments ──

fn draw_trip_segments(
    painter: &Painter, navi: &DiagramTabNavigation, app: &AppState,
    cache: &TripCache, station_heights: &[(StationKey, f32)], norm_cycle: Tick,
    selected_trips: Option<&[TripSelection]>,
    selection_strength: f32,
) {
    for (tk, segments) in cache {
        let class_stroke = app.source.trips.get_view(*tk)
            .and_then(|v| v.class)
            .and_then(|ck| app.source.classes.get_view(ck))
            .map(|cv| Stroke::new(cv.style.width as f32, cv.style.color))
            .unwrap_or(Stroke::new(1.0, Color32::GRAY));

        let mut stroke = class_stroke;
        let is_selected = selected_trips.map_or(false, |s| s.iter().any(|t| t.trip == *tk));
        if is_selected {
            stroke.width = stroke.width + stroke.width * 3.0 * selection_strength;
        }

        for seg in segments {
            if seg.len() < 2 { continue; }
            // Draw polyline between consecutive points in this segment
            for pair in seg.windows(2) {
                let a = &pair[0];
                let b = &pair[1];
                let ya = navi.logical_y_to_screen_y(station_heights[a.station_idx].1 as f64);
                let yb = navi.logical_y_to_screen_y(station_heights[b.station_idx].1 as f64);
                let xa = navi.logical_x_to_screen_x(a.arr.to_ticks().normalized_with(norm_cycle));
                let xb = navi.logical_x_to_screen_x(b.arr.to_ticks().normalized_with(norm_cycle));
                let mut c1 = Pos2::new(xa, ya);
                let mut c2 = Pos2::new(xb, yb);
                stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.x);
                stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c1.y);
                stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.x);
                stroke.round_center_to_pixel(painter.pixels_per_point(), &mut c2.y);
                painter.line_segment([c1, c2], stroke);
            }
            // Draw circles at each point
            for pt in seg.iter() {
                let y = navi.logical_y_to_screen_y(station_heights[pt.station_idx].1 as f64);
                let x = navi.logical_x_to_screen_x(pt.arr.to_ticks().normalized_with(norm_cycle));
                let pos = Pos2::new(x, y);
                painter.circle_filled(pos, 3.0, stroke.color);
            }
        }
    }
}

// ── Handle interaction (dragging to adjust times) ──

fn handle_entry_interaction(
    ui: &mut Ui, painter: &Painter, app: &mut AppState, navi: &DiagramTabNavigation,
    response: &egui::Response, station_heights: &[(StationKey, f32)], norm_cycle: Tick,
    selected_trips: Option<&[TripSelection]>,
) {
    let Some(trips) = selected_trips else { return; };
    if trips.is_empty() { return; }

    for sel in trips {
        let Some(view) = app.source.trips.get_view(sel.trip) else { continue; };
        let entries = view.schedule.entries();
        for (ei, e) in entries.iter().enumerate() {
            let (sk, arr) = match e {
                TEntry::Pinned { stn, arr: TravelMode::At(a), .. } => (*stn, *a),
                _ => continue,
            };
            let Some(si) = station_heights.iter().position(|(s, _)| *s == sk) else { continue; };
            let y = navi.logical_y_to_screen_y(station_heights[si].1 as f64);
            let x = navi.logical_x_to_screen_x(arr.to_ticks().normalized_with(norm_cycle));
            let handle_pos = Pos2::new(x, y);
            let handle_rect = Rect::from_center_size(handle_pos, Vec2::splat(15.0));

            // Check hover/click
            let handle_id = ui.id().with(("diag_handle", sel.trip, ei));
            let resp = ui.interact(handle_rect, handle_id, Sense::click_and_drag());

            // Draw handle
            let is_hovered = resp.hovered() || resp.dragged();
            painter.circle_filled(handle_pos, if is_hovered { 6.0 } else { 4.0 },
                if is_hovered { Color32::RED } else { Color32::WHITE });
            painter.circle_stroke(handle_pos, if is_hovered { 6.0 } else { 4.0 },
                Stroke::new(2.0, Color32::BLACK));

            if resp.dragged() {
                let delta = resp.drag_delta();
                if navi.zoom_x() > f32::EPSILON {
                    let delta_ticks = (delta.x as f64 / navi.zoom_x() as f64) as i64;
                    let dur = Duration((delta_ticks / 100) as i32);
                    if dur != Duration(0) {
                        let mut entries_vec: Vec<TEntry> = entries.to_vec();
                        let modified = match &entries_vec[ei] {
                            TEntry::Pinned { stn, trk, dep, id, .. } => {
                                TEntry::Pinned {
                                    stn: *stn, trk: *trk,
                                    arr: TravelMode::At(arr + dur),
                                    dep: *dep, id: *id,
                                }
                            }
                            _ => continue,
                        };
                        entries_vec[ei] = modified;
                        app.source.apply_command(Command::ChangeTripEntries {
                            key: sel.trip,
                            entries: entries_vec.into(),
                        });
                    }
                }
            }
            // Tooltip
            if resp.hovered() {
                let stn_name = app.source.stations.query(sk, |b| b.name.clone()).unwrap_or_default();
                resp.on_hover_text(format!("{} @ {}", arr, stn_name));
            }
        }
    }
}

// ── Main display ──

fn main_display(tab: &mut DiagramTab, app: &mut AppState, ui: &mut egui::Ui) {
    let route_view = app.source.routes.get_view(tab.route_key);
    let Some(route) = route_view else { ui.label("Route not found"); return; };

    let (response, mut painter) = ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

    tab.navi.visible_rect = response.rect;
    if tab.use_global_timer {
        tab.navi.x_offset = app.timer.read_ticks();
    }
    let moved = tab.navi.handle_navigation(ui, &response);
    if tab.use_global_timer && moved {
        app.timer.write_ticks(tab.navi.x_offset);
        app.timer.lock();
    } else { app.timer.unlock(); }

    // Build station heights
    let mut station_heights: Vec<(StationKey, f32)> = Vec::new();
    for (i, sk) in route.stations.iter().enumerate() {
        station_heights.push((*sk, i as f32 * 50.0));
    }
    if station_heights.is_empty() { return; }
    tab.navi.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);

    // Draw grids
    draw_lines::draw_station_lines(&mut painter, &tab.navi, station_heights.iter().copied(), ui.visuals(), app);
    draw_lines::draw_time_lines(&mut painter, &tab.navi);

    // Build/update trip cache every frame (simple approach)
    build_cache(tab, app, &station_heights);
    let cache = tab.trip_cache.as_ref().unwrap();
    let norm_cycle = app.project_settings.repeat_frequency.to_ticks();

    // Selection strength
    let selection_strength = ui.ctx().animate_bool(ui.id().with("sel"),
        matches!(app.selected_items, SelectedItems::Trips(_)));

    // Draw trip segments
    {
        let selected_trips = match &app.selected_items {
            SelectedItems::Trips(t) => Some(t.as_slice()),
            _ => None,
        };
        draw_trip_segments(&painter, &tab.navi, app, cache, &station_heights, norm_cycle, selected_trips, selection_strength);
    }

    // Handle interaction for selected trips (handle dragging)
    {
        let selected_trips: Vec<TripSelection> = match &app.selected_items {
            SelectedItems::Trips(t) => t.clone().into(),
            _ => Vec::new(),
        };
        let trips: &[TripSelection] = &selected_trips;
        handle_entry_interaction(ui, &painter, app, &tab.navi, &response, &station_heights, norm_cycle, if trips.is_empty() { None } else { Some(trips) });
    }

    // Handle click to select a trip
    let interact_pos = response.clicked().then(|| ui.input(|r| r.pointer.interact_pos())).flatten();
    if let Some(pos) = interact_pos {
        if let Some(sel) = select_trip(cache, pos, &station_heights, &tab.navi, norm_cycle) {
            let ctrl = ui.input(|r| r.modifiers.command);
            if ctrl { app.selected_items.toggle_selection(SelectedItem::Trip(sel)); }
            else { app.selected_items.set_single_selection(SelectedItem::Trip(sel)); }
        } else {
            // Click on empty space: if not extending trip, clear or create coordinate
            if !matches!(app.selected_items, SelectedItems::ExtendingTrip(_)) {
                app.selected_items = SelectedItems::None;
            }
        }
    }

    // Extending trip interaction
    if let SelectedItems::ExtendingTrip(ref mut sel) = app.selected_items {
        if response.contains_pointer() {
            if let Some(hover_pos) = ui.input(|r| r.pointer.hover_pos()) {
                let cand_y = tab.navi.screen_y_to_logical_y(hover_pos.y) as f32;
                let idx = station_heights.partition_point(|(_, y)| *y < cand_y);
                let (cand_stn, cand_h, cand_idx) = if idx == 0 {
                    station_heights.first().map(|(e, h)| (*e, *h, 0)).unwrap()
                } else if idx >= station_heights.len() {
                    station_heights.last().map(|(e, h)| (*e, *h, station_heights.len() - 1)).unwrap()
                } else {
                    let (pe, py) = station_heights[idx - 1];
                    let (ce, cy) = station_heights[idx];
                    if cand_y > (py + cy) / 2.0 { (ce, cy, idx) } else { (pe, py, idx - 1) }
                };
                let cand_t = tab.navi.screen_x_to_logical_x(hover_pos.x).to_timetable_time();
                let screen_y = tab.navi.logical_y_to_screen_y(cand_h as f64);
                let cross = Stroke::new(1.0, Color32::RED);
                painter.hline(response.rect.x_range(), screen_y, cross);
                painter.vline(hover_pos.x, response.rect.y_range(), cross);
                let stn_name = app.source.stations.query(cand_stn, |b| b.name.clone()).unwrap_or_default();
                painter.text(Pos2::new(hover_pos.x, screen_y), Align2::RIGHT_BOTTOM, stn_name.as_str(), FontId::default(), ui.visuals().text_color());
                painter.text(Pos2::new(hover_pos.x, screen_y), Align2::RIGHT_TOP, cand_t.to_string(), FontId::default(), ui.visuals().text_color());

                if let Some((prev_tt, prev_si)) = sel.previous_pos {
                    if let Some((_, ph)) = station_heights.get(prev_si).copied() {
                        let ppos = tab.navi.xy_to_screen_pos(prev_tt, ph as f64);
                        painter.line_segment([ppos, Pos2::new(hover_pos.x, screen_y)], cross);
                        painter.circle_filled(ppos, 3.0, Color32::RED);
                    }
                }

                if response.clicked() {
                    let tick = tab.navi.screen_x_to_logical_x(hover_pos.x);
                    sel.previous_pos = Some((tick, cand_idx));
                    static EID: AtomicU32 = AtomicU32::new(1);
                    let id = EID.fetch_add(1, Ordering::Relaxed);
                    if let Some(view) = app.source.trips.get_view(sel.trip) {
                        let mut ev: Vec<TEntry> = view.schedule.entries().to_vec();
                        ev.push(TEntry::Pinned { stn: cand_stn, trk: 0, arr: TravelMode::At(cand_t), dep: TravelMode::At(cand_t), id });
                        app.source.apply_command(Command::ChangeTripEntries { key: sel.trip, entries: ev.into() });
                    }
                }
            }
        }
    }

    // Draw time indicator
    let ticks = app.timer.read_ticks();
    let mut tx = tab.navi.logical_x_to_screen_x(ticks);
    let tis = Stroke::new(1.5, Color32::RED);
    tis.round_center_to_pixel(ui.pixels_per_point(), &mut tx);
    crate::widgets::indicators::display_time_indicator_indicator_horizontal(
        ui.id().with("time_indicator"), ui.clip_rect(), tx, tis.color, &painter);
    painter.vline(tx, response.rect.top()..=response.rect.bottom(), tis);
}
