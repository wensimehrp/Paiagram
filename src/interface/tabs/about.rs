use bevy::ecs::system::InMut;
use egui::Ui;

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct AboutTab;

impl super::Tab for AboutTab {
    const NAME: &'static str = "About";
    fn main_display(&self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {}
}

fn show_about(InMut(ui): InMut<Ui>) {

}
