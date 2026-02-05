use crate::trip::TripQuery;

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

fn show_trip((InMut(ui), InMut(tab)): (InMut<Ui>, InMut<TripTab>), trip_q: Query<TripQuery>) {
    let trip = trip_q.get(tab.trip_entity).unwrap();
    ui.heading(trip.name.as_str());
    ui.label(trip.schedule.len().to_string());
}
