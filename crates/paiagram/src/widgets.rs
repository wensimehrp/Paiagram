use egui::DragValue;
use egui::emath::Numeric;
use paiagram_core::time::{TDuration, TTime, TimetableTime};

use crate::font::TIMETABLTE_TEXT_STYLE;

pub(crate) mod buttons;
pub(crate) mod indicators;
pub(crate) mod search;
// pub(crate) mod timetable_popup;

/// [`DragValue`] for [`TimetableTime`].
pub(crate) struct TimeDragValue<'a>(pub TTime, pub &'a mut Option<TDuration>);

#[derive(Default, Clone, Copy)]
enum TimeDragValueStates {
    #[default]
    NotDragging,
    Dragging(TTime),
}

impl<'a> egui::Widget for TimeDragValue<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let id = ui.next_auto_id().with("time drag value");
        let mut current_state: TimeDragValueStates =
            ui.data_mut(|w| w.remove_temp(id)).unwrap_or_default();
        let mut new = match current_state {
            TimeDragValueStates::NotDragging => self.0,
            TimeDragValueStates::Dragging(t) => t,
        };
        ui.style_mut().drag_value_text_style = TIMETABLTE_TEXT_STYLE.clone();
        // TODO: find a way to display the second in smaller scripts, i.e. 12:00 ₀₀
        let widget = DragValue::new(&mut new)
            .update_while_editing(false)
            .custom_formatter(|v, _| TTime::from_f64(v).to_string())
            .custom_parser(|s| TTime::from_str(s).map(TTime::to_f64));
        let res = ui.add(widget);
        match current_state {
            TimeDragValueStates::NotDragging => {
                *self.1 = None;
                if res.dragged() {
                    current_state = TimeDragValueStates::Dragging(new)
                }
            }
            TimeDragValueStates::Dragging(..) => {
                current_state = TimeDragValueStates::Dragging(new);
                if !res.dragged() {
                    let dt = new - self.0;
                    *self.1 = Some(dt);
                    current_state = TimeDragValueStates::NotDragging;
                }
            }
        }
        ui.data_mut(|w| w.insert_temp(id, current_state));
        res
    }
}

/// [`DragValue`] for [`Duration`].
///
/// Like [`TimeDragValue`], this takes the value to display and an output slot that receives the
/// total change once a drag finishes.
pub(crate) struct DurationDragValue<'a>(pub TDuration, pub &'a mut Option<TDuration>);

#[derive(Default, Clone, Copy)]
enum DurationDragValueStates {
    #[default]
    NotDragging,
    Dragging(TDuration),
}

impl<'a> egui::Widget for DurationDragValue<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let id = ui.next_auto_id().with("duration drag value");
        let mut current_state: DurationDragValueStates =
            ui.data_mut(|w| w.remove_temp(id)).unwrap_or_default();
        let mut new = match current_state {
            DurationDragValueStates::NotDragging => self.0,
            DurationDragValueStates::Dragging(d) => d,
        };
        ui.style_mut().drag_value_text_style = TIMETABLTE_TEXT_STYLE.clone();
        let shift_pressed = ui.input(|r| r.modifiers.shift_only());
        let res = ui.add(
            DragValue::from_get_set(|v| {
                if let Some(v) = v {
                    if shift_pressed {
                        new = TDuration::from_f64(v);
                    } else {
                        new = TDuration::from_hms(0, (v / 60.0).round() as i32, 0);
                    }
                }
                new.to_f64()
            })
            .prefix("→ ")
            .custom_formatter(|v, _| TDuration::from_f64(v).to_string_no_arrow())
            .custom_parser(|s| TDuration::from_str(s).map(TDuration::to_f64)),
        );
        match current_state {
            DurationDragValueStates::NotDragging => {
                *self.1 = None;
                if res.dragged() {
                    current_state = DurationDragValueStates::Dragging(new);
                }
            }
            DurationDragValueStates::Dragging(..) => {
                current_state = DurationDragValueStates::Dragging(new);
                if !res.dragged() {
                    let dd = new - self.0;
                    *self.1 = Some(dd);
                    current_state = DurationDragValueStates::NotDragging;
                }
            }
        }
        ui.data_mut(|w| w.insert_temp(id, current_state));
        res
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
