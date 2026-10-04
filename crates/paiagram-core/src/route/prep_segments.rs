use crate::route::DiagramCache;
use crate::trip::{EstimateEntry, TEstimate};
use crate::{NodeKeyHashMap, TripKey, WorldSnapshot};

type TripPoint = (TEstimate, EstimateEntry, u32, f32);

/// All diagram placements of an entry's node, i.e. every `(layout line index, progress)` at
/// which the node can be drawn. Empty when the node is not part of the route.
fn placements<'a>(
    node_lookup: &'a NodeKeyHashMap<Vec<(u32, f32)>>,
    entry: &EstimateEntry,
) -> &'a [(u32, f32)] {
    node_lookup.get(&entry.node_key()).map_or(&[], Vec::as_slice)
}

pub(super) fn calc(
    cache: &mut DiagramCache,
    changed_entries: impl Iterator<Item = TripKey>,
    snap: &WorldSnapshot,
    node_lookup: &NodeKeyHashMap<Vec<(u32, f32)>>,
) {
    for trip_key in changed_entries {
        let Some(trip) = snap.trips.get(&trip_key) else {
            continue;
        };

        let trip_bucket = cache.map.entry(trip_key).or_default();
        trip_bucket.clear();

        let mut push_to_bucket = |points: Vec<TripPoint>| {
            if points.len() < 2 {
                return;
            }
            trip_bucket.push(points);
        };

        let trip_entries = trip.schedule.estimates(&snap.graph, |entries| entries.to_vec());
        if trip_entries.len() < 2 {
            continue;
        }

        // points, end index
        let mut local_edges: Vec<Vec<TripPoint>> = Vec::new();
        let mut previous_indices: &[(u32, f32)] = &[];

        if let Some((_, first_entry)) = trip_entries.first() {
            previous_indices = placements(node_lookup, first_entry);
        }

        for entry_idx in 0..trip_entries.len() {
            let (curr_estimate, curr_entry) = &trip_entries[entry_idx];
            let next = trip_entries.get(entry_idx + 1);

            if previous_indices.is_empty() {
                if let Some((_, next_entry)) = next {
                    previous_indices = placements(node_lookup, next_entry);
                }
                for it in local_edges.drain(..) {
                    push_to_bucket(it)
                }
                continue;
            }

            let Some(estimate) = curr_estimate else {
                for it in local_edges.drain(..) {
                    push_to_bucket(it)
                }
                if let Some((_, next_entry)) = next {
                    previous_indices = placements(node_lookup, next_entry);
                }
                continue;
            };

            let mut next_local_edges: Vec<Vec<TripPoint>> = Vec::new();

            for (current_line_index, current_line_progress) in previous_indices.iter().copied() {
                let matched_idx = local_edges
                    .iter()
                    .position(|it| current_line_index.abs_diff(it.last().unwrap().2) <= 1);

                let segment = if let Some(idx) = matched_idx {
                    let mut a = local_edges.swap_remove(idx);
                    a.push((
                        *estimate,
                        *curr_entry,
                        current_line_index,
                        current_line_progress,
                    ));
                    a
                } else {
                    vec![(
                        *estimate,
                        *curr_entry,
                        current_line_index,
                        current_line_progress,
                    )]
                };

                let mut segment = Some(segment);
                if let Some((_, next_entry)) = next {
                    // A segment can only be continued when the next entry has a placement on
                    // the same layout line or a neighbouring one; otherwise it ends here and
                    // the next entry starts a fresh segment.
                    let continues =
                        placements(node_lookup, next_entry).iter().any(|(next_line_index, _)| {
                            next_line_index.abs_diff(current_line_index) <= 1
                        });
                    if continues {
                        next_local_edges.push(segment.take().unwrap());
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
            if let Some((_, next_entry)) = next {
                previous_indices = placements(node_lookup, next_entry);
            }
        }

        for it in local_edges.drain(..) {
            push_to_bucket(it)
        }
    }
}
