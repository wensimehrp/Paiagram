use bevy::prelude::*;
use moonshine_core::save::prelude::*;

/// A vehicle set represents a collection of vehicles
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(Name, Save)]
pub struct VehicleSet;
