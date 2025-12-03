use core::f32;

use super::PageCache;
use crate::{
    interface::widgets::timetable_popup,
    intervals::Station,
    lines::DisplayedLine,
    units::time::{Duration, TimetableTime},
    vehicles::{
        AdjustTimetableEntry,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
    },
};
use bevy::{math::NormedVectorSpace, prelude::*};
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Popup, Pos2, Rect, Response, Sense,
    Shape, Stroke, Ui, UiBuilder, Vec2, emath,
    epaint::{CubicBezierShape, PathShape, QuadraticBezierShape},
    layers::ShapeIdx,
    response, vec2,
};

// Time and time-canvas related constants
const SECONDS_PER_WORLD_UNIT: f64 = 1.0; // world units -> seconds
const TICKS_PER_SECOND: i64 = 100;
const TICKS_PER_WORLD_UNIT: f64 = SECONDS_PER_WORLD_UNIT * TICKS_PER_SECOND as f64;
const LINE_ANIMATION_TIME: f32 = 0.2; // 0.2 seconds

pub struct DiagramPageCache {
    selected_line: Option<Entity>,
    background_acc_time: f32,
    interaction_acc_time: f32,
    previous_total_drag_delta: Option<f32>,
    stroke: Stroke,
    tick_offset: i64,
    vertical_offset: f32,
    heights: Option<Vec<(Entity, f32)>>,
    zoom: Vec2,
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
            selected_line: None,
            tick_offset: 0,
            vertical_offset: 0.0,
            stroke: Stroke {
                width: 1.0,
                color: Color32::BLACK,
            },
            heights: None,
            zoom: vec2(0.0005, 1.0),
        }
    }
}

type PointData<'a> = (Pos2, Option<Pos2>, &'a TimetableEntry, Entity);

struct RenderedVehicle<'a> {
    segments: Vec<Vec<PointData<'a>>>,
    stroke: Stroke,
    entity: Entity,
}

pub fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    mut displayed_lines: Populated<&mut DisplayedLine>,
    vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: Query<&TimetableEntry>,
    station_names: Query<&Name, With<Station>>,
    mut timetable_adjustment_writer: MessageWriter<AdjustTimetableEntry>,
    mut page_cache: Local<PageCache<Entity, DiagramPageCache>>,
    time: Res<Time>,
) {
    let Ok(mut displayed_line) = displayed_lines.get_mut(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };
    let state = page_cache.get_mut_or_insert_with(displayed_line_entity, DiagramPageCache::default);

    setup_ui_style(ui);
    ensure_heights(state, &displayed_line);

    Frame::canvas(ui.style())
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            let (mut response, mut painter) =
                ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

            let (vertical_visible, horizontal_visible, ticks_per_screen_unit) =
                calculate_visible_ranges(state, &response.rect);

            let visible_stations = state.get_visible_stations(vertical_visible.clone());

            draw_station_lines(
                state.vertical_offset,
                state.stroke,
                &mut painter,
                &response.rect,
                state.zoom.y,
                visible_stations,
                ui.pixels_per_point(),
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
                &displayed_line,
                &vehicles,
                &timetable_entries,
                visible_stations,
                &horizontal_visible,
                ticks_per_screen_unit,
                state,
                &response.rect,
            );

            handle_input_selection(&response, &rendered_vehicles, state);

            draw_vehicles(&mut painter, &rendered_vehicles, state, &time);

            draw_selection_overlay(
                ui,
                &mut painter,
                &rendered_vehicles,
                state,
                &mut timetable_adjustment_writer,
                &station_names,
            );

            handle_navigation(ui, &response, state);
        });
}

fn setup_ui_style(ui: &mut Ui) {
    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;
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

fn collect_rendered_vehicles<'a>(
    displayed_line: &DisplayedLine,
    vehicles: &Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: &'a Query<&TimetableEntry>,
    visible_stations: &[(Entity, f32)],
    horizontal_visible: &std::ops::Range<i64>,
    ticks_per_screen_unit: f64,
    state: &DiagramPageCache,
    screen_rect: &Rect,
) -> Vec<RenderedVehicle<'a>> {
    let mut rendered_vehicles = Vec::new();
    if let Some(children) = displayed_line.children.as_ref() {
        for (vehicle_entity, _name, schedule) in
            children.iter().filter_map(|e| vehicles.get(*e).ok())
        {
            let Some(visible_sets) = schedule.get_entries_range(
                TimetableTime((horizontal_visible.start / TICKS_PER_SECOND) as i32)
                    ..TimetableTime((horizontal_visible.end / TICKS_PER_SECOND) as i32),
                timetable_entries,
            ) else {
                continue;
            };

            let mut segments = Vec::new();
            for (initial_offset, set) in visible_sets {
                let mut current_segment: Vec<PointData> = Vec::new();
                for (entry, timetable_entity) in set {
                    let (Some(arrival_estimate), Some(departure_estimate)) =
                        (entry.arrival_estimate, entry.departure_estimate)
                    else {
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

                    let draw_height =
                        (*h - state.vertical_offset) * state.zoom.y + screen_rect.top();

                    let arrival_pos = Pos2 {
                        x: ticks_to_screen_x(
                            (arrival_estimate.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
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
                                (departure_estimate.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
                                screen_rect,
                                ticks_per_screen_unit,
                                state.tick_offset,
                            ),
                            y: draw_height,
                        });
                    }
                    current_segment.push((arrival_pos, departure_pos, entry, timetable_entity))
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
    }
    rendered_vehicles
}

fn handle_input_selection(
    response: &response::Response,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
) {
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let mut found = false;
        'check_selected: for vehicle in rendered_vehicles {
            for segment in &vehicle.segments {
                let mut points = segment
                    .iter()
                    .flat_map(|(a_pos, d_pos, ..)| std::iter::once(*a_pos).chain(*d_pos));

                if let Some(mut curr) = points.next() {
                    for next in points {
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

                        if dx * dx + dy * dy < 49.0 {
                            if let Some(selected_line) = state.selected_line
                                && selected_line == vehicle.entity
                            {
                            } else {
                                state.interaction_acc_time = 0.0;
                                state.selected_line = Some(vehicle.entity);
                            }
                            found = true;
                            break 'check_selected;
                        }
                        curr = next;
                    }
                }
            }
        }
        if !found {
            state.selected_line = None;
        }
    }
}

fn draw_vehicles(
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
    time: &Res<Time>,
) {
    let mut selected_vehicle = None;

    for vehicle in rendered_vehicles {
        if selected_vehicle.is_none()
            && let Some(selected_entity) = state.selected_line
            && selected_entity == vehicle.entity
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

    let background_strength =
        1.0 - ((state.background_acc_time / LINE_ANIMATION_TIME) - 1.0).powi(2);
    if background_strength > 0.1 {
        painter.rect_filled(
            painter.clip_rect(),
            CornerRadius::ZERO,
            Color32::from_additive_luminance((background_strength * 180.0) as u8),
        );
    }
}

fn draw_selection_overlay(
    ui: &mut Ui,
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
    timetable_adjustment_writer: &mut MessageWriter<AdjustTimetableEntry>,
    station_names: &Query<&Name, With<Station>>,
) {
    let Some(selected_entity) = state.selected_line else {
        return;
    };
    let Some(vehicle) = rendered_vehicles
        .iter()
        .find(|v| v.entity == selected_entity)
    else {
        return;
    };

    let line_strength = 1.0 - ((state.interaction_acc_time / LINE_ANIMATION_TIME) - 1.0).powi(2);

    let mut stroke = vehicle.stroke;
    stroke.width = line_strength * 3.0 * stroke.width + stroke.width;

    for (line_index, segment) in vehicle.segments.iter().enumerate() {
        let mut line_vec = Vec::with_capacity(segment.len() * 2);

        for idx in 0..segment.len().saturating_sub(1) {
            let (arrival_pos, departure_pos, entry, _) = segment[idx];
            let (next_arrival_pos, _, next_entry, _) = segment[idx + 1];
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

            let duration = next_entry.arrival_estimate.unwrap() - entry.departure_estimate.unwrap();

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

        let mut previous_entry: Option<(Pos2, Option<Pos2>, &TimetableEntry, Entity)> = None;
        for fragment in segment.iter().cloned() {
            let (mut arrival_pos, maybe_departure_pos, entry, entry_entity) = fragment;
            const HANDLE_SIZE: f32 = 4.5 + 4.5;
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
                    ui.scope_builder(
                        UiBuilder::new()
                            .sense(resp.sense)
                            .max_rect(rect)
                            .id(entry_entity.to_bits() as u128 | (line_index as u128) << 64),
                        // .id((entry_entity.to_bits() as u128) | ((line_index as u128) << 64)),
                        |ui| {
                            ui.set_min_size(ui.available_size());
                            let response = ui.response();
                            let fill = if matches!(entry.arrival, TravelMode::Flexible) {
                                stroke.color
                            } else if response.hovered() {
                                Color32::GRAY
                            } else {
                                Color32::WHITE
                            };
                            let handle_stroke = Stroke {
                                width: 2.0,
                                color: stroke.color,
                            };
                            Frame::canvas(ui.style())
                                .fill(fill)
                                .stroke(handle_stroke)
                                .corner_radius(CornerRadius::same(255))
                                .show(ui, |ui| {
                                    ui.set_min_size(ui.available_size());
                                });
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
                        entity: entry_entity,
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
                    ui.label(
                        entry
                            .arrival_estimate
                            .map_or("??".to_string(), |t| t.to_string()),
                    );
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
                            entry_entity,
                            entry,
                            previous_entry.map(|e| e.2),
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
                        UiBuilder::new().sense(resp.sense).max_rect(rect).id(
                            (entry_entity.to_bits() as u128 | (line_index as u128) << 64)
                                ^ (1 << 127),
                        ),
                        |ui| {
                            ui.set_min_size(ui.available_size());
                            let response = ui.response();
                            let fill = if matches!(
                                entry.departure.unwrap_or(TravelMode::Flexible),
                                TravelMode::Flexible
                            ) {
                                stroke.color
                            } else if response.hovered() {
                                Color32::GRAY
                            } else {
                                Color32::WHITE
                            };
                            let handle_stroke = Stroke {
                                width: 2.0,
                                color: stroke.color,
                            };
                            Frame::canvas(ui.style())
                                .fill(fill)
                                .stroke(handle_stroke)
                                .show(ui, |ui| {
                                    ui.set_min_size(ui.available_size());
                                });
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
                        entity: entry_entity,
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
                    ui.label(
                        entry
                            .departure_estimate
                            .map_or("??".to_string(), |t| t.to_string()),
                    );
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
                            entry_entity,
                            entry,
                            previous_entry.map(|e| e.2),
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
        366 * 86400 * TICKS_PER_SECOND,
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

fn draw_station_lines(
    vertical_offset: f32,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    zoom: f32,
    to_draw: &[(Entity, f32)],
    pixels_per_point: f32,
) {
    // Guard against invalid zoom
    for (entity, height) in to_draw {
        let mut draw_height = (*height - vertical_offset) * zoom + screen_rect.top();
        stroke.round_center_to_pixel(pixels_per_point, &mut draw_height);
        painter.hline(
            screen_rect.left()..=screen_rect.right(),
            draw_height,
            stroke,
        );
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
        current_stroke.color = current_stroke.color.gamma_multiply(strength as f32);
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

fn make_triangle_between(pos1: Pos2, pos2: Pos2, base_width: f32) -> Shape {
    // pointer towards b
    let dv = pos2 - pos1;
    let hyp = pos2.distance(pos1);
    let l = dv.y / hyp * base_width;
    let h = dv.x / hyp * base_width;
    let a = pos1
        + Vec2 {
            x: l / 2.0,
            y: -h / 2.0,
        };
    let b = pos1
        - Vec2 {
            x: l / 2.0,
            y: -h / 2.0,
        };
    Shape::convex_polygon(vec![pos2, b, a], Color32::BLACK, Stroke::default())
}
