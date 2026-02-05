use bevy::{ecs::entity::EntityHashSet, prelude::*};
use egui::{Pos2, Rect};

use crate::{
    entry::{EntryMode, EntryQuery},
    route::Route,
    settings::ProjectSettings,
    station::{Platform, PlatformEntries, Station, StationQuery},
    trip::class::{Class, DisplayedStroke},
};

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
    (InMut(buf), InRef(tab), In(screen_rect), In(ticks_per_screen_unit), In(visible_ticks)): (
        InMut<Vec<super::DrawnTrip>>,
        InRef<super::DiagramTab>,
        In<Rect>,
        In<f64>,
        In<std::ops::Range<i64>>,
    ),
    routes: Query<&Route>,
    trips: Query<crate::trip::TripQuery>,
    entries: Query<EntryQuery>,
    stations: Query<(), With<Station>>,
    platforms: Query<&ChildOf, With<Platform>>,
    class_strokes: Query<&DisplayedStroke, With<Class>>,
    settings: Res<ProjectSettings>,
) {
    buf.clear();

    let Ok(route) = routes.get(tab.route_entity) else {
        return;
    };

    let heights: Vec<(Entity, f32)> = route.iter().collect();
    let route_stops: EntityHashSet = route.stops.iter().copied().collect();
    if heights.is_empty() {
        return;
    }

    let vertical_visible =
        tab.y_offset..tab.y_offset + screen_rect.height() / tab.zoom.y.max(f32::EPSILON);

    let visible_stations = {
        let first_visible = heights
            .iter()
            .position(|(_, h)| *h > vertical_visible.start)
            .or_else(|| heights.iter().rposition(|(_, h)| *h <= vertical_visible.start));
        let last_visible = heights
            .iter()
            .rposition(|(_, h)| *h < vertical_visible.end);
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

    struct TripEntry<'a> {
        entity: Entity,
        station: Entity,
        estimate: Option<&'a crate::entry::EntryEstimate>,
        has_departure: bool,
    }

    for trip_entity in tab.trips.iter().copied() {
        let Ok(trip) = trips.get(trip_entity) else {
            continue;
        };

        let stroke = class_strokes
            .get(trip.class.0)
            .copied()
            .unwrap_or_default();

        let mut trip_entries_vec: Vec<TripEntry<'_>> = Vec::new();
        for entry_entity in trip.schedule.iter() {
            let Ok(entry) = entries.get(entry_entity) else {
                continue;
            };
            let Some(station_entity) = resolve_stop_station(entry.stop()) else {
                continue;
            };

            trip_entries_vec.push(TripEntry {
                entity: entry_entity,
                station: station_entity,
                estimate: entry.estimate,
                has_departure: entry.mode.dep.is_some(),
            });
        }

        if trip_entries_vec.len() < 2 {
            continue;
        }

        let mut base_min: Option<i64> = None;
        let mut base_max: Option<i64> = None;
        for entry in &trip_entries_vec {
            let Some(estimate) = entry.estimate else {
                continue;
            };
            let arrival_ticks = estimate.arr.0 as i64 * super::TICKS_PER_SECOND;
            let departure_ticks = estimate.dep.0 as i64 * super::TICKS_PER_SECOND;
            let local_min = arrival_ticks.min(departure_ticks);
            let local_max = arrival_ticks.max(departure_ticks);
            base_min = Some(base_min.map_or(local_min, |v| v.min(local_min)));
            base_max = Some(base_max.map_or(local_max, |v| v.max(local_max)));
        }

        let Some(base_min) = base_min else {
            continue;
        };
        let Some(base_max) = base_max else {
            continue;
        };

        let repeat_freq_ticks = settings.repeat_frequency.0 as i64 * super::TICKS_PER_SECOND;
        let (repeat_start, repeat_end) = if repeat_freq_ticks > 0 {
            let start = (visible_ticks.start - base_max).div_euclid(repeat_freq_ticks);
            let end = (visible_ticks.end - base_min).div_euclid(repeat_freq_ticks);
            (start, end)
        } else {
            (0, 0)
        };

        let mut drawn_segments = Vec::new();
        let mut drawn_entries = Vec::new();

        for repeat in repeat_start..=repeat_end {
            let repeat_offset = repeat * repeat_freq_ticks;

            let first_visible = trip_entries_vec.iter().position(|entry| {
                let Some(estimate) = entry.estimate else {
                    return false;
                };
                let arrival_ticks =
                    estimate.arr.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;
                let departure_ticks =
                    estimate.dep.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;
                !(departure_ticks < visible_ticks.start || arrival_ticks > visible_ticks.end)
            });
            let last_visible = trip_entries_vec.iter().rposition(|entry| {
                let Some(estimate) = entry.estimate else {
                    return false;
                };
                let arrival_ticks =
                    estimate.arr.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;
                let departure_ticks =
                    estimate.dep.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;
                !(departure_ticks < visible_ticks.start || arrival_ticks > visible_ticks.end)
            });

            let Some(first_visible) = first_visible else {
                continue;
            };
            let Some(last_visible) = last_visible else {
                continue;
            };

            let first_visible = first_visible.saturating_sub(2);
            let last_visible = (last_visible + 2).min(trip_entries_vec.len() - 1);
            let trip_entries = &trip_entries_vec[first_visible..=last_visible];

            if trip_entries.len() < 2 {
                continue;
            }

            let mut segments: Vec<Vec<TripPoint>> = Vec::new();

            let mut local_edges: Vec<(Vec<TripPoint>, usize)> = Vec::new();
            let mut previous_indices: Vec<usize> = Vec::new();

            if let Some(first) = trip_entries.first() {
                previous_indices = visible_stations
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (s, _))| if *s == first.station { Some(i) } else { None })
                    .collect();
            }

            for entry_idx in 0..trip_entries.len() {
                let entry = &trip_entries[entry_idx];
                let next = trip_entries.get(entry_idx + 1);

                if previous_indices.is_empty() {
                    if let Some(next_entry) = next {
                        previous_indices = visible_stations
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (s, _))| {
                                if *s == next_entry.station {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .collect();
                    }
                    for (segment, _) in local_edges.drain(..) {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }
                    continue;
                }

                let Some(estimate) = entry.estimate else {
                    for (segment, _) in local_edges.drain(..) {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }
                    if let Some(next_entry) = next {
                        previous_indices = visible_stations
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (s, _))| {
                                if *s == next_entry.station {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .collect();
                    }
                    continue;
                };

                let arrival_ticks =
                    estimate.arr.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;
                let departure_ticks =
                    estimate.dep.0 as i64 * super::TICKS_PER_SECOND + repeat_offset;

                let mut next_local_edges = Vec::new();

                for &current_line_index in &previous_indices {
                    let height = visible_stations[current_line_index].1;

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
                            arrival_ticks,
                            screen_rect,
                            ticks_per_screen_unit,
                            tab.x_offset,
                        ),
                        (height - tab.y_offset) * tab.zoom.y + screen_rect.top(),
                    );

                    let departure_pos = if entry.has_departure {
                        Pos2::new(
                            super::draw_lines::ticks_to_screen_x(
                                departure_ticks,
                                screen_rect,
                                ticks_per_screen_unit,
                                tab.x_offset,
                            ),
                            (height - tab.y_offset) * tab.zoom.y + screen_rect.top(),
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
                            if let Some((s, _)) = visible_stations.get(next_idx) {
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
                    previous_indices = visible_stations
                        .iter()
                        .enumerate()
                        .filter_map(|(i, (s, _))| {
                            if *s == next_entry.station {
                                Some(i)
                            } else {
                                None
                            }
                        })
                        .collect();
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
            continue;
        }

        buf.push(super::DrawnTrip {
            entity: trip_entity,
            stroke,
            points: drawn_segments,
            entries: drawn_entries,
        });
    }
}
