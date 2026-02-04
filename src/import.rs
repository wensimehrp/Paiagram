//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;
use std::path::PathBuf;

mod qetrc;
// mod oudiasecond;

pub struct ImportPlugin;
impl Plugin for ImportPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(qetrc::load_qetrc);
    }
}

#[derive(Event)]
pub struct LoadQETRC {
    pub content: String,
}

#[derive(Event)]
pub struct LoadOuDiaSecond {
    path: PathBuf,
}

fn normalize_times<'a>(mut time_iter: impl Iterator<Item = &'a mut TimetableTime> + 'a) {
    let Some(mut previous_time) = time_iter.next().copied() else {
        return;
    };
    for time in time_iter {
        if *time < previous_time {
            *time += Duration(86400);
        }
        previous_time = *time;
    }
}
