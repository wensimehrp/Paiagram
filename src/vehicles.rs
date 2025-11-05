use crate::basic::*;
use bevy::prelude::*;

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
#[derive(Component, Reflect)]
pub struct TimetableEntry {
    /// Arrival type at this stop
    pub arrival: ArrivalType,
    /// Departure type at this stop
    pub departure: DepartureType,
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
            );
    }
}

pub enum TimetableAdjustment {
    AdjustArrivalType(crate::vehicles::ArrivalType),
    AdjustArrivalTime(crate::basic::TimetableTime),
    AdjustDepartureType(crate::vehicles::DepartureType),
    AdjustDepartureTime(crate::basic::TimetableTime),
    AdjustStation(Entity),
    AdjustService(Option<Entity>),
    AdjustTrack(Option<Entity>),
    AdjustNote(Option<String>),
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
            AdjustArrivalType(nt) => entry.arrival.set_type(*nt),
            AdjustDepartureTime(dt) => entry.departure.adjust_time(dt.0),
            AdjustDepartureType(nt) => entry.departure.set_type(*nt),
            AdjustStation(ns) => entry.station = *ns,
            AdjustService(ns) => entry.service = *ns,
            AdjustTrack(nt) => entry.track = *nt,
            AdjustNote(note) => {
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
