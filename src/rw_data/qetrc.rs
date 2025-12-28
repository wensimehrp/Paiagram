// TODO: rewrite this shit
use crate::intervals::*;
use crate::lines::{DisplayedLine, DisplayedLineType};
use crate::units::canvas::CanvasLength;
// use crate::lines::*;
use crate::units::distance::Distance;
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::entries::{TimetableEntry, TravelMode, VehicleSchedule};
use crate::vehicles::services::VehicleService;
use crate::vehicles::vehicle_set::VehicleSet;
use crate::vehicles::*;
use bevy::platform::collections::HashMap;
use serde::Deserialize;
use serde_json;

#[derive(Deserialize)]
struct RawQETRCRoot {
    // qetrc_release: u32,
    // qetrc_version: String,
    #[serde(rename = "trains")]
    services: Vec<RawQETRCService>,
    // qETRC has the line field and the lines array, both contains line data
    // but for some unknown(tm) reason sometimes the `lines` field is missing
    // hence Option<T>
    /// A single line
    line: RawQETRCLine,
    /// Additional lines. This field does not exist in pyETRC, only in qETRC.
    lines: Option<Vec<RawQETRCLine>>,
    #[serde(rename = "circuits")]
    vehicles: Vec<RawQETRCVehicle>,
}

#[derive(Deserialize)]
struct RawQETRCLine {
    name: String,
    stations: Vec<RawQETRCStation>,
}

#[derive(Deserialize)]
struct RawQETRCStation {
    #[serde(rename = "zhanming")]
    name: String,
    #[serde(rename = "licheng")]
    distance: f32,
}

#[derive(Deserialize)]
struct RawQETRCService {
    #[serde(rename = "checi")]
    service_number: Vec<String>,
    // #[serde(rename = "type")]
    // service_type: String,
    timetable: Vec<RawQETRCTimetableEntry>,
}

#[derive(Deserialize)]
struct RawQETRCTimetableEntry {
    #[serde(rename = "business")]
    stops: Option<bool>,
    #[serde(rename = "ddsj")]
    arrival: String,
    #[serde(rename = "cfsj")]
    departure: String,
    #[serde(rename = "zhanming")]
    station_name: String,
}

#[derive(Deserialize)]
struct RawQETRCVehicle {
    #[serde(rename = "model")]
    make: String,
    name: String,
    #[serde(rename = "order")]
    services: Vec<RawQETRCVehicleServiceEntry>,
}

#[derive(Deserialize)]
struct RawQETRCVehicleServiceEntry {
    #[serde(rename = "checi")]
    service_number: String,
}

struct QETRCRoot {
    // release: u32,
    // version: String,
    services: Vec<QETRCService>,
    lines: Vec<QETRCLine>,
    vehicles: Vec<QETRCVehicle>,
}

struct QETRCLine {
    name: String,
    stations: Vec<QETRCStation>,
}

struct QETRCStation {
    name: String,
    distance: f32,
}

struct QETRCService {
    name: String,
    // service_type: String,
    timetable: Vec<QETRCTimetableEntry>,
}

impl QETRCService {
    fn shift_time(&mut self, time: Duration) {
        self.timetable.iter_mut().for_each(|entry| {
            entry.arrival += time;
            entry.departure += time;
        });
    }
}

struct QETRCTimetableEntry {
    stops: bool,
    arrival: TimetableTime,
    departure: TimetableTime,
    station_name: String,
}

struct QETRCVehicle {
    make: String,
    name: String,
    services: Vec<QETRCService>,
}

impl TryFrom<RawQETRCRoot> for QETRCRoot {
    type Error = String;
    fn try_from(value: RawQETRCRoot) -> Result<Self, Self::Error> {
        let mut services = HashMap::with_capacity(value.services.len());
        for raw_service in value.services {
            let service = QETRCService::try_from(raw_service)?;
            services.insert(service.name.clone(), service);
        }
        let mut vehicles = Vec::with_capacity(value.vehicles.len());
        for raw_vehicle in value.vehicles {
            // consume the services that matches
            // keep in track of the last entry
            let mut vehicle_services = Vec::with_capacity(raw_vehicle.services.len());
            let mut last_entry: Option<&QETRCTimetableEntry> = None;
            for raw_service in raw_vehicle.services {
                if let Some((_, mut service)) = services.remove_entry(&raw_service.service_number) {
                    let current_first_entry = service.timetable.first();
                    // if there is a last entry, and the current first entry is before it, shift
                    if let (Some(last), Some(current_first)) = (last_entry, current_first_entry)
                        && current_first.arrival < last.departure
                    {
                        // shift by 24 hours
                        service.shift_time(Duration(86400));
                    }
                    vehicle_services.push(service);
                    last_entry = vehicle_services.last().and_then(|s| s.timetable.last());
                }
            }
            vehicles.push(QETRCVehicle {
                make: raw_vehicle.make,
                name: raw_vehicle.name,
                services: vehicle_services,
            });
        }
        // make the remaining orphaned services into a vec
        let services = services.into_values().collect::<Vec<_>>();
        let mut lines = Vec::with_capacity(1 + value.lines.iter().len());
        lines.push(QETRCLine::try_from(value.line)?);
        if let Some(raw_lines) = value.lines {
            lines.extend(
                raw_lines
                    .into_iter()
                    .map(QETRCLine::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        Ok(QETRCRoot {
            // release: value.qetrc_release,
            // version: value.qetrc_version,
            services,
            lines,
            vehicles,
        })
    }
}

impl TryFrom<RawQETRCLine> for QETRCLine {
    type Error = String;
    fn try_from(value: RawQETRCLine) -> Result<Self, Self::Error> {
        Ok(QETRCLine {
            name: value.name,
            stations: value
                .stations
                .into_iter()
                .map(QETRCStation::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<RawQETRCStation> for QETRCStation {
    type Error = String;
    fn try_from(value: RawQETRCStation) -> Result<Self, Self::Error> {
        Ok(QETRCStation {
            name: value.name,
            distance: value.distance,
        })
    }
}

impl TryFrom<RawQETRCService> for QETRCService {
    type Error = String;
    fn try_from(value: RawQETRCService) -> Result<Self, Self::Error> {
        let name = value.service_number.first().cloned().unwrap_or_default();
        let mut timetable = value
            .timetable
            .into_iter()
            .map(QETRCTimetableEntry::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut last_departure = TimetableTime(0);
        for entry in &mut timetable {
            if entry.arrival < last_departure {
                entry.arrival.0 += 86400;
                entry.departure.0 += 86400;
            }
            if entry.arrival > entry.departure {
                entry.departure.0 += 86400;
            }
            last_departure = entry.departure;
        }
        Ok(QETRCService {
            name,
            // service_type: value.service_type,
            timetable,
        })
    }
}

impl TryFrom<RawQETRCTimetableEntry> for QETRCTimetableEntry {
    type Error = String;
    fn try_from(value: RawQETRCTimetableEntry) -> Result<Self, Self::Error> {
        Ok(QETRCTimetableEntry {
            stops: value.stops.unwrap_or(false),
            arrival: TimetableTime::from_str(&value.arrival).unwrap_or_default(),
            departure: TimetableTime::from_str(&value.departure).unwrap_or_default(),
            station_name: value.station_name,
        })
    }
}

fn parse_qetrc(json_str: &str) -> Result<QETRCRoot, String> {
    let raw: RawQETRCRoot = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let qetrc_data: QETRCRoot = raw.try_into().map_err(|e: String| e.to_string())?;
    // Adjust timetable entries to ensure strictly increasing arrival times
    Ok(qetrc_data)
}

use super::ModifyData;
use bevy::prelude::*;

// try to parse QETRC data into bevy ECS components

pub fn load_qetrc(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    mut existing_graph: ResMut<Graph>,
) {
    let mut data: Option<&str> = None;
    for modification in reader.read() {
        let ModifyData::LoadQETRC(d) = modification else {
            continue;
        };
        data = Some(d);
    }
    let Some(data) = data else {
        return;
    };
    let vehicle_set_entity = commands
        .spawn((VehicleSet, Name::new("qETRC Vehicle Set")))
        .id();
    let now = instant::Instant::now();
    let qetrc_data = parse_qetrc(data).map_err(|e| e.to_string()).unwrap();
    info!("Parsed QETRC data in {:.2?}", now.elapsed());
    let now = instant::Instant::now();
    // dedup first, then create entities
    // station hashmap for deduplication
    let mut stations = std::collections::HashMap::new();
    // graph hashmap that stores the intervals
    // create stations and intervals from lines
    for line in qetrc_data.lines {
        create_line_entities(&mut commands, line, &mut stations, &mut existing_graph);
    }
    // create services and their timetables
    // reuse the stations hashmap for looking up station entities
    ensure_stations_exist(
        &mut commands,
        &qetrc_data.services,
        &qetrc_data.vehicles,
        &mut stations,
    );
    // create vehicle entities
    for vehicle in qetrc_data.vehicles {
        let new_vehicle_entity = create_vehicle(&mut commands, vehicle, &stations);
        commands
            .entity(vehicle_set_entity)
            .add_child(new_vehicle_entity);
    }
    // now create the orphaned services and their timetables
    for service in qetrc_data.services {
        let new_vehicle_entity = create_vehicle_from_service(&mut commands, service, &stations);
        commands
            .entity(vehicle_set_entity)
            .add_child(new_vehicle_entity);
    }
    info!("Loaded QETRC data in {:.2?}", now.elapsed());
}

fn create_vehicle(
    commands: &mut Commands,
    vehicle: QETRCVehicle,
    stations: &std::collections::HashMap<String, Entity>,
) -> Entity {
    let vehicle_entity = commands
        .spawn((
            Vehicle,
            Name::new(format!("{} [{}]", vehicle.name, vehicle.make)),
        ))
        .id();
    let mut timetable_entries = Vec::new();
    for service in vehicle.services {
        let service_entity = commands
            .spawn((VehicleService { class: None }, Name::new(service.name)))
            .id();
        commands.entity(vehicle_entity).add_child(service_entity);
        let service_entries = create_timetable_entries(
            commands,
            &service.timetable,
            stations,
            vehicle_entity,
            Some(service_entity),
        );
        timetable_entries.extend(service_entries);
    }
    commands.entity(vehicle_entity).insert(VehicleSchedule {
        entities: timetable_entries,
        ..Default::default()
    });
    vehicle_entity
}

fn create_vehicle_from_service(
    commands: &mut Commands,
    service: QETRCService,
    stations: &std::collections::HashMap<String, Entity>,
) -> Entity {
    let vehicle_entity = commands
        .spawn((Vehicle, Name::new(service.name.clone())))
        .id();
    let service_entity = commands
        .spawn((VehicleService { class: None }, Name::new(service.name)))
        .id();
    commands.entity(vehicle_entity).add_child(service_entity);
    let timetable_entries = create_timetable_entries(
        commands,
        &service.timetable,
        stations,
        vehicle_entity,
        Some(service_entity),
    );
    commands.entity(vehicle_entity).insert(VehicleSchedule {
        entities: timetable_entries,
        ..Default::default()
    });
    vehicle_entity
}

fn create_timetable_entries(
    commands: &mut Commands,
    timetable: &[QETRCTimetableEntry],
    stations: &std::collections::HashMap<String, Entity>,
    vehicle_entity: Entity,
    service_entity: Option<Entity>,
) -> Vec<Entity> {
    let mut entries = Vec::with_capacity(timetable.len());
    for (i, entry) in timetable.iter().enumerate() {
        let Some(&station_entity) = stations.get(&entry.station_name) else {
            continue;
        };
        let timetable_entry = commands
            .spawn({
                TimetableEntry {
                    arrival: if entry.stops && entry.arrival == entry.departure {
                        TravelMode::Flexible
                    } else {
                        TravelMode::At(entry.arrival)
                    },
                    departure: if !entry.stops && entry.arrival == entry.departure {
                        None
                    } else {
                        Some(TravelMode::At(entry.departure))
                    },
                    station: station_entity,
                    service: service_entity,
                    track: None,
                }
            })
            .id();
        entries.push(timetable_entry);
        commands.entity(vehicle_entity).add_child(timetable_entry);
    }
    entries
}

fn create_line_entities(
    commands: &mut Commands,
    line: QETRCLine,
    stations: &mut std::collections::HashMap<String, Entity>,
    graph_map: &mut ResMut<Graph>,
) {
    let mut intervals: DisplayedLineType = Vec::with_capacity(line.stations.len());
    let Some(first_station) = line.stations.first() else {
        commands.spawn((
            DisplayedLine::new(intervals),
            Name::new(line.name),
        ));
        return;
    };
    let first_entity = get_or_create_station(commands, stations, first_station);
    intervals.push((first_entity, 0.0));
    let mut prev_station = first_station;
    let mut prev_entity = first_entity;
    for station in line.stations.iter().skip(1) {
        let next_entity = get_or_create_station(commands, stations, station);
        let distance_delta = (station.distance - prev_station.distance).abs();
        if !graph_map.contains_edge(prev_entity, next_entity) {
            let interval_entity = commands
                .spawn(crate::intervals::Interval {
                    length: Distance::from_km(distance_delta),
                    speed_limit: None,
                })
                .id();
            graph_map.add_edge(prev_entity, next_entity, interval_entity);
        }
        if !graph_map.contains_edge(next_entity, prev_entity) {
            let interval_entity = commands
                .spawn(crate::intervals::Interval {
                    length: Distance::from_km(distance_delta),
                    speed_limit: None,
                })
                .id();
            graph_map.add_edge(next_entity, prev_entity, interval_entity);
        }
        intervals.push((next_entity, distance_delta));
        prev_station = station;
        prev_entity = next_entity;
    }
    commands.spawn((
        DisplayedLine::new(intervals),
        Name::new(line.name),
    ));
}

fn get_or_create_station(
    commands: &mut Commands,
    stations: &mut std::collections::HashMap<String, Entity>,
    station: &QETRCStation,
) -> Entity {
    if let Some(&entity) = stations.get(&station.name) {
        entity
    } else {
        let entity = commands
            .spawn((Station::default(), Name::new(station.name.clone())))
            .id();
        stations.insert(station.name.clone(), entity);
        entity
    }
}

fn ensure_stations_exist(
    commands: &mut Commands,
    services: &[QETRCService],
    vehicles: &[QETRCVehicle],
    stations: &mut std::collections::HashMap<String, Entity>,
) {
    let mut create_station_if_needed = |station_name: &str| {
        if stations.contains_key(station_name) {
            return;
        }
        let station = commands
            .spawn((Station::default(), Name::new(station_name.to_string())))
            .id();
        stations.insert(station_name.to_string(), station);
    };

    for vehicle in vehicles.iter() {
        for service in vehicle.services.iter() {
            for entry in service.timetable.iter() {
                create_station_if_needed(&entry.station_name);
            }
        }
    }
    for service in services.iter() {
        for entry in service.timetable.iter() {
            create_station_if_needed(&entry.station_name);
        }
    }
}
