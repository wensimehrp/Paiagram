use bevy::prelude::*;
use egui::Response;
use paiagram_core::{
    entry::{
        AdjustEntryMode, EntryEstimate, EntryMode, EntryModeAdjustment, EntryQueryItem, TravelMode,
    },
    trip::TripQueryItem,
};

pub fn departure_popup(response: &Response, entry: &EntryQueryItem, commands: &mut Commands) {
    egui::Popup::menu(&response).show(|ui| {
        // at
        let at_time = entry.estimate.map(|e| e.dep);
        let mut adjustment = None;
        if ui
            .add_enabled(
                !matches!(entry.mode.dep, TravelMode::At(_)),
                egui::Button::new("At")
                    .right_text(at_time.map_or("--:--:--".to_string(), |e| e.to_string())),
            )
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetDeparture(TravelMode::At(
                at_time.unwrap_or_default(),
            )));
        };
        // for
        let for_dur = if let TravelMode::For(d) = entry.mode.dep {
            Some(d)
        } else if let Some(e) = entry.estimate {
            Some(e.dep - e.arr)
        } else {
            None
        };
        if ui
            .add_enabled(
                !matches!(entry.mode.dep, TravelMode::For(_)),
                egui::Button::new("For")
                    .right_text(for_dur.map_or("--:--:--".to_string(), |d| d.to_string())),
            )
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetDeparture(TravelMode::For(
                for_dur.unwrap_or_default(),
            )));
        };
        // flexible
        if ui
            .add_enabled(
                !matches!(entry.mode.dep, TravelMode::Flexible),
                egui::Button::new("Flexible"),
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
    });
}

pub fn arrival_popup(
    response: &Response,
    entry: &EntryQueryItem,
    parent: &TripQueryItem,
    entry_q: &Query<(&EntryMode, Option<&EntryEstimate>)>,
    commands: &mut Commands,
) {
    egui::Popup::menu(&response).show(|ui| {
        // at
        let at_time = entry.estimate.map(|e| e.arr);
        let mut adjustment = None;
        if ui
            .add_enabled(
                !matches!(entry.mode.arr, Some(TravelMode::At(_))),
                egui::Button::new("At")
                    .right_text(at_time.map_or("--:--:--".to_string(), |e| e.to_string())),
            )
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::At(
                at_time.unwrap_or_default(),
            ))));
        };
        // for
        let for_dur = if let Some(TravelMode::For(d)) = entry.mode.arr {
            Some(d)
        } else {
            entry.travel_duration(parent, entry_q)
        };
        if ui
            .add_enabled(
                !matches!(entry.mode.arr, Some(TravelMode::For(_))),
                egui::Button::new("For")
                    .right_text(for_dur.map_or("--:--:--".to_string(), |d| d.to_string())),
            )
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::For(
                for_dur.unwrap_or_default(),
            ))));
        };
        // flexible
        if ui
            .add_enabled(
                !matches!(entry.mode.arr, Some(TravelMode::Flexible)),
                egui::Button::new("Flexible"),
            )
            .clicked()
        {
            adjustment = Some(EntryModeAdjustment::SetArrival(Some(TravelMode::Flexible)));
        }
        if ui
            .add_enabled(
                !matches!(entry.mode.arr, None),
                egui::Button::new("Non-stop"),
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
    });
}
