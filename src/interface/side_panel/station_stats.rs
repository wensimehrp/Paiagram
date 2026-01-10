use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{In, InMut, Query},
};
use egui::Ui;
use moonshine_core::kind::Instance;

use crate::graph::Station;

pub fn show_station_stats(
    (InMut(ui), In(station_entity)): (InMut<Ui>, In<Instance<Station>>),
    station_name: Query<&Name, With<Station>>,
) {
    // Display basic statistics and edit functions
    ui.heading(
        station_name
            .get(station_entity.entity())
            .map_or("Unknown", Name::as_str),
    );
}
