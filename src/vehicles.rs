use crate::intervals::{self, Graph};
use crate::units::distance::Distance;
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::entries::{TimeEstimate, TimetableEntry, TimetableEntryCache};
use bevy::prelude::*;
use entries::TravelMode;
use smallvec::{SmallVec, smallvec};
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
                (
                    adjust_timetable_entry,
                    calculate_estimates,
                    populate_services,
                )
                    .chain()
                    .run_if(on_message::<AdjustTimetableEntry>),
            );
    }
}

fn populate_services(
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
    mut schedules: Populated<&mut entries::VehicleSchedule>,
    entries: Populated<(&TimetableEntry, &ChildOf)>,
) {
    for msg in msg_reader.read() {
        let AdjustTimetableEntry { entity, .. } = msg;
        let Ok((_, parent)) = entries.get(*entity) else {
            continue;
        };
        let Ok(mut schedule) = schedules.get_mut(parent.0) else {
            continue;
        };
        let mut pool: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)> = Vec::new();
        let mut start: usize = 0;
        let mut previous_service: Option<Entity> = None;
        for (idx, entry_entity) in schedule.entities.iter().enumerate() {
            let Ok((entry, _)) = entries.get(*entry_entity) else {
                start = idx;
                continue;
            };
            let Some(current_service) = entry.service else {
                start = idx;
                continue;
            };

            if let Some(prev) = previous_service {
                if prev != current_service {
                    match pool.binary_search_by_key(&prev, |(e, _)| *e) {
                        Ok(j) => {
                            pool[j].1.push(start..idx);
                        }
                        Err(j) => {
                            pool.insert(j, (prev, smallvec![start..idx]));
                        }
                    }
                    start = idx;
                    previous_service = Some(current_service);
                }
            } else {
                previous_service = Some(current_service);
                start = idx;
            }
        }
        if let Some(prev) = previous_service {
            let idx = schedule.entities.len();
            match pool.binary_search_by_key(&prev, |(e, _)| *e) {
                Ok(j) => {
                    pool[j].1.push(start..idx);
                }
                Err(j) => {
                    pool.insert(j, (prev, smallvec![start..idx]));
                }
            }
        }
        schedule.service_entities = pool;
    }
}

fn calculate_estimates(
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
    mut entries: Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    intervals: Populated<&intervals::Interval>,
    parents: Populated<&ChildOf>,
    schedules: Populated<&entries::VehicleSchedule>,
    graph: Res<Graph>,
) {
    let mut clear_estimates = |entries: &mut Populated<(
        &TimetableEntry,
        &mut TimetableEntryCache,
    )>,
                               stack: &mut Vec<(Entity, Option<Duration>, Option<Duration>)>| {
        for (timetable_entry_entity, _, _) in stack.iter() {
            if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                cache.estimate = None;
            }
        }
        stack.clear();
    };
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
            let (arrival, departure, station) = {
                let Ok((tte, _)) = entries.get(*timetable_entry_entity) else {
                    continue;
                };
                (tte.arrival, tte.departure, tte.station)
            };

            if let Some(v) = pending_time_and_station.take() {
                stable_time_and_station = Some(v);
            }
            match (arrival, departure.unwrap_or(TravelMode::Flexible)) {
                (TravelMode::At(at), TravelMode::At(dt)) => {
                    if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: at,
                            departure: dt,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, station));
                    pending_time_and_station = Some((dt, station));
                }
                (TravelMode::At(at), TravelMode::For(dd)) => {
                    if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: at,
                            departure: at + dd,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, station));
                    pending_time_and_station = Some((at + dd, station));
                }
                (TravelMode::At(at), TravelMode::Flexible) => {
                    if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: at,
                            departure: at,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, station));
                }
                (TravelMode::For(ad), TravelMode::At(dt)) => {
                    let arrival_estimate = if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        stable_time + ad
                    } else {
                        dt
                    };
                    if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: arrival_estimate,
                            departure: dt,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, Some(ad)));
                    stable_time_and_station = Some((dt, station));
                }
                (TravelMode::For(ad), TravelMode::For(dd)) => {
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                            cache.estimate = Some(TimeEstimate {
                                arrival: stable_time + ad,
                                departure: stable_time + ad + dd,
                            });
                        }
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, station));
                        pending_time_and_station = Some((stable_time + ad + dd, station));
                    } else {
                        stack.push((*timetable_entry_entity, Some(ad), Some(dd)));
                    }
                }
                (TravelMode::For(ad), TravelMode::Flexible) => {
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                            cache.estimate = Some(TimeEstimate {
                                arrival: stable_time + ad,
                                departure: stable_time + ad,
                            });
                        }
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, station));
                    } else {
                        stack.push((*timetable_entry_entity, Some(ad), None));
                    }
                }
                (TravelMode::Flexible, TravelMode::At(at)) => {
                    if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: at,
                            departure: at,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, station));
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
                let station = {
                    let Ok((tte, _)) = entries.get(*timetable_entry_entity) else {
                        clear_estimates(&mut entries, &mut stack);
                        continue 'iter_timetable;
                    };
                    tte.station
                };
                let interval_distance = if previous_station == station || arr_dur.is_some() {
                    None
                } else {
                    match graph.0.edge_weight(previous_station, station) {
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
                previous_station = station;
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
            if let Some(dur) = time_offset {
                current_time -= dur;
            }
            debug_assert_eq!(distances.len(), stack.len() + 1);
            while let (Some((timetable_entry_entity, arr_dur, dep_dur)), Some(distance)) =
                (stack.pop(), distances.pop())
            {
                if let Some(distance) = distance {
                    current_time -= distance / velocity;
                }
                let departure_estimate = current_time;
                if let Some(dur) = dep_dur {
                    current_time = current_time - dur;
                }
                let arrival_estimate = current_time;
                if let Some(dur) = arr_dur {
                    current_time = current_time - dur;
                }
                if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                    cache.estimate = Some(TimeEstimate {
                        arrival: arrival_estimate,
                        departure: departure_estimate,
                    });
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
