use paiagram_core::{
    entry::{EntryEstimate, EntryMode, EntryQuery, EntryQueryItem, TravelMode},
    station::{PlatformQuery, StationQuery},
    trip::{TripQuery, TripQueryItem},
};

use crate::widgets::timetable_popup::{
    arrival_popup, departure_popup, shift_at_value, shift_for_value,
};

use super::Tab;
use bevy::prelude::*;
use egui::{RectAlign, Ui, Vec2, vec2};
use egui_i18n::tr;
use bevy::ecs::entity::MapEntities;
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
    entry_mode_q: Query<(&EntryMode, Option<&EntryEstimate>)>,
    platform_q: Query<PlatformQuery>,
    station_q: Query<StationQuery>,
    mut commands: Commands,
) {
    let trip = trip_q.get(tab.trip_entity).unwrap();
    ui.heading(trip.name.as_str());
    ui.label(trip.schedule.len().to_string());
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(ui.id().with("lskdfjlsdkjflkdsjf"))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.label(tr!("trip-table-departure"));
                ui.end_row();
                for it in entry_q.iter_many(trip.schedule.iter()) {
                    row_ui(
                        &platform_q,
                        &station_q,
                        &entry_mode_q,
                        ui,
                        &trip,
                        &it,
                        &mut commands,
                    );
                    ui.end_row();
                }
            });
    });
}

fn row_ui(
    platform_q: &Query<PlatformQuery>,
    station_q: &Query<StationQuery>,
    entry_mode_q: &Query<(&EntryMode, Option<&EntryEstimate>)>,
    ui: &mut Ui,
    trip: &TripQueryItem,
    it: &EntryQueryItem,
    mut commands: &mut Commands,
) {
    const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);
    let platform = platform_q.get(it.stop()).unwrap();
    let station = platform.station(&station_q);

    // display station label
    ui.label(station.name.as_str());

    // display arrival button
    let arr_res = match it.mode.arr {
        None => ui.add_sized(BUTTON_SIZE, egui::Button::new("↓")),
        Some(TravelMode::Flexible) => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
        Some(TravelMode::At(t)) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, true),
        Some(TravelMode::For(d)) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE, true),
    };
    arrival_popup(
        &arr_res,
        &it,
        &trip,
        &entry_mode_q,
        RectAlign::LEFT,
        &mut commands,
    );

    // display departure button
    let dep_res = match it.mode.dep {
        TravelMode::Flexible => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
        TravelMode::At(t) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, false),
        TravelMode::For(d) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE, false),
    };
    departure_popup(&dep_res, &it, RectAlign::RIGHT, &mut commands);
}
