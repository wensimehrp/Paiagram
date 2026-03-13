//! # Settings
//! Module for user preferences and project settings.

use crate::{i18n::Language, units::time::Duration};
use bevy::prelude::*;

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UserPreferences>()
            .init_resource::<ProjectSettings>()
            .add_systems(
                Update,
                sync_preferences.run_if(resource_changed::<UserPreferences>),
            );
    }
}

#[derive(Reflect, Resource)]
#[reflect(Resource)]
pub struct UserPreferences {
    pub lang: Language,
    pub dark_mode: bool,
    pub developer_mode: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            lang: Language::EnCA,
            dark_mode: false,
            developer_mode: cfg!(debug_assertions),
        }
    }
}

/// Only run when the preferences change
fn sync_preferences(preferences: Res<UserPreferences>) {
    egui_i18n::set_language(preferences.lang.identifier());
}

#[derive(Reflect, Resource)]
#[reflect(Resource)]
pub struct ProjectSettings {
    pub remarks: String,
    pub authors: Vec<String>,
    pub repeat_frequency: Duration,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            remarks: String::new(),
            authors: Vec::new(),
            repeat_frequency: Duration::from_secs(86400),
        }
    }
}
