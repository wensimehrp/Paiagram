use bevy::ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    hierarchy::ChildOf,
    name::Name,
    query::With,
    system::{In, InMut, Local, Query, Res},
    world::Ref,
};
use egui::Ui;
use moonshine_core::kind::Instance;

use crate::{
    interface::tabs::PageCache,
    graph::{Graph, Interval, IntervalCache, Station},
    vehicles::entries::TimetableEntry,
};

pub fn show_interval_stats(
    (InMut(ui), In((s1, s2))): (InMut<Ui>, In<(Instance<Station>, Instance<Station>)>),
    mut interval_string: Local<String>,
    station_name: Query<&Name, With<Station>>,
    intervals: Query<(Ref<IntervalCache>, &Interval)>,
    graph: Res<Graph>,
    get_entry_parent: Query<&ChildOf, With<TimetableEntry>>,
    mut panel_buffer: Local<PageCache<Entity, Vec<Entity>>>,
) {
    // Display basic statistics and edit functions
    interval_string.clear();
    interval_string.push_str(
        station_name
            .get(s1.entity())
            .map_or("Unknown", Name::as_str),
    );
    interval_string.push_str(" ⇆ ");
    interval_string.push_str(
        station_name
            .get(s2.entity())
            .map_or("Unknown", Name::as_str),
    );
    ui.heading(interval_string.as_str());
    let Some(&edge) = graph.edge_weight(s1, s2) else {
        ui.label("Interval not found.");
        return;
    };
    let Ok((cache, info)) = intervals.get(edge.entity()) else {
        ui.label("Interval not found");
        return;
    };
    // only reconstruct when the cache changed
    let update_vehicle_buffer = |mut buffer: &mut Vec<Entity>| {
        buffer.clear();
        cache.passing_vehicles(&mut buffer, |e| get_entry_parent.get(e).ok());
        buffer.sort_unstable();
        buffer.dedup();
    };
    let passing_vehicles_buffer = panel_buffer.get_mut_or_insert_with(edge.entity(), || {
        let mut new_buffer = Vec::new();
        update_vehicle_buffer(&mut new_buffer);
        new_buffer
    });
    if cache.is_changed() {
        update_vehicle_buffer(passing_vehicles_buffer)
    }
    // TODO: move the strings into their separate buffers;
    egui::Grid::new("interval_info_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Length:");
            ui.monospace(info.length.to_string());
            ui.end_row();
            ui.label("Passing entries:");
            ui.monospace(cache.passing_entries.len().to_string());
            ui.end_row();
            ui.label("Passing vehicles:");
            ui.monospace(passing_vehicles_buffer.len().to_string());
        });
}
