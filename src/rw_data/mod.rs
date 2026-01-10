//! Module for reading and writing various data formats.

pub mod custom;
pub mod load_osm;
pub mod oudiasecond;
pub mod qetrc;
pub mod save;
pub mod write;

use bevy::prelude::*;
use serde::Deserialize;

use crate::units::time::{Duration, TimetableTime};

pub struct RwDataPlugin;

impl Plugin for RwDataPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ModifyData>().add_systems(
            FixedUpdate,
            (
                clear_resources,
                qetrc::load_qetrc,
                oudiasecond::load_oud2,
                custom::load_qetrc,
            )
                .chain()
                .run_if(on_message::<ModifyData>),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    QETRC,
    OuDiaSecond,
    Custom,
}

#[derive(Message)]
pub enum ModifyData {
    ClearAllData,
    LoadQETRC(String),
    LoadOuDiaSecond(String),
    LoadCustom(String),
    LoadOnlineData(String, DataType),
}

fn clear_resources(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    vehicles: Query<Entity, With<crate::vehicles::Vehicle>>,
    intervals: Query<Entity, With<crate::graph::Interval>>,
    stations: Query<Entity, With<crate::graph::Station>>,
) {
    let mut delete = false;
    for modification in reader.read() {
        if let ModifyData::ClearAllData = modification {
            delete = true;
        }
    }
    if !delete {
        return;
    }
    for vehicle in vehicles.iter() {
        commands.entity(vehicle).despawn_children().despawn();
    }
    for interval in intervals.iter() {
        commands.entity(interval).despawn_children().despawn();
    }
    for station in stations.iter() {
        commands.entity(station).despawn_children().despawn();
    }
    info!("Cleared all data from the application.");
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

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct UnparsedOnlineData(String);

fn load_online_data(url: In<String>, asset_server: Res<AssetServer>) {
    let handle: Handle<UnparsedOnlineData> = asset_server.load(url.0);
}
