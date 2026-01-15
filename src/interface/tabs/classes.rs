use super::Tab;
use egui::Ui;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClassesTab;

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn main_display(&mut self, _world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        ui.label("Classes tab is not yet implemented.");
    }
}

// TODO: add class display
