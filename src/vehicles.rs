use crate::{graph::Station, vehicles::{entries::TimetableEntry, services::VehicleService}};
use bevy::prelude::*;
use moonshine_core::kind::Instance;
use smallvec::{SmallVec, smallvec};
mod calculate_estimates;
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
                    entries::calculate_actual_route,
                    calculate_estimates::calculate_estimates,
                    entries::populate_services,
                )
                    .chain()
                    .run_if(on_message::<AdjustTimetableEntry>),
            );
    }
}

pub enum TimetableAdjustment {
    SetArrivalType(entries::TravelMode),
    AdjustArrivalTime(crate::units::time::Duration),
    SetDepartureType(Option<entries::TravelMode>),
    AdjustDepartureTime(crate::units::time::Duration),
    SetStation(Instance<Station>),
    SetService(Option<Instance<VehicleService>>),
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
    entries: Populated<&entries::TimetableEntry>,
) {
    for msg in reader.read() {
        let AdjustTimetableEntry { entity, adjustment } = msg;
        let entry = match entries.get(*entity) {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to adjust timetable entry {entity:?}: {e:?}");
                continue;
            }
        };
        let mut new_entry = entry.clone();

        use TimetableAdjustment::*;
        match adjustment {
            AdjustArrivalTime(dt) => new_entry.arrival.adjust_time(*dt),
            SetArrivalType(nt) => new_entry.arrival = *nt,
            AdjustDepartureTime(dt) => {
                new_entry.departure.as_mut().map(|d| d.adjust_time(*dt));
            }
            SetDepartureType(nt) => new_entry.departure = *nt,
            SetStation(ns) => new_entry.station = ns.entity(),
            SetService(ns) => new_entry.service = *ns,
            SetTrack(nt) => new_entry.track = *nt,
            SetNote(note) => {}
            PassThrough => return,
        }
        commands.entity(*entity).insert(new_entry);
    }
}
