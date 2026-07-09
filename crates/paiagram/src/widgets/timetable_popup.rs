use egui::{RectAlign, Response, Ui, Vec2, vec2};
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, TimetableTime};

use super::{DurationDragValue, TimeDragValue};

pub(crate) const POPUP_WIDTH: f32 = 130.0;
pub(crate) const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);

/// Display a departure time edit popup
pub(crate) fn departure_popup_inner(ui: &mut Ui, entry: &TEntry) {
    ui.set_width(POPUP_WIDTH);
    let (dep_mode, arr_mode) = match entry {
        TEntry::Pinned { arr, dep, .. } => (*dep, *arr),
        _ => return,
    };

    match dep_mode {
        TravelMode::At(t) => { ui.label(format!("Dep: {}", t)); }
        TravelMode::For(d) => { ui.label(format!("Dep: For {}", d)); }
        TravelMode::Flexible => { ui.label("Dep: Flexible"); }
    }

    if let TravelMode::At(a) = arr_mode {
        ui.label(format!("Arr: {}", a));
    }
}

/// Display an arrival time edit popup
pub(crate) fn arrival_popup_inner(ui: &mut Ui, entry: &TEntry) {
    ui.set_width(POPUP_WIDTH);
    match entry {
        TEntry::Pinned { arr: TravelMode::At(t), .. } => {
            ui.label(format!("Arr: {}", t));
        }
        TEntry::Pinned { arr: TravelMode::For(d), .. } => {
            ui.label(format!("Arr: For {}", d));
        }
        TEntry::Pinned { arr: TravelMode::Flexible, .. } => {
            ui.label("Arr: Flexible");
        }
        _ => {}
    }
}
