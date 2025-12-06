use bevy::ecs::{entity::Entity, message::MessageWriter};
use egui::{Button, RichText, Ui};

use crate::{
    units::time::{Duration, TimetableTime},
    vehicles::{
        AdjustTimetableEntry, TimetableAdjustment,
        entries::{TimetableEntry, TimetableEntryCache, TravelMode},
    },
};

pub fn popup(
    entry_entity: Entity,
    (current_entry, current_entry_cache): (&TimetableEntry, &TimetableEntryCache),
    previous_entry: Option<(&TimetableEntry, &TimetableEntryCache)>,
    msg_adjust_entry: &mut MessageWriter<AdjustTimetableEntry>,
    ui: &mut Ui,
    arrival: bool,
) {
    let dt = match (previous_entry, arrival) {
        (Some((pe, pec)), true) => {
            if let Some(ae) = current_entry_cache.estimate.as_ref().map(|e| e.arrival)
                && let Some(de) = pec.estimate.as_ref().map(|e| e.departure)
            {
                Some(ae - de)
            } else {
                None
            }
        }
        (None, true) => None,
        (_, false) => {
            if let Some(de) = current_entry_cache.estimate.as_ref().map(|e| e.departure)
                && let Some(ae) = current_entry_cache.estimate.as_ref().map(|e| e.arrival)
            {
                Some(de - ae)
            } else {
                None
            }
        }
    };
    let (show_at, show_for, show_flexible, show_bypass) = if arrival {
        match current_entry.arrival {
            TravelMode::At(_) => (false, true, true, false),
            TravelMode::For(_) => (true, false, true, false),
            TravelMode::Flexible => (true, true, false, false),
        }
    } else {
        match current_entry.departure {
            Some(TravelMode::At(_)) => (false, true, true, true),
            Some(TravelMode::For(_)) => (true, false, true, true),
            Some(TravelMode::Flexible) => (true, true, false, true),
            None => (true, true, true, false),
        }
    };
    ui.add_enabled_ui(show_at ^ show_for, |ui: &mut Ui| {
        ui.horizontal(|ui| {
            for (s, dt) in [("-10", -10), ("-1", -1), ("+1", 1), ("+10", 10)] {
                let response =
                    ui.add_sized((24.0, 18.0), Button::new(RichText::monospace(s.into())));
                if response.clicked() {
                    msg_adjust_entry.write(AdjustTimetableEntry {
                        entity: entry_entity,
                        adjustment: if arrival {
                            TimetableAdjustment::AdjustArrivalTime(Duration(dt))
                        } else {
                            TimetableAdjustment::AdjustDepartureTime(Duration(dt))
                        },
                    });
                }
                if response.secondary_clicked() {
                    msg_adjust_entry.write(AdjustTimetableEntry {
                        entity: entry_entity,
                        adjustment: if arrival {
                            TimetableAdjustment::AdjustArrivalTime(Duration(dt * 60))
                        } else {
                            TimetableAdjustment::AdjustArrivalTime(Duration(dt * 60))
                        },
                    });
                }
            }
        })
    });
    if ui
        .add_enabled(
            show_at,
            Button::new("At").right_text(RichText::monospace(
                if arrival {
                    current_entry_cache.estimate.as_ref().map(|e| e.arrival)
                } else {
                    current_entry_cache.estimate.as_ref().map(|e| e.departure)
                }
                .map_or("--:--:--".into(), |t| t.to_string().into()),
            )),
        )
        .clicked()
    {
        msg_adjust_entry.write(AdjustTimetableEntry {
            entity: entry_entity,
            adjustment: if arrival {
                TimetableAdjustment::SetArrivalType(TravelMode::At(
                    if let Some(ae) = current_entry_cache.estimate.as_ref().map(|e| e.arrival) {
                        ae
                    } else {
                        TimetableTime(0)
                    },
                ))
            } else {
                TimetableAdjustment::SetDepartureType(Some(TravelMode::At(
                    if let Some(de) = current_entry_cache.estimate.as_ref().map(|e| e.departure) {
                        de
                    } else {
                        TimetableTime(0)
                    },
                )))
            },
        });
    }
    if ui
        .add_enabled(
            show_for,
            Button::new("For").right_text(RichText::monospace(
                dt.map_or("--:--:--".into(), |t| t.to_string().into()),
            )),
        )
        .clicked()
    {
        msg_adjust_entry.write(AdjustTimetableEntry {
            entity: entry_entity,
            adjustment: if arrival {
                TimetableAdjustment::SetArrivalType(TravelMode::For(if let Some(dt) = dt {
                    dt
                } else {
                    Duration(0)
                }))
            } else {
                TimetableAdjustment::SetDepartureType(Some(TravelMode::For(if let Some(dt) = dt {
                    dt
                } else {
                    Duration(0)
                })))
            },
        });
    }
    if ui
        .add_enabled(show_flexible, Button::new("Flexible"))
        .clicked()
    {
        msg_adjust_entry.write(AdjustTimetableEntry {
            entity: entry_entity,
            adjustment: if arrival {
                TimetableAdjustment::SetArrivalType(TravelMode::Flexible)
            } else {
                TimetableAdjustment::SetDepartureType(Some(TravelMode::Flexible))
            },
        });
    }
    if arrival {
        return;
    }
    if ui
        .add_enabled(show_bypass, Button::new("Non-Stop"))
        .clicked()
    {
        msg_adjust_entry.write(AdjustTimetableEntry {
            entity: entry_entity,
            adjustment: TimetableAdjustment::SetDepartureType(None),
        });
    }
}
