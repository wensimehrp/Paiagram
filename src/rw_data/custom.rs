use crate::{
    graph::{Graph, Station},
    rw_data::ModifyData,
    units::{distance::Distance, time::TimetableTime},
    vehicles::{
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};
use bevy::{platform::collections::HashMap, prelude::*};
use moonshine_core::kind::*;
use serde::Deserialize;
use serde_json;

#[derive(Deserialize)]
struct Root {
    #[serde(rename = "工作日")]
    weekday: HashMap<String, LineMeta>,
    #[serde(rename = "双休日")]
    holiday: HashMap<String, LineMeta>,
}

#[derive(Deserialize)]
struct LineMeta(HashMap<String, HashMap<String, Vec<TrainInfo>>>);

#[derive(Deserialize)]
struct TrainInfo((String, String));

pub fn load_qetrc(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    mut graph: ResMut<Graph>,
) {
    let mut str: Option<&str> = None;
    for modification in reader.read() {
        match modification {
            ModifyData::LoadCustom(s) => str = Some(s.as_str()),
            _ => {}
        }
    }
    let Some(str) = str else {
        return;
    };
    let root: Root = match serde_json::from_str(str) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to parse custom data: {}", e);
            return;
        }
    };
    info!("Reading...");
    let mut station_map: HashMap<String, Instance<Station>> = HashMap::new();
    let vehicle_set: Entity = commands
        .spawn((VehicleSet, Name::new("New vehicle set")))
        .id();
    for (line_name, line_meta) in root.weekday.into_iter().chain(root.holiday.into_iter()) {
        for (train_number, train_info) in line_meta.0.into_iter().flat_map(|(_, d)| d.into_iter()) {
            let get_interval = |commands: &mut Commands,
                                graph: &mut Graph,
                                from: Instance<Station>,
                                to: Instance<Station>| {
                if let Some(&weight) = graph.edge_weight(from, to) {
                    return weight;
                };
                let interval_entity = commands
                    .spawn(Name::new(format!(
                        "Interval {} - {}",
                        from.entity().index(),
                        to.entity().index()
                    )))
                    .insert_instance(crate::graph::Interval {
                        length: Distance(1000),
                        speed_limit: None,
                    })
                    .into();
                graph.add_edge(from, to, interval_entity);
                interval_entity
            };
            let mut get_station = |commands: &mut Commands, graph: &mut Graph, name: String| {
                if let Some(&entity) = station_map.get(&name) {
                    return entity;
                }
                let station_entity = commands
                    .spawn(Name::new(name.clone()))
                    .insert_instance(crate::graph::Station::default())
                    .into();
                station_map.insert(name, station_entity);
                graph.add_node(station_entity);
                station_entity
            };
            let mut vehicle_schedule: Vec<Entity> = Vec::new();
            let mut times: Vec<(String, TimetableTime)> = Vec::new();
            let mut previous_station: Option<Instance<Station>> = None;
            let vehicle_entity = commands
                .spawn((crate::vehicles::Vehicle, Name::new(train_number)))
                .id();
            for TrainInfo((station_name, time)) in train_info {
                let stripped_time = time.strip_prefix("(").unwrap_or(&time);
                let stripped_time = stripped_time.strip_suffix("-").unwrap_or(stripped_time);
                let Some(timetable_time) = TimetableTime::from_str(stripped_time) else {
                    warn!("Invalid time format: {}", time);
                    continue;
                };
                times.push((station_name, timetable_time));
            }
            super::normalize_times(times.iter_mut().map(|(_, t)| t));
            for (station_name, timetable_time) in times {
                let station_entity = get_station(&mut commands, &mut graph, station_name);
                if let Some(previous_station) = previous_station {
                    let _ =
                        get_interval(&mut commands, &mut graph, previous_station, station_entity);
                    let _ =
                        get_interval(&mut commands, &mut graph, station_entity, previous_station);
                }
                previous_station = Some(station_entity);
                let entry_entity = commands
                    .spawn((TimetableEntry {
                        station: station_entity.entity(),
                        arrival: TravelMode::At(timetable_time),
                        departure: Some(TravelMode::Flexible),
                        service: None,
                        track: None,
                    },))
                    .id();
                vehicle_schedule.push(entry_entity);
                commands.entity(vehicle_entity).add_child(entry_entity);
            }
            commands.entity(vehicle_entity).insert(VehicleSchedule {
                entities: vehicle_schedule,
                ..Default::default()
            });
            commands.entity(vehicle_set).add_child(vehicle_entity);
        }
    }
}
