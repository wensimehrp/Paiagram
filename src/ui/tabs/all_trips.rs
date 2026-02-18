use bevy::prelude::*;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct AllTripsTab {
    #[entities]
    route_entity: Entity,
}

impl PartialEq for AllTripsTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_entity == other.route_entity
    }
}

impl super::Tab for AllTripsTab {
    const NAME: &'static str = "All Trips";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {

    }
}
