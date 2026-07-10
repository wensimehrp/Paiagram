use egui::{RectAlign, Response, Ui, Vec2, vec2};
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, TimetableTime};
use paiagram_core::{Command, TripKey};

use super::{DurationDragValue, TimeDragValue};

pub(crate) const POPUP_WIDTH: f32 = 130.0;
pub(crate) const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);

/// Helper: modify a single entry in a trip's schedule.
pub(crate) fn modify_entry(app: &mut super::super::tabs::AppState, trip_key: TripKey, entry_idx: usize, new_entry: TEntry) {
    if let Some(view) = app.source.trips.get_view(trip_key) {
        let mut entries: Vec<TEntry> = view.schedule.entries().to_vec();
        if entry_idx < entries.len() {
            entries[entry_idx] = new_entry;
            app.source.apply_command(Command::ChangeTripEntries {
                key: trip_key,
                entries: entries.into(),
            });
        }
    }
}

/// Extract arrival mode from a TEntry
pub(crate) fn get_arrival(entry: &TEntry) -> TravelMode {
    match entry {
        TEntry::Pinned { arr, .. } => *arr,
        _ => TravelMode::Flexible,
    }
}

/// Extract departure mode from a TEntry
pub(crate) fn get_departure(entry: &TEntry) -> TravelMode {
    match entry {
        TEntry::Pinned { dep, .. } => *dep,
        _ => TravelMode::Flexible,
    }
}

/// Show a drag-value for an "At" time, committing changes via `modify_entry`.
pub(crate) fn shift_at_value(
    t: TimetableTime,
    trip_key: TripKey,
    entry_idx: usize,
    app: &mut super::super::tabs::AppState,
    ui: &mut Ui,
    button_size: Vec2,
    is_arrival: bool,
) -> Response {
    let mut new_t = t;
    let res = ui.add_sized(button_size, TimeDragValue(&mut new_t));
    if res.changed() && new_t != t {
        if let Some(view) = app.source.trips.get_view(trip_key) {
            if let Some(entry) = view.schedule.entries().get(entry_idx) {
                if let TEntry::Pinned { stn, trk, arr, dep, id, .. } = entry {
                    let new_arr = if is_arrival { TravelMode::At(new_t) } else { *arr };
                    let new_dep = if is_arrival { *dep } else { TravelMode::At(new_t) };
                    modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                        stn: *stn, trk: *trk, arr: new_arr, dep: new_dep, id: *id,
                    });
                }
            }
        }
    }
    res
}

/// Show a drag-value for a "For" duration, committing changes via `modify_entry`.
pub(crate) fn shift_for_value(
    d: Duration,
    trip_key: TripKey,
    entry_idx: usize,
    app: &mut super::super::tabs::AppState,
    ui: &mut Ui,
    button_size: Vec2,
    is_arrival: bool,
) -> Response {
    let mut new_d = d;
    let res = ui.add_sized(button_size, DurationDragValue(&mut new_d));
    if res.changed() && new_d != d {
        if let Some(view) = app.source.trips.get_view(trip_key) {
            if let Some(entry) = view.schedule.entries().get(entry_idx) {
                if let TEntry::Pinned { stn, trk, arr, dep, id, .. } = entry {
                    let new_arr = if is_arrival { TravelMode::For(new_d) } else { *arr };
                    let new_dep = if is_arrival { *dep } else { TravelMode::For(new_d) };
                    modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                        stn: *stn, trk: *trk, arr: new_arr, dep: new_dep, id: *id,
                    });
                }
            }
        }
    }
    res
}

/// Internal helper for the arrival popup body.
pub(crate) fn arrival_popup_inner(
    ui: &mut Ui,
    entry_idx: usize,
    trip_key: TripKey,
    app: &mut super::super::tabs::AppState,
) {
    ui.set_width(POPUP_WIDTH);
    let Some(entry) = app.source.trips.get_view(trip_key)
        .and_then(|v| v.schedule.entries().get(entry_idx).copied())
    else { return; };

    match entry {
        TEntry::Pinned { arr, dep, stn, trk, id } => {
            let at_time = match arr {
                TravelMode::At(t) => t,
                _ => TimetableTime(0),
            };
            let for_dur = match arr {
                TravelMode::For(d) => d,
                _ => Duration(0),
            };
            let is_at = matches!(arr, TravelMode::At(_));
            let is_for = matches!(arr, TravelMode::For(_));
            let is_flex = matches!(arr, TravelMode::Flexible);

            // At
            if is_at {
                shift_at_value(at_time, trip_key, entry_idx, app, ui, BUTTON_SIZE, true);
            } else if ui.add_enabled(true, egui::Button::new("At").right_text(at_time.to_string())).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr: TravelMode::At(at_time), dep, id,
                });
            }
            // For
            if is_for {
                shift_for_value(for_dur, trip_key, entry_idx, app, ui, BUTTON_SIZE, true);
            } else if ui.add_enabled(true, egui::Button::new("For").right_text(for_dur.to_string())).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr: TravelMode::For(for_dur), dep, id,
                });
            }
            // Flexible
            if !is_flex && ui.add_enabled(true, egui::Button::new("Flexible")).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr: TravelMode::Flexible, dep, id,
                });
            }
            if is_flex {
                ui.label("Flexible");
            }
            // Non-stop: convert Pinned -> PinnedNonStop
            if ui.add_enabled(true, egui::Button::new("Non-stop").right_text("↓")).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::PinnedNonStop {
                    stn, trk, pass: dep, id,
                });
            }
        }
        TEntry::PinnedNonStop { stn, trk, pass, id } => {
            // Currently non-stop. Show options to convert back to Pinned.
            let is_at = matches!(pass, TravelMode::At(_));
            let is_for = matches!(pass, TravelMode::For(_));
            let is_flex = matches!(pass, TravelMode::Flexible);

            ui.label("Non-stop");
            if is_at {
                if let TravelMode::At(t) = pass {
                    shift_at_value(t, trip_key, entry_idx, app, ui, BUTTON_SIZE, true);
                }
            }
            // Convert back to Pinned with the same pass time as both arr and dep
            if ui.button("Make Stop").clicked() {
                let new_dep = match pass {
                    TravelMode::At(t) => TravelMode::At(t),
                    TravelMode::For(d) => TravelMode::For(d),
                    TravelMode::Flexible => TravelMode::Flexible,
                };
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr: pass, dep: new_dep, id,
                });
            }
        }
        _ => {}
    }
}

/// Internal helper for the departure popup body.
pub(crate) fn departure_popup_inner(
    ui: &mut Ui,
    entry_idx: usize,
    trip_key: TripKey,
    app: &mut super::super::tabs::AppState,
) {
    ui.set_width(POPUP_WIDTH);
    let Some(entry) = app.source.trips.get_view(trip_key)
        .and_then(|v| v.schedule.entries().get(entry_idx).copied())
    else { return; };

    match entry {
        TEntry::Pinned { arr, dep, stn, trk, id } => {
            let at_time = match dep {
                TravelMode::At(t) => t,
                _ => TimetableTime(0),
            };
            let for_dur = match dep {
                TravelMode::For(d) => d,
                _ => Duration(0),
            };
            let is_at = matches!(dep, TravelMode::At(_));
            let is_for = matches!(dep, TravelMode::For(_));
            let is_flex = matches!(dep, TravelMode::Flexible);

            if is_at {
                shift_at_value(at_time, trip_key, entry_idx, app, ui, BUTTON_SIZE, false);
            } else if ui.add_enabled(true, egui::Button::new("At").right_text(at_time.to_string())).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr, dep: TravelMode::At(at_time), id,
                });
            }
            if is_for {
                shift_for_value(for_dur, trip_key, entry_idx, app, ui, BUTTON_SIZE, false);
            } else if ui.add_enabled(true, egui::Button::new("For").right_text(for_dur.to_string())).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr, dep: TravelMode::For(for_dur), id,
                });
            }
            if !is_flex && ui.add_enabled(true, egui::Button::new("Flexible")).clicked() {
                modify_entry(app, trip_key, entry_idx, TEntry::Pinned {
                    stn, trk, arr, dep: TravelMode::Flexible, id,
                });
            }
            if is_flex {
                ui.label("Flexible");
            }
        }
        _ => {}
    }
}
