use bevy::{platform::collections::HashMap, prelude::*};
use moonshine_core::kind::{InsertInstance, Instance};
use serde::Deserialize;

use crate::{
    graph::{Graph, Station},
    rw_data::ModifyData,
    units::time::{Duration, TimetableTime},
    vehicles::{
        Vehicle,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Root {
    #[serde(rename = "version")]
    schema_version: u32,
    #[serde(rename = "vehicle-group-name")]
    name: String,
    game_properties: GameProperties,
    #[serde(default)]
    schedules: Vec<ExportSchedule>,
    orders: Vec<ExportOrder>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct GameProperties {
    ticks_per_minute: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ExportSchedule {
    slots: Vec<i32>,
    duration: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ExportOrder {
    // #[serde(rename = "type")]
    // order_type: OrderType,
    // stopping_pattern: Option<StoppingPattern>,
    destination_id: u32,
    destination_name: String,
    destination_location: DestinationPosition,
    travel_time: i32,
    wait_time: Option<i32>,
    // wait_fixed: Option<bool>,
    // stop_location: u32,
    schedule_index: Option<usize>,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "kebab-case")]
// enum OrderType {
//     GoToStation,
//     GoToWaypoint,
// }
//
// #[derive(Deserialize)]
// #[serde(rename_all = "kebab-case")]
// enum StoppingPattern {
//     GoNonstopVia,
// }

#[derive(Deserialize, Clone, Copy)]
struct DestinationPosition {
    #[serde(rename = "X")]
    x: f32,
    #[serde(rename = "Y")]
    y: f32,
}

impl Into<egui::Pos2> for DestinationPosition {
    fn into(self) -> egui::Pos2 {
        egui::Pos2 {
            x: self.x,
            y: self.y,
        }
    }
}

fn make_destination(
    destination_map: &mut HashMap<u32, Instance<Station>>,
    commands: &mut Commands,
    graph: &mut Graph,
    order: &ExportOrder,
) -> Instance<Station> {
    if let Some(s) = destination_map.get(&order.destination_id) {
        return *s;
    };
    // not found in list, insert instead
    let station = commands
        .spawn((Name::new(order.destination_name.clone()),))
        .insert_instance(Station(order.destination_location.into()))
        .into();
    destination_map.insert(order.destination_id, station);
    graph.add_node(station);
    station
}

pub fn load_jgrpp_timetable_export(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    mut graph: ResMut<Graph>,
) {
    let mut str: Option<&[String]> = None;
    for modification in reader.read() {
        match modification {
            ModifyData::LoadJGRPP(s) => str = Some(s.as_slice()),
            _ => {}
        }
    }
    let Some(str) = str else {
        return;
    };
    let mut destination_map: HashMap<u32, Instance<Station>> = HashMap::new();
    let vehicle_set_entity = commands
        .spawn((Name::new("JGRPP import set"), VehicleSet))
        .id();
    for maybe_root in str {
        let root: Root = match serde_json::from_str(maybe_root) {
            Ok(root) => root,
            Err(e) => {
                error!("Error while deserializing JGRPP timetable export: {:?}", e);
                continue;
            }
        };
        let tick_to_duration = |tick: i32| {
            let mins = tick as f32 / root.game_properties.ticks_per_minute as f32;
            Duration((mins * 60.0) as i32)
        };
        assert_eq!(root.schema_version, 1, "Version number must be 1!");
        // dyn, but this is trivial enough and it only runs once
        let schedules_iter: Box<dyn Iterator<Item = Vec<&ExportOrder>>> =
            if root.schedules.is_empty() {
                // simply chain from the start to the end
                Box::new(std::iter::once(
                    root.orders.iter().chain(root.orders.first()).collect(),
                ))
            } else {
                // wrap around the schedule to make sure those orders with scheduled dispatch settings are
                // always moved to the front.
                let first_trigger = root
                    .orders
                    .iter()
                    .position(|e| e.schedule_index.is_some())
                    .unwrap_or(0);
                let mut rotated = root.orders[first_trigger..]
                    .iter()
                    .chain(root.orders[..=first_trigger].iter())
                    .peekable();
                Box::new(std::iter::from_fn(move || {
                    let mut group = Vec::new();
                    if let Some(first) = rotated.next() {
                        group.push(first);
                    } else {
                        return None;
                    }
                    while let Some(item) = rotated.peek() {
                        if item.schedule_index.is_some() {
                            group.push(*item);
                            break;
                        } else {
                            group.push(rotated.next().unwrap());
                        }
                    }
                    if group.len() > 1 { Some(group) } else { None }
                }))
            };
        for raw_schedule in schedules_iter {
            let vehicle_entity = commands.spawn((Name::new(root.name.clone()), Vehicle)).id();
            commands
                .entity(vehicle_set_entity)
                .add_child(vehicle_entity);
            let mut vehicle_schedule = VehicleSchedule {
                start: TimetableTime(0),
                repeat: None,
                departures: Vec::new(),
                entities: Vec::new(),
            };
            let mut time_counter = TimetableTime(0);
            for order in raw_schedule {
                let station_instance =
                    make_destination(&mut destination_map, &mut commands, &mut graph, order);
                let entry = TimetableEntry {
                    station: station_instance.entity(),
                    arrival: {
                        let a = TravelMode::At(time_counter);
                        time_counter += tick_to_duration(order.travel_time);
                        a
                    },
                    departure: order.wait_time.map(|t| {
                        let a = TravelMode::At(time_counter);
                        time_counter += tick_to_duration(t);
                        a
                    }),
                    service: None,
                    track: None,
                };
                let entry_entity = commands.spawn(entry).id();
                commands.entity(vehicle_entity).add_child(entry_entity);
                if let Some(schedule_index) = order.schedule_index
                    && vehicle_schedule.repeat.is_none()
                {
                    let schedule = &root.schedules[schedule_index];
                    vehicle_schedule.repeat = Some(tick_to_duration(schedule.duration));
                    vehicle_schedule
                        .departures
                        .extend(schedule.slots.iter().cloned().map(tick_to_duration));
                }
                vehicle_schedule.entities.push(entry_entity);
            }
            if vehicle_schedule.repeat.is_none() {
                vehicle_schedule.repeat = Some(time_counter.as_duration());
                vehicle_schedule.departures = vec![Duration(0)]
            }
            commands.entity(vehicle_entity).insert(vehicle_schedule);
        }
    }
}
