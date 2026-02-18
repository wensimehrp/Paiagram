use super::{Navigatable, Tab};
use crate::entry::{
    AdjustEntryMode, EntryBundle, EntryEstimate, EntryMode, EntryModeAdjustment, EntryQuery,
    EntryStop, IsDerivedEntry, TravelMode,
};
use crate::export::ExportObject;
use crate::route::Route;
use crate::trip::class::DisplayedStroke;
use crate::trip::routing::AddEntryToTrip;
use crate::trip::{Trip, TripBundle, TripClass, TripSchedule};
use crate::ui::tabs::trip::TripTab;
use crate::ui::widgets::buttons;
use crate::ui::{GlobalTimer, OpenOrFocus};
use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;
use egui::epaint::TextShape;
use egui::{
    Align2, Color32, FontId, Id, Margin, NumExt, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use egui_i18n::tr;
use instant::Instant;
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
    /// Extending a trip
    ExtendingTrip {
        entry: Entity,
        previous_pos: Option<(TimetableTime, usize)>,
        last_time: Option<TimetableTime>,
        current_entry: Option<Entity>,
    },
}

// TODO: dt & td graphs
#[derive(Serialize, Deserialize, Clone)]
pub struct DiagramTab {
    /// X offset as ticks
    navi: DiagramTabNavigation,
    selected: Option<SelectedItem>,
    route_entity: Entity,
    trips: Vec<Entity>,
    #[serde(skip, default)]
    use_global_timer: bool,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuTripRendererState>>,
    #[serde(skip, default)]
    show_perf: bool,
    #[serde(skip, default)]
    last_frame_ms: f32,
    #[serde(skip, default)]
    last_gpu_prep_ms: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiagramTabNavigation {
    x_offset: i64,
    y_offset: f32,
    zoom: Vec2,
    #[serde(skip, default = "default_visible_rect")]
    visible_rect: Rect,
    // cache zone
    max_height: f32,
}

impl Default for DiagramTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0,
            y_offset: 0.0,
            zoom: Vec2::splat(1.0),
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
        self.route_entity == other.route_entity
    }
}

impl DiagramTab {
    pub fn new(route_entity: Entity) -> Self {
        Self {
            navi: DiagramTabNavigation::default(),
            selected: None,
            route_entity,
            trips: Vec::new(),
            use_global_timer: false,
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuTripRendererState::default(),
            )),
            show_perf: false,
            last_frame_ms: 0.0,
            last_gpu_prep_ms: 0.0,
        }
    }
}

impl MapEntities for DiagramTab {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.route_entity.map_entities(entity_mapper);
    }
}

impl Navigatable for DiagramTabNavigation {
    type XOffset = i64;
    type YOffset = f32;

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
    fn x_from_f64(&self, value: f64) -> Self::XOffset {
        value.trunc() as i64
    }
    fn x_to_f64(&self, value: Self::XOffset) -> f64 {
        value as f64
    }
    fn y_from_f32(&self, value: f32) -> Self::YOffset {
        value
    }
    fn y_to_f32(&self, value: Self::YOffset) -> f32 {
        value
    }
    fn screen_pos_to_xy(&self, pos: egui::Pos2) -> (Self::XOffset, Self::YOffset) {
        let rect = self.visible_rect;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let x = self.offset_x() + (pos.x - rect.left()) as f64 * ticks_per_screen_unit;
        let y = self.offset_y() + (pos.y - rect.top()) / self.zoom_y().max(f32::EPSILON);
        (x.trunc() as i64, y)
    }
    fn xy_to_screen_pos(&self, x: Self::XOffset, y: Self::YOffset) -> egui::Pos2 {
        let rect = self.visible_rect;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let screen_x = rect.left() + ((x as f64 - self.offset_x()) / ticks_per_screen_unit) as f32;
        let screen_y = rect.top() + (y - self.offset_y()) * self.zoom_y().max(f32::EPSILON);
        egui::Pos2::new(screen_x, screen_y)
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
    fn x_per_screen_unit(&self) -> Self::XOffset {
        (1.0 / self.zoom_x().max(f32::EPSILON) as f64) as i64
    }
    fn visible_x(&self) -> std::ops::Range<Self::XOffset> {
        let width = self.visible_rect().width() as f64;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let start = self.x_offset;
        let end = start + (width * ticks_per_screen_unit).ceil() as i64;
        start..end
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let height = self.visible_rect.height();
        let start = self.offset_y();
        let end = start + height / self.zoom_y().max(f32::EPSILON);
        start..end
    }
    fn y_per_screen_unit(&self) -> Self::YOffset {
        1.0 / self.zoom_y().max(f32::EPSILON)
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
    fn id(&self) -> Id {
        Id::new(self.route_entity)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
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
        ui.checkbox(&mut self.use_global_timer, "Use global timer");
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
                        last_time: None,
                    })
                }
            }
            Some(SelectedItem::ExtendingTrip {
                entry,
                previous_pos,
                current_entry,
                last_time,
            }) => {
                let mut name = world.get_mut::<Name>(entry).unwrap();
                name.mutate(|n| {
                    ui.text_edit_singleline(n);
                });
                if ui.button("Complete").clicked() {
                    self.selected = None
                }
            }
            Some(SelectedItem::TimetableEntry { entry, parent }) => {
                let is_derived = world.get::<IsDerivedEntry>(entry).is_some();
                if is_derived && ui.button("Convert to explicit").clicked() {
                    world.entity_mut(entry).remove::<IsDerivedEntry>();
                } else if !is_derived && ui.button("Delete").clicked() {
                    world.entity_mut(entry).despawn();
                }
                let mut name = world.get_mut::<Name>(parent).unwrap();
                name.mutate(|n| {
                    ui.text_edit_singleline(n);
                });
                if ui.button("Open trip view").clicked() {
                    world
                        .write_message(OpenOrFocus(crate::ui::MainTab::Trip(TripTab::new(parent))));
                }
                if ui.button("Extend").clicked() {
                    let mut last_time = None;
                    world
                        .run_system_cached_with(
                            |(InMut(last_time), In(parent)): (
                                InMut<Option<TimetableTime>>,
                                In<Entity>,
                            ),
                             schedule_q: Query<&TripSchedule, With<Trip>>,
                             entry_q: Query<&EntryEstimate>| {
                                let Ok(schedule) = schedule_q.get(parent) else {
                                    return;
                                };
                                let Some(time) =
                                    schedule.iter().rev().find_map(|e| entry_q.get(e).ok())
                                else {
                                    return;
                                };
                                *last_time = Some(time.dep);
                            },
                            (&mut last_time, parent),
                        )
                        .unwrap();
                    self.selected = Some(SelectedItem::ExtendingTrip {
                        entry: parent,
                        previous_pos: None,
                        current_entry: None,
                        last_time,
                    })
                }
            }
            Some(SelectedItem::Interval(a, b)) => {}
            Some(SelectedItem::Station(s)) => {}
        }
    }
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| main_display(self, world, ui));
    }
}

fn main_display(tab: &mut DiagramTab, world: &mut World, ui: &mut egui::Ui) {
    let route = world
        .get::<Route>(tab.route_entity)
        .expect("Entity should have a route");
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    let timer = world.resource::<GlobalTimer>();
    tab.navi.visible_rect = response.rect;
    if tab.use_global_timer {
        tab.navi.x_offset = timer.read_ticks();
    }
    let moved = tab.navi.handle_navigation(ui, &response);
    if tab.use_global_timer {
        timer.write_ticks(tab.navi.x_offset);
    }
    if moved {
        timer.try_lock(tab.route_entity);
    } else {
        timer.try_unlock(tab.route_entity);
    }
    let station_heights: Vec<_> = route.iter().collect();
    if station_heights.is_empty() {
        return;
    }
    tab.navi.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);
    let ticks_per_screen_unit = 1.0 / tab.navi.zoom_x().max(f32::EPSILON) as f64;
    let visible_ticks = tab.navi.visible_x();
    let to_screen_y = |h: f32| {
        tab.navi.visible_rect.top()
            + (h - tab.navi.offset_y()) * tab.navi.zoom_y().max(f32::EPSILON)
    };
    let screen_x_to_seconds = |screen_x: f32| -> TimetableTime {
        let ticks = tab.navi.offset_x()
            + (screen_x - tab.navi.visible_rect.left()) as f64 * ticks_per_screen_unit;
        TimetableTime((ticks / TICKS_PER_SECOND as f64) as i32)
    };
    let ticks_to_screen_x = |ticks: i64| -> f32 {
        tab.navi.visible_rect.left()
            + ((ticks as f64 - tab.navi.offset_x()) / ticks_per_screen_unit) as f32
    };
    let station_heights_screen_iter = station_heights.iter().copied().map(|(e, h)| {
        let height = to_screen_y(h);
        (e, height)
    });
    // note that order matters here: navigation must be handled before anything else is calculated
    let show_button = tab.navi.zoom.x.min(tab.navi.zoom.y) > 0.001;
    let button_strength = ui
        .ctx()
        .animate_bool(ui.id().with("all buttons animation"), show_button);
    draw_lines::draw_station_lines(
        tab.navi.y_offset,
        &mut painter,
        tab.navi.visible_rect,
        tab.navi.zoom.y,
        station_heights.iter().copied(),
        ui.pixels_per_point(),
        ui.visuals(),
        &world, // FIXME: make this a system instead of passing world into it
    );
    draw_lines::draw_time_lines(
        tab.navi.x_offset,
        &mut painter,
        tab.navi.visible_rect,
        ticks_per_screen_unit,
        &visible_ticks,
        ui.pixels_per_point(),
    );
    world
        .run_system_cached_with(
            calc_trip_lines::calculate_trips,
            (&mut tab.trips, tab.route_entity),
        )
        .unwrap();
    let mut trip_line_buf = Vec::new();
    // Calculate the visible trains
    let calc_context = calc_trip_lines::CalcContext::from_tab(
        &tab,
        tab.navi.visible_rect,
        ticks_per_screen_unit,
        visible_ticks.clone(),
    );
    world
        .run_system_cached_with(
            calc_trip_lines::calc,
            (&mut trip_line_buf, calc_context, &tab.trips),
        )
        .unwrap();
    if let Some(SelectedItem::ExtendingTrip {
        entry,
        previous_pos,
        current_entry,
        last_time,
    }) = &mut tab.selected
        && let (Some(interact_pos), is_touch_input) = ui.input(|r| {
            let pos = r.pointer.latest_pos().or(r.pointer.interact_pos());
            let is_touch = r.any_touches();
            (pos, is_touch)
        })
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
        let smoothed_screen_y = ui.ctx().data_mut(|data| {
            let smoothed: &mut f32 =
                data.get_temp_mut_or(ui.id().with("new line animation"), new_screen_pos.y);
            let t = egui::emath::exponential_smooth_factor(0.9, 0.05, dt);
            *smoothed = smoothed.lerp(new_screen_pos.y, t);
            // *smoothed = smoothed.lerp(new_screen_pos, t);
            let diff = (*smoothed - new_screen_pos.y).abs();
            if diff < 1.0 {
                *smoothed = new_screen_pos.y
            } else {
                requires_repaint = true
            }
            *smoothed
        });
        let smoothed_screen_pos = Pos2 {
            x: new_screen_pos.x,
            y: smoothed_screen_y,
        };
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
        let add_or_populate_entry = if is_touch_input {
            // TODO: show a button instead of using secondary_clicked
            response.secondary_clicked()
        } else {
            response.clicked()
        };
        if add_or_populate_entry {
            let hovered_time = screen_x_to_seconds(interact_pos.x);
            // normalize the time until the time difference is positive and less than 24 hours.
            let normalized_time = if let Some(last_time) = *last_time
                && let Some((last_clicked_time, _)) = *previous_pos
            {
                let diff = last_clicked_time - last_time;
                hovered_time - diff
            } else {
                hovered_time.normalized()
            };
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
                        mode.dep = Some(TravelMode::At(normalized_time));
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
                        TravelMode::At(normalized_time),
                        None,
                        new_station_entity,
                    ))
                    .id();
                world.write_message(AddEntryToTrip {
                    trip: *entry,
                    entry: new_child,
                });
                *current_entry = Some(new_child);
            }
            *previous_pos = Some((hovered_time, new_station_idx));
            *last_time = Some(normalized_time);
        }
    } else if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        if tab.selected.is_some() {
            tab.selected = None
        } else {
            tab.selected = handle_selection(&trip_line_buf, pos);
        }
    }
    let selection_strength = ui
        .ctx()
        .animate_bool(ui.id().with("selection"), tab.selected.is_some());
    // let use_gpu = tab.use_gpu;
    let mut selected_idx_rect: Option<(usize, Vec<Rect>)> = None;
    world
        .run_system_cached_with(
            draw_trip_lines,
            (
                &trip_line_buf,
                ui,
                &mut painter,
                tab.selected,
                &mut selected_idx_rect,
                button_strength,
            ),
        )
        .unwrap();
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
    let gpu_prep_start = Instant::now();
    gpu_draw::write_vertices(&trip_line_buf, ui.visuals().dark_mode, &mut state);
    tab.last_gpu_prep_ms = gpu_prep_start.elapsed().as_secs_f32() * 1000.0;
    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);
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
        for (i, (p_group, e_group)) in trip.points.iter().zip(trip.entries.iter()).enumerate() {
            let mut points = Vec::with_capacity(p_group.len() * 4);
            for segment in p_group.iter() {
                points.extend(segment.iter().copied());
            }
            if points.len() >= 2 {
                painter.line(points, stroke);
            }
            for (j, (points, e)) in p_group.iter().zip(e_group.iter().copied()).enumerate() {
                world
                    .run_system_cached_with(
                        draw_handles,
                        (
                            points,
                            e,
                            (i, j),
                            ui,
                            &mut painter,
                            tab.navi.zoom.x,
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
    if tab.show_perf {
        let mut text = format!(
            "GPU: on\nGPU prep: {:.2} ms\nFrame: {:.2} ms",
            tab.last_gpu_prep_ms, tab.last_frame_ms
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
        let galley = painter.layout_no_wrap(name, egui::FontId::proportional(13.0), draw_color);
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
            let last = points
                .last()
                .into_iter()
                .flat_map(|it| {
                    let [a, b, c, d] = it;
                    [[*a, *b], [*b, *c], [*c, *d]]
                })
                .zip(
                    entries
                        .last()
                        .into_iter()
                        .flat_map(|it| std::iter::repeat(*it).take(3)),
                );
            let entries_iter = entries.windows(2).flat_map(|w| {
                let [a, b] = w else { unreachable!() };
                std::iter::repeat(*a).take(4).chain(std::iter::once(*b))
            });
            for ([curr, next], e) in points
                .windows(2)
                .flat_map(|it| {
                    let [[a1, a2, a3, a4], [b, ..]] = it else {
                        unreachable!()
                    };
                    let mid = a4.lerp(*b, 0.5);
                    [[*a1, *a2], [*a2, *a3], [*a3, *a4], [*a4, mid], [mid, *b]]
                })
                .zip(entries_iter)
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
