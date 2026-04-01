use super::TripPoint;
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use paiagram_core::{
    entry::{EntryEstimate, EntryQuery},
    route::{Route, RouteTrips},
    station::ParentStationOrStation,
    trip::TripQuery,
};
use smallvec::SmallVec;
use vec1::{Vec1, vec1};

pub(crate) fn calc(
    (In(route_entity), InRef(heights), InMut(map)): (
        In<Entity>,
        InRef<[(Entity, f32)]>,
        InMut<Option<EntityHashMap<SmallVec<[Vec<TripPoint>; 1]>>>>,
    ),
    route_q: Query<(&RouteTrips, Ref<Route>)>,
    trip_q: Query<TripQuery>,
    changed_entries: Query<&ChildOf, Changed<EntryEstimate>>,
    entries: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
    mut invalidate_cache: Local<Vec<Entity>>,
) {
    let (trips, route) = route_q.get(route_entity).unwrap();

    let refresh_candidates = if map.is_none() || route.is_changed() {
        trips.as_slice()
    } else {
        invalidate_cache.clear();
        invalidate_cache.extend(changed_entries.iter().map(ChildOf::parent));
        invalidate_cache.sort_unstable();
        invalidate_cache.dedup();
        invalidate_cache.as_slice()
    };

    let map: &mut EntityHashMap<SmallVec<[Vec<TripPoint>; 1]>> = map.get_or_insert_default();

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

        let mut push_to_bucket = |points: Vec1<TripPoint>| {
            if points.len() < 2 {
                return;
            }
            trip_bucket.push(points.into_vec());
        };

        let trip_entries = trip.entries.as_slice();
        if trip_entries.len() < 2 {
            continue;
        }

        // points, end index
        let mut local_edges: Vec<Vec1<TripPoint>> = Vec::new();
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

            let mut next_local_edges: Vec<Vec1<TripPoint>> = Vec::new();

            for current_line_index in previous_indices.iter().copied() {
                let matched_idx = local_edges
                    .iter()
                    .position(|it| current_line_index.abs_diff(it.last().station_index) <= 1);

                let segment = if let Some(idx) = matched_idx {
                    let mut a = local_edges.swap_remove(idx);
                    a.push(TripPoint {
                        arr: estimate.arr,
                        dep: estimate.dep,
                        entry: curr.entity,
                        station_index: current_line_index,
                    });
                    a
                } else {
                    vec1![TripPoint {
                        arr: estimate.arr,
                        dep: estimate.dep,
                        entry: curr.entity,
                        station_index: current_line_index,
                    }]
                };

                let mut segment = Some(segment);
                if let Some(next_entry) = next {
                    for offset in [-1, 0, 1] {
                        let next_idx = (current_line_index as isize + offset) as usize;
                        if let Some((s, _)) = stations_for_layout.get(next_idx) {
                            if *s == next_entry.station {
                                let forwarded_segment = segment.take().unwrap();
                                next_local_edges.push(forwarded_segment);
                                break;
                            }
                        }
                    }
                }

                if let Some(it) = segment {
                    push_to_bucket(it);
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
