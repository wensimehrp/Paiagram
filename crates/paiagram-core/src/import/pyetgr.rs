// SPDX-License-Identifier: MPL-2.0
//! Module for handling the `.pyetgr` used by qETRC and pyETRC.
use std::borrow::Cow;
use std::num::NonZeroU32;

use ecow::string::ToEcoString;
use ecow::{EcoVec, eco_vec};
use egui::Color32;
use itertools::Itertools;
use log::warn;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_json;
use smallvec::SmallVec;

use crate::route::{RouteInterval, RouteIntervals, StationRecord};
use crate::time::{TTime, TimetableTime};
use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::units::distance::Distance;
use crate::{
    Interval, LonLat, Node, NodeKey, Route, RouteKey, ServiceClass, ServiceClassKey,
    Station as PaiagramStation, StationKey, StrokeStyle, Trip as PaiagramTrip, TripKey, Wfc,
    WorldSnapshot,
};

/// The root structure of the qETRC JSON data
#[derive(Deserialize)]
struct Root<'a> {
    // qetrc_release: u32,
    // qetrc_version: String,
    /// Trains in the original qETRC data. Each "train" corresponds to a
    /// [`crate::trip::Trip`] in Paiagram.
    #[serde(rename = "trains", default, borrow)]
    trips: Vec<Trip<'a>>,
    // qETRC has the line field and the lines array, both contains line data.
    // pyETRC only has the `line` field, while qETRC uses both to support multiple lines.
    // To keep compatibility with pyETRC, we keep the `line` field as is,
    // The lines would be chained together later with std::iter::once and chain
    /// A single [`Line`]
    line: Line<'a>,
    /// Additional [`Line`]s. This field does not exist in pyETRC, only in
    /// qETRC.
    #[serde(default)]
    lines: Vec<Line<'a>>,
    /// Vehicles in the qETRC data.
    /// They are named "circuits" in the original qETRC data. A "circuit" refers
    /// to a train that runs a set of tripss in a given period, which
    /// matches the concept of [`Vehicle`] in Paiagram.
    #[serde(rename = "circuits", default)]
    vehicles: Vec<Vehicle<'a>>,
    #[serde(default)]
    config: Config<'a>,
}

/// A line that is used as the foundation of connection in qETRC data
#[derive(Deserialize)]
struct Line<'a> {
    /// The name of the line
    name: Cow<'a, str>,
    /// [`Station`]s on the line.
    stations: Vec<Station<'a>>,
}

#[derive(Deserialize)]
struct Station<'a> {
    /// Station name
    #[serde(rename = "zhanming")]
    name: Cow<'a, str>,
    /// Distance from the start of the line, in kilometers
    #[serde(rename = "licheng")]
    distance_km: f32,
}

#[derive(Deserialize)]
struct Trip<'a> {
    /// Each trip may have multiple service numbers.
    /// In qETRC's case, the first service number is always the main one, and we
    /// use that one in Paiagram.
    #[serde(rename = "checi")] // checi is 车次
    trip_number: Vec<Cow<'a, str>>,
    #[serde(rename = "type")]
    service_class: Cow<'a, str>,
    /// The timetable entries of the service
    #[serde(default, borrow)]
    timetable: Vec<TimetableEntry<'a>>,
}

#[derive(Deserialize)]
struct TimetableEntry<'a> {
    /// Whether the train would stop and load/unload passengers or freight at
    /// the station.
    #[serde(rename = "business")]
    would_stop: Option<bool>,
    /// Arrival time in "HH:MM:SS" format. "ddsj" in the original qETRC data refers
    /// to "到达时间".
    #[serde(rename = "ddsj")]
    arr: &'a str,
    /// Departure time in "HH:MM:SS" format. "cfsj" in the original qETRC data
    /// refers to "出发时间".
    #[serde(rename = "cfsj")]
    dep: &'a str,
    /// Station name
    #[serde(rename = "zhanming")]
    station_name: Cow<'a, str>,
}

#[derive(Deserialize)]
struct Vehicle<'a> {
    /// Vehicle model
    #[serde(rename = "model")]
    make: Cow<'a, str>,
    /// Vehicle name
    name: Cow<'a, str>,
    /// Services that the vehicle runs.
    #[serde(default, rename = "order")]
    services: Vec<VehicleServiceEntry<'a>>,
}

#[derive(Deserialize)]
struct VehicleServiceEntry<'a> {
    /// Service number of the service
    #[serde(rename = "checi")]
    service_number: Cow<'a, str>,
}

#[derive(Deserialize, Default)]
struct Config<'a> {
    #[serde(borrow, default)]
    default_colors: FxHashMap<Cow<'a, str>, &'a str>,
}

pub(super) fn parse_pyetgr(data: &[u8]) -> Option<WorldSnapshot> {
    let mut world = WorldSnapshot::default();
    let root: Root = match serde_json::from_slice(data) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse `pyetgr` data: {e:?}");
            return None;
        }
    };
    let mut station_node_map: FxHashMap<&str, NodeKey> = FxHashMap::default();
    let mut service_class_map: FxHashMap<&str, ServiceClassKey> = FxHashMap::default();
    for (name, color) in &root.config.default_colors {
        // #RRGGBB
        // 0123456
        let (r, g, b) = (
            u8::from_str_radix(&color[3..=4], 16).unwrap(),
            u8::from_str_radix(&color[1..=2], 16).unwrap(),
            u8::from_str_radix(&color[5..=6], 16).unwrap(),
        );
        let key = ServiceClassKey::new();
        service_class_map.insert(&*name, key);
        world.service_classes.insert(
            key,
            Wfc::new(ServiceClass {
                name: name.to_eco_string(),
                style: StrokeStyle {
                    color: Color32::from_rgb(r, g, b),
                    width: 1,
                },
            }),
        );
    }
    for line in [&root.line].into_iter().chain(root.lines.iter()) {
        let mut route_intervals = Vec::with_capacity(line.stations.len());
        for station in &line.stations {
            if station_node_map.contains_key(&*station.name) {
                continue;
            }
            let stn_key = StationKey::new();
            let stn_info = PaiagramStation {
                name: station.name.to_eco_string(),
                pos: LonLat::ZERO,
            };
            let node_key = NodeKey::new();
            let node_info = Node {
                name: "".into(),
                parent: stn_key,
                pos: LonLat::ZERO,
                is_platform: true,
            };
            route_intervals.push(RouteInterval {
                station_record: StationRecord::All(stn_key),
                milestone: Some(Distance::from_km(station.distance_km)),
                canvas_length: None,
                nodes: EcoVec::new(),
            });
            world.stations.insert(stn_key, Wfc::new(stn_info));
            world.nodes.insert(node_key, Wfc::new(node_info));
            station_node_map.insert(&station.name, node_key);
        }
        world.routes.insert(
            RouteKey::new(),
            Wfc::new(Route {
                name: line.name.to_eco_string(),
                intervals: RouteIntervals(route_intervals),
            }),
        );
    }
    for line in [&root.line].into_iter().chain(root.lines.iter()) {
        for ((prev_stn, &prev_key), (curr_stn, &curr_key)) in line
            .stations
            .iter()
            .map(|stn| (stn, station_node_map.get(&*stn.name).unwrap()))
            .tuple_windows()
        {
            let length = curr_stn.distance_km - prev_stn.distance_km;
            let length = Distance::from_km(length).0;
            world.intervals.insert(
                (prev_key, curr_key),
                Wfc::new(Interval {
                    nodes: eco_vec![],
                    length: NonZeroU32::new(length as u32),
                }),
            );
        }
    }
    for trip in root.trips {
        let mut times: Vec<(NodeKey, TTime, TTime)> = trip
            .timetable
            .iter()
            .filter_map(|e| {
                Some((
                    *station_node_map.get(&*e.station_name)?,
                    TimetableTime::from_str(e.arr)?,
                    TimetableTime::from_str(e.dep)?,
                ))
            })
            .collect();
        super::normalize_times(times.iter_mut().flat_map(|(_, arr, dep)| [arr, dep].into_iter()));
        let entries: EcoVec<_> = times
            .into_iter()
            .filter_map(|(node, arr, dep)| {
                if arr == dep {
                    Some(TEntry::PinnedPass {
                        node,
                        pass: TravelMode::At(arr),
                        external: false,
                        id: TEntryId::new(),
                    })
                } else {
                    Some(TEntry::PinnedStop {
                        node,
                        arr: TravelMode::At(arr),
                        dep: TravelMode::At(dep),
                        external: false,
                        id: TEntryId::new(),
                    })
                }
            })
            .collect();
        world.trips.insert(
            TripKey::new(),
            Wfc::new(PaiagramTrip {
                name: trip.trip_number[0].to_eco_string(),
                schedule: TripSchedule::new(entries),
                service_class: None,
                vehicles: SmallVec::new(),
            }),
        );
    }
    Some(world)
}
