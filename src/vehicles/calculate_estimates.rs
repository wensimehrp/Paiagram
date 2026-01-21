//! Calculate the time estimates for each entry in a timetable

use super::AdjustTimetableEntry;
use super::entries::{self, TravelMode};
use crate::graph::{self, Graph, Station};
use crate::units::distance::Distance;
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::entries::{TimeEstimate, TimetableEntry, TimetableEntryCache};
use bevy::prelude::*;
use moonshine_core::kind::Instance;

enum UnwindParams {
    At {
        previous: Option<(TimetableTime, Instance<Station>)>,
        time_offset: Option<Duration>,
    },
    For {
        previous: Option<(TimetableTime, Instance<Station>)>,
        current: (TimetableTime, Instance<Station>),
    },
}

fn clear_estimates(
    entries: &mut Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    stack: &mut Vec<(Entity, Duration)>,
) {
    for (timetable_entry_entity, _) in stack.iter() {
        if let Ok((_, mut cache)) = entries.get_mut(*timetable_entry_entity) {
            cache.estimate = None;
        }
    }
    stack.clear();
}

fn build_distances(
    entries: &mut Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    stack: &Vec<(Entity, Duration)>,
    distances: &mut Vec<Option<Distance>>,
    graph: &Graph,
    intervals: &Query<&graph::Interval>,
    mut previous_station: Instance<Station>,
    include_last_edge: bool,
    current_station: Instance<Station>,
) -> bool {
    distances.clear();
    for (timetable_entry_entity, _) in stack.iter() {
        let station = {
            let Ok((tte, _)) = entries.get(*timetable_entry_entity) else {
                return false;
            };
            tte.station()
        };
        let interval_distance = if previous_station == station {
            None
        } else {
            match graph.edge_weight(previous_station, station) {
                Some(w) => intervals.get(w.entity()).ok().map(|i| i.length),
                None => None,
            }
        };
        if previous_station != station && interval_distance.is_none() {
            return false;
        }
        distances.push(interval_distance);
        previous_station = station;
    }
    if include_last_edge {
        let last = match graph.edge_weight(previous_station, current_station) {
            Some(w) => intervals.get(w.entity()).ok().map(|i| i.length),
            None => None,
        };
        if last.is_none() {
            return false;
        }
        distances.push(last);
    } else {
        distances.push(None);
    }
    true
}

fn process_schedule(
    entries: &mut Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    intervals: &Query<&graph::Interval>,
    graph: &Graph,
    schedule: &entries::VehicleScheduleCache,
    stack: &mut Vec<(Entity, Duration)>,
    distances: &mut Vec<Option<Distance>>,
) {
    stack.clear();
    let mut stable_time_and_station: Option<(TimetableTime, Instance<Station>)> = None;
    let mut pending_time_and_station: Option<(TimetableTime, Instance<Station>)> = None;
    let mut unwind_params: Option<UnwindParams> = None;
    for timetable_entry_entity in schedule.actual_route.iter().flatten().map(|e| e.inner()) {
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
                unwind_params = Some(UnwindParams::At {
                    previous: stable_time_and_station,
                    time_offset: None,
                });
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
                unwind_params = Some(UnwindParams::At {
                    previous: stable_time_and_station,
                    time_offset: None,
                });
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
                unwind_params = Some(UnwindParams::At {
                    previous: stable_time_and_station,
                    time_offset: None,
                });
                stable_time_and_station = Some((at, station));
            }
            (TravelMode::For(ad), TravelMode::At(dt)) => {
                let Some((stable_time, _)) = stable_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                    cache.estimate = Some(TimeEstimate {
                        arrival: stable_time + ad,
                        departure: dt,
                    });
                }
                unwind_params = Some(UnwindParams::For {
                    previous: stable_time_and_station,
                    current: (stable_time + ad, station),
                });
                stable_time_and_station = Some((stable_time + ad, station));
                pending_time_and_station = Some((dt, station));
            }
            (TravelMode::For(ad), TravelMode::For(dd)) => {
                let Some((stable_time, _)) = stable_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                    cache.estimate = Some(TimeEstimate {
                        arrival: stable_time + ad,
                        departure: stable_time + ad + dd,
                    });
                }
                unwind_params = Some(UnwindParams::For {
                    previous: stable_time_and_station,
                    current: (stable_time + ad, station),
                });
                stable_time_and_station = Some((stable_time + ad, station));
                pending_time_and_station = Some((stable_time + ad + dd, station));
            }
            (TravelMode::For(ad), TravelMode::Flexible) => {
                let Some((stable_time, _)) = stable_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                    cache.estimate = Some(TimeEstimate {
                        arrival: stable_time + ad,
                        departure: stable_time + ad,
                    });
                }
                unwind_params = Some(UnwindParams::For {
                    previous: stable_time_and_station,
                    current: (stable_time + ad, station),
                });
                stable_time_and_station = Some((stable_time + ad, station));
            }
            (TravelMode::Flexible, TravelMode::At(at)) => {
                if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                    cache.estimate = Some(TimeEstimate {
                        arrival: at,
                        departure: at,
                    });
                }
                unwind_params = Some(UnwindParams::At {
                    previous: stable_time_and_station,
                    time_offset: None,
                });
                stable_time_and_station = Some((at, station));
            }
            (TravelMode::Flexible, TravelMode::For(dd)) => {
                stack.push((timetable_entry_entity, dd));
            }
            (TravelMode::Flexible, TravelMode::Flexible) => {
                stack.push((timetable_entry_entity, Duration(0)));
            }
        };
        let Some(unwind_params) = unwind_params.take() else {
            continue;
        };
        match unwind_params {
            UnwindParams::For {
                previous: previous_time_and_station,
                current: current_time_and_station,
            } => {
                if stack.is_empty() {
                    continue;
                }
                let Some((previous_time, previous_station)) = previous_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                let (mut current_time, current_station) = current_time_and_station;
                let mut total_time = current_time - previous_time;
                for (_, dep_dur) in stack.iter() {
                    total_time -= *dep_dur;
                }
                if !build_distances(
                    entries,
                    stack,
                    distances,
                    graph,
                    intervals,
                    previous_station,
                    true,
                    current_station,
                ) {
                    clear_estimates(entries, stack);
                    continue;
                }
                let total_distance = distances
                    .iter()
                    .filter_map(|d| *d)
                    .map(|d| d.0)
                    .sum::<i32>();
                if total_time.0 <= 0 || total_distance <= 0 {
                    clear_estimates(entries, stack);
                    continue;
                }
                let velocity = Distance(total_distance) / total_time;
                debug_assert_eq!(distances.len(), stack.len() + 1);
                while let (Some((timetable_entry_entity, dep_dur)), Some(distance)) =
                    (stack.pop(), distances.pop())
                {
                    if let Some(distance) = distance {
                        current_time -= distance / velocity;
                    }
                    let departure_estimate = current_time;
                    current_time = current_time - dep_dur;
                    let arrival_estimate = current_time;
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: arrival_estimate,
                            departure: departure_estimate,
                        });
                    }
                }
                continue;
            }
            UnwindParams::At {
                previous: previous_time_and_station,
                time_offset,
            } => {
                if stack.is_empty() {
                    continue;
                }
                let Some((previous_time, previous_station)) = previous_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                let Some((mut current_time, current_station)) = stable_time_and_station else {
                    clear_estimates(entries, stack);
                    continue;
                };
                let mut total_time = current_time - previous_time;
                for (_, dep_dur) in stack.iter() {
                    total_time -= *dep_dur;
                }
                if !build_distances(
                    entries,
                    stack,
                    distances,
                    graph,
                    intervals,
                    previous_station,
                    time_offset.is_none(),
                    current_station,
                ) {
                    clear_estimates(entries, stack);
                    continue;
                }
                let total_distance = distances
                    .iter()
                    .filter_map(|d| *d)
                    .map(|d| d.0)
                    .sum::<i32>();
                if let Some(dur) = time_offset {
                    total_time -= dur;
                }
                if total_time.0 <= 0 || total_distance <= 0 {
                    clear_estimates(entries, stack);
                    continue;
                }
                let velocity = Distance(total_distance) / total_time;
                if let Some(dur) = time_offset {
                    current_time -= dur;
                }
                debug_assert_eq!(distances.len(), stack.len() + 1);
                while let (Some((timetable_entry_entity, dep_dur)), Some(distance)) =
                    (stack.pop(), distances.pop())
                {
                    if let Some(distance) = distance {
                        current_time -= distance / velocity;
                    }
                    let departure_estimate = current_time;
                    current_time = current_time - dep_dur;
                    let arrival_estimate = current_time;
                    if let Ok((_, mut cache)) = entries.get_mut(timetable_entry_entity) {
                        cache.estimate = Some(TimeEstimate {
                            arrival: arrival_estimate,
                            departure: departure_estimate,
                        });
                    }
                }
            }
        }
    }
    clear_estimates(entries, stack);
}

pub fn calculate_estimates(
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
    mut entries: Populated<(&TimetableEntry, &mut TimetableEntryCache)>,
    intervals: Query<&graph::Interval>,
    parents: Populated<&ChildOf>,
    schedules: Populated<&entries::VehicleScheduleCache>,
    graph: Res<Graph>,
    mut stack: Local<Vec<(Entity, Duration)>>,
    mut distances: Local<Vec<Option<Distance>>>,
) {
    let messages = msg_reader.read().collect::<Vec<_>>();
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
        process_schedule(
            &mut entries,
            &intervals,
            &graph,
            schedule,
            &mut stack,
            &mut distances,
        );
    }
}
