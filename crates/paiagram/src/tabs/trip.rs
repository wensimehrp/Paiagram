use egui::{
    Align2, AtomExt, Button, Color32, FontFamily, FontId, Popup, RectAlign, RichText, Ui, Vec2,
    WidgetText, vec2,
};
use egui_i18n::tr;
use paiagram_core::time::TimetableTime;
use paiagram_core::trip::TravelMode::{At, Flexible, For};
use paiagram_core::trip::{TEntry, TEstimate, TravelMode, TripSchedule};
use paiagram_core::{Command, Source, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::widgets::{DurationDragValue, TimeDragValue};
use crate::{App, UiCommand};

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
                schedule.estimates(&app.source.intervals, |estimates| {
                    for (estimate, entry) in estimates.into_iter().copied() {
                        row_ui(
                            tab.trip,
                            &schedule,
                            estimates,
                            estimate,
                            entry,
                            &app.source,
                            &mut app.ui_action_queue,
                            &mut app.command_queue,
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
    estimates: &[(Option<TEstimate>, TEntry)],
    estimate: Option<TEstimate>,
    entry: TEntry,
    source: &Source,
    ui_queue: &mut Vec<UiCommand>,
    cmd_queue: &mut Vec<Command>,
    ui: &mut Ui,
) {
    const BTN_SIZE: Vec2 = vec2(70.0, 18.0);
    let Some(station) = source.nodes.query(entry.node_key(), |view| *view.parent) else {
        ui.label("No station");
        return;
    };
    source.stations.query(station, |view| {
        let mut text = RichText::new(view.name.as_str());
        if matches!(entry, TEntry::Derived { .. }) {
            text = text.weak();
        }
        if ui.button(text.atom_align(Align2::LEFT_CENTER)).clicked() {
            // ui_queue.push(OpenOrFocus(MainTab::Trip(())));
        };
    });
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
            TEntry::Derived { .. } => (
                ui.add_sized(wide_size, Button::new(fmt_str(|e| e.arr, "||"))),
                None,
            ),
            TEntry::Pinned { arr, dep, .. } => (
                match arr {
                    For(mut d) => ui.add_sized(BTN_SIZE, DurationDragValue(&mut d)),
                    At(t) => ui.add_sized(BTN_SIZE, TimeDragValue(t, &mut arr_pass_dur)),
                    Flexible => ui.add_sized(BTN_SIZE, Button::new(fmt_str(|e| e.arr, "--:--:--"))),
                },
                Some(match dep {
                    For(mut d) => ui.add_sized(BTN_SIZE, DurationDragValue(&mut d)),
                    At(t) => ui.add_sized(BTN_SIZE, TimeDragValue(t, &mut dep_dur)),
                    Flexible => ui.add_sized(BTN_SIZE, Button::new(fmt_str(|e| e.dep, "--:--:--"))),
                }),
            ),
            TEntry::PinnedNonStop { pass, .. } => (
                match pass {
                    For(mut d) => ui.add_sized(wide_size, DurationDragValue(&mut d)),
                    At(t) => ui.add_sized(wide_size, TimeDragValue(t, &mut arr_pass_dur)),
                    Flexible => ui.add_sized(wide_size, Button::new(fmt_str(|e| e.arr, "||"))),
                },
                None,
            ),
        })
        .inner;

    if let Some(dur) = arr_pass_dur {
        cmd_queue.push(Command::ShiftTripEntryArrOrPass {
            key: trip_key,
            id: entry.id(),
            dur,
        });
    }

    if let Some(dur) = dep_dur {
        cmd_queue.push(Command::ShiftTripEntryDep {
            key: trip_key,
            id: entry.id(),
            dur,
        });
    }

    let res1_align = if matches!(entry, TEntry::Pinned { .. }) {
        RectAlign::LEFT
    } else {
        RectAlign::RIGHT
    };

    Popup::menu(&res1).align(res1_align).show(|ui| {
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
        let t = estimate.map(|e| e.arr).unwrap_or_default();
        let mut d = schedule.arr_to_dur(estimates, entry.id()).unwrap_or_default();
        if ui.add(TimeDragValue(t, &mut None)).clicked() {
            // do something
        };
        if ui.add(DurationDragValue(&mut d)).clicked() {
            // do something
        };
        if ui.button("Flexible").clicked() {
            // do something
        };
    });
    let Some(res2) = res2 else {
        return;
    };
    Popup::menu(&res2).align(RectAlign::RIGHT).show(|ui| {
        let t = estimate.map(|e| e.arr).unwrap_or_default();
        let mut d = estimate.map(|e| e.duration()).unwrap_or_default();
        if ui.add(TimeDragValue(t, &mut None)).clicked() {
            // do something
        };
        if ui.add(DurationDragValue(&mut d)).clicked() {
            // do something
        };
        if ui.button("Flexible").clicked() {
            // do something
        };
    });
}
