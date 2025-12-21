use super::PageCache;
use crate::colors;
use crate::interface::SelectedElement;
use crate::interface::side_panel::CurrentTab;
use crate::interface::tabs::Tab;
use crate::vehicles::entries::{ActualRouteEntry, VehicleScheduleCache};
use crate::{
    interface::widgets::{buttons, timetable_popup},
    intervals::{Station, StationCache},
    lines::DisplayedLine,
    units::time::{Duration, TimetableTime},
    vehicles::{
        AdjustTimetableEntry,
        entries::{TimetableEntry, TimetableEntryCache, TravelMode, VehicleSchedule},
    },
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Popup, Pos2, Rect, Sense, Shape, Stroke,
    Ui, UiBuilder, Vec2, response, vec2,
};

// Time and time-canvas related constants
// const SECONDS_PER_WORLD_UNIT: f64 = 1.0; // world units -> seconds
const TICKS_PER_SECOND: i64 = 100;
// const TICKS_PER_WORLD_UNIT: f64 = SECONDS_PER_WORLD_UNIT * TICKS_PER_SECOND as f64;
const LINE_ANIMATION_TIME: f32 = 0.2; // 0.2 seconds

// TODO: implement multi select and editing
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SelectedEntityType {
    Vehicle(Entity),
    TimetableEntry { entry: Entity, vehicle: Entity },
    Interval((Entity, Entity)),
    Station(Entity),
    Map(Entity),
}

pub struct DiagramPageCache {
    background_acc_time: f32,
    interaction_acc_time: f32,
    previous_total_drag_delta: Option<f32>,
    stroke: Stroke,
    tick_offset: i64,
    vertical_offset: f32,
    heights: Option<Vec<(Entity, f32)>>,
    zoom: Vec2,
    vehicle_entities: Vec<Entity>,
}

impl DiagramPageCache {
    // linear search is quicker for a small data set
    fn get_visible_stations(&self, range: std::ops::Range<f32>) -> &[(Entity, f32)] {
        let Some(heights) = &self.heights else {
            return &[];
        };
        let first_visible = heights.iter().position(|(_, h)| *h > range.start);
        let last_visible = heights.iter().rposition(|(_, h)| *h < range.end);
        if let (Some(mut first_visible), Some(mut last_visible)) = (first_visible, last_visible) {
            first_visible = first_visible.saturating_sub(1);
            last_visible = (last_visible + 1).min(heights.len() - 1);
            &heights[first_visible..=last_visible]
        } else {
            &[]
        }
    }
}

impl Default for DiagramPageCache {
    fn default() -> Self {
        Self {
            background_acc_time: 0.0,
            interaction_acc_time: 0.0,
            previous_total_drag_delta: None,
            tick_offset: 0,
            vertical_offset: 0.0,
            stroke: Stroke {
                width: 1.0,
                color: Color32::BLACK,
            },
            heights: None,
            zoom: vec2(0.0005, 1.0),
            vehicle_entities: Vec::new(),
        }
    }
}

type PointData<'a> = (
    Pos2,
    Option<Pos2>,
    (&'a TimetableEntry, &'a TimetableEntryCache),
    ActualRouteEntry,
);

struct RenderedVehicle<'a> {
    segments: Vec<Vec<PointData<'a>>>,
    stroke: Stroke,
    entity: Entity,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct DiagramTab {
    pub displayed_line_entity: Entity,
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn main_display(&mut self, world: &mut World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_diagram, (ui, self.displayed_line_entity))
        {
            error!(
                "UI Error while displaying diagram ({}): {}",
                self.displayed_line_entity, e
            )
        }
    }
    fn id(&self) -> egui::Id {
        egui::Id::new(self.displayed_line_entity)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    displayed_lines: Populated<Ref<DisplayedLine>>,
    vehicles_query: Populated<(Entity, &Name, &VehicleSchedule, &VehicleScheduleCache)>,
    entry_parents: Query<&ChildOf, With<TimetableEntry>>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    station_names: Query<&Name, With<Station>>,
    station_updated: Query<&StationCache, Changed<StationCache>>,
    station_caches: Query<&StationCache, With<Station>>,
    mut selected_element: ResMut<SelectedElement>,
    mut timetable_adjustment_writer: MessageWriter<AdjustTimetableEntry>,
    mut page_cache: Local<PageCache<Entity, DiagramPageCache>>,
    mut visible_stations_scratch: Local<Vec<(Entity, f32)>>,
    time: Res<Time>,
) {
    let Ok(displayed_line) = displayed_lines.get(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };
    let state = page_cache.get_mut_or_insert_with(displayed_line_entity, DiagramPageCache::default);

    state.stroke.color = ui.visuals().text_color();

    let entries_updated = displayed_line.is_changed()
        || state.vehicle_entities.is_empty()
        || displayed_line
            .stations
            .iter()
            .copied()
            .any(|(s, _)| station_updated.get(s).is_ok());

    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;

    if entries_updated {
        for station in displayed_line
            .stations
            .iter()
            .filter_map(|(s, _)| station_caches.get(*s).ok())
        {
            station.passing_vehicles(&mut state.vehicle_entities, |e| entry_parents.get(e).ok());
        }
        state.vehicle_entities.sort();
        state.vehicle_entities.dedup();
    }

    ensure_heights(state, &displayed_line);

    Frame::canvas(ui.style())
        .fill(if ui.visuals().dark_mode {
            Color32::BLACK
        } else {
            Color32::WHITE
        })
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            let (response, mut painter) =
                ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

            let (vertical_visible, horizontal_visible, ticks_per_screen_unit) =
                calculate_visible_ranges(state, &response.rect);

            // `get_visible_stations` returns a slice borrowed from `state`, so copy it out
            // (into a reusable buffer) to avoid holding an immutable borrow of `state`
            // across later mutations.
            visible_stations_scratch.clear();
            visible_stations_scratch
                .extend_from_slice(state.get_visible_stations(vertical_visible.clone()));
            let visible_stations: &[(Entity, f32)] = visible_stations_scratch.as_slice();

            draw_station_lines(
                state.vertical_offset,
                state.stroke,
                &mut painter,
                &response.rect,
                state.zoom.y,
                visible_stations,
                ui.pixels_per_point(),
                ui.visuals().text_color(),
                |e| station_names.get(e).ok().map(|s| s.as_str()),
            );

            draw_time_lines(
                state.tick_offset,
                state.stroke,
                &mut painter,
                &response.rect,
                ticks_per_screen_unit,
                &horizontal_visible,
                ui.pixels_per_point(),
            );

            let rendered_vehicles = collect_rendered_vehicles(
                |e| timetable_entries.get(e).ok(),
                |e| vehicles_query.get(e).ok(),
                visible_stations,
                &horizontal_visible,
                ticks_per_screen_unit,
                state,
                &response.rect,
                &state.vehicle_entities,
            );

            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                handle_input_selection(
                    pos,
                    &rendered_vehicles,
                    visible_stations,
                    &response.rect,
                    state.vertical_offset,
                    state.zoom.y,
                    &mut state.interaction_acc_time,
                    &mut selected_element,
                );
            }

            draw_vehicles(
                &mut painter,
                &rendered_vehicles,
                state,
                &mut selected_element,
                &time,
                ui.ctx(),
            );

            match selected_element.0 {
                None => {}
                Some(SelectedEntityType::Vehicle(v)) => {
                    draw_vehicle_selection_overlay(
                        ui,
                        &mut painter,
                        &rendered_vehicles,
                        state,
                        v,
                        &mut timetable_adjustment_writer,
                        &station_names,
                    );
                }
                Some(SelectedEntityType::TimetableEntry {
                    entry: e,
                    vehicle: v,
                }) => {}
                Some(SelectedEntityType::Interval(i)) => {
                    draw_interval_selection_overlay(
                        ui,
                        &mut painter,
                        response.rect,
                        state.vertical_offset,
                        state.zoom.y,
                        i,
                        visible_stations,
                    );
                }
                Some(SelectedEntityType::Station((s))) => {
                    draw_station_selection_overlay(
                        ui,
                        &mut painter,
                        response.rect,
                        state.vertical_offset,
                        state.zoom.y,
                        s,
                        visible_stations,
                    );
                }
                Some(SelectedEntityType::Map(_)) => {
                    todo!("Do this bro")
                }
            }

            handle_navigation(ui, &response, state);
        });
}

fn ensure_heights(state: &mut DiagramPageCache, displayed_line: &DisplayedLine) {
    if state.heights.is_none() {
        let mut current_height = 0.0;
        let mut heights = Vec::new();
        for (station, distance) in &displayed_line.stations {
            current_height += distance.abs().log2().max(1.0) * 15f32;
            heights.push((*station, current_height))
        }
        state.heights = Some(heights);
    }
}

fn calculate_visible_ranges(
    state: &DiagramPageCache,
    rect: &Rect,
) -> (std::ops::Range<f32>, std::ops::Range<i64>, f64) {
    let vertical_visible =
        state.vertical_offset..rect.height() / state.zoom.y + state.vertical_offset;
    let horizontal_visible =
        state.tick_offset..state.tick_offset + (rect.width() as f64 / state.zoom.x as f64) as i64;
    let ticks_per_screen_unit =
        (horizontal_visible.end - horizontal_visible.start) as f64 / rect.width() as f64;
    (vertical_visible, horizontal_visible, ticks_per_screen_unit)
}

fn collect_rendered_vehicles<'a, F, G>(
    get_timetable_entries: F,
    get_vehicles: G,
    visible_stations: &[(Entity, f32)],
    horizontal_visible: &std::ops::Range<i64>,
    ticks_per_screen_unit: f64,
    state: &DiagramPageCache,
    screen_rect: &Rect,
    vehicles: &[Entity],
) -> Vec<RenderedVehicle<'a>>
where
    F: Fn(Entity) -> Option<(&'a TimetableEntry, &'a TimetableEntryCache)> + Copy + 'a,
    G: Fn(
            Entity,
        ) -> Option<(
            Entity,
            &'a Name,
            &'a VehicleSchedule,
            &'a VehicleScheduleCache,
        )> + 'a,
{
    let mut rendered_vehicles = Vec::new();
    for (vehicle_entity, _name, schedule, schedule_cache) in
        vehicles.iter().copied().filter_map(|e| get_vehicles(e))
    {
        let Some(visible_sets) = schedule_cache.get_entries_range(
            schedule,
            TimetableTime((horizontal_visible.start / TICKS_PER_SECOND) as i32)
                ..TimetableTime((horizontal_visible.end / TICKS_PER_SECOND) as i32),
            get_timetable_entries,
        ) else {
            continue;
        };

        let mut segments = Vec::new();
        for (initial_offset, set) in visible_sets {
            let mut current_segment: Vec<PointData> = Vec::new();
            for ((entry, entry_cache), timetable_entity) in set {
                let Some(estimate) = &entry_cache.estimate else {
                    if !current_segment.is_empty() {
                        segments.push(std::mem::take(&mut current_segment));
                    }
                    continue;
                };
                let Some((_, h)) = visible_stations.iter().find(|(s, _)| *s == entry.station)
                else {
                    if !current_segment.is_empty() {
                        segments.push(std::mem::take(&mut current_segment));
                    }
                    continue;
                };

                let draw_height = (*h - state.vertical_offset) * state.zoom.y + screen_rect.top();

                let arrival_pos = Pos2 {
                    x: ticks_to_screen_x(
                        (estimate.arrival.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
                        screen_rect,
                        ticks_per_screen_unit,
                        state.tick_offset,
                    ),
                    y: draw_height,
                };

                let mut departure_pos = None;
                if let Some(departure_mode) = entry.departure
                    && !matches!(departure_mode, TravelMode::Flexible)
                {
                    departure_pos = Some(Pos2 {
                        x: ticks_to_screen_x(
                            (estimate.departure.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
                            screen_rect,
                            ticks_per_screen_unit,
                            state.tick_offset,
                        ),
                        y: draw_height,
                    });
                }
                current_segment.push((
                    arrival_pos,
                    departure_pos,
                    (entry, entry_cache),
                    timetable_entity,
                ))
            }
            if !current_segment.is_empty() {
                segments.push(current_segment);
            }
        }
        rendered_vehicles.push(RenderedVehicle {
            segments,
            stroke: state.stroke,
            entity: vehicle_entity,
        });
    }
    rendered_vehicles
}

fn handle_input_selection(
    pointer_pos: Pos2,
    rendered_vehicles: &[RenderedVehicle],
    visible_stations: &[(Entity, f32)],
    screen_rect: &Rect,
    vertical_offset: f32,
    zoom_y: f32,
    interaction_acc_time: &mut f32,
    selected_entity: &mut Option<SelectedEntityType>,
) {
    const VEHICLE_SELECTION_RADIUS: f32 = 7.0;
    const STATION_SELECTION_RADIUS: f32 = VEHICLE_SELECTION_RADIUS;
    *interaction_acc_time = 0.0;
    if selected_entity.is_some() {
        *selected_entity = None;
        return;
    };
    let mut found: Option<SelectedEntityType> = None;
    'check_selected: for vehicle in rendered_vehicles {
        for segment in &vehicle.segments {
            let mut points = segment
                .iter()
                .flat_map(|(a_pos, d_pos, ..)| std::iter::once(*a_pos).chain(*d_pos));

            if let Some(mut curr) = points.next() {
                for next in points {
                    let a = pointer_pos.x - curr.x;
                    let b = pointer_pos.y - curr.y;
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
                    let dx = pointer_pos.x - px;
                    let dy = pointer_pos.y - py;

                    if dx * dx + dy * dy < VEHICLE_SELECTION_RADIUS.powi(2) {
                        found = Some(SelectedEntityType::Vehicle(vehicle.entity));
                        break 'check_selected;
                    }
                    curr = next;
                }
            }
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
    // Handle station lines after vehicle lines,
    for (station_entity, height) in visible_stations {
        let y = (*height - vertical_offset) * zoom_y + screen_rect.top();
        if (y - STATION_SELECTION_RADIUS..y + STATION_SELECTION_RADIUS).contains(&pointer_pos.y) {
            found = Some(SelectedEntityType::Station(*station_entity));
            break;
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
    for w in visible_stations.windows(2) {
        let [(e1, h1), (e2, h2)] = w else {
            continue;
        };
        let y1 = (*h1 - vertical_offset) * zoom_y + screen_rect.top();
        let y2 = (*h2 - vertical_offset) * zoom_y + screen_rect.top();
        let (min_y, max_y) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        if (min_y..max_y).contains(&pointer_pos.y) {
            found = Some(SelectedEntityType::Interval((*e1, *e2)));
            break;
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
}

fn draw_vehicles(
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
    selected_entity: &mut Option<SelectedEntityType>,
    time: &Res<Time>,
    ctx: &egui::Context,
) {
    let mut selected_vehicle = None;

    for vehicle in rendered_vehicles {
        if selected_vehicle.is_none()
            && let Some(selected_entity) = selected_entity
            && matches!(selected_entity, SelectedEntityType::Vehicle(e) if *e == vehicle.entity)
        {
            selected_vehicle = Some(vehicle);
            continue;
        }

        for segment in &vehicle.segments {
            let points = segment
                .iter()
                .flat_map(|(a, d, ..)| std::iter::once(*a).chain(*d))
                .collect::<Vec<_>>();
            painter.line(points, vehicle.stroke);
        }
    }

    if selected_vehicle.is_none() {
        state.background_acc_time -= time.delta_secs();
    } else {
        state.background_acc_time += time.delta_secs();
    }
    state.interaction_acc_time += time.delta_secs();
    state.background_acc_time = state.background_acc_time.clamp(0.0, LINE_ANIMATION_TIME);
    state.interaction_acc_time = state.interaction_acc_time.clamp(0.0, LINE_ANIMATION_TIME);
    if (f32::EPSILON..LINE_ANIMATION_TIME).contains(&state.background_acc_time)
        || (f32::EPSILON..LINE_ANIMATION_TIME).contains(&state.interaction_acc_time)
    {
        ctx.request_repaint();
    }
    let background_strength = state.background_acc_time / LINE_ANIMATION_TIME;
    if background_strength > 0.1 {
        painter.rect_filled(painter.clip_rect(), CornerRadius::ZERO, {
            let amt = (background_strength * 180.0) as u8;
            if ctx.theme().default_visuals().dark_mode {
                Color32::from_black_alpha(amt)
            } else {
                Color32::from_white_alpha(amt)
            }
        });
    }
}

fn draw_vehicle_selection_overlay(
    ui: &mut Ui,
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
    selected_entity: Entity,
    timetable_adjustment_writer: &mut MessageWriter<AdjustTimetableEntry>,
    station_names: &Query<&Name, With<Station>>,
) {
    let Some(vehicle) = rendered_vehicles
        .iter()
        .find(|v| selected_entity == v.entity)
    else {
        return;
    };

    let line_strength = 1.0 - ((state.interaction_acc_time / LINE_ANIMATION_TIME) - 1.0).powi(2);

    let mut stroke = vehicle.stroke;
    stroke.width = line_strength * 3.0 * stroke.width + stroke.width;

    for (line_index, segment) in vehicle.segments.iter().enumerate() {
        let mut line_vec = Vec::with_capacity(segment.len() * 2);

        for idx in 0..segment.len().saturating_sub(1) {
            let (arrival_pos, departure_pos, (_, entry_cache), _) = segment[idx];
            let (next_arrival_pos, _, (next_entry, next_entry_cache), _) = segment[idx + 1];
            let signal_stroke = Stroke {
                width: 1.0 + line_strength,
                color: if matches!(next_entry.arrival, TravelMode::For(_)) {
                    Color32::BLUE
                } else {
                    Color32::ORANGE
                },
            };

            line_vec.push(arrival_pos);
            let mut curr_pos = if let Some(d_pos) = departure_pos {
                line_vec.push(d_pos);
                d_pos
            } else {
                arrival_pos
            };

            let mut next_pos = next_arrival_pos;

            curr_pos.y += 5.0;
            next_pos.y += 5.0;

            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.x);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.y);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.x);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.y);

            let duration = next_entry_cache.estimate.as_ref().unwrap().arrival
                - entry_cache.estimate.as_ref().unwrap().departure;

            let points = if next_pos.y <= curr_pos.y {
                vec![curr_pos, Pos2::new(next_pos.x, curr_pos.y), next_pos]
            } else {
                vec![curr_pos, Pos2::new(curr_pos.x, next_pos.y), next_pos]
            };
            painter.add(Shape::line(points, signal_stroke));

            if duration != Duration(0) {
                let time = duration.to_hms();
                let text = format!("{}:{:02}", time.0 * 60 + time.1, time.2);
                let duration_text = painter.layout_no_wrap(
                    text,
                    egui::FontId::monospace(15.0),
                    signal_stroke.color,
                );
                painter.galley(
                    Pos2 {
                        x: (curr_pos.x + next_pos.x - duration_text.size().x) / 2.0,
                        y: curr_pos.y.max(next_pos.y) + 1.0,
                    },
                    duration_text,
                    signal_stroke.color,
                );
            }
        }

        if let Some(last_pos) = segment.last() {
            line_vec.push(last_pos.0);
            if let Some(departure) = last_pos.1 {
                line_vec.push(departure)
            }
        }
        painter.line(line_vec, stroke);

        let mut previous_entry: Option<(
            Pos2,
            Option<Pos2>,
            (&TimetableEntry, &TimetableEntryCache),
            ActualRouteEntry,
        )> = None;
        if state.zoom.x.min(state.zoom.y) < 0.0002 {
            continue;
        }
        for fragment in segment
            .iter()
            .cloned()
            .filter(|(_, _, _, a)| matches!(a, ActualRouteEntry::Nominal(_)))
        {
            let (mut arrival_pos, maybe_departure_pos, (entry, entry_cache), entry_entity) =
                fragment;
            const HANDLE_SIZE: f32 = 12.0;
            const CIRCLE_HANDLE_SIZE: f32 = 7.0;
            const TRIANGLE_HANDLE_SIZE: f32 = 10.0;
            const DASH_HANDLE_SIZE: f32 = 9.0;
            let departure_pos: Pos2;
            if let Some(unwrapped_pos) = maybe_departure_pos {
                if (arrival_pos.x - unwrapped_pos.x).abs() < HANDLE_SIZE {
                    let midpoint_x = (arrival_pos.x + unwrapped_pos.x) / 2.0;
                    arrival_pos.x = midpoint_x - HANDLE_SIZE / 2.0;
                    let mut pos = unwrapped_pos;
                    pos.x = midpoint_x + HANDLE_SIZE / 2.0;
                    departure_pos = pos;
                } else {
                    departure_pos = unwrapped_pos;
                }
            } else {
                arrival_pos.x -= HANDLE_SIZE / 2.0;
                let mut pos = arrival_pos;
                pos.x += HANDLE_SIZE;
                departure_pos = pos;
            };
            let arrival_point_response =
                ui.put(Rect::from_pos(arrival_pos).expand(5.2), |ui: &mut Ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                    ui
                        .scope_builder(
                            UiBuilder::new()
                                .sense(resp.sense)
                                .max_rect(rect)
                                .id(entry_entity.inner().to_bits() as u128
                                    | (line_index as u128) << 64),
                            |ui| {
                                ui.set_min_size(ui.available_size());
                                let response = ui.response();
                                let fill = if response.hovered() {
                                    Color32::GRAY
                                } else {
                                    Color32::WHITE
                                };
                                let handle_stroke = Stroke {
                                    width: 2.5,
                                    color: stroke.color,
                                };
                                match entry.arrival {
                                    TravelMode::At(_) => buttons::circle_button_shape(
                                        painter,
                                        arrival_pos,
                                        CIRCLE_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                    TravelMode::For(_) => buttons::dash_button_shape(
                                        painter,
                                        arrival_pos,
                                        DASH_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                    TravelMode::Flexible => buttons::triangle_button_shape(
                                        painter,
                                        arrival_pos,
                                        TRIANGLE_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                };
                            },
                        )
                        .response
                });

            if arrival_point_response.drag_started() {
                state.previous_total_drag_delta = None;
            }
            if let Some(total_drag_delta) = arrival_point_response.total_drag_delta() {
                let previous_drag_delta = state.previous_total_drag_delta.unwrap_or(0.0);
                let duration = Duration(
                    ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                        / state.zoom.x as f64
                        / TICKS_PER_SECOND as f64) as i32,
                );
                if duration != Duration(0) {
                    timetable_adjustment_writer.write(AdjustTimetableEntry {
                        entity: entry_entity.inner(),
                        adjustment: crate::vehicles::TimetableAdjustment::AdjustArrivalTime(
                            duration,
                        ),
                    });
                    state.previous_total_drag_delta = Some(
                        previous_drag_delta
                            + (duration.0 as f64 * TICKS_PER_SECOND as f64 * state.zoom.x as f64)
                                as f32,
                    );
                }
            }
            if arrival_point_response.drag_stopped() {
                state.previous_total_drag_delta = None;
            }
            if arrival_point_response.dragged() {
                arrival_point_response.show_tooltip_ui(|ui| {
                    ui.label(entry_cache.estimate.as_ref().unwrap().arrival.to_string());
                    ui.label(
                        station_names
                            .get(entry.station)
                            .map_or("??", |s| s.as_str()),
                    );
                });
            } else {
                Popup::menu(&arrival_point_response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            entry_entity.inner(),
                            (entry, entry_cache),
                            previous_entry.map(|e| {
                                let e = e.2;
                                (e.0, e.1)
                            }),
                            timetable_adjustment_writer,
                            ui,
                            true,
                        );
                    });
            }

            let departure_point_response =
                ui.put(Rect::from_pos(departure_pos).expand(4.5), |ui: &mut Ui| {
                    let (rect, resp) = ui.allocate_exact_size(
                        ui.available_size(),
                        if matches!(
                            entry.departure.unwrap_or(TravelMode::Flexible),
                            TravelMode::Flexible
                        ) {
                            Sense::click()
                        } else {
                            Sense::click_and_drag()
                        },
                    );
                    ui.scope_builder(
                        UiBuilder::new()
                            .sense(resp.sense)
                            .max_rect(rect)
                            .id((entry_entity.inner().to_bits() as u128
                                | (line_index as u128) << 64)
                                ^ (1 << 127)),
                        |ui| {
                            ui.set_min_size(ui.available_size());
                            let response = ui.response();
                            let fill = if response.hovered() {
                                Color32::GRAY
                            } else {
                                Color32::WHITE
                            };
                            let handle_stroke = Stroke {
                                width: 2.5,
                                color: stroke.color,
                            };
                            match entry.departure {
                                Some(TravelMode::At(_)) => buttons::circle_button_shape(
                                    painter,
                                    departure_pos,
                                    CIRCLE_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                Some(TravelMode::For(_)) => buttons::dash_button_shape(
                                    painter,
                                    departure_pos,
                                    DASH_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                Some(TravelMode::Flexible) => buttons::triangle_button_shape(
                                    painter,
                                    departure_pos,
                                    TRIANGLE_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                None => buttons::double_triangle(
                                    painter,
                                    departure_pos,
                                    DASH_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                            };
                        },
                    )
                    .response
                });

            if departure_point_response.drag_started() {
                state.previous_total_drag_delta = None;
            }
            if let Some(total_drag_delta) = departure_point_response.total_drag_delta() {
                let previous_drag_delta = state.previous_total_drag_delta.unwrap_or(0.0);
                let duration = Duration(
                    ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                        / state.zoom.x as f64
                        / TICKS_PER_SECOND as f64) as i32,
                );
                if duration != Duration(0) {
                    timetable_adjustment_writer.write(AdjustTimetableEntry {
                        entity: entry_entity.inner(),
                        adjustment: crate::vehicles::TimetableAdjustment::AdjustDepartureTime(
                            duration,
                        ),
                    });
                    state.previous_total_drag_delta = Some(
                        previous_drag_delta
                            + (duration.0 as f64 * TICKS_PER_SECOND as f64 * state.zoom.x as f64)
                                as f32,
                    );
                }
            }
            if departure_point_response.drag_stopped() {
                state.previous_total_drag_delta = None;
            }
            if departure_point_response.dragged() {
                departure_point_response.show_tooltip_ui(|ui| {
                    ui.label(entry_cache.estimate.as_ref().unwrap().departure.to_string());
                    ui.label(
                        station_names
                            .get(entry.station)
                            .map_or("??", |s| s.as_str()),
                    );
                });
            } else {
                Popup::menu(&departure_point_response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            entry_entity.inner(),
                            (entry, entry_cache),
                            previous_entry.map(|e| {
                                let e = e.2;
                                (e.0, e.1)
                            }),
                            timetable_adjustment_writer,
                            ui,
                            false,
                        );
                    });
                previous_entry = Some(fragment);
            }
        }
    }
}

fn draw_station_selection_overlay(
    ui: &mut Ui,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    station_entity: Entity,
    visible_stations: &[(Entity, f32)],
) {
    let stations = visible_stations
        .iter()
        .copied()
        .filter_map(|(s, h)| if s == station_entity { Some(h) } else { None });
    for station in stations {
        let station_height = (station - vertical_offset) * zoom_y + screen_rect.top();
        painter.rect(
            Rect::from_two_pos(
                Pos2 {
                    x: screen_rect.left(),
                    y: station_height,
                },
                Pos2 {
                    x: screen_rect.right(),
                    y: station_height,
                },
            )
            .expand2(Vec2 { x: -1.0, y: 7.0 }),
            4,
            Color32::BLUE.linear_multiply(0.5),
            Stroke::new(1.0, Color32::BLUE),
            egui::StrokeKind::Middle,
        );
    }
}

fn draw_interval_selection_overlay(
    ui: &mut Ui,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    (s1, s2): (Entity, Entity),
    visible_stations: &[(Entity, f32)],
) {
    for w in visible_stations.windows(2) {
        let [(e1, h1), (e2, h2)] = w else { continue };
        if !((*e1 == s1 && *e2 == s2) || (*e1 == s2 && *e2 == s1)) {
            continue;
        }
        let station_height_1 = (h1 - vertical_offset) * zoom_y + screen_rect.top();
        let station_height_2 = (h2 - vertical_offset) * zoom_y + screen_rect.top();
        painter.rect(
            Rect::from_two_pos(
                Pos2 {
                    x: screen_rect.left(),
                    y: station_height_1,
                },
                Pos2 {
                    x: screen_rect.right(),
                    y: station_height_2,
                },
            )
            .expand2(Vec2 { x: -1.0, y: 7.0 }),
            4,
            Color32::GREEN.linear_multiply(0.5),
            Stroke::new(1.0, Color32::GREEN),
            egui::StrokeKind::Middle,
        );
    }
}

fn handle_navigation(ui: &mut Ui, response: &response::Response, state: &mut DiagramPageCache) {
    let mut zoom_delta: Vec2 = Vec2::default();
    let mut translation_delta: Vec2 = Vec2::default();
    ui.input(|input| {
        zoom_delta = input.zoom_delta_2d();
        translation_delta = input.translation_delta();
    });
    if let Some(pos) = response.hover_pos() {
        let old_zoom = state.zoom;
        let mut new_zoom = state.zoom * zoom_delta;
        new_zoom.x = new_zoom.x.clamp(0.00001, 0.4);
        new_zoom.y = new_zoom.y.clamp(0.025, 2048.0);
        let rel_pos = (pos - response.rect.min) / response.rect.size();
        let world_width_before = response.rect.width() as f64 / old_zoom.x as f64;
        let world_width_after = response.rect.width() as f64 / new_zoom.x as f64;
        let world_pos_before_x = state.tick_offset as f64 + rel_pos.x as f64 * world_width_before;
        let new_tick_offset =
            (world_pos_before_x - rel_pos.x as f64 * world_width_after).round() as i64;
        let world_height_before = response.rect.height() as f64 / old_zoom.y as f64;
        let world_height_after = response.rect.height() as f64 / new_zoom.y as f64;
        let world_pos_before_y =
            state.vertical_offset as f64 + rel_pos.y as f64 * world_height_before;
        let new_vertical_offset =
            (world_pos_before_y - rel_pos.y as f64 * world_height_after) as f32;
        state.zoom = new_zoom;
        state.tick_offset = new_tick_offset;
        state.vertical_offset = new_vertical_offset;
    }

    let ticks_per_screen_unit = 1.0 / state.zoom.x as f64;

    state.tick_offset -=
        (ticks_per_screen_unit * (response.drag_delta().x + translation_delta.x) as f64) as i64;
    state.vertical_offset -= (response.drag_delta().y + translation_delta.y) / state.zoom.y;
    state.tick_offset = state.tick_offset.clamp(
        -366 * 86400 * TICKS_PER_SECOND,
        366 * 86400 * TICKS_PER_SECOND
            - (response.rect.width() as f64 / state.zoom.x as f64) as i64,
    );
    const TOP_BOTTOM_PADDING: f32 = 30.0;
    let max_height = state
        .heights
        .as_ref()
        .unwrap()
        .last()
        .map(|(_, h)| *h)
        .unwrap_or(0.0);
    state.vertical_offset = if response.rect.height() / state.zoom.y
        > (max_height + TOP_BOTTOM_PADDING * 2.0 / state.zoom.y)
    {
        (-response.rect.height() / state.zoom.y + max_height) / 2.0
    } else {
        state.vertical_offset.clamp(
            -TOP_BOTTOM_PADDING / state.zoom.y,
            max_height - response.rect.height() / state.zoom.y + TOP_BOTTOM_PADDING / state.zoom.y,
        )
    }
}

fn draw_station_lines<'a, F>(
    vertical_offset: f32,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    zoom: f32,
    to_draw: &[(Entity, f32)],
    pixels_per_point: f32,
    text_color: Color32,
    mut get_station_name: F,
) where
    F: FnMut(Entity) -> Option<&'a str>,
{
    // Guard against invalid zoom
    for (entity, height) in to_draw.iter().copied() {
        let mut draw_height = (height - vertical_offset) * zoom + screen_rect.top();
        stroke.round_center_to_pixel(pixels_per_point, &mut draw_height);
        painter.hline(
            screen_rect.left()..=screen_rect.right(),
            draw_height,
            stroke,
        );
        let Some(station_name) = get_station_name(entity) else {
            continue;
        };
        let layout = painter.layout_no_wrap(
            station_name.to_string(),
            egui::FontId::proportional(13.0),
            text_color,
        );
        let layout_pos = Pos2 {
            x: screen_rect.left(),
            y: draw_height - layout.size().y,
        };
        painter.galley(layout_pos, layout, text_color);
    }
}

fn ticks_to_screen_x(
    ticks: i64,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    offset_ticks: i64,
) -> f32 {
    let base = (ticks - offset_ticks) as f64 / ticks_per_screen_unit;
    screen_rect.left() + base as f32
}

/// Draw vertical time lines and labels
fn draw_time_lines(
    tick_offset: i64,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    visible_ticks: &std::ops::Range<i64>,
    pixels_per_point: f32,
) {
    const MAX_SCREEN_WIDTH: f64 = 64.0;
    const MIN_SCREEN_WIDTH: f64 = 32.0;
    const SIZES: &[i64] = &[
        TICKS_PER_SECOND * 1,            // 1 second
        TICKS_PER_SECOND * 10,           // 10 seconds
        TICKS_PER_SECOND * 30,           // 30 seconds
        TICKS_PER_SECOND * 60,           // 1 minute
        TICKS_PER_SECOND * 60 * 5,       // 5 minutes
        TICKS_PER_SECOND * 60 * 10,      // 10 minutes
        TICKS_PER_SECOND * 60 * 30,      // 30 minutes
        TICKS_PER_SECOND * 60 * 60,      // 1 hour
        TICKS_PER_SECOND * 60 * 60 * 4,  // 4 hours
        TICKS_PER_SECOND * 60 * 60 * 24, // 1 day
    ];
    let mut drawn: Vec<i64> = Vec::with_capacity(30);

    // align the first tick to a spacing boundary that is <= visible start.
    let first_visible_position = SIZES
        .iter()
        .position(|s| *s as f64 / ticks_per_screen_unit * 1.5 > MIN_SCREEN_WIDTH)
        .unwrap_or(0);
    let visible = &SIZES[first_visible_position..];
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start - visible_ticks.start.rem_euclid(*spacing) - spacing;
        let mut tick = first;
        let strength = (((*spacing as f64 / ticks_per_screen_unit * 1.5) - MIN_SCREEN_WIDTH)
            / (MAX_SCREEN_WIDTH - MIN_SCREEN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        let mut current_stroke = stroke;
        if strength.is_finite() {
            // strange bug here
            current_stroke.color = current_stroke.color.gamma_multiply(strength as f32);
        }
        current_stroke.width = 0.5;
        while tick <= visible_ticks.end {
            tick += *spacing;
            if drawn.contains(&tick) {
                continue;
            }
            let mut x = ticks_to_screen_x(tick, screen_rect, ticks_per_screen_unit, tick_offset);
            current_stroke.round_center_to_pixel(pixels_per_point, &mut x);
            painter.vline(x, screen_rect.top()..=screen_rect.bottom(), current_stroke);
            drawn.push(tick);
            let time = TimetableTime((tick / 100) as i32);
            let text = match i + first_visible_position {
                0..=2 => time.to_hmsd().2.to_string(),
                3..=8 => format!("{}:{:02}", time.to_hmsd().0, time.to_hmsd().1),
                _ => time.to_string(),
            };
            let label = painter.layout_no_wrap(text, FontId::monospace(13.0), current_stroke.color);
            painter.galley(
                Pos2 {
                    x: x - label.size().x / 2.0,
                    y: screen_rect.top(),
                },
                label,
                current_stroke.color,
            );
        }
    }
}
