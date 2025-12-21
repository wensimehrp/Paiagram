use egui::Ui;
use super::Tab;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct ClassesTab;

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn main_display(&mut self, _world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        ui.label("Classes tab is not yet implemented.");
    }
    fn display_display(&mut self, _world: &mut bevy::ecs::world::World, _ui: &mut Ui) {}
    fn edit_display(&mut self, _world: &mut bevy::ecs::world::World, _ui: &mut Ui) {}
}

// TODO: add class display
