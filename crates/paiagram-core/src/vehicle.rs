//! A [`Vehicle`] is the executor of a [`crate::trip::Trip`]. Each Trip should be executed by a
//! vehicle. Vehicles contain multiple trips, just like how trips contain multiple entries.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

/// Definition of the Vehicle.
#[derive(Default, Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name)]
pub struct Vehicle {
    /// What trips does the vehicle contain
    #[entities]
    pub trips: Vec<Entity>,
}

#[derive(QueryData)]
pub struct VehicleQuery {
    name: &'static Name,
    vehicle: &'static Vehicle,
}
