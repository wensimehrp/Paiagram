use crate::intervals::{self, Graph};
use crate::units::distance::Distance;
use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;
use entries::TravelMode;
pub mod entries;
pub mod services;
pub mod vehicle_set;

#[derive(Debug, Component)]
#[require(Name, entries::VehicleSchedule)]
pub struct Vehicle;

pub struct VehiclesPlugin;

impl Plugin for VehiclesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AdjustTimetableEntry>()
            .add_message::<AdjustVehicle>()
            .add_systems(
                FixedUpdate,
                (adjust_timetable_entry, calculate_estimates)
                    .chain()
                    .run_if(on_message::<AdjustTimetableEntry>),
            );
    }
}

fn calculate_estimates(
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
    mut entries: Populated<&mut entries::TimetableEntry>,
    intervals: Populated<&intervals::Interval>,
    parents: Populated<&ChildOf>,
    schedules: Populated<&entries::VehicleSchedule>,
    graph: Res<Graph>,
) {
    fn clear_estimates(
        entries: &mut Populated<&mut entries::TimetableEntry>,
        stack: &mut Vec<(Entity, Option<Duration>, Option<Duration>)>,
    ) {
        for (timetable_entry_entity, _, _) in stack.iter() {
            let Ok(mut tte) = entries.get_mut(*timetable_entry_entity) else {
                continue;
            };
            tte.arrival_estimate = None;
            tte.departure_estimate = None;
        }
        stack.clear();
    }
    for msg in msg_reader.read() {
        let AdjustTimetableEntry { entity, .. } = msg;
        let Ok(entry) = parents.get(*entity) else {
            continue;
        };
        let Ok(schedule) = schedules.get(entry.0) else {
            continue;
        };
        let mut stack: Vec<(Entity, Option<Duration>, Option<Duration>)> = Vec::new();
        let mut stable_time_and_station: Option<(TimetableTime, Entity)> = None;
        let mut pending_time_and_station: Option<(TimetableTime, Entity)> = None;
        let mut unwind_params: Option<(Option<(TimetableTime, Entity)>, Option<Duration>)> = None;
        'iter_timetable: for timetable_entry_entity in schedule.entities.iter() {
            let Ok(mut tte) = entries.get_mut(*timetable_entry_entity) else {
                continue;
            };
            if let Some(v) = pending_time_and_station.take() {
                stable_time_and_station = Some(v);
            }
            match (tte.arrival, tte.departure.unwrap_or(TravelMode::Flexible)) {
                (TravelMode::At(at), TravelMode::At(dt)) => {
                    tte.arrival_estimate = Some(at);
                    tte.departure_estimate = Some(dt);
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, tte.station));
                    pending_time_and_station = Some((dt, tte.station));
                }
                (TravelMode::At(at), TravelMode::For(dd)) => {
                    tte.arrival_estimate = Some(at);
                    tte.departure_estimate = Some(at + dd);
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, tte.station));
                    pending_time_and_station = Some((at + dd, tte.station));
                }
                (TravelMode::At(at), TravelMode::Flexible) => {
                    tte.arrival_estimate = Some(at);
                    tte.departure_estimate = Some(at);
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, tte.station));
                }
                (TravelMode::For(ad), TravelMode::At(dt)) => {
                    tte.departure_estimate = Some(dt);
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        tte.arrival_estimate = Some(stable_time + ad);
                    } else {
                        tte.arrival_estimate = Some(dt);
                    }
                    unwind_params = Some((stable_time_and_station, Some(ad)));
                    stable_time_and_station = Some((dt, tte.station));
                }
                (TravelMode::For(ad), TravelMode::For(dd)) => {
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        tte.arrival_estimate = Some(stable_time + ad);
                        tte.departure_estimate = Some(stable_time + ad + dd);
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, tte.station));
                        pending_time_and_station = Some((stable_time + ad + dd, tte.station));
                    } else {
                        stack.push((*timetable_entry_entity, Some(ad), Some(dd)));
                    }
                }
                (TravelMode::For(ad), TravelMode::Flexible) => {
                    info!(?stable_time_and_station);
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        tte.arrival_estimate = Some(stable_time + ad);
                        tte.departure_estimate = Some(stable_time + ad);
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, tte.station));
                    } else {
                        stack.push((*timetable_entry_entity, Some(ad), None));
                    }
                }
                (TravelMode::Flexible, TravelMode::At(at)) => {
                    tte.arrival_estimate = Some(at);
                    tte.departure_estimate = Some(at);
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, tte.station));
                }
                (TravelMode::Flexible, TravelMode::For(dd)) => {
                    stack.push((*timetable_entry_entity, None, Some(dd)));
                }
                (TravelMode::Flexible, TravelMode::Flexible) => {
                    stack.push((*timetable_entry_entity, None, None));
                }
            };
            let Some((previous_time_and_station, time_offset)) = unwind_params.take() else {
                continue;
            };
            if stack.is_empty() {
                continue;
            }
            let Some((previous_time, mut previous_station)) = previous_time_and_station else {
                clear_estimates(&mut entries, &mut stack);
                continue;
            };
            let Some((mut current_time, current_station)) = stable_time_and_station else {
                clear_estimates(&mut entries, &mut stack);
                continue;
            };
            let mut distances = Vec::with_capacity(stack.len() + 1);
            let mut total_time = current_time - previous_time;
            for (timetable_entry_entity, arr_dur, dep_dur) in stack.iter() {
                let Ok(tte) = entries.get(*timetable_entry_entity) else {
                    clear_estimates(&mut entries, &mut stack);
                    continue 'iter_timetable;
                };
                let interval_distance = if previous_station == tte.station || arr_dur.is_some() {
                    None
                } else {
                    match graph.0.edge_weight(previous_station, tte.station) {
                        Some(w) => {
                            if let Ok(interval) = intervals.get(*w) {
                                Some(interval.length)
                            } else {
                                clear_estimates(&mut entries, &mut stack);
                                continue 'iter_timetable;
                            }
                        }
                        None => {
                            clear_estimates(&mut entries, &mut stack);
                            continue 'iter_timetable;
                        }
                    }
                };
                if let Some(dur) = arr_dur {
                    total_time -= *dur;
                }
                if let Some(dur) = dep_dur {
                    total_time -= *dur;
                }
                distances.push(interval_distance);
                previous_station = tte.station;
            }
            distances.push(if time_offset.is_none() {
                match graph.0.edge_weight(previous_station, current_station) {
                    Some(w) => {
                        if let Ok(interval) = intervals.get(*w) {
                            Some(interval.length)
                        } else {
                            clear_estimates(&mut entries, &mut stack);
                            continue 'iter_timetable;
                        }
                    }
                    None => {
                        clear_estimates(&mut entries, &mut stack);
                        continue 'iter_timetable;
                    }
                }
            } else {
                None
            });
            let total_distance = distances
                .iter()
                .filter_map(|d| *d)
                .map(|d| d.0)
                .sum::<i32>();
            if let Some(dur) = time_offset {
                total_time -= dur;
            }
            let velocity = Distance(total_distance) / total_time;
            info!(?velocity, ?total_distance, ?total_time, ?distances);
            if let Some(dur) = time_offset {
                current_time -= dur;
            }
            debug_assert_eq!(distances.len(), stack.len() + 1);
            while let (Some((timetable_entry_entity, arr_dur, dep_dur)), Some(distance)) =
                (stack.pop(), distances.pop())
            {
                let Ok(mut tte) = entries.get_mut(timetable_entry_entity) else {
                    continue;
                };
                if let Some(distance) = distance {
                    current_time -= distance / velocity;
                }
                tte.departure_estimate = Some(current_time);
                if let Some(dur) = dep_dur {
                    current_time = current_time - dur;
                }
                tte.arrival_estimate = Some(current_time);
                if let Some(dur) = arr_dur {
                    current_time = current_time - dur;
                }
            }
        }
        clear_estimates(&mut entries, &mut stack);
    }
}

pub enum TimetableAdjustment {
    SetArrivalType(entries::TravelMode),
    AdjustArrivalTime(crate::units::time::Duration),
    SetDepartureType(Option<entries::TravelMode>),
    AdjustDepartureTime(crate::units::time::Duration),
    SetStation(Entity),
    SetService(Option<Entity>),
    SetTrack(Option<Entity>),
    SetNote(Option<String>),
    PassThrough,
}

#[derive(Message)]
pub struct AdjustTimetableEntry {
    pub entity: Entity,
    pub adjustment: TimetableAdjustment,
}

pub enum VehicleAdjustment {
    AddEntry(usize, Entity),
    RemoveEntry(Entity),
    Rename(String),
    Remove,
}

#[derive(Message)]
pub struct AdjustVehicle {
    pub entity: Entity,
    pub adjustment: VehicleAdjustment,
}

pub fn adjust_timetable_entry(
    mut commands: Commands,
    mut reader: MessageReader<AdjustTimetableEntry>,
    mut entries: Populated<&mut entries::TimetableEntry>,
) {
    for msg in reader.read() {
        let AdjustTimetableEntry { entity, adjustment } = msg;
        let mut entry = match entries.get_mut(*entity) {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to adjust timetable entry {entity:?}: {e:?}");
                continue;
            }
        };

        use TimetableAdjustment::*;
        match adjustment {
            AdjustArrivalTime(dt) => entry.arrival.adjust_time(*dt),
            SetArrivalType(nt) => entry.arrival = *nt,
            AdjustDepartureTime(dt) => {
                entry.departure.as_mut().map(|d| d.adjust_time(*dt));
            }
            SetDepartureType(nt) => entry.departure = *nt,
            SetStation(ns) => entry.station = *ns,
            SetService(ns) => entry.service = *ns,
            SetTrack(nt) => entry.track = *nt,
            SetNote(note) => {}
            PassThrough => (),
        }
    }
}
