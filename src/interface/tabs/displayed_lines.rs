use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{InMut, Local, Populated, Query},
};
use egui::{Frame, Response, RichText, ScrollArea, Sense, Ui, UiBuilder, Vec2};
use serde::{Deserialize, Serialize};

const PANEL_DEFAULT_SIZE: f32 = 20.0;

use crate::{
    graph::{Station, StationEntries},
    interface::tabs::{PageCache, Tab},
    lines::DisplayedLine,
};

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DisplayedLinesTab;

impl Tab for DisplayedLinesTab {
    const NAME: &'static str = "Available Lines";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_displayed_lines, ui) {
            bevy::log::error!("UI Error while displaying displayed lines page: {}", e)
        }
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

fn full_width_button(ui: &mut Ui, text: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2 {
            x: ui.available_width(),
            y: 15.0,
        },
        Sense::click(),
    );
    let res = ui.scope_builder(UiBuilder::new().sense(resp.sense).max_rect(rect), |ui| {
        let response = ui.response();
        let visuals = ui.style().interact(&response);
        let mut stroke = visuals.bg_stroke;
        stroke.width = 1.5;
        Frame::canvas(ui.style())
            .fill(visuals.bg_fill.gamma_multiply(0.2))
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.add(egui::Label::new(text).truncate())
            })
    });
    res.response
}

fn show_displayed_lines(
    InMut(ui): InMut<Ui>,
    displayed_lines: Query<(Entity, &mut DisplayedLine, &Name)>,
    station_names: Query<(Entity, &Name, &StationEntries), With<Station>>,
    mut selected_line: Local<Option<Entity>>,
    mut selected_station_cache: Local<PageCache<Entity, Option<Entity>>>,
) {
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .min_width(PANEL_DEFAULT_SIZE)
        .show_inside(ui, |ui| {
            if full_width_button(ui, "Overview").clicked() {
                *selected_line = None;
            }
            for (line_entity, _, name) in displayed_lines.iter() {
                if full_width_button(ui, name.as_str()).clicked() {
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

pub fn show_overview(ui: &mut Ui) {
    ui.heading("Overview");
}

pub fn show_line<'a, F>(
    ui: &mut Ui,
    line: &DisplayedLine,
    selected_station: &mut Option<Entity>,
    get_station_info: F,
) where
    F: Fn(Entity) -> Option<(Entity, &'a Name, &'a StationEntries)> + Copy + 'a,
{
    egui::SidePanel::left("inner_left_panel")
        .resizable(true)
        .min_width(PANEL_DEFAULT_SIZE)
        .show_inside(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for (station_entity, station_name, _) in line
                    .stations()
                    .iter()
                    .filter_map(|(e, _)| get_station_info(e.entity()))
                {
                    if full_width_button(ui, station_name.as_str()).clicked() {
                        *selected_station = Some(station_entity);
                    }
                }
            })
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
