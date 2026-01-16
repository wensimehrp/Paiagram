use super::Tab;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct InspectorTab;

impl Tab for InspectorTab {
    const NAME: &'static str = "Inspector";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        bevy_inspector_egui::bevy_inspector::ui_for_world(world, ui);
    }
}
