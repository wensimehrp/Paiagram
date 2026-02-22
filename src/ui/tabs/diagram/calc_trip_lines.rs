use bevy::{
    ecs::entity::{EntityHashMap, EntityHashSet},
    prelude::*,
};
use egui::{Pos2, Rect};
use rayon::prelude::*;

use crate::{
    entry::{EntryMode, EntryQuery},
    route::Route,
    settings::ProjectSettings,
    station::{Platform, PlatformEntries, Station, StationQuery},
    trip::class::{Class, DisplayedStroke},
    units::time::Tick,
};

pub struct CalcContext {
    pub route_entity: Entity,
    pub y_offset: f64,
    pub zoom_y: f32,
    pub x_offset: Tick,
    pub screen_rect: Rect,
    pub ticks_per_screen_unit: f64,
    pub visible_ticks: std::ops::Range<Tick>,
}

impl CalcContext {
    pub fn from_tab(
        tab: &super::DiagramTab,
        screen_rect: Rect,
        ticks_per_screen_unit: f64,
        visible_ticks: std::ops::Range<Tick>,
    ) -> Self {
        Self {
            route_entity: tab.route_entity,
            y_offset: tab.navi.y_offset,
            zoom_y: tab.navi.zoom.y,
            x_offset: tab.navi.x_offset,
            screen_rect,
            ticks_per_screen_unit,
            visible_ticks,
        }
    }
}

pub fn calculate_trips(
    (InMut(buf), In(route_entity)): (InMut<Vec<Entity>>, In<Entity>),
    routes: Query<&Route>,
    stations: Query<StationQuery>,
    platform_entries: Query<&PlatformEntries>,
    entries: Query<&ChildOf, With<EntryMode>>,
) {
    // TODO: cache trips
    let mut update = false;
    update |= buf.is_empty();
    if !update {
        return;
    }
    buf.clear();
    let route = routes.get(route_entity).unwrap();
    let mut trips = EntityHashSet::new();
    for station_e in route.stops.iter().cloned() {
        let a = stations.get(station_e).unwrap();
        trips.extend(
            a.passing_entries(&platform_entries)
                .map(|e| entries.get(e).unwrap().0),
        );
    }
    buf.extend(trips.iter().copied());
}

pub fn calc(
    (InMut(buf), In(ctx), InRef(trips)): (
        InMut<Vec<super::DrawnTrip>>,
        In<CalcContext>,
        InRef<[Entity]>,
    ),
    routes: Query<&Route>,
    trip_q: Query<crate::trip::TripQuery>,
    entries: Query<EntryQuery>,
    stations: Query<(), With<Station>>,
    platforms: Query<&ChildOf, With<Platform>>,
    class_strokes: Query<&DisplayedStroke, With<Class>>,
    settings: Res<ProjectSettings>,
) {
    buf.clear();

    let Ok(route) = routes.get(ctx.route_entity) else {
        return;
    };

    let heights: Vec<(Entity, f32)> = route.iter().collect();
    let route_stops: EntityHashSet = route.stops.iter().copied().collect();
    if heights.is_empty() {
        return;
    }

    let vertical_visible =
        ctx.y_offset as f32..ctx.y_offset as f32 + ctx.screen_rect.height() / ctx.zoom_y.max(f32::EPSILON);

    let visible_stations = {
        let first_visible = heights
            .iter()
            .position(|(_, h)| *h > vertical_visible.start)
            .or_else(|| {
                heights
                    .iter()
                    .rposition(|(_, h)| *h <= vertical_visible.start)
            });
        let last_visible = heights.iter().rposition(|(_, h)| *h < vertical_visible.end);
        if let (Some(first_visible), Some(mut last_visible)) = (first_visible, last_visible) {
            let first_visible = first_visible.saturating_sub(2);
            last_visible = (last_visible + 1).min(heights.len() - 1);
            &heights[first_visible..=last_visible]
        } else {
            &[]
        }
    };

    if visible_stations.is_empty() {
        return;
    }

    let resolve_stop_station = |stop: Entity| -> Option<Entity> {
        if route_stops.contains(&stop) {
            return Some(stop);
        }
        if stations.get(stop).is_ok() {
            return Some(stop);
        }
        let parent = platforms.get(stop).ok().map(|p| p.parent())?;
        if route_stops.contains(&parent) {
            return Some(parent);
        }
        None
    };

    #[derive(Clone, Copy)]
    struct TripPoint {
        arr: Pos2,
        dep: Pos2,
        entry: Entity,
    }

    #[derive(Clone, Copy)]
    struct TripEntryData {
        entity: Entity,
        station: Entity,
        arr_ticks: Option<Tick>,
        dep_ticks: Option<Tick>,
        has_departure: bool,
    }

    struct TripData {
        entity: Entity,
        stroke: DisplayedStroke,
        entries: Vec<TripEntryData>,
    }

    let use_full_trip = true;
    let stations_for_layout = if use_full_trip {
        &heights[..]
    } else {
        visible_stations
    };
    let visible_station_set: EntityHashSet = visible_stations.iter().map(|(s, _)| *s).collect();
    let mut station_index_map: EntityHashMap<Vec<usize>> = EntityHashMap::new();
    for (idx, (station, _)) in stations_for_layout.iter().enumerate() {
        station_index_map.entry(*station).or_default().push(idx);
    }

    let mut trip_data = Vec::with_capacity(trips.len());
    for trip_entity in trips.iter().copied() {
        let Ok(trip) = trip_q.get(trip_entity) else {
            continue;
        };

        let stroke = class_strokes.get(trip.class.0).copied().unwrap_or_default();

        let mut trip_entries_vec: Vec<TripEntryData> = Vec::new();
        for entry_entity in trip.schedule.iter() {
            let Ok(entry) = entries.get(entry_entity) else {
                continue;
            };
            let Some(station_entity) = resolve_stop_station(entry.stop()) else {
                continue;
            };
            let (arr_ticks, dep_ticks) = if let Some(estimate) = entry.estimate {
                let arrival_ticks = Tick::from_timetable_time(estimate.arr);
                let departure_ticks = Tick::from_timetable_time(estimate.dep);
                (Some(arrival_ticks), Some(departure_ticks))
            } else {
                (None, None)
            };

            trip_entries_vec.push(TripEntryData {
                entity: entry_entity,
                station: station_entity,
                arr_ticks,
                dep_ticks,
                has_departure: entry.mode.arr.is_some(),
            });
        }

        if trip_entries_vec.len() < 2 {
            continue;
        }

        trip_data.push(TripData {
            entity: trip_entity,
            stroke,
            entries: trip_entries_vec,
        });
    }

    let repeat_freq_ticks = Tick::from_timetable_time(crate::units::time::TimetableTime(
        settings.repeat_frequency.0,
    ));

    let drawn: Vec<super::DrawnTrip> = trip_data
        .par_iter()
        .filter_map(|trip| {
            if use_full_trip
                && !trip
                    .entries
                    .iter()
                    .any(|entry| visible_station_set.contains(&entry.station))
            {
                return None;
            }

            let mut base_min: Option<Tick> = None;
            let mut base_max: Option<Tick> = None;
            for entry in &trip.entries {
                let (Some(arrival_ticks), Some(departure_ticks)) =
                    (entry.arr_ticks, entry.dep_ticks)
                else {
                    continue;
                };
                let local_min = Tick(arrival_ticks.0.min(departure_ticks.0));
                let local_max = Tick(arrival_ticks.0.max(departure_ticks.0));
                base_min = Some(base_min.map_or(local_min, |v| v.min(local_min)));
                base_max = Some(base_max.map_or(local_max, |v| v.max(local_max)));
            }

            let Some(base_min) = base_min else {
                return None;
            };
            let Some(base_max) = base_max else {
                return None;
            };

            let (repeat_start, repeat_end) = if repeat_freq_ticks.0 > 0 {
                let start = (ctx.visible_ticks.start.0 - base_max.0).div_euclid(repeat_freq_ticks.0);
                let end = (ctx.visible_ticks.end.0 - base_min.0).div_euclid(repeat_freq_ticks.0);
                (start, end)
            } else {
                (0, 0)
            };

            let mut drawn_segments = Vec::new();
            let mut drawn_entries = Vec::new();

            for repeat in repeat_start..=repeat_end {
                let repeat_offset = repeat * repeat_freq_ticks.0;

                let first_visible = trip.entries.iter().position(|entry| {
                    let (Some(arrival_ticks), Some(departure_ticks)) =
                        (entry.arr_ticks, entry.dep_ticks)
                    else {
                        return false;
                    };
                    let arrival_ticks = arrival_ticks.0 + repeat_offset;
                    let departure_ticks = departure_ticks.0 + repeat_offset;
                    !(departure_ticks < ctx.visible_ticks.start.0
                        || arrival_ticks > ctx.visible_ticks.end.0)
                });
                let last_visible = trip.entries.iter().rposition(|entry| {
                    let (Some(arrival_ticks), Some(departure_ticks)) =
                        (entry.arr_ticks, entry.dep_ticks)
                    else {
                        return false;
                    };
                    let arrival_ticks = arrival_ticks.0 + repeat_offset;
                    let departure_ticks = departure_ticks.0 + repeat_offset;
                    !(departure_ticks < ctx.visible_ticks.start.0
                        || arrival_ticks > ctx.visible_ticks.end.0)
                });

                let Some(first_visible) = first_visible else {
                    continue;
                };
                let Some(last_visible) = last_visible else {
                    continue;
                };

                let trip_entries = if use_full_trip {
                    &trip.entries[..]
                } else {
                    let first_visible = first_visible.saturating_sub(2);
                    let last_visible = (last_visible + 2).min(trip.entries.len() - 1);
                    &trip.entries[first_visible..=last_visible]
                };

                if trip_entries.len() < 2 {
                    continue;
                }

                let mut segments: Vec<Vec<TripPoint>> = Vec::new();

                let mut local_edges: Vec<(Vec<TripPoint>, usize)> = Vec::new();
                let mut previous_indices: Vec<usize> = Vec::new();

                if let Some(first) = trip_entries.first() {
                    if let Some(indices) = station_index_map.get(&first.station) {
                        previous_indices = indices.clone();
                    }
                }

                for entry_idx in 0..trip_entries.len() {
                    let entry = &trip_entries[entry_idx];
                    let next = trip_entries.get(entry_idx + 1);

                    if previous_indices.is_empty() {
                        if let Some(next_entry) = next {
                            if let Some(indices) = station_index_map.get(&next_entry.station) {
                                previous_indices = indices.clone();
                            }
                        }
                        for (segment, _) in local_edges.drain(..) {
                            if segment.len() >= 2 {
                                segments.push(segment);
                            }
                        }
                        continue;
                    }

                    let (Some(arrival_ticks), Some(departure_ticks)) =
                        (entry.arr_ticks, entry.dep_ticks)
                    else {
                        for (segment, _) in local_edges.drain(..) {
                            if segment.len() >= 2 {
                                segments.push(segment);
                            }
                        }
                        if let Some(next_entry) = next {
                            if let Some(indices) = station_index_map.get(&next_entry.station) {
                                previous_indices = indices.clone();
                            }
                        }
                        continue;
                    };

                    let arrival_ticks = Tick(arrival_ticks.0 + repeat_offset);
                    let departure_ticks = Tick(departure_ticks.0 + repeat_offset);

                    let mut next_local_edges = Vec::new();

                    for &current_line_index in &previous_indices {
                        let Some((_, height)) = stations_for_layout.get(current_line_index) else {
                            continue;
                        };

                        let matched_idx = local_edges
                            .iter()
                            .position(|(_, idx)| current_line_index.abs_diff(*idx) <= 1);

                        let mut segment = if let Some(idx) = matched_idx {
                            local_edges.swap_remove(idx).0
                        } else {
                            Vec::new()
                        };

                        let arrival_pos = Pos2::new(
                            super::draw_lines::ticks_to_screen_x(
                                arrival_ticks.0,
                                ctx.screen_rect,
                                ctx.ticks_per_screen_unit,
                                ctx.x_offset.0,
                            ),
                            (height - ctx.y_offset as f32) * ctx.zoom_y + ctx.screen_rect.top(),
                        );

                        let departure_pos = if entry.has_departure {
                            Pos2::new(
                                super::draw_lines::ticks_to_screen_x(
                                    departure_ticks.0,
                                    ctx.screen_rect,
                                    ctx.ticks_per_screen_unit,
                                    ctx.x_offset.0,
                                ),
                                (height - ctx.y_offset as f32) * ctx.zoom_y + ctx.screen_rect.top(),
                            )
                        } else {
                            arrival_pos
                        };

                        segment.push(TripPoint {
                            arr: arrival_pos,
                            dep: departure_pos,
                            entry: entry.entity,
                        });

                        let mut continued = false;
                        if let Some(next_entry) = next {
                            for offset in [-1, 0, 1] {
                                let next_idx = (current_line_index as isize + offset) as usize;
                                if let Some((s, _)) = stations_for_layout.get(next_idx) {
                                    if *s == next_entry.station {
                                        next_local_edges.push((segment.clone(), next_idx));
                                        continued = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if !continued && segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }

                    for (segment, _) in local_edges.drain(..) {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }

                    local_edges = next_local_edges;
                    if let Some(next_entry) = next {
                        if let Some(indices) = station_index_map.get(&next_entry.station) {
                            previous_indices = indices.clone();
                        }
                    }
                }

                for (segment, _) in local_edges {
                    if segment.len() >= 2 {
                        segments.push(segment);
                    }
                }

                if segments.is_empty() {
                    continue;
                }

                for segment in segments {
                    let mut cubics = Vec::new();
                    let mut segment_entries = Vec::new();

                    for point in segment {
                        cubics.push([point.arr, point.arr, point.dep, point.dep]);
                        segment_entries.push(point.entry);
                    }

                    if !cubics.is_empty() {
                        drawn_segments.push(cubics);
                        drawn_entries.push(segment_entries);
                    }
                }
            }

            if drawn_segments.is_empty() {
                return None;
            }

            Some(super::DrawnTrip {
                entity: trip.entity,
                stroke: trip.stroke,
                points: drawn_segments,
                entries: drawn_entries,
            })
        })
        .collect();

    buf.extend(drawn);
}
