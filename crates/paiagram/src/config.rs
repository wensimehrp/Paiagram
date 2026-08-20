// SPDX-License-Identifier: MPL-2.0
//! The settings for the app

use paiagram_core::time::TDuration;

#[derive(Default)]
pub(crate) enum AntialiasingMode {
    #[default]
    On,
    Off,
}

#[derive(Default)]
pub(crate) enum LevelOfDetailMode {
    #[default]
    X1,
    X2,
    X4,
}

#[derive(Default)]
pub(crate) struct Preferences {
    pub dev_mode: bool,
    pub aa_mode: AntialiasingMode,
    pub lod_mode: LevelOfDetailMode,
}

#[derive(Default)]
pub(crate) struct Settings {
    pub repeat_frequency: TDuration,
}
