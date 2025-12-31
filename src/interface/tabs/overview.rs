use bevy::prelude::*;
use egui::{self, Ui};

pub struct OverviewTab;

impl super::Tab for OverviewTab {
    const NAME: &'static str = "Overview";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {

    }
}

fn show_overview(
    InMut(ui): InMut<Ui>,
) {

}
