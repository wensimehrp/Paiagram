use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

/// A vehicle is the "executor" of a [`crate::trip::Trip`].
#[derive(Default, Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name)]
pub struct Vehicle {
    #[entities]
    pub trips: Vec<Entity>,
}

#[derive(QueryData)]
pub struct VehicleQuery {
    name: &'static Name,
    vehicle: &'static Vehicle,
}
