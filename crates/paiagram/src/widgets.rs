use egui::emath::Numeric;
use egui::*;
use paiagram_core::time::{TDuration, TTime};

use crate::font::TIMETABLTE_TEXT_STYLE;

pub(crate) mod buttons;
pub(crate) mod indicators;
pub(crate) mod search;
// pub(crate) mod timetable_popup;

/// [`DragValue`] for [`TimetableTime`].
pub(crate) struct TimeDragValue<'a>(pub &'a mut TTime);

impl<'a> Widget for TimeDragValue<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.style_mut().drag_value_text_style = TIMETABLTE_TEXT_STYLE.clone();
        let widget = DragValue::new(&mut self.0.0)
            .custom_formatter(|v, _| TTime::from_f64(v).to_string())
            .custom_parser(|s| TTime::from_str(s).map(TTime::to_f64));
        ui.add(widget)
    }
}

/// [`DragValue`] for [`Duration`].
pub(crate) struct DurationDragValue<'a>(pub &'a mut TDuration);

impl<'a> Widget for DurationDragValue<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.style_mut().drag_value_text_style = TIMETABLTE_TEXT_STYLE.clone();
        let widget = DragValue::new(&mut self.0.0)
            .custom_formatter(|v, _| TDuration::from_f64(v).to_string())
            .custom_parser(|s| TDuration::from_str(s).map(TDuration::to_f64));
        ui.add(widget)
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
