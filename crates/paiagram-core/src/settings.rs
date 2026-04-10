//! # Settings
//! Module for user preferences and project settings.

use crate::{i18n::Language, units::time::Duration};
use bevy::prelude::*;

#[derive(Default, Reflect, Copy, Clone, Debug, PartialEq, Eq)]
pub enum TripRenderMode {
    #[default]
    Gpu,
    Cpu,
}

#[derive(Reflect, Copy, Clone, Debug, PartialEq, Eq)]
pub enum AntialiasingMode {
    On,
    Off,
}

impl Default for AntialiasingMode {
    fn default() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Off
        } else {
            Self::On
        }
    }
}

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
    pub trip_render_mode: TripRenderMode,
    pub antialiasing_mode: AntialiasingMode,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            lang: Language::EnCA,
            dark_mode: false,
            developer_mode: cfg!(debug_assertions),
            trip_render_mode: TripRenderMode::default(),
            antialiasing_mode: AntialiasingMode::default(),
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
