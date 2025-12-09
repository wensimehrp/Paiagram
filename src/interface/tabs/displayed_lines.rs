use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{InMut, Local, Populated, Query},
};
use egui::Ui;

use crate::{
    interface::tabs::PageCache,
    intervals::{Station, StationCache},
    lines::DisplayedLine,
};

pub fn list_displayed_lines(
    InMut(ui): InMut<Ui>,
    mut displayed_lines: Query<(Entity, &mut DisplayedLine, &Name)>,
    station_names: Query<(Entity, &Name, &StationCache), With<Station>>,
    mut selected_line: Local<Option<Entity>>,
    mut selected_station_cache: Local<PageCache<Entity, Option<Entity>>>,
) {
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .min_width(20.0)
        .show_inside(ui, |ui| {
            if ui.button("Overview").clicked() {
                *selected_line = None;
            }
            for (line_entity, _, name) in displayed_lines.iter() {
                if ui.button(name.as_str()).clicked() {
                    *selected_line = Some(line_entity);
                }
            }
        });
    if let Some(line_entity) = *selected_line {
        let selected_station = selected_station_cache.get_mut_or_insert_with(line_entity, || None);
        if let Ok((_, displayed_line, _)) = displayed_lines.get(line_entity) {
            show_line(ui, displayed_line, selected_station, |e| {
                station_names.get(e).ok()
            });
        }
    } else {
        show_overview(ui);
    }
}

pub fn show_overview(ui: &mut Ui) {}

pub fn show_line<'a, F>(
    ui: &mut Ui,
    line: &DisplayedLine,
    selected_station: &mut Option<Entity>,
    get_station_info: F,
) where
    F: Fn(Entity) -> Option<(Entity, &'a Name, &'a StationCache)> + Copy + 'a,
{
    egui::SidePanel::left("inner_left_panel")
        .resizable(true)
        .min_width(20.0)
        .show_inside(ui, |ui| {
            for (station_entity, station_name, _) in line
                .stations
                .iter()
                .filter_map(|(e, _)| get_station_info(*e))
            {
                if ui.button(station_name.as_str()).clicked() {
                    *selected_station = Some(station_entity);
                }
            }
        });
    let Some(station_entity) = selected_station else {
        return;
    };
    let Some((_, station_name, station_cache)) = get_station_info(*station_entity) else {
        *selected_station = None;
        return;
    };
    ui.heading(station_name.as_str());
}
