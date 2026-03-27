use bevy::prelude::*;
use egui::emath::Numeric;
use egui::{Response, Ui, Vec2, vec2};
use paiagram_core::{
    entry::{
        AdjustEntryMode, EntryEstimate, EntryMode, EntryModeAdjustment, EntryQueryItem, TravelMode,
    },
    trip::TripQueryItem,
    units::time::{Duration, TimetableTime},
};

const POPUP_WIDTH: f32 = 130.0;
const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);

pub fn departure_popup(response: &Response, entry: &EntryQueryItem, commands: &mut Commands) {
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| departure_popup_inner(ui, entry, commands));
}

pub fn shift_at_value(
    t: TimetableTime,
    trip_entity: Entity,
    ui: &mut Ui,
    commands: &mut Commands,
    button_size: Vec2,
    is_arrival: bool,
) -> Response {
    let mut new_t = t;
    let res = ui.add_sized(
        button_size,
        egui::DragValue::new(&mut new_t)
            .custom_formatter(|v, _| TimetableTime::from_f64(v).to_string())
            .custom_parser(|s| TimetableTime::from_str(s).map(TimetableTime::to_f64)),
    );
    if res.changed() {
        commands.trigger(AdjustEntryMode {
            entity: trip_entity,
            adj: if is_arrival {
                EntryModeAdjustment::ShiftArrival(new_t - t)
            } else {
                EntryModeAdjustment::ShiftDeparture(new_t - t)
            },
        });
    }
    res
}

pub fn shift_for_value(
    d: Duration,
    trip_entity: Entity,
    ui: &mut Ui,
    commands: &mut Commands,
    button_size: Vec2,
    is_arrival: bool,
) -> Response {
    let mut new_d = d;
    let res = ui.add_sized(
        button_size,
        egui::DragValue::new(&mut new_d)
            .prefix("→ ")
            .custom_formatter(|v, _| Duration::from_f64(v).to_string_no_arrow())
            .custom_parser(|s| Duration::from_str(s).map(Duration::to_f64)),
    );
    if res.changed() {
        commands.trigger(AdjustEntryMode {
            entity: trip_entity,
            adj: if is_arrival {
                EntryModeAdjustment::ShiftArrival(new_d - d)
            } else {
                EntryModeAdjustment::ShiftDeparture(new_d - d)
            },
        });
    }
    res
}

pub fn departure_popup_inner(ui: &mut Ui, entry: &EntryQueryItem, commands: &mut Commands) {
    ui.set_width(POPUP_WIDTH);
    // at
    let mut adjustment = None;
    let at_time = entry.estimate.map(|e| e.dep).unwrap_or_default();
    let enable_at = !matches!(entry.mode.dep, TravelMode::At(_));
    if enable_at {
        if ui
            .add(egui::Button::new("At").right_text(at_time.to_string()))
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetDeparture(TravelMode::At(at_time)));
        };
    } else {
        shift_at_value(at_time, entry.entity, ui, commands, BUTTON_SIZE, false);
    }
    // for
    let for_dur = if let TravelMode::For(d) = entry.mode.dep {
        Some(d)
    } else if let Some(e) = entry.estimate {
        Some(e.dep - e.arr)
    } else {
        None
    }
    .unwrap_or_default();
    let enable_for = !matches!(entry.mode.dep, TravelMode::For(_));
    if enable_for {
        if ui
            .add(egui::Button::new("For").right_text(for_dur.to_string()))
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetDeparture(TravelMode::For(for_dur)));
            ui.close();
        }
    } else {
        shift_for_value(for_dur, entry.entity, ui, commands, BUTTON_SIZE, false);
    }
    // flexible
    if ui
        .add_enabled(
            !matches!(entry.mode.dep, TravelMode::Flexible),
            egui::Button::new("Flexible").right_text("〇"),
        )
        .clicked()
    {
        adjustment = Some(EntryModeAdjustment::SetDeparture(TravelMode::Flexible));
    }
    if let Some(adj) = adjustment {
        commands.trigger(AdjustEntryMode {
            entity: entry.entity,
            adj,
        });
    }
}

pub fn arrival_popup(
    response: &Response,
    entry: &EntryQueryItem,
    parent: &TripQueryItem,
    entry_q: &Query<(&EntryMode, Option<&EntryEstimate>)>,
    commands: &mut Commands,
) {
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| arrival_popup_inner(ui, entry, parent, entry_q, commands));
}

pub fn arrival_popup_inner(
    ui: &mut Ui,
    entry: &EntryQueryItem,
    parent: &TripQueryItem,
    entry_q: &Query<(&EntryMode, Option<&EntryEstimate>)>,
    commands: &mut Commands,
) {
    ui.set_width(POPUP_WIDTH);
    // at
    let at_time = entry.estimate.map(|e| e.arr).unwrap_or_default();
    let mut adjustment = None;
    let enable_at = !matches!(entry.mode.arr, Some(TravelMode::At(_)));
    if enable_at {
        if ui
            .add(egui::Button::new("At").right_text(at_time.to_string()))
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::At(
                at_time,
            ))));
        };
    } else {
        shift_at_value(at_time, entry.entity, ui, commands, BUTTON_SIZE, true);
    }
    // for
    let for_dur = if let Some(TravelMode::For(d)) = entry.mode.arr {
        Some(d)
    } else {
        entry.travel_duration(parent, entry_q)
    }
    .unwrap_or_default();
    let enable_for = !matches!(entry.mode.arr, Some(TravelMode::For(_)));
    if enable_for {
        if ui
            .add(egui::Button::new("For").right_text(for_dur.to_string()))
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::For(
                for_dur,
            ))));
        };
    } else {
        shift_for_value(for_dur, entry.entity, ui, commands, BUTTON_SIZE, true);
    }
    // flexible
    if ui
        .add_enabled(
            !matches!(entry.mode.arr, Some(TravelMode::Flexible)),
            egui::Button::new("Flexible").right_text("〇"),
        )
        .clicked()
    {
        adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::Flexible)));
    }
    // non-stop
    if ui
        .add_enabled(
            !matches!(entry.mode.arr, None),
            egui::Button::new("Non-stop").right_text("↓"),
        )
        .clicked()
    {
        adjustment = Some(EntryModeAdjustment::SetArrival(None));
    }
    if let Some(adj) = adjustment {
        commands.trigger(AdjustEntryMode {
            entity: entry.entity,
            adj,
        });
    }
}
