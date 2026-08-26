use egui::{Button, Color32, Popup, RectAlign, RichText, Sense, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::time::TimetableTime;
use paiagram_core::trip::TravelMode::Flexible;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::{Command, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
use crate::widgets::{DurationDragValue, TimeDragValue};
// use crate::widgets::timetable_popup::{
//     arrival_popup, departure_popup, shift_at_value, shift_for_value,
// };

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TripTab {
    trip: TripKey,
    show_derived: bool,
    name_edit_buf: Option<String>,
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn title(&self) -> WidgetText {
        tr!("tab-trip").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        show_trip(self, app, ui);
    }
}

impl TripTab {
    pub(crate) fn new(trip: TripKey) -> Self {
        Self {
            trip,
            show_derived: false,
            name_edit_buf: None,
        }
    }
}

fn show_trip(tab: &mut TripTab, app: &mut App, ui: &mut Ui) {
    let Some((name, schedule)) =
        app.trips.query(tab.trip, |view| (view.name.clone(), view.schedule.clone()))
    else {
        return;
    };
    egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(buf) = tab.name_edit_buf.as_mut() {
                let res = ui.text_edit_singleline(buf);
                res.request_focus();
                if res.lost_focus() {
                    app.command_queue.push(Command::RenameTrip {
                        key: tab.trip,
                        name: buf.as_str().into(),
                    });
                    tab.name_edit_buf = None;
                }
            } else if ui.button(RichText::new(name.as_str()).size(24.0)).clicked() {
                tab.name_edit_buf = Some(String::from(name));
            }
        })
    });
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
            egui::Grid::new(ui.id().with("trip ui")).num_columns(2).striped(true).show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.end_row();
                // Remove button background
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                for entry in schedule
                    .entries()
                    .iter()
                    .filter(|e| !matches!(e, TEntry::Derived { .. }) || tab.show_derived)
                {
                    row_ui(*entry, app, ui);
                    ui.end_row();
                }
            });
        });
    });
}

fn row_ui(entry: TEntry, app: &mut App, ui: &mut Ui) {
    const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);
    let Some(station) = app.nodes.query(entry.node_key(), |view| *view.parent) else {
        ui.label("No station");
        return;
    };
    app.stations.query(station, |view| {
        ui.label(view.name.as_str());
    });
    let mut wide_size = BUTTON_SIZE;
    wide_size.x *= 2.0;
    wide_size.x += ui.spacing().item_spacing.x;
    let mut dd1 = None;
    let mut dd2 = None;
    let (res1, res2) = ui
        .horizontal(|ui| match entry {
            TEntry::Derived { .. } => (
                None,
                ui.add_sized(wide_size, Button::new(RichText::new("(↓)").weak())),
            ),
            TEntry::Pinned { arr, dep, .. } => (
                Some(match arr {
                    TravelMode::For(mut d) => ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d)),
                    TravelMode::At(t) => ui.add_sized(BUTTON_SIZE, TimeDragValue(t, &mut dd1)),
                    Flexible => ui.add_sized(BUTTON_SIZE, Button::new("〇")),
                }),
                match dep {
                    TravelMode::For(mut d) => ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d)),
                    TravelMode::At(t) => ui.add_sized(BUTTON_SIZE, TimeDragValue(t, &mut dd2)),
                    Flexible => ui.add_sized(BUTTON_SIZE, Button::new("〇")),
                },
            ),
            TEntry::PinnedNonStop { pass, .. } => (
                None,
                match pass {
                    TravelMode::For(mut d) => ui.add_sized(wide_size, DurationDragValue(&mut d)),
                    TravelMode::At(t) => ui.add_sized(wide_size, TimeDragValue(t, &mut dd2)),
                    Flexible => ui.add_sized(wide_size, Button::new("↓")),
                },
            ),
        })
        .inner;

    Popup::menu(&res2).align(RectAlign::RIGHT).show(|ui| {
        // display departure stuff and change mode
        if ui
            .button(match entry {
                TEntry::Derived { .. } => "Pin",
                TEntry::Pinned { .. } => "Make Non-stop",
                TEntry::PinnedNonStop { .. } => "Make stop",
            })
            .clicked()
        {
            // do something
        }
        match match entry {
            TEntry::Derived { .. } => return,
            TEntry::Pinned { dep, .. } => dep,
            TEntry::PinnedNonStop { pass, .. } => pass,
        } {
            TravelMode::At(t) => {
                ui.add(TimeDragValue(t, &mut None));
            }
            TravelMode::For(mut d) => {
                ui.add(DurationDragValue(&mut d));
            }
            TravelMode::Flexible => {
                ui.button("123");
            }
        }
    });
    let Some(res1) = res1 else {
        return;
    };
    Popup::menu(&res1).align(RectAlign::LEFT).show(|ui| {
        // only arrival
    });
}
