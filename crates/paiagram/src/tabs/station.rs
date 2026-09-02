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
        if stations
            .query(self.stn_key, |view| {
                ScrollArea::both().show(ui, |ui| {
                    Frame::new().inner_margin(6).show(ui, |ui| {
                        station_ui(
                            view,
                            &app.source,
                            &mut app.ui_action_queue,
                            &mut app.command_queue,
                            ui,
                        );
                    })
                });
            })
            .is_none()
        {
            ui.centered_and_justified(|ui| ui.heading("Station does not exist!"));
        };
    }
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
}

fn station_ui(
    view: StationBorrow,
    source: &Source,
    ui_cmds: &mut Vec<UiCommand>,
    world_cmds: &mut Vec<Command>,
    ui: &mut Ui,
) {
    ui.heading(view.name.as_str());
    let mut all_trips: Vec<TripKey> = Vec::new();
    for nd_source in view.nodes.iter().copied() {
        let Some(neighbour_iter) = source.nodes.query(nd_source, |view| {
            [view.incoming, view.outgoing].into_iter().flatten().copied()
        }) else {
            continue;
        };
        for target in neighbour_iter {
            source.intervals.query((nd_source, target), |view| {
                all_trips.extend(view.trips.iter())
            });
        }
    }
    all_trips.sort_unstable();
    all_trips.dedup();
    let buckets: [Vec<TripKey>; 24] = std::array::from_fn(|_| Vec::new());
    for trip in all_trips {}
}
