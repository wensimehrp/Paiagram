use paiagram_core::{
    interval::IntervalQuery,
    station::{PlatformQuery, StationQuery},
    trip::TripQuery,
    vehicle::VehicleQuery,
};

use super::Tab;
use bevy::prelude::*;
use egui::Ui;
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default, MapEntities)]
pub struct StartTab;

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        world.run_system_cached_with(show_start, ui).unwrap();
        if ui.button(tr!("tab-start-merge-stations-by-name")).clicked() {
            world
                .run_system_cached(paiagram_core::graph::merge_station_by_name)
                .unwrap();
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
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
    ui.heading(tr!("program-name"));
    egui::Grid::new("start info grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label(tr!("tab-start-amount-vehicles"));
            ui.label(vehicles.count().to_string());
            ui.end_row();
            ui.label(tr!("tab-start-amount-trips"));
            ui.label(trips.count().to_string());
            ui.end_row();
            ui.label(tr!("tab-start-amount-stations"));
            ui.label(stations.count().to_string());
            ui.end_row();
            ui.label(tr!("tab-start-amount-platforms"));
            ui.label(platforms.count().to_string());
            ui.end_row();
            ui.label(tr!("tab-start-amount-intervals"));
            ui.label(intervals.count().to_string());
        });
}
