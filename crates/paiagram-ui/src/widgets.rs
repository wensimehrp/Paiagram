use egui::DragValue;
use egui::emath::Numeric;
use paiagram_core::units::time::{Duration, TimetableTime};

pub mod buttons;
pub mod indicators;
pub mod timetable_popup;

/// [`DragValue`] for [`TimetableTime`].
pub struct TimeDragValue<'a>(pub &'a mut TimetableTime);

impl<'a> egui::Widget for TimeDragValue<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let shift_pressed = ui.input(|r| r.modifiers.shift_only());
        ui.add(
            DragValue::from_get_set(|v| {
                if let Some(v) = v {
                    if shift_pressed {
                        *self.0 = TimetableTime::from_f64(v);
                    } else {
                        *self.0 = TimetableTime::from_hms(0, (v / 60.0).round() as i32, 0);
                    }
                }
                self.0.to_f64()
            })
            .custom_formatter(|v, _| TimetableTime::from_f64(v).to_string())
            .custom_parser(|s| TimetableTime::from_str(s).map(TimetableTime::to_f64)),
        )
    }
}

/// [`DragValue`] for [`TimetableTime`], in Japanese timetable style.
pub struct TimeDragValueOud<'a>(pub &'a mut TimetableTime, pub bool);

impl<'a> egui::Widget for TimeDragValueOud<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let display_second = self.1;
        let shift_pressed = ui.input(|r| r.modifiers.shift_only());
        ui.add(
            DragValue::from_get_set(|v| {
                if let Some(v) = v {
                    if shift_pressed && display_second {
                        *self.0 = TimetableTime::from_f64(v);
                    } else {
                        *self.0 = TimetableTime::from_hms(0, (v / 60.0).round() as i32, 0);
                    }
                }
                self.0.to_f64()
            })
            .custom_formatter(|v, _| TimetableTime::from_f64(v).to_oud2_str(display_second))
            .custom_parser(|s| TimetableTime::from_oud2_str(s).map(TimetableTime::to_f64)),
        )
    }
}

/// [`DragValue`] for [`Duration`].
pub struct DurationDragValue<'a>(pub &'a mut Duration);

impl<'a> egui::Widget for DurationDragValue<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let shift_pressed = ui.input(|r| r.modifiers.shift_only());
        ui.add(
            DragValue::from_get_set(|v| {
                if let Some(v) = v {
                    if shift_pressed {
                        *self.0 = Duration::from_f64(v);
                    } else {
                        *self.0 = Duration::from_hms(0, (v / 60.0).round() as i32, 0);
                    }
                }
                self.0.to_f64()
            })
            .prefix("→ ")
            .custom_formatter(|v, _| Duration::from_f64(v).to_string_no_arrow())
            .custom_parser(|s| Duration::from_str(s).map(Duration::to_f64)),
        )
    }
}
