use bevy::ecs::{
    name::Name,
    system::{InMut, Query},
};
use egui::Ui;

use crate::vehicles::entries::{VehicleSchedule, VehicleScheduleCache};
use super::Tab;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct ServicesTab;

impl Tab for ServicesTab {
    const NAME: &'static str = "Services";
    fn main_display(&self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_services, ui) {
            bevy::log::error!("UI Error while displaying services page: {}", e)
        }
    }
}

fn show_services(
    InMut(ui): InMut<Ui>,
    schedules: Query<(&Name, &VehicleSchedule, &VehicleScheduleCache)>,
) {

}
