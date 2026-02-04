use super::Tab;
use bevy::prelude::*;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct InspectorTab;

impl MapEntities for InspectorTab {
    fn map_entities<E: EntityMapper>(&mut self, _entity_mapper: &mut E) {}
}

impl Tab for InspectorTab {
    const NAME: &'static str = "Inspector";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        bevy_inspector_egui::bevy_inspector::ui_for_world(world, ui);
    }
}
