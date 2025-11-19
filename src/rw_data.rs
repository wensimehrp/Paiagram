pub mod oudiasecond;
pub mod qetrc;

use bevy::prelude::*;

pub struct RwDataPlugin;

impl Plugin for RwDataPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ModifyData>().add_systems(
            FixedUpdate,
            (clear_resources, qetrc::load_qetrc, oudiasecond::load_oud2)
                .chain()
                .run_if(on_message::<ModifyData>),
        );
    }
}

#[derive(Message)]
pub enum ModifyData {
    ClearAllData,
    LoadQETRC(String),
    LoadOuDiaSecond(String),
}

fn clear_resources(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    vehicles: Query<Entity, With<crate::vehicles::Vehicle>>,
    intervals: Query<Entity, With<crate::intervals::Interval>>,
    stations: Query<Entity, With<crate::intervals::Station>>,
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
