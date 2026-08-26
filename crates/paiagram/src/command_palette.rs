use ecow::EcoString;
use egui::{Context, Key, Modifiers, NumExt, Ui};
use egui_i18n::tr;
use paiagram_core::{RouteKey, StationKey, TripKey};

use super::MainTab;
use crate::tabs::all_tabs::*;
use crate::widgets::search::build_matcher;
use crate::{App, UiCommand};

// TODO: make this based on settings
// TODO: make this a resource instead?

#[derive(Default)]
pub(crate) struct CommandPalette {
    visible: bool,
    query: String,
    matched: Vec<(EcoString, MatchedType)>,
    selected_alternative: usize,
}

#[derive(Clone, Copy)]
enum MatchedType {
    Route(RouteKey),
    Station(StationKey),
    Trip(TripKey),
    Tab(fn() -> MainTab),
    LoadOuDiaSecond,
}

impl CommandPalette {
    fn clear(&mut self) {
        self.visible = false;
        self.query.clear();
        self.matched.clear();
        self.selected_alternative = 0;
    }
    pub(crate) fn show(&mut self, ctx: &Context, app: &mut App) {
        self.visible |= ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::P));
        self.visible &= !ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if !self.visible {
            return;
        }

        let screen_rect = ctx.content_rect();
        let width = 300.0;
        let max_height = 320.0.at_most(screen_rect.height());

        egui::Window::new("Command Palette")
            .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
            .fixed_size([width, max_height])
            .pivot(egui::Align2::CENTER_TOP)
            .resizable(false)
            .scroll(false)
            .title_bar(false)
            .show(ctx, |ui| {
                egui::Frame::new().inner_margin(2).show(ui, |ui| self.window_content_ui(ui, app))
            });
    }

    fn window_content_ui(&mut self, ui: &mut Ui, app: &mut App) {
        // query a bunch of stuff from the ECS, then throw them in
        let enter_pressed = ui.input_mut(|i| i.consume_key(Default::default(), Key::Enter));
        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .desired_width(f32::INFINITY)
                .lock_focus(true)
                .hint_text("Perform an action..."),
        );
        text_response.request_focus();
        let scroll_to_selected_alternative = if text_response.changed() {
            self.selected_alternative = 0;
            true
        } else {
            false
        };

        #[rustfmt::skip]
        let panel_info: [(EcoString, MatchedType); 3] = [
            (tr!("tab-start").into(), MatchedType::Tab(|| MainTab::Start(StartTab)),),
            (tr!("tab-settings").into(), MatchedType::Tab(|| MainTab::Config(ConfigTab)),),
            ("Load OuDiaSecond".into(), MatchedType::LoadOuDiaSecond),
        ];

        let candidates_iter = panel_info
            .into_iter()
            .chain(app.trips.iter().map(|v| (v.name.clone(), MatchedType::Trip(v.key))))
            .chain(app.stations.iter().map(|v| (v.name.clone(), MatchedType::Station(v.key))));

        if text_response.changed() {
            self.matched.clear();
            let matcher = build_matcher(&self.query);
            let extender = candidates_iter.filter(|(n, _)| matcher.is_match(n.as_str())).take(100);
            self.matched.extend(extender)
        }

        let Some(item) = egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.alternatives_ui(ui, enter_pressed, scroll_to_selected_alternative)
            })
            .inner
        else {
            return;
        };
        if let Some(tab) = match item {
            MatchedType::Route(k) => None,
            MatchedType::Station(k) => None,
            MatchedType::Trip(k) => Some(MainTab::Trip(TripTab::new(k))),
            MatchedType::Tab(f) => Some(f()),
            MatchedType::LoadOuDiaSecond => {
                // TODO
                None
            }
        } {
            app.ui_action_queue.push(UiCommand::OpenOrFocus(tab));
        }
        self.clear();
    }

    fn alternatives_ui(
        &mut self,
        ui: &mut Ui,
        enter_pressed: bool,
        mut scroll_to_selected_alternative: bool,
    ) -> Option<MatchedType> {
        let mut ret: Option<MatchedType> = None;
        scroll_to_selected_alternative |=
            ui.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown));

        let item_height = 16.0;
        let mut num_alternatives: usize = 0;

        for (i, (name, matched_type)) in self.matched.iter().enumerate() {
            let selected = i == self.selected_alternative;
            let response = ui.add_sized(
                egui::vec2(ui.available_width(), item_height),
                egui::Button::new(name.as_str()).right_text(match matched_type {
                    MatchedType::Route(_) => "(Route)",
                    MatchedType::Station(_) => "(Station)",
                    MatchedType::Trip(_) => "(Trip)",
                    MatchedType::Tab(_) => "(Tab)",
                    _ => "(Action)",
                }),
            );
            if response.clicked() {
                ret = Some(*matched_type);
            }
            if selected {
                ui.painter().rect_filled(
                    response.rect.expand(1.0),
                    2,
                    ui.visuals().selection.bg_fill.gamma_multiply(0.5),
                );

                if enter_pressed {
                    ret = Some(*matched_type);
                }

                if scroll_to_selected_alternative {
                    ui.scroll_to_rect(response.rect, None);
                }
            }
            num_alternatives += 1;
        }

        if num_alternatives == 0 {
            ui.weak("Nothing matched...");
        }

        self.selected_alternative = self.selected_alternative.saturating_sub(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp)),
        );
        self.selected_alternative = self.selected_alternative.saturating_add(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown)),
        );

        self.selected_alternative =
            self.selected_alternative.clamp(0, num_alternatives.saturating_sub(1));

        ret
    }
}
