use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{In, InMut, Local, Query, Res},
};
use egui::Ui;

use crate::intervals::{Graph, Interval, IntervalCache, Station};

pub fn show_interval_stats(
    (InMut(ui), In((s1, s2))): (InMut<Ui>, In<(Entity, Entity)>),
    mut interval_string: Local<String>,
    station_name: Query<&Name, With<Station>>,
    intervals: Query<(&IntervalCache, &Interval)>,
    graph: Res<Graph>,
) {
    // Display basic statistics and edit functions
    interval_string.clear();
    interval_string.push_str(station_name.get(s1).map_or("Unknown", Name::as_str));
    interval_string.push_str(" ⇆ ");
    interval_string.push_str(station_name.get(s2).map_or("Unknown", Name::as_str));
    ui.heading(interval_string.as_str());
    let Some(&edge) = graph.edge_weight(s1, s2) else {
        ui.label("Interval not found.");
        return;
    };
    let Ok((cache, info)) = intervals.get(edge) else {
        ui.label("Interval not found");
        return;
    };
    egui::Grid::new("interval_info_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Length:");
            ui.monospace(info.length.to_string());
            ui.end_row();
            ui.label("Passing entries:");
            ui.monospace(cache.passing_entries.len().to_string());
        });
}
