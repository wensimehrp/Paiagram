use bevy::{platform::collections::HashMap, prelude::*};
use moonshine_core::kind::*;
use serde::Deserialize;
use serde_json;

use crate::{
    intervals::{Graph, Interval, Station as IntervalStation},
    lines::DisplayedLine,
    rw_data::ModifyData,
    units::{distance::Distance, time::TimetableTime},
    vehicles::{
        entries::{TravelMode, VehicleSchedule},
        services::VehicleService,
        vehicle_set::VehicleSet,
    },
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
}

/// A line that is used as the foundation of connection in qETRC data
#[derive(Deserialize)]
struct Line {
    /// The name of the line
    name: String,
    /// [`Station`]s on the line.
    stations: Vec<Station>,
}

#[derive(Deserialize)]
struct Station {
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
    // #[serde(rename = "type")]
    // service_type: String,
    /// The timetable entries of the service
    timetable: Vec<TimetableEntry>,
}

#[derive(Deserialize)]
struct TimetableEntry {
    /// Whether the train would stop and load/unload passengers or freight at the station.
    #[serde(rename = "business")]
    stops: Option<bool>,
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

struct ProcessedEntry {
    arrival: TimetableTime,
    departure: TimetableTime,
    station_entity: Instance<IntervalStation>,
    service_entity: Instance<VehicleService>,
}

pub fn load_qetrc(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    mut graph: ResMut<Graph>,
) {
    let mut str: Option<&str> = None;
    for modification in reader.read() {
        match modification {
            ModifyData::LoadQETRC(s) => str = Some(s.as_str()),
            _ => {}
        }
    }
    let Some(str) = str else {
        return;
    };
    let root: Root = match serde_json::from_str(str) {
        Ok(r) => r,
        // TODO: handle warning better
        // TODO: add log page and warning banner
        Err(e) => {
            warn!("Failed to parse QETRC data: {e:?}");
            return;
        }
    };
    let lines_iter = std::iter::once(root.line).chain(root.lines.into_iter().flatten());
    let mut station_map: HashMap<String, Instance<crate::intervals::Station>> = HashMap::new();
    fn make_station(
        name: String,
        commands: &mut Commands,
        station_map: &mut HashMap<String, Instance<crate::intervals::Station>>,
        graph: &mut Graph,
    ) -> Instance<crate::intervals::Station> {
        if let Some(&entity) = station_map.get(&name) {
            return entity;
        }
        let station_entity = commands
            .spawn(Name::new(name.clone()))
            .insert_instance(crate::intervals::Station::default())
            .into();
        station_map.insert(name, station_entity);
        graph.add_node(station_entity);
        station_entity
    }
    for line in lines_iter {
        let mut entity_distances: Vec<(Instance<crate::intervals::Station>, f32)> =
            Vec::with_capacity(line.stations.len());
        for station in line.stations {
            let e = make_station(station.name, &mut commands, &mut station_map, &mut graph);
            entity_distances.push((e, station.distance_km));
        }
        for w in entity_distances.windows(2) {
            let [(prev, prev_d), (this, this_d)] = w else {
                unreachable!()
            };
            // TODO: handle one way stations and intervals
            let e1 = commands
                .spawn_instance(Interval {
                    speed_limit: None,
                    length: Distance::from_km((this_d - prev_d).abs()),
                })
                .into();
            let e2 = commands
                .spawn_instance(Interval {
                    speed_limit: None,
                    length: Distance::from_km((this_d - prev_d).abs()),
                })
                .into();
            graph.add_edge(*prev, *this, e1);
            graph.add_edge(*this, *prev, e2);
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
            DisplayedLine::new(entity_distances.into_iter().map(|(e, d)| (e, d)).collect()),
        ));
    }
    let mut service_pool: HashMap<String, Vec<ProcessedEntry>> =
        HashMap::with_capacity(root.services.len());
    for service in root.services {
        let service_name = service
            .service_number
            .get(0)
            .cloned()
            .unwrap_or("<Unnamed>".into());
        // TODO: handle class
        let service_entity = commands
            .spawn((Name::new(service_name.clone()),))
            .insert_instance(VehicleService { class: None })
            .into();
        let mut processed_entries: Vec<ProcessedEntry> =
            Vec::with_capacity(service.timetable.len());
        for entry in service.timetable {
            let station_entity = make_station(
                entry.station_name,
                &mut commands,
                &mut station_map,
                &mut graph,
            );
            let a = TimetableTime::from_str(&entry.arrival).unwrap_or_default();
            let d = TimetableTime::from_str(&entry.departure).unwrap_or_default();
            processed_entries.push(ProcessedEntry {
                arrival: a,
                departure: d,
                station_entity,
                service_entity,
            });
        }
        service_pool.insert(service_name, processed_entries);
    }
    let vehicle_set_entity = commands
        .spawn((Name::new("qETRC Vehicle Set"), VehicleSet))
        .id();
    for vehicle in root.vehicles {
        let processed_entries: Vec<ProcessedEntry> = vehicle
            .services
            .iter()
            .filter_map(|s| service_pool.remove(&s.service_number))
            .flatten()
            .collect();
        make_vehicle(
            format!("{} [{}]", vehicle.name, vehicle.make),
            &mut commands,
            processed_entries,
            vehicle_set_entity,
        );
    }
    for (service_name, entries) in service_pool {
        make_vehicle(service_name, &mut commands, entries, vehicle_set_entity);
    }
}

fn make_vehicle(
    name: String,
    commands: &mut Commands,
    mut processed_entries: Vec<ProcessedEntry>,
    vehicle_set_entity: Entity,
) {
    let vehicle_entity = commands
        .spawn((Name::new(name), crate::vehicles::Vehicle))
        .id();
    commands
        .entity(vehicle_set_entity)
        .add_child(vehicle_entity);
    let mut entry_entites: Vec<Entity> = Vec::with_capacity(processed_entries.len());
    super::normalize_times(
        processed_entries
            .iter_mut()
            .flat_map(|t| std::iter::once(&mut t.arrival).chain(std::iter::once(&mut t.departure))),
    );
    for ps in processed_entries {
        let (arrival_mode, departure_mode) = if ps.arrival == ps.departure {
            (TravelMode::At(ps.arrival), None)
        } else {
            (
                TravelMode::At(ps.arrival),
                Some(TravelMode::At(ps.departure)),
            )
        };
        let entry_entity = commands
            .spawn(crate::vehicles::entries::TimetableEntry {
                arrival: arrival_mode,
                departure: departure_mode,
                station: ps.station_entity,
                service: Some(ps.service_entity),
                track: None,
            })
            .id();
        commands.entity(vehicle_entity).add_child(entry_entity);
        entry_entites.push(entry_entity);
    }
    commands.entity(vehicle_entity).insert(VehicleSchedule {
        entities: entry_entites,
        ..Default::default()
    });
}
