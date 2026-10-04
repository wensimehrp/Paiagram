// SPDX-License-Identifier: MPL-2.0
//! Module for handling GTFS Static feeds (<https://gtfs.org/>).
//!
//! GTFS is more general than the other supported formats: it has no notion of a
//! single ordered line, and stops are referenced by id from the stop times. We
//! map it onto Paiagram as follows:
//!
//! - every GTFS stop becomes a [`Node`]; stops sharing a `parent_station` are grouped under a
//!   single [`Station`];
//! - every route becomes a [`Route`], whose station sequence is taken from the first of its trips;
//! - every trip becomes a [`Trip`], with one schedule entry per stop time;
//! - an [`Interval`] is created for every pair of consecutive stops in a trip.

use std::collections::HashMap;
use std::io::Cursor;

use ecow::string::ToEcoString;
use ecow::{EcoVec, eco_vec};
use egui::Color32;
use gtfs_structures::{Gtfs, Stop};
use smallvec::SmallVec;

use crate::graph::IntervalDirection;
use crate::route::{RouteInterval, RouteIntervals, StationRecord};
use crate::time::TimetableTime;
use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::{
    Interval, LonLat, Node, NodeKey, NodeKeyHashMap, Route, RouteKey, ServiceClass,
    ServiceClassKey, Station, StationKey, StrokeStyle, Trip, TripKey, Wfc, Wgs84LonLat,
    WorldSnapshot,
};

/// The position of a GTFS stop. Missing coordinates default to `(0, 0)`.
fn stop_lonlat(stop: &Stop) -> LonLat {
    Wgs84LonLat::new(stop.longitude.unwrap_or(0.0), stop.latitude.unwrap_or(0.0)).into()
}

pub(super) fn parse_gtfs(data: &[u8]) -> Result<WorldSnapshot, Box<dyn std::error::Error>> {
    let gtfs = Gtfs::from_reader(Cursor::new(data))?;
    let mut world = WorldSnapshot::default();

    // Stops with a `parent_station` become nodes of that station; standalone
    // stops become a station of their own.
    let mut station_keys: HashMap<String, StationKey> = HashMap::new();
    let mut node_keys: HashMap<String, NodeKey> = HashMap::new();
    let mut node_station: HashMap<String, StationKey> = HashMap::new();
    let mut node_pos: NodeKeyHashMap<LonLat> = NodeKeyHashMap::default();

    for (stop_id, stop) in &gtfs.stops {
        let station_id = stop.parent_station.clone().unwrap_or_else(|| stop_id.clone());
        let station_key = *station_keys.entry(station_id.clone()).or_insert_with(|| {
            let parent = gtfs.stops.get(&station_id);
            let name = parent
                .and_then(|s| s.name.clone())
                .or_else(|| stop.name.clone())
                .unwrap_or_else(|| station_id.clone());
            // Prefer the parent's coordinates, but fall back to the platform's when
            // the parent has none.
            let pos = parent
                .filter(|s| s.longitude.is_some() && s.latitude.is_some())
                .map_or_else(|| stop_lonlat(stop), |s| stop_lonlat(s));
            let key = StationKey::new();
            world.stations.insert(
                key,
                Wfc::new(Station {
                    name: name.to_eco_string(),
                    pos,
                }),
            );
            key
        });

        let pos = stop_lonlat(stop);
        let name = stop.name.clone().unwrap_or_else(|| stop_id.clone());
        let node_key = NodeKey::new();
        world.graph.insert_node(
            node_key,
            Node {
                name: name.to_eco_string(),
                parent: station_key,
                pos,
            },
        );
        node_keys.insert(stop_id.clone(), node_key);
        node_station.insert(stop_id.clone(), station_key);
        node_pos.insert(node_key, pos);
    }

    let mut route_classes: HashMap<String, ServiceClassKey> = HashMap::new();
    for (route_id, route) in &gtfs.routes {
        let color = route.color();
        let key = ServiceClassKey::new();
        let name = route
            .short_name
            .clone()
            .or_else(|| route.long_name.clone())
            .unwrap_or_else(|| route_id.clone());
        world.service_classes.insert(
            key,
            Wfc::new(ServiceClass {
                name: name.to_eco_string(),
                style: StrokeStyle {
                    color: Color32::from_rgb(color.r, color.g, color.b),
                    width: 1,
                },
            }),
        );
        route_classes.insert(route_id.clone(), key);
    }

    let mut trips_by_route: HashMap<&str, Vec<&gtfs_structures::Trip>> = HashMap::new();
    for trip in gtfs.trips.values() {
        trips_by_route.entry(trip.route_id.as_str()).or_default().push(trip);
    }
    for (route_id, route) in &gtfs.routes {
        let Some(trip) = trips_by_route.get(route_id.as_str()).and_then(|trips| trips.first())
        else {
            continue;
        };

        for window in trip.stop_times.windows(2) {
            let (Some(&a), Some(&b)) = (
                node_keys.get(&window[0].stop.id),
                node_keys.get(&window[1].stop.id),
            ) else {
                continue;
            };
            if a == b {
                continue;
            }
            // whoops I forgot to set up nodes
            let (hi, lo) = if a > b { (a, b) } else { (b, a) };
            world.graph.insert_interval(
                (a, b),
                Interval {
                    nodes: eco_vec![node_pos[&hi], node_pos[&lo]],
                    length: None,
                    direction: IntervalDirection::Both,
                },
            );
        }

        let intervals = trip
            .stop_times
            .iter()
            .filter_map(|stop_time| {
                let station_key = node_station.get(&stop_time.stop.id)?;
                Some(RouteInterval {
                    station_record: StationRecord::All(*station_key),
                    milestone: None,
                    canvas_length: None,
                    nodes: EcoVec::new(),
                })
            })
            .collect::<Vec<_>>();
        let name = route
            .short_name
            .clone()
            .or_else(|| route.long_name.clone())
            .unwrap_or_else(|| route_id.clone());
        world.routes.insert(
            RouteKey::new(),
            Wfc::new(Route {
                name: name.to_eco_string(),
                intervals: RouteIntervals(intervals),
            }),
        );
    }

    // 5. Trips.
    for (trip_id, trip) in &gtfs.trips {
        let mut entries = EcoVec::new();
        for stop_time in &trip.stop_times {
            let Some(&node) = node_keys.get(&stop_time.stop.id) else {
                continue;
            };
            let dep = stop_time.departure_time.map(|s| TimetableTime(s as i32));
            let arr = stop_time.arrival_time.map(|s| TimetableTime(s as i32));
            let (arr_or_pass, dep) = match (arr, dep) {
                (Some(arr), Some(dep)) if arr != dep => {
                    (TravelMode::At(arr), Some(TravelMode::At(dep)))
                }
                // A stop with `arrival == departure` is a pass-through.
                (Some(arr), Some(_)) => (TravelMode::At(arr), None),
                (Some(arr), None) => (TravelMode::At(arr), None),
                // The first stop of a trip often only has a departure time.
                (None, Some(dep)) => (TravelMode::At(dep), None),
                (None, None) => (TravelMode::Flexible, None),
            };
            entries.push(TEntry {
                node,
                arr_or_pass,
                dep,
                id: TEntryId::new(),
            });
        }
        if entries.is_empty() {
            continue;
        }
        let name = trip
            .trip_short_name
            .clone()
            .or_else(|| trip.trip_headsign.clone())
            .unwrap_or_else(|| trip_id.clone());
        world.trips.insert(
            TripKey::new(),
            Wfc::new(Trip {
                name: name.to_eco_string(),
                schedule: TripSchedule::new(entries),
                service_class: route_classes.get(&trip.route_id).copied(),
                vehicles: SmallVec::new(),
            }),
        );
    }

    Ok(world)
}
