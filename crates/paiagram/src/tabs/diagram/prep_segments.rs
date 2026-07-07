use paiagram_core::trip::TEntry;
use paiagram_core::units::time::{Tick, TimetableTime};
use paiagram_core::{RouteKey, Source, StationKey, TripKey, TripKeyHashMap};
use smallvec::SmallVec;
use vec1::Vec1;

use super::TripPoint;
use crate::App;

/// Calculate trip segments for a route. Returns true if the cache was updated.
pub(crate) fn calc(
    route_key: RouteKey,
    heights: &[(StationKey, f32)],
    map: &mut Option<TripKeyHashMap<SmallVec<[Vec1<TripPoint>; 1]>>>,
    app: &App,
) -> bool {
    let Some(route_handle) = app.routes.get_handle(route_key) else {
        return false;
    };
    let route_stations = app.routes.get_stations(route_handle);

    // Build a map from station key to its index in the heights list
    let station_to_index: std::collections::HashMap<StationKey, usize> = heights
        .iter()
        .enumerate()
        .map(|(i, (sk, _))| (*sk, i))
        .collect();

    // Create the mapping for trips
    let new_map = map.get_or_insert_default();
    new_map.clear();

    // Iterate over all trips and find ones that visit stations on this route
    for trip_key in app.trips.keys() {
        let trip_key = *trip_key;
        let Some(trip_view) = app.trips.get_view(trip_key) else {
            continue;
        };

        let entries = trip_view.schedule.entries();
        if entries.len() < 2 {
            continue;
        }

        let mut segment = Vec1::new(TripPoint {
            arr: TimetableTime::ZERO,
            dep: TimetableTime::ZERO,
            station_index: 0,
        });
        let mut has_valid_entry = false;

        for entry in entries {
            match entry {
                TEntry::Pinned {
                    stn, arr, dep: _, ..
                }
                | TEntry::PinnedNonStop { stn, pass: arr, .. }
                | TEntry::PinnedExternalNonStop { stn, pass: arr, .. } => {
                    if let Some(&station_idx) = station_to_index.get(stn) {
                        if !has_valid_entry {
                            // First valid entry
                            segment[0] = TripPoint {
                                arr: TimetableTime::ZERO,
                                dep: TimetableTime::ZERO,
                                station_index: station_idx,
                            };
                            has_valid_entry = true;
                        } else {
                            segment.push(TripPoint {
                                arr: match arr {
                                    paiagram_core::trip::TravelMode::At(t) => *t,
                                    _ => TimetableTime::ZERO,
                                },
                                dep: match arr {
                                    paiagram_core::trip::TravelMode::At(t) => *t,
                                    _ => TimetableTime::ZERO,
                                },
                                station_index: station_idx,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        if has_valid_entry && segment.len() >= 2 {
            let bucket = new_map.entry(trip_key).or_default();
            bucket.push(segment);
        }
    }

    true
}
