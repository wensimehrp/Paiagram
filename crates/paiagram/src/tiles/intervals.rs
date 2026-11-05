use egui::*;
use paiagram_core::{IntervalDirection, IntervalKey};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::tiles::trip::TripTile;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct IntervalsTile {
    focused: Option<IntervalKey>,
}

impl PartialEq for IntervalsTile {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl super::Tile for IntervalsTile {
    const NAME: &'static str = "Intervals";
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        puffin::profile_function!();
        Panel::left("interval left panel").show(ui, |ui| {
            puffin::profile_scope!("interval_list");
            let spacing = ui.spacing().interact_size.y;
            let total_rows = app.graph.intervals().len();
            let scroll_area = ScrollArea::vertical().auto_shrink(false);
            scroll_area.show_rows(ui, spacing, total_rows, |ui, row_range| {
                ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                    for (&interval_key, interval) in
                        app.graph.intervals().iter().skip(row_range.start).take(row_range.count())
                    {
                        let IntervalKey { hi, lo } = interval_key;
                        let hi_text =
                            app.graph.nodes().get(&hi).map_or("Unknown", |wfc| wfc.name.as_str());
                        let lo_text =
                            app.graph.nodes().get(&lo).map_or("Unknown", |wfc| wfc.name.as_str());
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
        });
        CentralPanel::default_margins().show(ui, |ui| {
            puffin::profile_scope!("interval_details");
            let Some(interval) = self.focused else {
                ui.vertical_centered_justified(|ui| ui.heading("Nothing focused"));
                return;
            };
            let Some(interval) = app.graph.intervals().get(&interval) else {
                ui.vertical_centered_justified(|ui| ui.heading("Interval does not exist"));
                return;
            };
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for trip_key in &interval.cache.trips {
                    let Some(trip) = app.trips.get(trip_key) else {
                        continue;
                    };
                    if ui.button(trip.name.as_str()).clicked() {
                        app.open_or_focus(TripTile::new(*trip_key));
                    }
                }
            });
        });
    }
}
