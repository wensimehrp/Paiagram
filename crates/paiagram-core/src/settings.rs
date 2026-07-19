//! User preferences and project settings.

use crate::units::time::Duration;

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub enum AntialiasingMode {
    #[default]
    On,
    Off,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LevelOfDetailMode {
    #[default]
    Off,
    Lod2,
    Lod4,
}

impl LevelOfDetailMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Off => 1,
            Self::Lod2 => 2,
            Self::Lod4 => 4,
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
    pub dark_mode: bool, // TODO: this should be handled by egui instead.
    pub developer_mode: bool,
    pub antialiasing_mode: AntialiasingMode,
    pub level_of_detail_mode: LevelOfDetailMode,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            lang: Language::EnCA,
            dark_mode: false,
            developer_mode: cfg!(debug_assertions),
            antialiasing_mode: AntialiasingMode::default(),
            level_of_detail_mode: LevelOfDetailMode::default(),
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
