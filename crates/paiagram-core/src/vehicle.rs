use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

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
