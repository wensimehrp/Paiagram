use egui::*;
use egui_i18n::tr;
use paiagram_core::time::TimetableTime;
use paiagram_core::trip::TravelMode::{self, At, Flexible, For};
use paiagram_core::trip::{EstimateEntry, TEstimate, TripSchedule};
use paiagram_core::{TripKey, WorldSnapshot};
use paiagram_selection::SelectedItems;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::widgets::{DurationDragValue, TimeDragValue};
use crate::{App, UiCommand};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct TripTab {
    trip_key: Option<TripKey>,
    show_derived: bool,
    name_edit_buf: Option<String>,
}

impl PartialEq for TripTab {
    fn eq(&self, other: &Self) -> bool {
        self.trip_key == other.trip_key
    }
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn title(&self) -> WidgetText {
        tr!("tab-trip").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        puffin::profile_function!();
        show_trip(self, app, ui);
    }
}

impl TripTab {
    pub(crate) fn new(trip_key: TripKey) -> Self {
        Self {
            trip_key: Some(trip_key),
            show_derived: false,
            name_edit_buf: None,
        }
    }
}

fn show_trip(tab: &mut TripTab, app: &mut App, ui: &mut Ui) {
    puffin::profile_function!();
    let snap = &app.source.snap;
    let trip_key = tab.trip_key.or(match &app.selected_items {
        SelectedItems::Trips(trips) => Some(*trips.first()),
        _ => None,
    });
    let select_a_trip = |ui: &mut Ui, existing_key: &mut Option<TripKey>| {
        let selected_text = existing_key.map_or("Current selected trip", |key| {
            snap.trips.get(&key).map_or("Unknown", |wfc| &wfc.name)
        });
        let combo_box =
            ComboBox::from_id_salt("Selected existing trip").selected_text(selected_text);
        combo_box.show_ui(ui, |ui| {
            ui.selectable_value(existing_key, None, "Current selected trip");
            ScrollArea::vertical().show_rows(
                ui,
                ui.spacing().interact_size.y,
                app.trips.len(),
                |ui, range| {
                    for (key, trip) in app.trips.iter().skip(range.start).take(range.count()) {
                        ui.selectable_value(existing_key, Some(*key), trip.name.as_str());
                    }
                },
            );
        });
    };
    let Some(trip_key) = trip_key else {
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            ui.heading("No trips selected");
            select_a_trip(ui, &mut tab.trip_key);
        });
        return;
    };
    let Some(trip) = snap.trips.get(&trip_key) else {
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            ui.heading("Selected trip doesn't exist");
            select_a_trip(ui, &mut tab.trip_key);
        });
        return;
    };
    Frame::new().inner_margin(6.0).show(ui, |ui| {
        ui.heading(trip.name.as_str());
        select_a_trip(ui, &mut tab.trip_key);
    });
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            Grid::new("trip ui").num_columns(2).striped(true).show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.end_row();
                // Remove button background
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                trip.schedule.estimates(&snap.graph, |estimates| {
                    for (estimate, entry) in estimates.into_iter().copied() {
                        row_ui(
                            trip_key,
                            &trip.schedule,
                            estimates,
                            estimate,
                            entry,
                            snap,
                            ui,
                        );
                        ui.end_row();
                    }
                });
            });
        });
    });
}

fn row_ui(
    trip_key: TripKey,
    schedule: &TripSchedule,
    estimates: &[(Option<TEstimate>, EstimateEntry)],
    estimate: Option<TEstimate>,
    entry: EstimateEntry,
    snap: &WorldSnapshot,
    ui: &mut Ui,
) {
    puffin::profile_function!();
    const BTN_SIZE: Vec2 = vec2(70.0, 18.0);
    let Some(node) = snap.graph.nodes().get(&entry.node_key()) else {
        ui.label("No station");
        return;
    };
    if let Some(stn) = snap.stations.get(&node.parent) {
        let mut text = RichText::new(stn.name.as_str());
        if matches!(entry, EstimateEntry::Derived(..)) {
            text = text.weak();
        }
        if ui.button(text).clicked() {
            // TODO: push station
        };
    } else {
        ui.colored_label(Color32::RED, "Invalid Station");
    };
    let mut wide_size = BTN_SIZE;
    wide_size.x *= 2.0;
    wide_size.x += ui.spacing().item_spacing.x;
    let mut arr_pass_dur = None;
    let mut dep_dur = None;
    let fmt_str = |f: fn(TEstimate) -> TimetableTime, placeholder: &str| -> RichText {
        RichText::new(if let Some(e) = estimate {
            f(e).to_string()
        } else {
            placeholder.to_string()
        })
        .weak()
        .font(FontId::new(13.0, FontFamily::Name("timetable font".into())))
    };
    let (res1, res2) = ui
        .horizontal(|ui| match entry {
            EstimateEntry::Derived(..) => (
                ui.add_sized(wide_size, Button::new(fmt_str(|e| e.arr, "||"))),
                None,
            ),
            EstimateEntry::Pinned(entry) => (
                match entry.arr_or_pass {
                    For(d) => ui.add_sized(BTN_SIZE, DurationDragValue(d, &mut arr_pass_dur)),
                    At(t) => ui.add_sized(BTN_SIZE, TimeDragValue(t, &mut arr_pass_dur)),
                    Flexible => ui.add_sized(BTN_SIZE, Button::new(fmt_str(|e| e.arr, "--:--:--"))),
                },
                entry.dep.map(|dep| match dep {
                    For(d) => ui.add_sized(BTN_SIZE, DurationDragValue(d, &mut dep_dur)),
                    At(t) => ui.add_sized(BTN_SIZE, TimeDragValue(t, &mut dep_dur)),
                    Flexible => ui.add_sized(BTN_SIZE, Button::new(fmt_str(|e| e.dep, "--:--:--"))),
                }),
            ),
        })
        .inner;

    // if let Some(dur) = arr_pass_dur {
    //     cmd_queue.push(Command::TripEntryShiftArrOrPass {
    //         key: trip_key,
    //         id: entry.id(),
    //         dur,
    //     });
    // }

    // if let Some(dur) = dep_dur {
    //     cmd_queue.push(Command::TripEntryShiftDep {
    //         key: trip_key,
    //         id: entry.id(),
    //         dur,
    //     });
    // }

    let res1_align = if matches!(entry, EstimateEntry::Pinned(entry) if entry.dep.is_none()) {
        RectAlign::LEFT
    } else {
        RectAlign::RIGHT
    };

    Popup::menu(&res1).align(res1_align).show(|ui| {
        let EstimateEntry::Pinned(entry) = entry else {
            ui.button("Insert");
            return;
        };
        // display departure stuff and change mode
        let t = estimate.map(|e| e.arr).unwrap_or_default();
        let d = schedule.arr_to_dur(estimates, entry.id).unwrap_or_default();
        let mut new_mode = None;
        if ui.add(TimeDragValue(t, &mut None)).clicked() {
            new_mode = Some(TravelMode::At(t));
        };
        if ui.add(DurationDragValue(d, &mut None)).clicked() {
            new_mode = Some(TravelMode::For(d));
        };
        if ui.button("Flexible").clicked() {
            new_mode = Some(TravelMode::Flexible);
        };
        let mut new_entry = entry;
        if let Some(mode) = new_mode {
            new_entry.arr_or_pass = mode;
            // cmd_queue.push(Command::TripEntryChange {
            //     key: trip_key,
            //     id: new_entry.id(),
            //     new_entry,
            // });
        }
    });
    let Some(res2) = res2 else {
        return;
    };
    Popup::menu(&res2).align(RectAlign::RIGHT).show(|ui| {
        let EstimateEntry::Pinned(entry) = entry else {
            return;
        };
        let t = estimate.map(|e| e.arr).unwrap_or_default();
        let d = estimate.map(|e| e.duration()).unwrap_or_default();
        let mut new_mode = None;
        if ui.add(TimeDragValue(t, &mut None)).clicked() {
            new_mode = Some(Some(TravelMode::At(t)));
        };
        if ui.add(DurationDragValue(d, &mut None)).clicked() {
            new_mode = Some(Some(TravelMode::For(d)));
        };
        if ui.button("Flexible").clicked() {
            new_mode = Some(Some(TravelMode::Flexible));
        };
        let mut new_entry = entry;
        if let Some(mode) = new_mode {
            new_entry.dep = mode;
            // cmd_queue.push(Command::TripEntryChange {
            //     key: trip_key,
            //     id: new_entry.id(),
            //     new_entry,
            // });
        }
    });
}
