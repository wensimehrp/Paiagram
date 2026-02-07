use super::{Navigatable, Tab};
use crate::entry::{EntryQuery, TravelMode};
use crate::route::Route;
use crate::trip::class::DisplayedStroke;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use egui::{Align2, Color32, FontId, Id, Margin, Painter, Pos2, Rect, Sense, Ui, Vec2};
use instant::Instant;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
mod calc_trip_lines;
mod draw_lines;
mod gpu_draw;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum SelectedItem {
    TimetableEntry { entry: Entity, parent: Entity },
    Interval(Entity, Entity),
    Station(Entity),
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
const TICKS_PER_SECOND: i64 = 100;

#[derive(Debug)]
pub struct DrawnTrip {
    entity: Entity,
    stroke: DisplayedStroke,
    points: Vec<Vec<[Pos2; 4]>>,
    entries: Vec<Vec<Entity>>,
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
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
                let station_heights: Vec<_> = route
                    .iter()
                    .map(|(e, h)| (e, h, world.get::<Name>(e).unwrap().as_str()))
                    .collect();
                self.max_height = station_heights.last().map_or(0.0, |(_, h, _)| *h);
                let _route_name = world.get::<Name>(self.route_entity).unwrap().as_str();
                let (response, mut painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
                // note that order matters here: navigation must be handled before anything else is calculated
                self.handle_navigation(ui, &response);
                let (visible_ticks, ticks_per_screen_unit) = self.time_view(response.rect);
                draw_lines::draw_station_lines(
                    self.y_offset,
                    &mut painter,
                    response.rect,
                    self.zoom.y,
                    station_heights.iter().copied(),
                    ui.pixels_per_point(),
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
                world
                    .run_system_cached_with(
                        calc_trip_lines::calc,
                        (
                            &mut trip_line_buf,
                            &self,
                            response.rect,
                            ticks_per_screen_unit,
                            visible_ticks.clone(),
                        ),
                    )
                    .unwrap();
                if response.clicked()
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
                    for (p_group, e_group) in trip.points.iter().zip(trip.entries.iter()) {
                        let mut points = Vec::with_capacity(p_group.len() * 4);
                        for segment in p_group.iter() {
                            points.extend(segment.iter().copied());
                        }
                        if points.len() >= 2 {
                            painter.line(points, stroke);
                        }
                        for (points, e) in p_group.iter().zip(e_group.iter().copied()) {
                            world
                                .run_system_cached_with(draw_handles, (points, e, &mut painter))
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
    (InRef(trips), InMut(ui), InMut(painter), In(selected), InMut(ret)): (
        InRef<[DrawnTrip]>,
        InMut<Ui>,
        InMut<Painter>,
        In<Option<SelectedItem>>,
        InMut<Option<(usize, Vec<Rect>)>>,
    ),
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
            return;
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
    (InRef(p), In(e), InMut(painter)): (InRef<[Pos2]>, In<Entity>, InMut<Painter>),
    entry_q: Query<EntryQuery>,
) {
    let entry = entry_q.get(e).unwrap();
    if entry.is_derived() {
        return;
    }
    painter.circle(
        p[1],
        4.0,
        if matches!(entry.mode.arr, TravelMode::Flexible) {
            Color32::YELLOW
        } else {
            Color32::WHITE
        },
        egui::Stroke {
            width: 1.0,
            color: Color32::BLACK,
        },
    );
    painter.circle(
        p[2],
        4.0,
        if matches!(
            entry.mode.dep.unwrap_or(TravelMode::Flexible),
            TravelMode::Flexible
        ) {
            Color32::YELLOW
        } else {
            Color32::WHITE
        },
        egui::Stroke {
            width: 1.0,
            color: Color32::BLACK,
        },
    );
}
