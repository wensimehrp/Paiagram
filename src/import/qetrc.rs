use bevy::{platform::collections::HashMap, prelude::*};
use egui::Color32;
use moonshine_core::kind::*;
use serde::Deserialize;
use serde_json;

use crate::{
    colors::DisplayColor,
    entry::{EntryBundle, TravelMode},
    graph::Graph,
    route::Route,
    station::Station,
    trip::{
        TripBundle, TripClass,
        class::{Class, ClassBundle, DisplayedStroke},
    },
    units::{distance::Distance, time::TimetableTime},
};

/// The root structure of the qETRC JSON data
#[derive(Deserialize)]
struct Root {
    // qetrc_release: u32,
    // qetrc_version: String,
    /// Trains in the original qETRC data. Each "train" corresponds to a [`VehicleService`] in Paiagram.
    #[serde(rename = "trains")]
    services: Vec<Service>,
    // qETRC has the line field and the lines array, both contains line data.
    // pyETRC only has the `line` field, while qETRC uses both to support multiple lines.
    // To keep compatibility with pyETRC, we keep the `line` field as is,
    // The lines would be chained together later with std::iter::once and chain
    /// A single [`Line`]
    line: Line,
    /// Additional [`Line`]s. This field does not exist in pyETRC, only in qETRC.
    lines: Option<Vec<Line>>,
    /// Vehicles in the qETRC data.
    /// They are named "circuits" in the original qETRC data. A "circuit" refers to a train that runs a set of services
    /// in a given period, which matches the concept of [`Vehicle`] or [`VehicleSchedule`] in Paiagram.
    #[serde(rename = "circuits")]
    vehicles: Vec<Vehicle>,
    config: Option<Config>,
}

/// A line that is used as the foundation of connection in qETRC data
#[derive(Deserialize)]
struct Line {
    /// The name of the line
    name: String,
    /// [`Station`]s on the line.
    stations: Vec<QStation>,
}

#[derive(Deserialize)]
struct QStation {
    /// Station name
    #[serde(rename = "zhanming")]
    name: String,
    /// Distance from the start of the line, in kilometers
    #[serde(rename = "licheng")]
    distance_km: f32,
}

#[derive(Deserialize)]
struct Service {
    /// Each service may have multiple service numbers.
    /// In qETRC's case, the first service number is always the main one, and we use that one in Paiagram.
    #[serde(rename = "checi")]
    service_number: Vec<String>,
    #[serde(rename = "type")]
    service_type: String,
    /// The timetable entries of the service
    timetable: Vec<TimetableEntry>,
}

#[derive(Deserialize)]
struct TimetableEntry {
    /// Whether the train would stop and load/unload passengers or freight at the station.
    #[serde(rename = "business")]
    would_stop: Option<bool>,
    /// Arrival time in "HH:MM" format. "ddsj" in the original qETRC data refers to "到达时间".
    #[serde(rename = "ddsj")]
    arrival: String,
    /// Departure time in "HH:MM" format. "cfsj" in the original qETRC data refers to "出发时间".
    #[serde(rename = "cfsj")]
    departure: String,
    /// Station name
    #[serde(rename = "zhanming")]
    station_name: String,
}

#[derive(Deserialize)]
struct Vehicle {
    /// Vehicle model
    #[serde(rename = "model")]
    make: String,
    /// Vehicle name
    name: String,
    /// Services that the vehicle runs.
    #[serde(rename = "order")]
    services: Vec<VehicleServiceEntry>,
}

#[derive(Deserialize)]
struct VehicleServiceEntry {
    /// Service number of the service
    #[serde(rename = "checi")]
    service_number: String,
}

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    default_colors: HashMap<String, String>,
}

pub fn load_qetrc(event: On<super::LoadQETRC>, mut commands: Commands, mut graph: ResMut<Graph>) {
    let root: Root = match serde_json::from_str(&event.content) {
        Ok(r) => r,
        // TODO: handle warning better
        // TODO: add log page and warning banner
        Err(e) => {
            warn!("Failed to parse QETRC data: {e:?}");
            return;
        }
    };
    let lines_iter = std::iter::once(root.line).chain(root.lines.into_iter().flatten());
    let mut station_map: HashMap<String, Instance<Station>> = HashMap::new();
    let mut class_map: HashMap<String, Instance<Class>> = HashMap::new();
    if let Some(config) = root.config {
        for (class, color) in config.default_colors {
            // #RRGGBB
            // 0123456
            let (r, g, b) = (
                u8::from_str_radix(&color[1..=2], 16).unwrap(),
                u8::from_str_radix(&color[3..=4], 16).unwrap(),
                u8::from_str_radix(&color[5..=6], 16).unwrap(),
            );
            super::make_class(&class, &mut class_map, &mut commands, || ClassBundle {
                class: Class::default(),
                name: Name::new(class.clone()),
                stroke: DisplayedStroke {
                    width: 1.0,
                    color: DisplayColor::Custom(Color32::from_rgb(r, g, b)),
                },
            });
        }
    }
    for line in lines_iter {
        let mut entity_distances: Vec<(Instance<Station>, f32)> =
            Vec::with_capacity(line.stations.len());
        for station in line.stations {
            let e = super::make_station(&station.name, &mut station_map, &mut graph, &mut commands);
            entity_distances.push((e, station.distance_km));
        }
        for w in entity_distances.windows(2) {
            let [(prev, prev_d), (this, this_d)] = w else {
                unreachable!()
            };
            // TODO: handle one way stations and intervals
            let length = Distance::from_km((this_d - prev_d).abs());
            super::add_interval_pair(
                &mut graph,
                &mut commands,
                prev.entity(),
                this.entity(),
                length,
            );
        }
        let mut previous_distance_km = entity_distances.first().map_or(0.0, |(_, d)| *d);
        for (_, distance_km) in entity_distances.iter_mut().skip(1) {
            let current_distance_km = *distance_km;
            *distance_km -= previous_distance_km;
            previous_distance_km = current_distance_km;
        }
        // create a new displayed line
        commands.spawn((
            Name::new(line.name),
            Route {
                stops: entity_distances.iter().map(|(e, _)| e.entity()).collect(),
                lengths: entity_distances.iter().copied().map(|(_, d)| d).collect(),
            },
        ));
    }
    let mut trip_pool: HashMap<String, Entity> = HashMap::with_capacity(root.services.len());
    for service in root.services {
        let mut entries: Vec<_> = service
            .timetable
            .into_iter()
            .map(|e| {
                (
                    TimetableTime::from_str(&e.arrival).unwrap(),
                    TimetableTime::from_str(&e.departure).unwrap(),
                    super::make_station(
                        &e.station_name,
                        &mut station_map,
                        &mut graph,
                        &mut commands,
                    ),
                )
            })
            .collect();
        super::normalize_times(
            entries
                .iter_mut()
                .flat_map(|(a, d, _)| std::iter::once(a).chain(std::iter::once(d))),
        );
        let trip_class =
            super::make_class(&service.service_type, &mut class_map, &mut commands, || {
                ClassBundle {
                    class: Class::default(),
                    name: Name::new(service.service_type.clone()),
                    stroke: DisplayedStroke::default(),
                }
            });
        let trip_entity = commands
            .spawn(TripBundle::new(
                &service.service_number[0],
                TripClass(trip_class.entity()),
            ))
            .with_children(|bundle| {
                for (arr, dep, stop) in entries {
                    if dep < arr {
                        info!(?arr, ?dep, ?service.service_number)
                    }
                    debug_assert!(dep >= arr);
                    let dep = (dep != arr).then(|| TravelMode::At(dep));
                    let arr = TravelMode::At(arr);
                    bundle.spawn(EntryBundle::new(arr, dep, stop.entity()));
                }
            })
            .id();
        trip_pool.insert(service.service_number[0].clone(), trip_entity);
    }
    for vehicle in root.vehicles {
        let vehicle_name = format!("{} [{}]", vehicle.name, vehicle.make);
        let mut v = crate::vehicle::Vehicle::default();
        for number in vehicle.services.iter().map(|it| &it.service_number) {
            let Some(&e) = trip_pool.get(number) else {
                warn!(
                    "Vehicle {} has trip {} but the trip isn't in pool",
                    vehicle_name, number
                );
                continue;
            };
            v.trips.push(e);
        }
        commands.spawn((Name::new(vehicle_name), v));
    }
}
