use egui::emath::Numeric;
use paiagram_core::units::time::{Duration, TimetableTime};

pub mod buttons;
pub mod indicators;
pub mod timetable_popup;

pub fn time_drag_value(t: &mut TimetableTime) -> egui::DragValue<'_> {
    egui::DragValue::new(t)
        .custom_formatter(|v, _| TimetableTime::from_f64(v).to_string())
        .custom_parser(|s| TimetableTime::from_str(s).map(TimetableTime::to_f64))
}

pub fn time_drag_value_oud(t: &mut TimetableTime) -> egui::DragValue<'_> {
    egui::DragValue::new(t)
        .custom_formatter(|v, _| TimetableTime::from_f64(v).to_oud2_str(false))
        .custom_parser(|s| TimetableTime::from_oud2_str(s).map(TimetableTime::to_f64))
}

pub fn duration_drag_value(d: &mut Duration) -> egui::DragValue<'_> {
    egui::DragValue::new(d)
        .prefix("→ ")
        .custom_formatter(|v, _| Duration::from_f64(v).to_string_no_arrow())
        .custom_parser(|s| Duration::from_str(s).map(Duration::to_f64))
}
