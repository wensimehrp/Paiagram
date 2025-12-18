use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{In, InMut, Query},
};
use egui::Ui;

use crate::intervals::Station;

pub fn show_station_stats(
    (InMut(ui), In(station_entity)): (InMut<Ui>, In<Entity>),
    station_name: Query<&Name, With<Station>>,
) {
    // Display basic statistics and edit functions
    ui.heading(
        station_name
            .get(station_entity)
            .map_or("Unknown", Name::as_str),
    );
}
