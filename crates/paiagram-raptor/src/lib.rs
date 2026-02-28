//! # RAPTOR Module
//! This crate bridges between Paiagram types and the raptor-rs crate

// TODO: in case if the crate author updates, switch to iterators instead of vectors

use bevy::{ecs::system::SystemParam, prelude::*};
use paiagram_core::entry::EntryQuery;
use paiagram_core::station::{ParentStationOrStation, PlatformEntries, StationQuery};
use paiagram_core::trip::TripQuery;
use raptor::Timetable;

pub use raptor::Journey;
pub fn make_query_data(
    (In(departure), In(start), In(end)): (In<usize>, In<Entity>, In<Entity>),
    info: RaptorTimetable<'_, '_>,
) -> Vec<raptor::Journey<Entity, Entity>> {
    // TODO: add a more reasonable limitation for transfers
    info.raptor(10, departure, start, end)
}

#[derive(SystemParam)]
pub struct RaptorTimetable<'w, 's> {
    station_query: Query<'w, 's, StationQuery>,
    platform_entry_query: Query<'w, 's, &'static PlatformEntries>,
    parent_query: Query<'w, 's, &'static ChildOf>,
    parent_station_or_station_query: Query<'w, 's, ParentStationOrStation>,
    trip_query: Query<'w, 's, TripQuery>,
    entry_query: Query<'w, 's, EntryQuery>,
}

impl RaptorTimetable<'_, '_> {
    fn normalize_stop(&self, stop: Entity) -> Option<Entity> {
        self.parent_station_or_station_query
            .get(stop)
            .ok()
            .map(|it| it.parent())
    }

    fn route_stop_sequence(&self, route: Entity) -> Vec<Entity> {
        let Ok(trip) = self.trip_query.get(route) else {
            return Vec::new();
        };

        self.entry_query
            .iter_many(trip.schedule.iter())
            .filter_map(|entry| self.normalize_stop(entry.stop()))
            .collect()
    }
}

impl raptor::Timetable for RaptorTimetable<'_, '_> {
    type Route = Entity; // in this case it is the same as trips
    type Stop = Entity;
    type Trip = Entity;

    fn get_routes_serving_stop(&self, stop: Self::Stop) -> Vec<Self::Route> {
        let Ok(station) = self.station_query.get(stop) else {
            return Vec::new();
        };

        let mut routes = std::collections::BTreeSet::new();
        for entry in station.passing_entries(&self.platform_entry_query) {
            if let Ok(parent) = self.parent_query.get(entry) {
                routes.insert(parent.parent());
            }
        }
        routes.into_iter().collect()
    }

    fn get_arrival_time(&self, trip: Self::Trip, stop: Self::Stop) -> raptor::Tau {
        let Ok(route_trip) = self.trip_query.get(trip) else {
            return raptor::Tau::MAX;
        };

        self.entry_query
            .iter_many(route_trip.schedule.iter())
            .find_map(|it| {
                let normalized_stop = self.normalize_stop(it.stop())?;
                if normalized_stop == stop {
                    return it.estimate.map(|estimate| estimate.arr.0 as usize);
                }
                None
            })
            .unwrap_or(raptor::Tau::MAX)
    }

    fn get_departure_time(&self, trip: Self::Trip, stop: Self::Stop) -> raptor::Tau {
        let Ok(route_trip) = self.trip_query.get(trip) else {
            return raptor::Tau::MAX;
        };

        self.entry_query
            .iter_many(route_trip.schedule.iter())
            .find_map(|it| {
                let normalized_stop = self.normalize_stop(it.stop())?;
                if normalized_stop == stop {
                    return it.estimate.map(|estimate| estimate.dep.0 as usize);
                }
                None
            })
            .unwrap_or(raptor::Tau::MAX)
    }

    fn get_earlier_stop(
        &self,
        route: Self::Route,
        left: Self::Stop,
        right: Self::Stop,
    ) -> Self::Stop {
        let stops = self.route_stop_sequence(route);
        let pos_l = stops.iter().position(|&s| s == left);
        let pos_r = stops.iter().position(|&s| s == right);

        match (pos_l, pos_r) {
            (Some(_), None) => left,
            (None, Some(_)) => right,
            (Some(l), Some(r)) => {
                if l < r {
                    left
                } else {
                    right
                }
            }
            (None, None) => right,
        }
    }

    fn get_earliest_trip(
        &self,
        route: Self::Route,
        at: raptor::Tau,
        boarding_stop: Self::Stop,
    ) -> Option<Self::Trip> {
        let dep = self.get_departure_time(route, boarding_stop);
        if dep >= at && dep != raptor::Tau::MAX {
            Some(route)
        } else {
            None
        }
    }

    fn get_footpaths_from(&self, _stop: Self::Stop) -> Vec<Self::Stop> {
        Vec::new()
    }

    fn get_stops_after(&self, route: Self::Route, stop: Self::Stop) -> Vec<Self::Stop> {
        self.route_stop_sequence(route)
            .into_iter()
            .skip_while(|&s| s != stop)
            .collect()
    }
}
