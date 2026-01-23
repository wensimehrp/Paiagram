use bevy::{ecs::system::RunSystemOnce, prelude::*};
use itertools::Itertools;
use moonshine_core::kind::Instance;
use serde::Serialize;

use crate::{
    graph::{Graph, Station},
    lines::DisplayedLine,
    units::time::TimetableTime,
    vehicles::entries::{
        TimetableEntry, TimetableEntryCache, VehicleSchedule, VehicleScheduleCache,
    },
};
pub struct TypstTimetable;

#[derive(Default, Serialize)]
#[serde(rename_all = "kebab-case")]
struct Root {
    stations: Vec<(String, StationSettings)>,
    vehicles: Vec<ExportedVehicle>,
}

#[derive(Serialize)]
struct StationSettings {
    show_arrival: bool,
    show_departure: bool,
    show_line_above: bool,
    show_line_below: bool,
}

impl Default for StationSettings {
    fn default() -> Self {
        Self {
            show_arrival: false,
            show_departure: true,
            show_line_above: false,
            show_line_below: false,
        }
    }
}

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum OperateMode {
    /// The vehicle does not operate at this stop at all.
    /// Equivalent to "..." in Japanese timetables
    NoOperation,
    /// The vehicle does not operate at this stop at all.
    /// Equivalent to "||"
    Skip,
    /// The vehicle bypasses this stop
    /// Equivalent to "re"
    NonStop(i32),
    Stop(i32, i32),
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct ExportedVehicle {
    name: String,
    schedule: Vec<OperateMode>,
}

impl<I: Iterator<Item = Entity>> super::ExportObject<(I, Entity)> for TypstTimetable {
    fn extension(&self) -> impl AsRef<str> {
        ".json"
    }
    fn filename(&self) -> impl AsRef<str> {
        "exported_timetable"
    }
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        input: (I, Entity),
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Handle the return values here
        let (vehicle_entities, displayed_line_entity) = input;
        let Some(displayed_line) = world.get::<DisplayedLine>(displayed_line_entity) else {
            return Err("Displayed line entity does not have a DisplayedLine component".into());
        };
        let mut root = Root::default();
        let station_list = displayed_line
            .stations
            .iter()
            .map(|(station, _)| *station)
            .collect::<Vec<_>>();
        root.stations.extend(station_list.iter().map(|s| {
            let name = world
                .get::<Name>(s.entity())
                .map_or("<unnamed>".into(), Name::to_string);
            (name, StationSettings::default())
        }));
        for entity in vehicle_entities {
            let mut vehicle = ExportedVehicle {
                name: String::new(),
                schedule: vec![OperateMode::Skip; station_list.len()],
            };
            if let Err(e) = world.run_system_once_with(
                make_json,
                (
                    &mut vehicle.name,
                    &mut vehicle.schedule,
                    entity,
                    &station_list,
                ),
            ) {
                // TODO
            };
            root.vehicles.push(vehicle);
        }
        serde_json::to_writer_pretty(buffer, &root)?;
        Ok(())
    }
}

fn make_json(
    (InMut(vehicle_name), InMut(vehicle_schedule), In(entity), InRef(station_list)): (
        InMut<String>,
        InMut<[OperateMode]>,
        In<Entity>,
        InRef<[Instance<Station>]>,
    ),
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    vehicles: Query<(&Name, &VehicleSchedule, &VehicleScheduleCache)>,
    graph: Res<Graph>,
) {
    let Ok((name, schedule, schedule_cache)) = vehicles.get(entity) else {
        return;
    };
    *vehicle_name = name.to_string();
    for (entry, entry_cache) in schedule_cache
        .actual_route
        .as_deref()
        .into_iter()
        .flatten()
        .filter_map(|e| timetable_entries.get(e.inner()).ok())
    {
        let Some(time) = entry_cache.estimate.as_ref() else {
            continue;
        };
        for i in station_list.iter().positions(|s| *s == entry.station()) {
            if entry.departure.is_none() {
                vehicle_schedule[i] = OperateMode::NonStop(time.departure.0)
            } else {
                vehicle_schedule[i] = OperateMode::Stop(time.arrival.0, time.departure.0)
            }
        }
    }
    for curr in 0..vehicle_schedule.len() {
        let current_station = station_list[curr];
        let prev_is_connected = curr == 0
            || station_list
                .get(curr - 1)
                .map_or(false, |s| graph.contains_edge(*s, current_station));
        let next_is_connected = station_list
            .get(curr + 1)
            .map_or(false, |s| graph.contains_edge(current_station, *s));
        let prev_is_continuous = curr == 0
            || vehicle_schedule
                .get(curr - 1)
                .map_or(false, |om| *om != OperateMode::Skip);
        let next_is_continuous = vehicle_schedule
            .get(curr + 1)
            .map_or(false, |om| *om != OperateMode::Skip);
        let invalid = (!prev_is_continuous && !next_is_continuous)
            || (!prev_is_connected && prev_is_continuous && !next_is_continuous)
            || (!next_is_connected && next_is_continuous && !prev_is_continuous);
        if invalid {
            vehicle_schedule[curr] = OperateMode::Skip
        }
    }
    if let Some(i1) = vehicle_schedule
        .iter()
        .position(|om| !matches!(om, OperateMode::Skip))
    {
        for om in vehicle_schedule[..i1].iter_mut() {
            *om = OperateMode::NoOperation
        }
    };
    if let Some(i2) = vehicle_schedule
        .iter()
        .rposition(|om| !matches!(om, OperateMode::Skip))
    {
        if i2 + 1 < vehicle_schedule.len() {
            for om in vehicle_schedule[i2 + 1..].iter_mut() {
                *om = OperateMode::NoOperation
            }
        }
    }
}
