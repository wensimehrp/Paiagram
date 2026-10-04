use egui::*;
use paiagram_core::{IntervalDirection, IntervalKey};
use serde::{Deserialize, Serialize};

use crate::tabs::trip::TripTab;
use crate::{App, MainTab, UiCommand};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct IntervalsTab {
    focused: Option<IntervalKey>,
}

impl PartialEq for IntervalsTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl super::Tab for IntervalsTab {
    const NAME: &'static str = "Intervals";
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Panel::left(ui.id().with("interval left panel")).frame(Frame::NONE).show(ui, |ui| {
            ScrollArea::vertical().auto_shrink(false).show_rows(
                ui,
                ui.spacing().interact_size.y,
                app.source.graph.intervals().len(),
                |ui, row_range| {
                    Frame::new().inner_margin(6.0).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                            for (&interval_key, interval) in app
                                .source
                                .graph
                                .intervals()
                                .iter()
                                .skip(row_range.start)
                                .take(row_range.count())
                            {
                                let IntervalKey { hi, lo } = interval_key;
                                let hi_text = app
                                    .graph
                                    .nodes()
                                    .get(&hi)
                                    .map_or("Unknown", |wfc| wfc.name.as_str());
                                let lo_text = app
                                    .graph
                                    .nodes()
                                    .get(&lo)
                                    .map_or("Unknown", |wfc| wfc.name.as_str());
                                let di_text = match interval.direction {
                                    IntervalDirection::Both => "<=>",
                                    IntervalDirection::HiToLo => "->",
                                    IntervalDirection::LoToHi => "<-",
                                };
                                let response = Button::selectable(
                                    self.focused == Some(interval_key),
                                    format!("{hi_text} {di_text} {lo_text}"),
                                )
                                .truncate()
                                .atom_ui(ui);
                                if response.clicked() {
                                    self.focused = Some(interval_key);
                                }
                            }
                        });
                    });
                },
            );
        });
        let mut ui_action_queue = std::mem::take(&mut app.ui_action_queue);
        CentralPanel::no_frame().show(ui, |ui| {
            let Some(interval) = self.focused else {
                ui.vertical_centered_justified(|ui| ui.heading("Nothing focused"));
                return;
            };
            let Some(interval) = app.source.graph.intervals().get(&interval) else {
                ui.vertical_centered_justified(|ui| ui.heading("Interval does not exist"));
                return;
            };
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                Frame::new().inner_margin(6.0).show(ui, |ui| {
                    for trip_key in &interval.cache.trips {
                        let Some(trip) = app.trips.get(trip_key) else {
                            continue;
                        };
                        if ui.button(trip.name.as_str()).clicked() {
                            ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Trip(
                                TripTab::new(*trip_key),
                            )));
                        }
                    }
                })
            });
        });
        app.ui_action_queue = ui_action_queue;
    }
}
