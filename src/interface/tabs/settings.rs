use super::Tab;
use crate::settings::ApplicationSettings;
use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SettingsTab;

impl Tab for SettingsTab {
    const NAME: &'static str = "Settings";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_settings, ui) {
            bevy::log::error!("UI Error while displaying settings page: {}", e)
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-settings").into()
    }
}

fn show_settings(InMut(ui): InMut<Ui>, mut settings: ResMut<ApplicationSettings>) {
    ui.add(&mut *settings);
}
