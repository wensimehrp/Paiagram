use crate::{
    basic::TimetableTime,
    status_bar_text::SetStatusBarText,
    vehicles::{AdjustTimetableEntry, ArrivalType, TimetableAdjustment, TimetableEntry},
};
use bevy::prelude::{Entity, MessageWriter};
use bevy_egui::egui::{
    self, Align, Layout, Margin, Popup, PopupCloseBehavior, Sense, TextEdit, TextStyle, Ui,
    UiBuilder,
};
use std::borrow::Cow;

pub fn time_widget(
    ui: &mut Ui,
    arrival: ArrivalType,
    arrival_estimate: Option<TimetableTime>,
    previous_departure_estimate: Option<TimetableTime>,
    entity: Entity,
    text_edit_buffer: &mut Option<(String, Entity, bool)>,
    msg_writer: &mut MessageWriter<AdjustTimetableEntry>,
) {
    let sense = Sense::click_and_drag();
    let (rect, base_response) = ui.allocate_at_least(ui.available_size(), sense);
    let inner = ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            if let Some((text, cached_entity, is_focused)) = text_edit_buffer
                && entity == *cached_entity
            {
                let response = ui.add(
                    TextEdit::singleline(text)
                        .font(TextStyle::Monospace)
                        .margin(Margin {
                            left: 0,
                            right: 0,
                            top: 2,
                            bottom: 0,
                        }),
                );
                if *is_focused {
                    response.request_focus();
                    *is_focused = false;
                }
                if response.lost_focus() {
                    if let Some(parsed_time) = parse_time_input(&text) {
                        msg_writer.write(AdjustTimetableEntry {
                            entity,
                            adjustment: TimetableAdjustment::SetArrivalType(parsed_time),
                        });
                    }
                    *text_edit_buffer = None;
                    return;
                }
                // remove all text except [0-9+-:] from the input
                let filtered_text: String = text
                    .chars()
                    .filter(|c| {
                        c.is_ascii_digit()
                            || *c == '+'
                            || *c == '-'
                            || *c == ':'
                            || *c == '.'
                            || *c == '>'
                    })
                    .collect();
                *text_edit_buffer = Some((filtered_text, entity, *is_focused));
            } else {
                let response = ui.monospace(format!("{}", arrival));
                if base_response.union(response).double_clicked() {
                    *text_edit_buffer = Some((format!("{}", arrival), entity, true));
                }
            }
        },
    );
    let (show_at, show_for, show_flexible) = match arrival {
        ArrivalType::At(_) => (false, true, true),
        ArrivalType::Duration(_) => (true, false, true),
        ArrivalType::Flexible => (true, true, false),
    };
    Popup::menu(&base_response.union(inner.response))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            use egui::{Button, RichText};
            ui.add_enabled_ui(show_flexible, |ui| {
                ui.horizontal(|ui| {
                    for (s, dt) in [("-10", -10), ("-1", -1), ("+1", 1), ("+10", 10)] {
                        let mut response =
                            ui.add_sized((24.0, 18.0), Button::new(RichText::monospace(s.into())));
                        if response.clicked() {
                            msg_writer.write(AdjustTimetableEntry {
                                entity,
                                adjustment: TimetableAdjustment::AdjustArrivalTime(TimetableTime(
                                    dt,
                                )),
                            });
                        }
                        if response.secondary_clicked() {
                            msg_writer.write(AdjustTimetableEntry {
                                entity,
                                adjustment: TimetableAdjustment::AdjustArrivalTime(TimetableTime(
                                    dt * 60,
                                )),
                            });
                        }
                    }
                })
            });
            if ui
                .add_enabled(
                    show_at,
                    Button::new("At").right_text(RichText::monospace(
                        arrival_estimate
                            .and_then(|v| Some(format!("{}", v)))
                            .unwrap_or("--:--:--".into())
                            .into(),
                    )),
                )
                .clicked()
            {
                msg_writer.write(AdjustTimetableEntry {
                    entity,
                    adjustment: TimetableAdjustment::SetArrivalType(
                        if let Some(arrival_estimate) = arrival_estimate {
                            ArrivalType::At(arrival_estimate)
                        } else {
                            ArrivalType::At(TimetableTime(0))
                        },
                    ),
                });
                ui.close();
            };
            let time_difference =
                if let (Some(arrival_estimate), Some(previous_departure_estimate)) =
                    (arrival_estimate, previous_departure_estimate)
                {
                    Some(arrival_estimate - previous_departure_estimate)
                } else {
                    None
                };
            if ui
                .add_enabled(
                    show_for,
                    Button::new("For").right_text(RichText::monospace(
                        if let Some(time_difference) = time_difference {
                            format!("{}", time_difference)
                        } else {
                            "-> --:--:--".into()
                        }
                        .into(),
                    )),
                )
                .clicked()
            {
                msg_writer.write(AdjustTimetableEntry {
                    entity,
                    adjustment: TimetableAdjustment::SetArrivalType(
                        if let Some(time_difference) = time_difference {
                            ArrivalType::Duration(time_difference)
                        } else {
                            ArrivalType::Duration(TimetableTime(0))
                        },
                    ),
                });
                ui.close();
            };
            if ui
                .add_enabled(show_flexible, Button::new("Flexible"))
                .clicked()
            {
                msg_writer.write(AdjustTimetableEntry {
                    entity,
                    adjustment: TimetableAdjustment::SetArrivalType(ArrivalType::Flexible),
                });
                ui.close();
            };
        });
}

fn parse_time_input(mut input: &str) -> Option<ArrivalType> {
    // check if the input starts with ->
    let mut is_duration = false;
    if input.starts_with("->") {
        // remove the ->
        input = &input[2..];
        is_duration = true;
    } else if input.starts_with(">>") {
        return Some(ArrivalType::Flexible);
    }
    let time = TimetableTime::from_str(input)?;
    if is_duration {
        Some(ArrivalType::Duration(time))
    } else {
        Some(ArrivalType::At(time))
    }
}
