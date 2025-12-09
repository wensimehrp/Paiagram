use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;

use crate::settings::ApplicationSettings;

pub fn show_setting_menu(
    InMut(ui): InMut<Ui>,
    settings: ResMut<ApplicationSettings>
) {

}
