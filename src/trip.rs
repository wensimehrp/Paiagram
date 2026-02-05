use crate::{
    entry,
    trip::class::{Class, DisplayedStroke},
    units::time::Duration,
    vehicle::Vehicle,
};
use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};
use smallvec::SmallVec;

pub mod class;
pub mod routing;

pub struct TripPlugin;
impl Plugin for TripPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(routing::RoutingPlugin)
            .add_observer(update_add_trip_vehicles)
            .add_observer(update_remove_trip_vehicles)
            .add_observer(update_remove_vehicle_trips);
    }
}

/// Marker component for a trip
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(TripVehicles, TripSchedule, Name)]
pub struct Trip;

/// Trip bundle.
/// Spawn with [`EntityCommands::with_children`]
#[derive(Bundle)]
pub struct TripBundle {
    trip: Trip,
    vehicles: TripVehicles,
    name: Name,
    class: TripClass,
}

impl TripBundle {
    pub fn new(name: &str, class: TripClass) -> Self {
        Self {
            trip: Trip,
            vehicles: TripVehicles::default(),
            name: Name::from(name),
            class,
        }
    }
}

/// Marker component for timing reference trips.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct IsTimingReference;

/// A trip in the world
#[derive(Default, Reflect, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
pub struct TripVehicles(#[entities] pub SmallVec<[Entity; 1]>);

/// The class of the trip
#[derive(Reflect, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
#[relationship(relationship_target = class::Class)]
#[require(Name)]
pub struct TripClass(#[entities] pub Entity);

/// A type alias for [`Children`]
pub type TripSchedule = Children;

/// Common query data for trips
#[derive(QueryData)]
pub struct TripQuery {
    trip: &'static Trip,
    pub vehicles: &'static TripVehicles,
    pub name: &'static Name,
    pub class: &'static TripClass,
    pub schedule: &'static TripSchedule,
}

impl<'w, 's> TripQueryItem<'w, 's> {
    /// The duration of the trip, from the first entry's arrival time to the last entry's
    /// departure time. This method only checks the first and last entries' times, hence
    /// any intermediate entries are not considered.
    fn duration<'a>(&self, q: &Query<'a, 'a, &entry::EntryEstimate>) -> Option<Duration> {
        let beg = self.schedule.first().cloned()?;
        let end = self.schedule.last().cloned()?;
        let end_t = q.get(end).ok()?;
        let beg_t = q.get(beg).ok()?;
        Some(end_t.dep - beg_t.arr)
    }
    fn stroke<'a>(&self, q: &Query<'a, 'a, &DisplayedStroke, With<Class>>) -> DisplayedStroke {
        q.get(self.class.entity()).unwrap().clone()
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This removes vehicles from trip data.
fn update_remove_trip_vehicles(
    removed_vehicle: On<Remove, Vehicle>,
    mut trips: Populated<&mut TripVehicles>,
    vehicles: Query<&Vehicle>,
) {
    let veh = removed_vehicle.entity;
    let Ok(Vehicle {
        trips: remove_pending,
    }) = vehicles.get(veh)
    else {
        return;
    };
    for &pending in remove_pending {
        let Ok(mut trip_vehicles) = trips.get_mut(pending) else {
            return;
        };
        trip_vehicles.retain(|v| *v != veh);
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This adds vehicles into trip data.
fn update_add_trip_vehicles(
    removed_vehicle: On<Add, Vehicle>,
    mut trips: Populated<&mut TripVehicles>,
    vehicles: Query<&Vehicle>,
) {
    let veh = removed_vehicle.entity;
    let Ok(Vehicle { trips: add_pending }) = vehicles.get(veh) else {
        return;
    };
    for pending in add_pending.iter().copied() {
        let Ok(mut trip_vehicles) = trips.get_mut(pending) else {
            return;
        };
        trip_vehicles.push(veh);
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This removes trips from vehicle data.
fn update_remove_vehicle_trips(
    removed_trip: On<Remove, TripVehicles>,
    mut vehicles: Populated<&mut Vehicle>,
    trips: Query<&TripVehicles>,
) {
    let trip = removed_trip.entity;
    let Ok(remove_pending) = trips.get(trip) else {
        return;
    };
    for &pending in &remove_pending.0 {
        let Ok(mut trip_vehicles) = vehicles.get_mut(pending) else {
            return;
        };
        trip_vehicles.trips.retain(|v| *v != trip);
    }
}
