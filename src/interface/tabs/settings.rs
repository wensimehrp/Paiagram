use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;

use crate::settings::ApplicationSettings;

pub fn show_settings(InMut(ui): InMut<Ui>, mut settings: ResMut<ApplicationSettings>) {
    ui.checkbox(&mut settings.enable_romaji_search, "Enable Romaji search");
    ui.checkbox(
        &mut settings.show_performance_stats,
        "Show performance statistics",
    );
}
