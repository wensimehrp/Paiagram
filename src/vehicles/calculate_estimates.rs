use super::AdjustTimetableEntry;
use super::entries::{self, TravelMode};
use crate::graph::{self, Graph, Station};
use crate::units::distance::Distance;
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::entries::{TimeEstimate, TimetableEntry, TimetableEntryCache};
use bevy::prelude::*;
use moonshine_core::kind::Instance;

pub fn calculate_estimates(
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
    mut entries: Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    mut intervals: Query<&graph::Interval>,
    parents: Populated<&ChildOf>,
    schedules: Populated<&entries::VehicleScheduleCache>,
    graph: Res<Graph>,
    mut stack: Local<Vec<(Entity, Option<Duration>, Option<Duration>)>>,
    mut distances: Local<Vec<Option<Distance>>>,
) {
    fn clear_estimates(
        entries: &mut Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
        stack: &mut Vec<(Entity, Option<Duration>, Option<Duration>)>,
    ) {
        for (timetable_entry_entity, _, _) in stack.iter() {
            if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
                cache.estimate = None;
            }
        }
        stack.clear();
    }

    let mut messages = msg_reader.read().collect::<Vec<_>>();
    if messages.is_empty() {
        return;
    }

    for msg in messages {
        let AdjustTimetableEntry { entity, .. } = msg;
        let Ok(entry) = parents.get(*entity) else {
            continue;
        };
        let Ok(schedule) = schedules.get(entry.0) else {
            continue;
        };
        stack.clear();
        let mut stable_time_and_station: Option<(TimetableTime, Instance<Station>)> = None;
        let mut pending_time_and_station: Option<(TimetableTime, Instance<Station>)> = None;
        let mut unwind_params: Option<(
            Option<(TimetableTime, Instance<Station>)>,
            Option<Duration>,
        )> = None;
        'iter_timetable: for timetable_entry_entity in
            schedule.actual_route.iter().flatten().map(|e| e.inner())
        {
            let (arrival, departure, station) = {
                let Ok((tte, _)) = entries.get(timetable_entry_entity) else {
                    continue;
                };
                (tte.arrival, tte.departure, tte.station())
            };

            if let Some(v) = pending_time_and_station.take() {
                stable_time_and_station = Some(v);
            }
            match (arrival, departure.unwrap_or(TravelMode::Flexible)) {
                (TravelMode::At(at), TravelMode::At(dt)) => {
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
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
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
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
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
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
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
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
                        if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                            cache.estimate = Some(TimeEstimate {
                                arrival: stable_time + ad,
                                departure: stable_time + ad + dd,
                            });
                        }
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, station));
                        pending_time_and_station = Some((stable_time + ad + dd, station));
                    } else {
                        stack.push((timetable_entry_entity, Some(ad), Some(dd)));
                    }
                }
                (TravelMode::For(ad), TravelMode::Flexible) => {
                    if stack.is_empty()
                        && let Some((stable_time, _)) = stable_time_and_station
                    {
                        if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                            cache.estimate = Some(TimeEstimate {
                                arrival: stable_time + ad,
                                departure: stable_time + ad,
                            });
                        }
                        unwind_params = Some((stable_time_and_station, Some(ad)));
                        stable_time_and_station = Some((stable_time + ad, station));
                    } else {
                        stack.push((timetable_entry_entity, Some(ad), None));
                    }
                }
                (TravelMode::Flexible, TravelMode::At(at)) => {
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: at,
                            departure: at,
                        });
                    }
                    unwind_params = Some((stable_time_and_station, None));
                    stable_time_and_station = Some((at, station));
                }
                (TravelMode::Flexible, TravelMode::For(dd)) => {
                    stack.push((timetable_entry_entity, None, Some(dd)));
                }
                (TravelMode::Flexible, TravelMode::Flexible) => {
                    stack.push((timetable_entry_entity, None, None));
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
            distances.clear();
            let mut total_time = current_time - previous_time;
            for (timetable_entry_entity, arr_dur, dep_dur) in stack.iter() {
                let station = {
                    let Ok((tte, _)) = entries.get(*timetable_entry_entity) else {
                        clear_estimates(&mut entries, &mut stack);
                        continue 'iter_timetable;
                    };
                    tte.station()
                };
                let interval_distance = if previous_station == station || arr_dur.is_some() {
                    None
                } else {
                    match graph.edge_weight(previous_station, station) {
                        Some(w) => {
                            if let Ok(interval) = intervals.get(w.entity()) {
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
                match graph.edge_weight(previous_station, current_station) {
                    Some(w) => {
                        if let Ok(interval) = intervals.get(w.entity()) {
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
