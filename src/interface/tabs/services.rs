use bevy::ecs::{
    name::Name,
    system::{InMut, Query},
};
use egui::Ui;

use crate::vehicles::entries::{VehicleSchedule, VehicleScheduleCache};

pub fn show_services(
    InMut(ui): InMut<Ui>,
    schedules: Query<(&Name, &VehicleSchedule, &VehicleScheduleCache)>,
) {
}
