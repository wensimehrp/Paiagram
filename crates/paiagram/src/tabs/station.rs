use egui::*;
use paiagram_core::*;
use serde::{Deserialize, Serialize};

use crate::UiCommand;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub(crate) struct StationTab {
    stn_key: StationKey,
}

impl super::Tab for StationTab {
    const NAME: &'static str = "Station";
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        let stations = &app.source.stations;
        let Some(stn) = stations.get(&self.stn_key) else {
            ui.centered_and_justified(|ui| ui.heading("Station does not exist!"));
            return;
        };
        ScrollArea::both().show(ui, |ui| {
            Frame::new().inner_margin(6).show(ui, |ui| {
                station_ui(
                    &stn.data,
                    &stn.cache,
                    &app.source,
                    &mut app.ui_action_queue,
                    ui,
                );
            })
        });
    }
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
}

fn station_ui(
    stn: &Station,
    cache: &StationCache,
    source: &Source,
    ui_cmds: &mut Vec<UiCommand>,
    ui: &mut Ui,
) {
    ui.heading(stn.name.as_str());
    let mut all_trips: Vec<TripKey> = Vec::new();
    all_trips.sort_unstable();
    all_trips.dedup();
    let buckets: [Vec<TripKey>; 24] = std::array::from_fn(|_| Vec::new());
    for trip in all_trips {}
}
