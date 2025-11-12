use crate::{basic::*, intervals::Graph};
use bevy::{prelude::*, render::render_resource::Tlas};

/// A vehicle that follows a schedule
#[derive(Component)]
#[require(Name, Schedule)]
pub struct Vehicle;

/// A service that a vehicle would operate on
#[derive(Component)]
#[require(Name)]
pub struct Service {
    pub class: Option<Entity>,
}

/// A Driver that drives a vehicle
#[derive(Component)]
#[require(Name)]
pub struct Driver;

#[derive(Component)]
#[require(Name)]
pub struct Class;

/// A list of times to timetable entries, with the key being the
/// time offset from the start of the schedule.
#[derive(Component, Default, Reflect)]
pub struct Schedule(pub ScheduleStart, pub Vec<Entity>);

/// How would the timetable start
#[derive(Reflect)]
pub enum ScheduleStart {
    /// The vehicle would repeat the schedule every fixed interval
    /// The first parameter is the start time offset,
    /// the second is the interval duration.
    Repeat(TimetableTime, TimetableTime),
    /// The vehicle would start at specific times
    At(Vec<TimetableTime>),
}

impl Default for ScheduleStart {
    fn default() -> Self {
        ScheduleStart::Repeat(TimetableTime(0), TimetableTime(86400))
    }
}

/// How would the vehicle arrive at a stop
#[derive(Debug, Default, Reflect, Clone, Copy)]
pub enum ArrivalType {
    /// The vehicle would travel for a fixed duration and arrive
    Duration(TimetableTime),
    /// The vehicle would arrive at a specific time
    At(TimetableTime),
    /// The vehicle arrives at a flexible time,
    #[default]
    Flexible,
}

impl std::fmt::Display for ArrivalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArrivalType::Duration(t) => write!(f, "-> {}", t),
            ArrivalType::At(t) => write!(f, "{}", t),
            ArrivalType::Flexible => write!(f, ">>"),
        }
    }
}

impl ArrivalType {
    pub fn adjust_time(&mut self, amount: i32) {
        match self {
            ArrivalType::Duration(t) => {
                t.0 += amount;
                t.0 = t.0.max(0);
            }
            ArrivalType::At(t) => {
                t.0 += amount;
            }
            ArrivalType::Flexible => {
                warn!("Cannot adjust flexible arrival time");
            }
        }
    }
    pub fn set_type(&mut self, new_type: ArrivalType) {
        *self = new_type;
    }
    pub fn time(self) -> Option<TimetableTime> {
        match self {
            Self::At(time) | Self::Duration(time) => Some(time),
            Self::Flexible => None,
        }
    }
}

/// How would the vehicle depart from a stop
#[derive(Debug, Default, Reflect, Clone, Copy)]
pub enum DepartureType {
    /// The vehicle would stay for a fixed duration
    Duration(TimetableTime),
    /// The vehicle would wait until a specific time
    At(TimetableTime),
    /// The vehicle does not stop at this station
    NonStop,
    /// The vehicle would stay for a flexible duration,
    /// depending on the condition. E.g., a bus may skip
    /// a stop if no passengers are waiting, and no passengers
    /// are getting off.
    #[default]
    Flexible,
}

impl DepartureType {
    pub fn adjust_time(&mut self, amount: i32) {
        match self {
            DepartureType::Duration(t) => {
                t.0 += amount;
                t.0 = t.0.max(0);
            }
            DepartureType::At(t) => {
                t.0 += amount;
            }
            DepartureType::NonStop => {
                warn!("Cannot adjust non-stop departure time");
            }
            DepartureType::Flexible => {
                warn!("Cannot adjust flexible departure time");
            }
        }
    }
    pub fn set_type(&mut self, new_type: DepartureType) {
        *self = new_type;
    }
}

impl std::fmt::Display for DepartureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepartureType::Duration(t) => write!(f, "-> {}", t),
            DepartureType::At(t) => write!(f, "{}", t),
            DepartureType::NonStop => write!(f, "··"),
            DepartureType::Flexible => write!(f, ">>"),
        }
    }
}

/// A single entry in a timetable, representing a stop at a station
#[derive(Component, Reflect, Clone, Copy)]
pub struct TimetableEntry {
    /// Arrival type at this stop
    pub arrival: ArrivalType,
    /// Estimate of the arrival time
    pub arrival_estimate: Option<TimetableTime>,
    /// Departure type at this stop
    pub departure: DepartureType,
    /// Estimate of the departure time.
    pub departure_estimate: Option<TimetableTime>,
    /// Station ID
    pub station: Entity, // Reference to the station/node entity
    /// Service ID when the vehicle arrives at this this stop
    pub service: Option<Entity>, // Reference to the service entity
    /// Track ID, if applicable
    pub track: Option<Entity>,
}

pub struct VehiclesPlugin;

impl Plugin for VehiclesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AdjustTimetableEntry>()
            .add_message::<AdjustVehicle>()
            .add_systems(
                FixedUpdate,
                adjust_timetable_entry.run_if(on_message::<AdjustTimetableEntry>),
            )
            .add_systems(
                FixedUpdate,
                adjust_vehicle.run_if(on_message::<AdjustVehicle>),
            )
            .add_systems(
                FixedPostUpdate,
                calculate_estimations.run_if(on_message::<AdjustTimetableEntry>),
            );
    }
}

/// Calculates time estimations.
/// The diagram and station timetable graph requires a time anchor in order to display the entries. Arrival/departure
/// types that are not the `At` type does not provide info about their absolute position, thus these relative entries'
/// time anchor need to be calculated.
fn calculate_estimations(
    schedules: Populated<&Schedule>,
    graph: Res<Graph>,
    intervals: Populated<&crate::intervals::Interval>,
    mut entries: Populated<(&mut TimetableEntry, &ChildOf)>,
    mut msg_reader: MessageReader<AdjustTimetableEntry>,
) {
    // first assume that all entries are valid, then do the error handling
    // this should still work even if the timetable time is negative
    let mut stack: Vec<(Entity, Option<TimetableTime>)> = Vec::with_capacity(32);
    let mut total_distances: Vec<TrackDistance> = Vec::with_capacity(32);
    for adjusted in msg_reader.read() {
        let Some(schedule) = (match entries.get(adjusted.entity) {
            Ok((_, parent)) => schedules.get(parent.0).ok(),
            Err(_) => None,
        }) else {
            continue;
        };
        stack.clear();
        total_distances.clear();
        let mut current_time_and_location: Option<(Entity, TimetableTime)> = None;
        let mut pending_time_and_location: Option<(Entity, TimetableTime)> = None;
        let mut pending_entry: Option<(Entity, Option<TimetableTime>)> = None;
        'iter_entries: for entity in schedule.1.iter() {
            let mut unwind_stack: Option<(bool, Option<(Entity, TimetableTime)>)> = None;
            {
                let Ok((mut entry, _)) = entries.get_mut(*entity) else {
                    continue;
                };
                if pending_time_and_location.is_some() {
                    current_time_and_location = pending_time_and_location;
                }
                if let Some(pending_entry) = pending_entry.take() {
                    stack.push(pending_entry);
                }
                match entry.arrival {
                    ArrivalType::At(t) => {
                        entry.arrival_estimate = Some(t);
                        unwind_stack = Some((true, current_time_and_location));
                        current_time_and_location = Some((entry.station, t));
                    }
                    ArrivalType::Duration(t) => {
                        if stack.is_empty()
                            && let Some((_, unwrapped_time)) = current_time_and_location
                        {
                            entry.arrival_estimate = Some(unwrapped_time + t);
                            current_time_and_location = Some((entry.station, unwrapped_time + t));
                        } else {
                            stack.push((*entity, Some(t)));
                        }
                    }
                    ArrivalType::Flexible => {
                        stack.push((*entity, None));
                    }
                }
                match entry.departure {
                    DepartureType::At(t) => {
                        entry.departure_estimate = Some(t);
                        if unwind_stack.is_none() {
                            // the arrival is not of AT type, thus directly replace the current time and location
                            unwind_stack = Some((false, current_time_and_location));
                            current_time_and_location = Some((entry.station, t));
                        } else {
                            // the arrival is of AT type. This means that the stack would be unwound right after
                            // processing the departure.
                            pending_time_and_location = Some((entry.station, t));
                        }
                    }
                    DepartureType::Duration(t) => {
                        if stack.is_empty()
                            && let Some((_, unwrapped_time)) = current_time_and_location
                        {
                            entry.departure_estimate = Some(unwrapped_time + t);
                            if unwind_stack.is_none() {
                                current_time_and_location =
                                    Some((entry.station, unwrapped_time + t));
                            } else {
                                pending_time_and_location =
                                    Some((entry.station, unwrapped_time + t));
                            }
                        } else {
                            if unwind_stack.is_none() {
                                stack.push((*entity, Some(t)));
                            } else {
                                pending_entry = Some((*entity, Some(t)));
                            }
                        }
                    }
                    DepartureType::Flexible | DepartureType::NonStop => {
                        if let ArrivalType::At(t) = entry.arrival {
                            entry.departure_estimate = Some(t);
                        } else {
                            if unwind_stack.is_none() {
                                stack.push((*entity, None));
                            } else {
                                pending_entry = Some((*entity, None));
                            }
                        }
                    }
                }
            }
            if let Some((mut is_departure, previous_time_and_location)) = unwind_stack {
                let Some((mut previous_station, previous_time)) = previous_time_and_location else {
                    while let Some((entity, _)) = stack.pop() {
                        let Ok((mut entry, _)) = entries.get_mut(entity) else {
                            continue;
                        };
                        if is_departure {
                            entry.departure_estimate = None;
                        } else {
                            entry.arrival_estimate = None;
                        }
                        is_departure = !is_departure;
                    }
                    total_distances.clear();
                    continue 'iter_entries;
                };
                let Some((current_station, current_time)) = current_time_and_location else {
                    stack.clear();
                    total_distances.clear();
                    continue 'iter_entries;
                };
                let mut total_time = current_time - previous_time;
                total_distances.clear();
                for (intermediate_entry, time_span) in stack.iter() {
                    let Ok((entry, _)) = entries.get(*intermediate_entry) else {
                        stack.clear();
                        total_distances.clear();
                        continue 'iter_entries;
                    };
                    if let Some(time_span) = time_span {
                        total_time -= *time_span;
                    }
                    if previous_station == entry.station {
                        total_distances.push(TrackDistance(0));
                    } else {
                        total_distances.push(
                            match graph.0.edge_weight(previous_station, entry.station) {
                                // there exists a direct path between the two stations
                                Some(interval_entity) => match intervals.get(*interval_entity) {
                                    Ok(interval) => interval.length,
                                    Err(_) => {
                                        // there isn't a valid interval
                                        stack.clear();
                                        total_distances.clear();
                                        continue 'iter_entries;
                                    }
                                },
                                // such path does not exist. Try using Dijkstra
                                // TODO: implement Dijkstra
                                None => {
                                    stack.clear();
                                    total_distances.clear();
                                    continue 'iter_entries;
                                }
                            },
                        );
                    }
                    previous_station = entry.station;
                }
                info!(?total_time);
                // the last interval must be non-determinant
                if previous_station == current_station {
                    total_distances.push(TrackDistance(0));
                } else {
                    total_distances.push(
                        match graph.0.edge_weight(previous_station, current_station) {
                            // there exists a direct path between the two stations
                            Some(interval_entity) => match intervals.get(*interval_entity) {
                                Ok(interval) => interval.length,
                                Err(_) => {
                                    // there isn't a valid interval
                                    stack.clear();
                                    total_distances.clear();
                                    continue 'iter_entries;
                                }
                            },
                            // such path does not exist. Try using Dijkstra
                            // TODO: implement Dijkstra
                            None => {
                                stack.clear();
                                total_distances.clear();
                                continue 'iter_entries;
                            }
                        },
                    );
                }
                debug_assert_eq!(total_distances.len(), stack.len() + 1);
                let total_distance = TrackDistance(
                    total_distances
                        .iter()
                        .map(|distance| distance.0)
                        .sum::<i32>(),
                );
                if total_time == TimetableTime(0) {
                    stack.clear();
                    total_distances.clear();
                    continue 'iter_entries;
                }
                let mut current_time = current_time;
                let speed = total_distance / total_time;
                if speed == Speed(0.0) {
                    stack.clear();
                    continue 'iter_entries;
                }
                while let (Some((intermediate_entry, time_span)), Some(interval_distance)) =
                    (stack.pop(), total_distances.pop())
                {
                    let Ok((mut entry, _)) = entries.get_mut(intermediate_entry) else {
                        stack.clear();
                        total_distances.clear();
                        continue 'iter_entries;
                    };
                    let estimated_time = if interval_distance == TrackDistance(0) {
                        current_time
                    } else {
                        current_time - interval_distance / speed
                    };
                    if is_departure {
                        entry.departure_estimate = Some(estimated_time)
                    } else {
                        entry.arrival_estimate = Some(estimated_time)
                    }
                    current_time = estimated_time;
                    if let Some(time_span) = time_span {
                        current_time -= time_span;
                    }
                    is_departure = !is_departure;
                }
            }
        }
        // handle the remaining entries
        // this is for if the schedule don't have an absolute entry at the end.
        let mut is_departure = true;
        while let Some((entity, _)) = stack.pop() {
            let Ok((mut entry, _)) = entries.get_mut(entity) else {
                continue;
            };
            if is_departure {
                entry.departure_estimate = None;
            } else {
                entry.arrival_estimate = None;
            }
            is_departure = !is_departure;
        }
        total_distances.clear();
    }
}

pub enum TimetableAdjustment {
    SetArrivalType(crate::vehicles::ArrivalType),
    AdjustArrivalTime(crate::basic::TimetableTime),
    SetDepartureType(crate::vehicles::DepartureType),
    AdjustDepartureTime(crate::basic::TimetableTime),
    SetStation(Entity),
    SetService(Option<Entity>),
    SetTrack(Option<Entity>),
    SetNote(Option<String>),
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
    mut entries: Populated<&mut TimetableEntry>,
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
            AdjustArrivalTime(dt) => entry.arrival.adjust_time(dt.0),
            SetArrivalType(nt) => entry.arrival.set_type(*nt),
            AdjustDepartureTime(dt) => entry.departure.adjust_time(dt.0),
            SetDepartureType(nt) => entry.departure.set_type(*nt),
            SetStation(ns) => entry.station = *ns,
            SetService(ns) => entry.service = *ns,
            SetTrack(nt) => entry.track = *nt,
            SetNote(note) => {
                if let Some(text) = note {
                    commands.entity(*entity).insert(crate::basic::Note {
                        text: text.clone(),
                        modified_time: chrono::Utc::now().timestamp(),
                        created_time: chrono::Utc::now().timestamp(),
                    });
                } else {
                    commands.entity(*entity).remove::<crate::basic::Note>();
                }
            }
        }
    }
}

pub fn adjust_vehicle(
    mut commands: Commands,
    mut reader: MessageReader<AdjustVehicle>,
    mut vehicles: Populated<(&mut Schedule, &mut Name), With<Vehicle>>,
) {
    for msg in reader.read() {
        let AdjustVehicle { entity, adjustment } = msg;
        let (mut schedule, mut name) = match vehicles.get_mut(*entity) {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to adjust service {entity:?}: {e:?}");
                continue;
            }
        };

        use VehicleAdjustment::*;
        match adjustment {
            AddEntry(idx, entry) => {
                // add after idx
                schedule.1.insert(*idx + 1, *entry);
            }
            RemoveEntry(entry) => {
                schedule.1.retain(|e| e != entry);
            }
            Rename(new_name) => {
                name.set(new_name.clone());
            }
            Remove => {
                commands.entity(*entity).despawn_children().despawn();
            }
        }
    }
}
