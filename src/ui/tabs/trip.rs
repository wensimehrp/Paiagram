use crate::{
    entry::EntryQuery,
    station::{PlatformQuery, StationQuery},
    trip::TripQuery,
};

use super::Tab;
use bevy::prelude::*;
use egui::Ui;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, MapEntities, Clone)]
pub struct TripTab {
    #[entities]
    trip_entity: Entity,
}

impl PartialEq for TripTab {
    fn eq(&self, other: &Self) -> bool {
        self.trip_entity == other.trip_entity
    }
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        world.run_system_cached_with(show_trip, (ui, self)).unwrap();
    }
}

impl TripTab {
    pub fn new(trip_entity: Entity) -> Self {
        Self { trip_entity }
    }
}

fn show_trip(
    (InMut(ui), InMut(tab)): (InMut<Ui>, InMut<TripTab>),
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    platform_q: Query<PlatformQuery>,
    station_q: Query<StationQuery>,
) {
    let trip = trip_q.get(tab.trip_entity).unwrap();
    ui.heading(trip.name.as_str());
    ui.label(trip.schedule.len().to_string());
    egui::Grid::new(ui.id().with("lskdfjlsdkjflkdsjf"))
        .num_columns(1)
        .show(ui, |ui| {
            for it in entry_q.iter_many(trip.schedule) {
                let platform = platform_q.get(it.stop()).unwrap();
                let station = platform.station(&station_q);
                ui.label(station.name.as_str());
                ui.end_row();
            }
        });
}
