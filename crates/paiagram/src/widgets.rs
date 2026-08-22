use egui::emath::Numeric;
use egui::{DragValue, Pos2};
use paiagram_core::units::time::{TDuration, TimetableTime};

pub(crate) mod buttons;
pub(crate) mod indicators;
// pub(crate) mod timetable_popup;

/// [`DragValue`] for [`TimetableTime`].
pub(crate) struct TimeDragValue<'a>(pub &'a mut TimetableTime);

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
pub(crate) struct TimeDragValueOud<'a>(pub &'a mut TimetableTime, pub bool);

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
pub(crate) struct DurationDragValue<'a>(pub &'a mut TDuration);

impl<'a> egui::Widget for DurationDragValue<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let shift_pressed = ui.input(|r| r.modifiers.shift_only());
        ui.add(
            DragValue::from_get_set(|v| {
                if let Some(v) = v {
                    if shift_pressed {
                        *self.0 = TDuration::from_f64(v);
                    } else {
                        *self.0 = TDuration::from_hms(0, (v / 60.0).round() as i32, 0);
                    }
                }
                self.0.to_f64()
            })
            .prefix("→ ")
            .custom_formatter(|v, _| TDuration::from_f64(v).to_string_no_arrow())
            .custom_parser(|s| TDuration::from_str(s).map(TDuration::to_f64)),
        )
    }
}

pub(crate) enum LogoStroke {
    _2(bool, [f32; 2], [f32; 2], bool),
    _3(bool, [f32; 2], [f32; 2], [f32; 2], bool),
}

pub(crate) const LOGO_COORDINATES: &[LogoStroke] = &[
    LogoStroke::_2(true, [10.5, 4.50], [16.5, 16.5], true),
    LogoStroke::_2(false, [6.50, 6.50], [11.5, 16.5], false),
    LogoStroke::_2(false, [4.50, 9.50], [10.5, 9.50], false),
    LogoStroke::_2(true, [9.50, 6.50], [15.5, 18.5], true),
    LogoStroke::_2(false, [15.5, 13.5], [19.5, 13.5], false),
    LogoStroke::_3(false, [14.5, 11.5], [15.5, 11.5], [19.5, 9.50], true),
    LogoStroke::_3(false, [11.5, 11.5], [10.5, 11.5], [4.50, 14.5], true),
    LogoStroke::_2(false, [13.5, 9.50], [14.0, 9.50], false),
    LogoStroke::_2(false, [12.5, 13.5], [12.0, 13.5], false),
];
