use super::TripPoint;
use crate::tabs::diagram::CachedTrip;
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use paiagram_core::{
    entry::{EntryEstimate, EntryQuery},
    route::{Route, RouteTrips},
    station::ParentStationOrStation,
};
use smallvec::SmallVec;

pub fn calc(
    (In(route_entity), InRef(heights), InMut(map)): (
        In<Entity>,
        InRef<[(Entity, f32)]>,
        InMut<Option<EntityHashMap<SmallVec<[CachedTrip; 1]>>>>,
    ),
    route_q: Query<(&RouteTrips, Ref<Route>)>,
    trip_q: Query<paiagram_core::trip::TripQuery>,
    changed_entries: Query<&ChildOf, Changed<EntryEstimate>>,
    entries: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
    mut invalidate_cache: Local<Vec<Entity>>,
) {
    let (trips, route) = route_q.get(route_entity).unwrap();

    let refresh_candidates = if map.is_none() || route.is_changed() {
        info!("none");
        trips.as_slice()
    } else {
        invalidate_cache.clear();
        invalidate_cache.extend(changed_entries.iter().map(ChildOf::parent));
        invalidate_cache.sort_unstable();
        invalidate_cache.dedup();
        invalidate_cache.as_slice()
    };

    let map: &mut EntityHashMap<SmallVec<[CachedTrip; 1]>> = map.get_or_insert_default();

    #[derive(Clone, Copy)]
    struct TripEntryData {
        entity: Entity,
        station: Entity,
        estimate: Option<EntryEstimate>,
    }

    struct TripData {
        entity: Entity,
        // stroke: DisplayedStroke,
        entries: Vec<TripEntryData>,
    }

    let stations_for_layout = &heights[..];
    let mut station_index_map: EntityHashMap<SmallVec<[usize; 2]>> = EntityHashMap::new();
    for (idx, (station, _)) in stations_for_layout.iter().enumerate() {
        station_index_map.entry(*station).or_default().push(idx);
    }

    let mut trip_data = Vec::with_capacity(refresh_candidates.len());
    for trip_entity in refresh_candidates.iter().copied() {
        let trip = trip_q.get(trip_entity).unwrap();
        let mut trip_entries_vec: Vec<TripEntryData> = Vec::new();
        for entry_entity in trip.schedule.iter().copied() {
            let entry = entries.get(entry_entity).unwrap();
            let station_entity = parent_station_or_station
                .get(entry.stop())
                .unwrap()
                .parent();
            trip_entries_vec.push(TripEntryData {
                entity: entry_entity,
                station: station_entity,
                estimate: entry.estimate.copied(),
            });
        }

        if trip_entries_vec.len() < 2 {
            continue;
        }

        trip_data.push(TripData {
            entity: trip_entity,
            entries: trip_entries_vec,
        });
    }

    for trip in trip_data.into_iter() {
        let trip_bucket = map.entry(trip.entity).or_default();
        trip_bucket.clear();

        let mut push_to_bucket =
            |(start_index, points, end_index): (usize, Vec<TripPoint>, usize)| {
                if points.len() < 2 {
                    return;
                }
                let is_going_down = end_index > start_index;
                trip_bucket.push(CachedTrip {
                    start_index,
                    is_going_down,
                    points,
                });
            };

        let trip_entries = trip.entries.as_slice();
        if trip_entries.len() < 2 {
            continue;
        }

        // Start index, points, end index
        let mut local_edges: Vec<(usize, Vec<TripPoint>, usize)> = Vec::new();
        let mut previous_indices: &[usize] = &[];

        if let Some(first) = trip_entries.first()
            && let Some(indices) = station_index_map.get(&first.station)
        {
            previous_indices = indices.as_slice();
        }

        for entry_idx in 0..trip_entries.len() {
            let curr = &trip_entries[entry_idx];
            let next = trip_entries.get(entry_idx + 1);

            if previous_indices.is_empty() {
                if let Some(next) = next {
                    if let Some(indices) = station_index_map.get(&next.station) {
                        previous_indices = indices.as_slice();
                    } else {
                        previous_indices = &[];
                    }
                }
                for it in local_edges.drain(..) {
                    push_to_bucket(it)
                }
                continue;
            }

            let Some(estimate) = curr.estimate else {
                for it in local_edges.drain(..) {
                    push_to_bucket(it)
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

            let mut next_local_edges: Vec<(usize, Vec<TripPoint>, usize)> = Vec::new();

            for &current_line_index in previous_indices {
                let matched_idx = local_edges
                    .iter()
                    .position(|(_, _, idx)| current_line_index.abs_diff(*idx) <= 1);

                let mut segment = if let Some(idx) = matched_idx {
                    local_edges.swap_remove(idx)
                } else {
                    (current_line_index, Vec::new(), current_line_index)
                };

                segment.1.push(TripPoint {
                    arr: estimate.arr,
                    dep: estimate.dep,
                    entry: curr.entity,
                });

                // The current point belongs to the current line index.
                segment.2 = current_line_index;

                let mut segment = Some(segment);
                if let Some(next_entry) = next {
                    for offset in [-1, 0, 1] {
                        let next_idx = (current_line_index as isize + offset) as usize;
                        if let Some((s, _)) = stations_for_layout.get(next_idx) {
                            if *s == next_entry.station {
                                let mut forwarded_segment = segment.take().unwrap();
                                forwarded_segment.2 = next_idx;
                                next_local_edges.push(forwarded_segment);
                                break;
                            }
                        }
                    }
                }

                if let Some(segment) = segment {
                    push_to_bucket(segment);
                }
            }

            for it in local_edges.drain(..) {
                push_to_bucket(it)
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

        for it in local_edges.drain(..) {
            push_to_bucket(it)
        }
    }
}
