use egui::{RectAlign, Response, Ui, Vec2, vec2};
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, TimetableTime};
use paiagram_core::TripKey;

use super::{DurationDragValue, TimeDragValue};
use crate::App;

pub(crate) const POPUP_WIDTH: f32 = 130.0;
pub(crate) const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);

pub(crate) fn departure_popup(
    app: &mut App,
    response: &Response,
    trip_key: TripKey,
    entry_idx: usize,
    alignment: RectAlign,
) {
    egui::Popup::menu(&response)
        .align(alignment)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| departure_popup_ui(app, ui, trip_key, entry_idx));
}

fn departure_popup_ui(app: &mut App, ui: &mut Ui, trip_key: TripKey, entry_idx: usize) {
    ui.set_width(POPUP_WIDTH);

    let entries = get_trip_entries(app, trip_key);
    let Some(entry) = entries.get(entry_idx).copied() else {
        return;
    };

    let TEntry::Pinned { dep: old_dep, .. } = entry else {
        return;
    };

    let mut new_mode = old_dep;
    let changed = departure_mode_ui(ui, &old_dep, &mut new_mode);

    if changed {
        apply_entry_change(app, trip_key, entry_idx, |e| {
            if let TEntry::Pinned { dep, .. } = e {
                *dep = new_mode;
            }
        });
    }
}

pub(crate) fn arrival_popup(
    app: &mut App,
    response: &Response,
    trip_key: TripKey,
    entry_idx: usize,
    alignment: RectAlign,
) {
    egui::Popup::menu(&response)
        .align(alignment)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| arrival_popup_ui(app, ui, trip_key, entry_idx));
}

fn arrival_popup_ui(app: &mut App, ui: &mut Ui, trip_key: TripKey, entry_idx: usize) {
    ui.set_width(POPUP_WIDTH);

    let entries = get_trip_entries(app, trip_key);
    let Some(entry) = entries.get(entry_idx).copied() else {
        return;
    };

    let TEntry::Pinned { arr: old_arr, .. } = entry else {
        return;
    };

    let mut new_mode = old_arr;
    let changed = arrival_mode_ui(ui, &old_arr, &mut new_mode);

    if changed {
        apply_entry_change(app, trip_key, entry_idx, |e| {
            if let TEntry::Pinned { arr, .. } = e {
                *arr = new_mode;
            }
        });
    }
}

fn departure_mode_ui(ui: &mut Ui, current: &TravelMode, new_mode: &mut TravelMode) -> bool {
    let at_time = extract_time(current);
    let for_dur = extract_duration(current);
    let mut changed = false;

    // "At" mode
    if !matches!(current, TravelMode::At(_)) {
        if ui
            .add(egui::Button::new("At").right_text(at_time.to_string()))
            .clicked()
        {
            *new_mode = TravelMode::At(at_time);
            changed = true;
        }
    } else {
        let mut t = at_time;
        let res = ui.add_sized(BUTTON_SIZE, TimeDragValue(&mut t));
        if res.changed() {
            *new_mode = TravelMode::At(t);
            changed = true;
        }
    }

    // "For" mode
    if !matches!(current, TravelMode::For(_)) {
        if ui
            .add(egui::Button::new("For").right_text(for_dur.to_string()))
            .clicked()
        {
            *new_mode = TravelMode::For(for_dur);
            changed = true;
        }
    } else {
        let mut d = for_dur;
        let res = ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d));
        if res.changed() {
            *new_mode = TravelMode::For(d);
            changed = true;
        }
    }

    // Flexible mode
    if !matches!(current, TravelMode::Flexible) {
        if ui
            .add_enabled(true, egui::Button::new("Flexible").right_text("〇"))
            .clicked()
        {
            *new_mode = TravelMode::Flexible;
            changed = true;
        }
    }

    changed
}

/// Build the UI for editing an arrival mode.
/// Returns true if the mode was changed.
fn arrival_mode_ui(ui: &mut Ui, current: &TravelMode, new_mode: &mut TravelMode) -> bool {
    let at_time = extract_time(current);
    let for_dur = extract_duration(current);
    let mut changed = false;

    // "At" mode
    if !matches!(current, TravelMode::At(_)) {
        if ui
            .add(egui::Button::new("At").right_text(at_time.to_string()))
            .clicked()
        {
            *new_mode = TravelMode::At(at_time);
            changed = true;
        }
    } else {
        let mut t = at_time;
        let res = ui.add_sized(BUTTON_SIZE, TimeDragValue(&mut t));
        if res.changed() {
            *new_mode = TravelMode::At(t);
            changed = true;
        }
    }

    // "For" mode
    if !matches!(current, TravelMode::For(_)) {
        if ui
            .add(egui::Button::new("For").right_text(for_dur.to_string()))
            .clicked()
        {
            *new_mode = TravelMode::For(for_dur);
            changed = true;
        }
    } else {
        let mut d = for_dur;
        let res = ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d));
        if res.changed() {
            *new_mode = TravelMode::For(d);
            changed = true;
        }
    }

    // flexible
    if !matches!(current, TravelMode::Flexible) {
        if ui
            .add_enabled(true, egui::Button::new("Flexible").right_text("〇"))
            .clicked()
        {
            *new_mode = TravelMode::Flexible;
            changed = true;
        }
    }

    changed
}

/// Get the time from a TravelMode (defaults to 0 if not applicable).
fn extract_time(mode: &TravelMode) -> TimetableTime {
    match mode {
        TravelMode::At(t) => *t,
        TravelMode::For(_) => TimetableTime(0),
        TravelMode::Flexible => TimetableTime(0),
    }
}

/// Get the duration from a TravelMode (defaults to 0 if not applicable).
fn extract_duration(mode: &TravelMode) -> Duration {
    match mode {
        TravelMode::For(d) => *d,
        TravelMode::At(_) => Duration(0),
        TravelMode::Flexible => Duration(0),
    }
}

/// Get the entries for a trip.
fn get_trip_entries(app: &App, trip_key: TripKey) -> Vec<TEntry> {
    app.trips
        .get_handle(trip_key)
        .map(|handle| app.trips.get_entries(handle).to_vec())
        .unwrap_or_default()
}

/// Apply a change to a specific entry in a trip via the command system.
fn apply_entry_change(
    app: &mut App,
    trip_key: TripKey,
    entry_idx: usize,
    modify: impl FnOnce(&mut TEntry),
) {
    let Some(handle) = app.trips.get_handle(trip_key) else {
        return;
    };
    let mut entries: Vec<TEntry> = app.trips.get_entries(handle).to_vec();
    if let Some(entry) = entries.get_mut(entry_idx) {
        modify(entry);
        app.source.apply_command(paiagram_core::Command::ChangeTripEntries {
            key: trip_key,
            entries: entries.into(),
        });
    }
}
