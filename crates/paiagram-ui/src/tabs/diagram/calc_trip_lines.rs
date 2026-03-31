use crate::tabs::Navigatable;
use bevy::{
    ecs::entity::{EntityHashMap, EntityHashSet},
    prelude::*,
};
use egui::Pos2;
use paiagram_core::{
    entry::EntryQuery,
    route::{Route, RouteTrips},
    settings::ProjectSettings,
    station::{ParentStationOrStation, Platform, Station},
    trip::class::{Class, DisplayedStroke},
    units::time::Tick,
};
use rayon::prelude::*;
use smallvec::SmallVec;

pub fn calc(
    (InMut(buf), InRef(navi), In(route_entity)): (
        InMut<Vec<super::DrawnTrip>>,
        InRef<super::DiagramTabNavigation>,
        In<Entity>,
    ),
    routes: Query<(&Route, &RouteTrips)>,
    trip_q: Query<paiagram_core::trip::TripQuery>,
    entries: Query<EntryQuery>,
    stations: Query<(), With<Station>>,
    platforms: Query<&ChildOf, With<Platform>>,
    class_strokes: Query<&DisplayedStroke, With<Class>>,
    parent_station_or_station: Query<ParentStationOrStation>,
    settings: Res<ProjectSettings>,
) {
    buf.clear();

    let (route, trips) = routes.get(route_entity).unwrap();

    let heights: Vec<(Entity, f32)> = route.iter().collect();
    let route_stops: EntityHashSet = route.stops.iter().copied().collect();
    if heights.is_empty() {
        return;
    }

    let vertical_visible = navi.visible_y();
    let visible_ticks = navi.visible_x();

    /*
    let visible_stations = {
        let first_visible = heights
            .iter()
            .position(|(_, h)| *h > vertical_visible.start as f32)
            .or_else(|| {
                heights
                    .iter()
                    .rposition(|(_, h)| *h <= vertical_visible.start as f32)
            });
        let last_visible = heights
            .iter()
            .rposition(|(_, h)| *h < vertical_visible.end as f32);
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
    */

    let resolve_stop_station =
        |stop: Entity| -> Entity { parent_station_or_station.get(stop).unwrap().parent() };

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
        arr_tick: Option<Tick>,
        dep_tick: Option<Tick>,
    }

    struct TripData {
        entity: Entity,
        stroke: DisplayedStroke,
        entries: Vec<TripEntryData>,
    }

    let stations_for_layout = &heights[..];
    // let visible_station_set: EntityHashSet = visible_stations.iter().map(|(s, _)| *s).collect();
    let mut station_index_map: EntityHashMap<SmallVec<[usize; 2]>> = EntityHashMap::new();
    for (idx, (station, _)) in stations_for_layout.iter().enumerate() {
        station_index_map.entry(*station).or_default().push(idx);
    }

    let mut trip_data = Vec::with_capacity(trips.len());
    for trip_entity in trips.iter().copied() {
        let trip = trip_q.get(trip_entity).unwrap();
        let stroke = class_strokes.get(trip.class.0).copied().unwrap();

        let mut trip_entries_vec: Vec<TripEntryData> = Vec::new();
        for entry_entity in trip.schedule.iter().copied() {
            let entry = entries.get(entry_entity).unwrap();
            let station_entity = resolve_stop_station(entry.stop());
            let (arr_tick, dep_tick) = if let Some(estimate) = entry.estimate {
                let arrival_ticks = Tick::from_timetable_time(estimate.arr);
                let departure_ticks = Tick::from_timetable_time(estimate.dep);
                (Some(arrival_ticks), Some(departure_ticks))
            } else {
                (None, None)
            };

            trip_entries_vec.push(TripEntryData {
                entity: entry_entity,
                station: station_entity,
                arr_tick,
                dep_tick,
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

    let repeat_freq_ticks = Tick::from_timetable_time(paiagram_core::units::time::TimetableTime(
        settings.repeat_frequency.0,
    ));

    let draw_iter = trip_data
        .into_iter()
        .filter_map(|trip| {
            let mut base_min: Option<Tick> = None;
            let mut base_max: Option<Tick> = None;
            for entry in &trip.entries {
                let (Some(arrival_ticks), Some(departure_ticks)) = (entry.arr_tick, entry.dep_tick)
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
                let start = (visible_ticks.start.0 - base_max.0).div_euclid(repeat_freq_ticks.0);
                let end = (visible_ticks.end.0 - base_min.0).div_euclid(repeat_freq_ticks.0);
                (start, end)
            } else {
                (0, 0)
            };

            let mut drawn_segments = Vec::new();
            let mut drawn_entries = Vec::new();

            for repeat in repeat_start..=repeat_end {
                let repeat_offset = repeat * repeat_freq_ticks.0;

                /*
                let first_visible = trip.entries.iter().position(|entry| {
                    let (Some(arrival_ticks), Some(departure_ticks)) =
                        (entry.arr_tick, entry.dep_tick)
                    else {
                        return false;
                    };
                    let arrival_ticks = arrival_ticks.0 + repeat_offset;
                    let departure_ticks = departure_ticks.0 + repeat_offset;
                    !(departure_ticks < visible_ticks.start.0
                        || arrival_ticks > visible_ticks.end.0)
                });
                let last_visible = trip.entries.iter().rposition(|entry| {
                    let (Some(arrival_ticks), Some(departure_ticks)) =
                        (entry.arr_tick, entry.dep_tick)
                    else {
                        return false;
                    };
                    let arrival_ticks = arrival_ticks.0 + repeat_offset;
                    let departure_ticks = departure_ticks.0 + repeat_offset;
                    !(departure_ticks < visible_ticks.start.0
                        || arrival_ticks > visible_ticks.end.0)
                });

                let Some(first_visible) = first_visible else {
                    continue;
                };
                let Some(last_visible) = last_visible else {
                    continue;
                };
                */

                // let trip_entries = {
                //     let first_visible = first_visible.saturating_sub(2);
                //     let last_visible = (last_visible + 2).min(trip.entries.len() - 1);
                //     &trip.entries[first_visible..=last_visible]
                // };

                let trip_entries = trip.entries.as_slice();

                if trip_entries.len() < 2 {
                    continue;
                }

                let mut segments: Vec<Vec<TripPoint>> = Vec::new();

                let mut local_edges: Vec<(Vec<TripPoint>, usize)> = Vec::new();
                let mut previous_indices: &[usize] = &[];

                if let Some(first) = trip_entries.first() {
                    if let Some(indices) = station_index_map.get(&first.station) {
                        previous_indices = indices.as_slice();
                    }
                }

                for entry_idx in 0..trip_entries.len() {
                    let entry = &trip_entries[entry_idx];
                    let next = trip_entries.get(entry_idx + 1);

                    if previous_indices.is_empty() {
                        if let Some(next_entry) = next {
                            if let Some(indices) = station_index_map.get(&next_entry.station) {
                                previous_indices = indices.as_slice();
                            } else {
                                previous_indices = &[];
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
                        (entry.arr_tick, entry.dep_tick)
                    else {
                        for (segment, _) in local_edges.drain(..) {
                            if segment.len() >= 2 {
                                segments.push(segment);
                            }
                        }
                        if let Some(next_entry) = next {
                            if let Some(indices) = station_index_map.get(&next_entry.station) {
                                previous_indices = indices.as_slice();
                            } else {
                                previous_indices = &[];
                            }
                        }
                        continue;
                    };

                    let arrival_ticks = Tick(arrival_ticks.0 + repeat_offset);
                    let departure_ticks = Tick(departure_ticks.0 + repeat_offset);

                    let mut next_local_edges = Vec::new();

                    for &current_line_index in previous_indices {
                        let Some((_, height)) = stations_for_layout.get(current_line_index) else {
                            continue;
                        };

                        let matched_idx = local_edges
                            .iter()
                            .position(|(_, idx)| current_line_index.abs_diff(*idx) <= 1);

                        let mut segment = Some(if let Some(idx) = matched_idx {
                            local_edges.swap_remove(idx).0
                        } else {
                            Vec::new()
                        });

                        let arrival_pos = navi.xy_to_screen_pos(arrival_ticks, *height as f64);
                        let departure_pos = navi.xy_to_screen_pos(departure_ticks, *height as f64);

                        segment.as_mut().unwrap().push(TripPoint {
                            arr: arrival_pos,
                            dep: departure_pos,
                            entry: entry.entity,
                        });

                        let mut forwarded = None;
                        if let Some(next_entry) = next {
                            for offset in [-1, 0, 1] {
                                let next_idx = (current_line_index as isize + offset) as usize;
                                if let Some((s, _)) = stations_for_layout.get(next_idx) {
                                    if *s == next_entry.station {
                                        forwarded = Some((segment.take().unwrap(), next_idx));
                                        break;
                                    }
                                }
                            }
                        }

                        if let Some(edge) = forwarded {
                            next_local_edges.push(edge);
                        } else if let Some(segment) = segment
                            && segment.len() >= 2
                        {
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
                            previous_indices = indices.as_slice();
                        } else {
                            previous_indices = &[];
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
        });

    buf.extend(draw_iter);
}
