use paiagram_core::{
    interval::IntervalQuery,
    station::{PlatformQuery, StationQuery},
    trip::TripQuery,
    vehicle::VehicleQuery,
};

use super::Tab;
use bevy::prelude::*;
use egui::{ScrollArea, Ui};
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Debug, Default, Clone, Copy, EnumIter, PartialEq, Serialize, Deserialize)]
enum CurrentField {
    #[default]
    List,
    About,
    Misc,
}

impl CurrentField {
    fn name(self) -> &'static str {
        match self {
            Self::List => "List",
            Self::About => "About",
            Self::Misc => "Misc",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct StartTab {
    current_field: CurrentField,
}

impl Default for StartTab {
    fn default() -> Self {
        Self {
            current_field: CurrentField::default(),
        }
    }
}

impl MapEntities for StartTab {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {}
}

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        match self.current_field {
            CurrentField::List => {
                if let Err(e) = world.run_system_cached_with(show_start, ui) {
                    error!("UI Error while displaying start page: {e}")
                }
            }
            CurrentField::About => {
                show_about(ui);
            }
            CurrentField::Misc => {}
        }
        if ui.button("Merge stations by name").clicked() {
            world
                .run_system_cached(paiagram_core::graph::merge_station_by_name)
                .unwrap();
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
    }
    fn edit_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        // if let Err(e) = world.run_system_cached_with(tree_view::show_tree_view, ui) {
        //     error!("UI Error while displaying tree view: {e}")
        // }
    }
    fn display_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        // for field in CurrentField::iter() {
        //     ui.add_sized(vec2(ui.available_width(), 20.0), |ui: &mut Ui| {
        //         ui.selectable_value(&mut self.current_field, field, field.name())
        //     });
        // }
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}

fn show_start(
    InMut(ui): InMut<Ui>,
    vehicles: Query<VehicleQuery>,
    trips: Query<TripQuery>,
    stations: Query<StationQuery>,
    platforms: Query<PlatformQuery>,
    intervals: Query<IntervalQuery>,
) {
    ui.heading("Paiagram");
    egui::Grid::new("start info grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Amount of vehicles:");
            ui.label(vehicles.count().to_string());
            ui.end_row();
            ui.label("Amount of trips:");
            ui.label(trips.count().to_string());
            ui.end_row();
            ui.label("Amount of stations:");
            ui.label(stations.count().to_string());
            ui.end_row();
            ui.label("Amount of platforms:");
            ui.label(platforms.count().to_string());
            ui.end_row();
            ui.label("Amount of intervals:");
            ui.label(intervals.count().to_string());
        });
}

fn show_about(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        let max_width = (620.0f32).min(ui.available_width()) - 40.0;
        ui.set_max_width(max_width);
        ui.add_space(20.0);
        ui.heading(tr!("program-name"));
        ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
        ui.monospace(format!("Revision: {}", git_version::git_version!()));
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        ui.label("A high-performance transport timetable diagramming and analysis tool built with egui and Bevy.");
        ui.add_space(20.0);

        ui.collapsing("Authors", |ui| {
            ui.label("• Lead Developer: Jeremy Gao");
            ui.label("• Contributors: We don't have any yet");
        });

        ui.collapsing("Third-party Libraries", |ui| {
            ui.label("Paiagram is made possible by the following open-source projects:");
            ui.horizontal(|ui| {
                ui.label("•");
                ui.hyperlink_to("egui", "https://github.com/emilk/egui");
            });
            ui.horizontal(|ui| {
                ui.label("•");
                ui.hyperlink_to("Bevy Engine", "https://bevyengine.org/");
            });
            ui.horizontal(|ui| {
                ui.label("•");
                ui.hyperlink_to("Petgraph", "https://docs.rs/petgraph/latest/petgraph/");
            });
            ui.label("• Other Rust libraries. See cargo.toml and cargo.lock for a complete list of libraries used.");
        });

        ui.collapsing("License Information", |ui| {
            ui.label("Paiagram is a free software. If you bought it, you're likely scammed :-(");
            ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.monospace(include_str!("../../../../LICENSE.md"));
                });
        });

        ui.collapsing("Contact & Support", |ui| {
            ui.label("• Bug Reports: We don't have one yet.");
            ui.label("• Discussions: We don't have one yet.");
            ui.horizontal(|ui| {
                ui.label("• Email: ");
                ui.hyperlink("mailto://wensimehrp@gmail.com");
            });
        });

        ui.collapsing("Special Thanks", |ui| {
            ui.label("• x.e.p., for showing how to make stuff");
            ui.label("• Tantacurl, for showing how make accessible, reliable, and cool stuff");
        });

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.hyperlink_to("GitHub", "https://github.com/wensimehrp/Paiagram");
            ui.label("•");
            ui.hyperlink_to("Documentation", "https://wensimehrp.github.io/Paiagram");
        });
        ui.add_space(20.0);
    });
}
