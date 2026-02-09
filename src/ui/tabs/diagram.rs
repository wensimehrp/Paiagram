use super::{Navigatable, Tab};
use crate::entry::{
    AdjustEntryMode, EntryBundle, EntryMode, EntryModeAdjustment, EntryQuery, EntryStop, TravelMode,
};
use crate::export::ExportObject;
use crate::route::Route;
use crate::station::Station;
use crate::trip::class::DisplayedStroke;
use crate::trip::{Trip, TripBundle, TripClass};
use crate::ui::widgets::buttons;
use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;
use egui::epaint::TextShape;
use egui::{Align2, Color32, FontId, Id, Margin, NumExt, Painter, Pos2, Rect, Sense, Ui, Vec2};
use egui_i18n::tr;
use instant::Instant;
use itertools::Itertools;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub mod calc_trip_lines;
mod draw_lines;
mod gpu_draw;

/// The current selected item
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum SelectedItem {
    /// A timetable entry
    TimetableEntry { entry: Entity, parent: Entity },
    /// An interval connecting two stations
    Interval(Entity, Entity),
    /// A station
    Station(Entity),
    /// A trip
    ExtendingTrip {
        entry: Entity,
        previous_pos: Option<(TimetableTime, usize)>,
        current_entry: Option<Entity>,
    },
}

// TODO: dt & td graphs
#[derive(Serialize, Deserialize, Clone)]
pub struct DiagramTab {
    /// X offset as ticks
    x_offset: i64,
    y_offset: f32,
    zoom: Vec2,
    selected: Option<SelectedItem>,
    route_entity: Entity,
    // cache zone
    max_height: f32,
    trips: Vec<Entity>,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuTripRendererState>>,
    #[serde(skip, default)]
    show_perf: bool,
    #[serde(skip, default)]
    last_frame_ms: f32,
    #[serde(skip, default)]
    last_draw_ms: f32,
    #[serde(skip, default)]
    last_gpu_prep_ms: f32,
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_entity == other.route_entity
    }
}

impl DiagramTab {
    pub fn new(route_entity: Entity) -> Self {
        Self {
            x_offset: 0,
            y_offset: 0.0,
            zoom: Vec2 { x: 0.0001, y: 0.2 },
            selected: None,
            route_entity,
            max_height: 0.0,
            trips: Vec::new(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuTripRendererState::default(),
            )),
            show_perf: false,
            last_frame_ms: 0.0,
            last_draw_ms: 0.0,
            last_gpu_prep_ms: 0.0,
        }
    }
    pub fn time_view(&self, rect: egui::Rect) -> (std::ops::Range<i64>, f64) {
        let width = rect.width().max(1.0);
        let zoom_x = self.zoom.x.max(f32::EPSILON);

        let visible_ticks = self.x_offset..self.x_offset + (width as f64 / zoom_x as f64) as i64;
        let ticks_per_screen_unit = (visible_ticks.end - visible_ticks.start) as f64 / width as f64;

        (visible_ticks, ticks_per_screen_unit)
    }
}

impl MapEntities for DiagramTab {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.route_entity.map_entities(entity_mapper);
    }
}

impl Navigatable for DiagramTab {
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
        self.x_offset as f64
    }
    fn offset_y(&self) -> f32 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f32) {
        self.x_offset = offset_x.round() as i64;
        self.y_offset = offset_y;
    }
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x.clamp(0.00005, 0.4), zoom_y.clamp(0.1, 2048.0))
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        self.x_offset = self.x_offset.clamp(
            -366 * 86400 * TICKS_PER_SECOND,
            366 * 86400 * TICKS_PER_SECOND
                - (response.rect.width() as f64 / self.zoom.x as f64) as i64,
        );
        const TOP_BOTTOM_PADDING: f32 = 30.0;
        self.y_offset = if response.rect.height() / self.zoom.y
            > (self.max_height + TOP_BOTTOM_PADDING * 2.0 / self.zoom.y)
        {
            (-response.rect.height() / self.zoom.y + self.max_height) / 2.0
        } else {
            self.y_offset.clamp(
                -TOP_BOTTOM_PADDING / self.zoom.y,
                self.max_height - response.rect.height() / self.zoom.y
                    + TOP_BOTTOM_PADDING / self.zoom.y,
            )
        }
    }
}

/// Time and time-canvas related constant. How many ticks is a second
pub const TICKS_PER_SECOND: i64 = 100;

#[derive(Debug)]
pub struct DrawnTrip {
    pub entity: Entity,
    pub stroke: DisplayedStroke,
    pub points: Vec<Vec<[Pos2; 4]>>,
    pub entries: Vec<Vec<Entity>>,
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn export_display(&mut self, world: &mut World, ui: &mut Ui) {
        use crate::export::typst_diagram::{TypstDiagram, TypstModule};
        ui.strong(tr!("tab-diagram-save-typst-module"));
        ui.label(tr!("tab-diagram-save-typst-module-desc"));
        if ui.button(tr!("export")).clicked() {
            TypstModule.export_to_file(world, ());
        }
        ui.strong(tr!("tab-diagram-export-json-data"));
        ui.label(tr!("tab-diagram-export-json-data"));
        if ui.button(tr!("export")).clicked() {
            TypstDiagram.export_to_file(world, (self.route_entity, &self.trips));
        }
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        match self.selected {
            None => {
                ui.strong("New Trip");
                ui.label("Create a new trip from scratch");
                if ui.button("Create a new trip").clicked() {
                    let default_class = world
                        .resource::<crate::class::ClassResource>()
                        .default_class;
                    let new_trip = world
                        .commands()
                        .spawn(TripBundle::new("New Trip", TripClass(default_class)))
                        .id();
                    self.trips.push(new_trip);
                    self.selected = Some(SelectedItem::ExtendingTrip {
                        entry: new_trip,
                        previous_pos: None,
                        current_entry: None,
                    })
                }
            }
            Some(SelectedItem::ExtendingTrip {
                entry,
                previous_pos,
                current_entry,
            }) => {
                if ui.button("Complete").clicked() {
                    self.selected = None
                }
            }
            Some(SelectedItem::TimetableEntry { entry: _, parent }) => {
                if ui.button("Extend").clicked() {
                    self.selected = Some(SelectedItem::ExtendingTrip {
                        entry: parent,
                        previous_pos: None,
                        current_entry: None,
                    })
                }
            }
            Some(SelectedItem::Interval(a, b)) => {}
            Some(SelectedItem::Station(s)) => {}
        }
    }
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_perf, "Perf");
        });
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .show(ui, |ui| {
                let frame_start = Instant::now();
                let route = world
                    .get::<Route>(self.route_entity)
                    .expect("Entity should have a route");
                let (response, mut painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
                self.handle_navigation(ui, &response);
                let station_heights: Vec<_> = route.iter().collect();
                let (visible_ticks, ticks_per_screen_unit) = self.time_view(response.rect);
                let to_screen_y = |h: f32| (h - self.y_offset) * self.zoom.y + response.rect.top();
                let screen_x_to_seconds = |screen_x: f32| -> TimetableTime {
                    let ticks = (screen_x - response.rect.left()) as f64 * ticks_per_screen_unit
                        + self.x_offset as f64;
                    TimetableTime((ticks / TICKS_PER_SECOND as f64) as i32)
                };
                let ticks_to_screen_x = |ticks: i64| -> f32 {
                    let base = (ticks - self.x_offset) as f64 / ticks_per_screen_unit;
                    response.rect.left() + base as f32
                };
                let station_heights_screen_iter = station_heights.iter().copied().map(|(e, h)| {
                    let height = to_screen_y(h);
                    (e, height)
                });
                if station_heights.is_empty() {
                    return;
                }
                self.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);
                // note that order matters here: navigation must be handled before anything else is calculated
                let show_button = self.zoom.x.min(self.zoom.y) > 0.0005;
                let button_strength = ui
                    .ctx()
                    .animate_bool(ui.id().with("all buttons animation"), show_button);
                draw_lines::draw_station_lines(
                    self.y_offset,
                    &mut painter,
                    response.rect,
                    self.zoom.y,
                    station_heights.iter().copied(),
                    ui.pixels_per_point(),
                    &world, // FIXME: make this a system instead of passing world into it
                );
                draw_lines::draw_time_lines(
                    self.x_offset,
                    &mut painter,
                    response.rect,
                    ticks_per_screen_unit,
                    &visible_ticks,
                    ui.pixels_per_point(),
                );
                world
                    .run_system_cached_with(
                        calc_trip_lines::calculate_trips,
                        (&mut self.trips, self.route_entity),
                    )
                    .unwrap();
                let mut trip_line_buf = Vec::new();
                // Calculate the visible trains
                let calc_context = calc_trip_lines::CalcContext::from_tab(
                    &self,
                    response.rect,
                    ticks_per_screen_unit,
                    visible_ticks.clone(),
                );
                world
                    .run_system_cached_with(
                        calc_trip_lines::calc,
                        (&mut trip_line_buf, calc_context, &self.trips),
                    )
                    .unwrap();
                if let Some(SelectedItem::ExtendingTrip {
                    entry,
                    previous_pos,
                    current_entry,
                }) = &mut self.selected
                    && let Some(interact_pos) = ui.input(|r| r.pointer.hover_pos())
                {
                    let mut new_screen_pos: Pos2 = interact_pos;
                    let new_station_idx: usize;
                    let idx = station_heights_screen_iter
                        .clone()
                        .rposition(|(_, height)| height < interact_pos.y)
                        .unwrap_or(0);
                    let (_, prev_height) = station_heights[idx];
                    let prev_height = to_screen_y(prev_height);
                    if let Some((_, next_height)) = station_heights.get(idx + 1).copied()
                        && interact_pos.y > (to_screen_y(next_height) + prev_height) / 2.0
                    {
                        new_screen_pos.y = to_screen_y(next_height);
                        new_station_idx = idx + 1
                    } else {
                        new_screen_pos.y = prev_height;
                        new_station_idx = idx;
                    }
                    let new_station_entity = station_heights[new_station_idx].0;
                    // Smoothing animations
                    // see https://github.com/rerun-io/egui_tiles/blob/f86273ba8ff9f44a9817067abbf977ba5cdcb9fa/src/tree.rs#L438-L493
                    let mut requires_repaint = false;
                    let dt = ui.ctx().input(|input| input.stable_dt).at_most(0.1);
                    let smoothed_screen_pos = ui.ctx().data_mut(|data| {
                        let smoothed: &mut Pos2 = data
                            .get_temp_mut_or(ui.id().with("new line animation"), new_screen_pos);
                        let t = egui::emath::exponential_smooth_factor(0.9, 0.05, dt);
                        *smoothed = smoothed.lerp(new_screen_pos, t);
                        let diff = smoothed.distance(new_screen_pos);
                        if diff < 1.0 {
                            *smoothed = new_screen_pos
                        } else {
                            requires_repaint = true
                        }
                        *smoothed
                    });
                    if requires_repaint {
                        ui.ctx().request_repaint();
                    }
                    let stroke = DisplayedStroke::default().egui_stroke(ui.visuals().dark_mode);
                    painter.line_segment([smoothed_screen_pos, interact_pos], stroke);
                    let station_name = world.get::<Name>(new_station_entity).unwrap();
                    painter.circle_filled(smoothed_screen_pos, 4.0, ui.visuals().text_color());
                    painter.text(
                        smoothed_screen_pos,
                        Align2::LEFT_BOTTOM,
                        station_name,
                        FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    );
                    if let Some((t, idx)) = *previous_pos {
                        let (_, h) = station_heights[idx];
                        let prev_pos = Pos2::new(
                            ticks_to_screen_x(t.0 as i64 * TICKS_PER_SECOND),
                            to_screen_y(h),
                        );
                        painter.line_segment([prev_pos, smoothed_screen_pos], stroke);
                    }
                    if response.clicked() {
                        let hovered_time = screen_x_to_seconds(interact_pos.x);
                        let should_spawn = match current_entry {
                            Some(current_entry_unwrapped) => {
                                let stop = world
                                    .get::<EntryStop>(*current_entry_unwrapped)
                                    .unwrap()
                                    .entity();
                                if stop == new_station_entity {
                                    let mut mode = world
                                        .get_mut::<EntryMode>(*current_entry_unwrapped)
                                        .unwrap();
                                    mode.dep = Some(TravelMode::At(hovered_time));
                                    *current_entry = None;
                                    false
                                } else {
                                    true
                                }
                            }
                            None => true,
                        };
                        if should_spawn {
                            let new_child = world
                                .spawn(EntryBundle::new(
                                    TravelMode::At(hovered_time),
                                    None,
                                    new_station_entity,
                                ))
                                .id();
                            world.entity_mut(*entry).add_child(new_child);
                            *current_entry = Some(new_child);
                        }
                        *previous_pos = Some((hovered_time, new_station_idx))
                    }
                } else if response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    if self.selected.is_some() {
                        self.selected = None
                    } else {
                        self.selected = handle_selection(&trip_line_buf, pos);
                    }
                }
                let selection_strength = ui
                    .ctx()
                    .animate_bool(ui.id().with("selection"), self.selected.is_some());
                // let use_gpu = self.use_gpu;
                let mut selected_idx_rect: Option<(usize, Vec<Rect>)> = None;
                world
                    .run_system_cached_with(
                        draw_trip_lines,
                        (
                            &trip_line_buf,
                            ui,
                            &mut painter,
                            self.selected,
                            &mut selected_idx_rect,
                            button_strength,
                        ),
                    )
                    .unwrap();
                let mut state = self.gpu_state.lock();
                if let Some(target_format) = ui.ctx().data(|data| {
                    data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(Id::new(
                        "wgpu_target_format",
                    ))
                }) {
                    state.target_format = Some(target_format);
                }
                if let Some(msaa_samples) = ui
                    .ctx()
                    .data(|data| data.get_temp::<u32>(Id::new("wgpu_msaa_samples")))
                {
                    state.msaa_samples = msaa_samples;
                }
                let gpu_prep_start = Instant::now();
                gpu_draw::write_vertices(&trip_line_buf, ui.visuals().dark_mode, &mut state);
                self.last_gpu_prep_ms = gpu_prep_start.elapsed().as_secs_f32() * 1000.0;
                let callback = gpu_draw::paint_callback(response.rect, self.gpu_state.clone());
                painter.add(callback);
                let draw_start = Instant::now();
                self.last_draw_ms = draw_start.elapsed().as_secs_f32() * 1000.0;
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
                if let Some((idx, rects)) = selected_idx_rect {
                    let trip = &trip_line_buf[idx];
                    let stroke = egui::Stroke {
                        width: trip.stroke.width + 3.0 * selection_strength * trip.stroke.width,
                        color: trip.stroke.color.get(ui.visuals().dark_mode),
                    };
                    for (i, (p_group, e_group)) in
                        trip.points.iter().zip(trip.entries.iter()).enumerate()
                    {
                        let mut points = Vec::with_capacity(p_group.len() * 4);
                        for segment in p_group.iter() {
                            points.extend(segment.iter().copied());
                        }
                        if points.len() >= 2 {
                            painter.line(points, stroke);
                        }
                        for (j, (points, e)) in
                            p_group.iter().zip(e_group.iter().copied()).enumerate()
                        {
                            world
                                .run_system_cached_with(
                                    draw_handles,
                                    (
                                        points,
                                        e,
                                        (i, j),
                                        ui,
                                        &mut painter,
                                        self.zoom.x,
                                        button_strength.min(selection_strength),
                                    ),
                                )
                                .unwrap();
                        }
                    }
                    for rect in rects {
                        painter.rect(
                            rect,
                            8,
                            Color32::BLUE.gamma_multiply(0.5),
                            egui::Stroke {
                                width: 1.0,
                                color: Color32::BLUE,
                            },
                            egui::StrokeKind::Middle,
                        );
                    }
                }
                self.last_frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
                if self.show_perf {
                    let mut text = format!(
                        "GPU: on\nGPU prep: {:.2} ms\nDraw: {:.2} ms\nFrame: {:.2} ms",
                        self.last_gpu_prep_ms, self.last_draw_ms, self.last_frame_ms
                    );
                    if let Some(info) = ui
                        .ctx()
                        .data(|data| data.get_temp::<String>(Id::new("wgpu_adapter_info")))
                    {
                        text.push_str("\n");
                        text.push_str(&info);
                    }
                    let color = if ui.visuals().dark_mode {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    };
                    let pos = response.rect.left_top() + Vec2::new(6.0, 6.0);
                    painter.text(pos, Align2::LEFT_TOP, text, FontId::monospace(12.0), color);
                }
            });
    }
}

/// Takes a buffer the calculate trains

fn draw_trip_lines<'a>(
    (InRef(trips), InMut(ui), InMut(painter), In(selected), InMut(ret), In(strength)): (
        InRef<[DrawnTrip]>,
        InMut<Ui>,
        InMut<Painter>,
        In<Option<SelectedItem>>,
        InMut<Option<(usize, Vec<Rect>)>>,
        In<f32>,
    ),
    name_q: Query<&Name, With<Trip>>,
) {
    for (idx, trip) in trips.iter().enumerate() {
        if let Some(SelectedItem::TimetableEntry { entry, parent }) = selected
            && trip.entity == parent
        {
            let rects = trip
                .points
                .iter()
                .flatten()
                .zip(trip.entries.iter().flatten())
                .filter_map(|(p, e)| {
                    if *e == entry {
                        Some(Rect::from_two_pos(p[1], p[2]).expand(8.0))
                    } else {
                        None
                    }
                })
                .collect();
            *ret = Some((idx, rects));
            break;
        }
    }
    if strength < 0.1 {
        return;
    }
    for trip in trips.iter() {
        let draw_color = trip
            .stroke
            .color
            .get(ui.visuals().dark_mode)
            .gamma_multiply(strength);
        let name = name_q.get(trip.entity).unwrap().to_string();
        let galley = painter.layout_no_wrap(name, egui::FontId::proportional(14.0), draw_color);
        for ([.., curr], [next, ..]) in trip.points.iter().filter_map(|it| {
            if let (Some(a), Some(b)) = (it.get(0), it.get(1)) {
                return Some((a, b));
            } else {
                return None;
            }
        }) {
            let angle = (*next - *curr).angle();
            let text_shape = TextShape::new(
                *curr
                    - Vec2 {
                        y: galley.rect.height(),
                        x: 0.0,
                    },
                galley.clone(),
                draw_color,
            )
            .with_angle_and_anchor(angle, Align2::LEFT_BOTTOM);
            painter.add(text_shape);
        }
    }
}

fn handle_selection(drawn_trips: &[DrawnTrip], pos: Pos2) -> Option<SelectedItem> {
    const VEHICLE_SELECTION_RADIUS: f32 = 7.0;
    const STATION_SELECTION_RADIUS: f32 = VEHICLE_SELECTION_RADIUS;
    for trip in drawn_trips {
        for (points, entries) in trip.points.iter().zip(trip.entries.iter()) {
            let entries_iter = entries
                .iter()
                .flat_map(|it| std::iter::repeat(it).take(4))
                .copied();
            for (w, e) in points.as_flattened().windows(2).zip(entries_iter) {
                let [curr, next] = w else { unreachable!() };
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

                if dx * dx + dy * dy < VEHICLE_SELECTION_RADIUS.powi(2) {
                    return Some(SelectedItem::TimetableEntry {
                        entry: e,
                        parent: trip.entity,
                    });
                }
            }
        }
    }
    return None;
}

fn draw_handles(
    (InRef(p), In(e), In(salt), InMut(ui), InMut(mut painter), In(zoom_x), In(strength)): (
        InRef<[Pos2]>,
        In<Entity>,
        In<impl std::hash::Hash + Copy>,
        InMut<Ui>,
        InMut<Painter>,
        In<f32>,
        In<f32>,
    ),
    entry_q: Query<EntryQuery>,
    name_q: Query<&Name>,
    mut commands: Commands,
    mut prev_drag_delta: Local<Option<f32>>,
) {
    let entry = entry_q.get(e).unwrap();
    if entry.is_derived() || strength <= 0.1 {
        return;
    }
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
    let arrival_id = ui.id().with((e, "arr", salt));
    let arrival_response = ui.interact(arrival_rect, arrival_id, Sense::click_and_drag());
    let arrival_fill = if arrival_response.hovered() {
        Color32::GRAY
    } else {
        Color32::WHITE
    }
    .linear_multiply(strength);
    match entry.mode.arr {
        TravelMode::At(_) => buttons::circle_button_shape(
            &mut painter,
            arrival_pos,
            CIRCLE_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
        TravelMode::For(_) => buttons::dash_button_shape(
            &mut painter,
            arrival_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
        TravelMode::Flexible => buttons::triangle_button_shape(
            &mut painter,
            arrival_pos,
            TRIANGLE_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
    };

    if arrival_response.drag_started() {
        *prev_drag_delta = None;
    }
    if let Some(total_drag_delta) = arrival_response.total_drag_delta() {
        if zoom_x > f32::EPSILON {
            let previous_drag_delta = prev_drag_delta.unwrap_or(0.0);
            let duration = Duration(
                ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                    / zoom_x as f64
                    / TICKS_PER_SECOND as f64) as i32,
            );
            if duration != Duration(0) {
                commands.trigger(AdjustEntryMode {
                    entity: e,
                    adj: EntryModeAdjustment::ShiftArrival(duration),
                });
                *prev_drag_delta = Some(
                    previous_drag_delta
                        + (duration.0 as f64 * TICKS_PER_SECOND as f64 * zoom_x as f64) as f32,
                );
            }
        }
    }
    if arrival_response.drag_stopped() {
        *prev_drag_delta = None;
    }
    if arrival_response.dragged() || arrival_response.hovered() {
        arrival_response.on_hover_ui(|ui| {
            if let Some(estimate) = entry.estimate {
                ui.label(estimate.arr.to_string());
            }
            ui.label(name_q.get(entry.stop()).map_or("??", |n| n.as_str()));
        });
    }

    let dep_sense = match entry.mode.dep {
        Some(TravelMode::Flexible) | None => Sense::click(),
        _ => Sense::click_and_drag(),
    };
    let departure_rect = Rect::from_center_size(departure_pos, Vec2::splat(HANDLE_SIZE));
    let departure_id = ui.id().with((e, "dep", salt));
    let departure_response = ui.interact(departure_rect, departure_id, dep_sense);
    let departure_fill = if departure_response.hovered() {
        Color32::GRAY
    } else {
        Color32::WHITE
    }
    .linear_multiply(strength);
    match entry.mode.dep {
        Some(TravelMode::At(_)) => buttons::circle_button_shape(
            &mut painter,
            departure_pos,
            CIRCLE_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
        Some(TravelMode::For(_)) => buttons::dash_button_shape(
            &mut painter,
            departure_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
        Some(TravelMode::Flexible) => buttons::triangle_button_shape(
            &mut painter,
            departure_pos,
            TRIANGLE_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
        None => buttons::double_triangle(
            &mut painter,
            departure_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
    };

    if departure_response.drag_started() {
        *prev_drag_delta = None;
    }
    if let Some(total_drag_delta) = departure_response.total_drag_delta() {
        if zoom_x > f32::EPSILON {
            let previous_drag_delta = prev_drag_delta.unwrap_or(0.0);
            let duration = Duration(
                ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                    / zoom_x as f64
                    / TICKS_PER_SECOND as f64) as i32,
            );
            if duration != Duration(0) {
                commands.trigger(AdjustEntryMode {
                    entity: e,
                    adj: EntryModeAdjustment::ShiftDeparture(duration),
                });
                *prev_drag_delta = Some(
                    previous_drag_delta
                        + (duration.0 as f64 * TICKS_PER_SECOND as f64 * zoom_x as f64) as f32,
                );
            }
        }
    }
    if departure_response.drag_stopped() {
        *prev_drag_delta = None;
    }
    if departure_response.dragged() || departure_response.hovered() {
        departure_response.on_hover_ui(|ui| {
            if let Some(estimate) = entry.estimate {
                ui.label(estimate.dep.to_string());
            }
            ui.label(name_q.get(entry.stop()).map_or("??", |n| n.as_str()));
        });
    }
}
