use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{In, InMut, Local, Query},
};
use egui::Ui;

use crate::intervals::Station;

pub fn show_interval_stats(
    (InMut(ui), In((s1, s2))): (InMut<Ui>, In<(Entity, Entity)>),
    mut interval_string: Local<String>,
    station_name: Query<&Name, With<Station>>,
) {
    // Display basic statistics and edit functions
    interval_string.clear();
    interval_string.push_str(station_name.get(s1).map_or("Unknown", Name::as_str));
    interval_string.push_str(" ⇆ ");
    interval_string.push_str(station_name.get(s2).map_or("Unknown", Name::as_str));
    ui.heading(interval_string.clone());
}
