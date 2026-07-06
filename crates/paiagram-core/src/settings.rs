//! User preferences and project settings.

use serde::{Deserialize, Serialize};

use crate::units::time::Duration;

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AntialiasingMode {
    #[default]
    On,
    Off,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserPreferences {
    pub lang: String,
    pub dark_mode: bool,
    pub developer_mode: bool,
    pub antialiasing_mode: AntialiasingMode,
    pub level_of_detail_mode: LevelOfDetailMode,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            lang: "en-CA".to_string(),
            dark_mode: false,
            developer_mode: cfg!(debug_assertions),
            antialiasing_mode: AntialiasingMode::default(),
            level_of_detail_mode: LevelOfDetailMode::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
