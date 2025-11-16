use bevy::prelude::*;

#[derive(Debug, Component)]
#[require(Name)]
pub struct VehicleService {
    pub class: Option<Entity>,
}

#[derive(Debug, Component)]
#[require(Name)]
pub struct VehicleClass {
    pub color: [u8; 3],
}
