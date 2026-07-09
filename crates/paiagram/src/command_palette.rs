use std::sync::LazyLock;

use egui::{Context, Key, NumExt, Ui};
use egui_i18n::tr;
use ib_matcher::matcher::{IbMatcher, PinyinMatchConfig, RomajiMatchConfig};
use ib_matcher::pinyin::PinyinNotation;
use paiagram_core::{RouteKey, StationKey, TripKey, Source};

use super::tabs::all_tabs::*;
use super::tabs::AppState;
use crate::MainTab;

// TODO: make this based on settings
static PINYIN_MATCH_DATA: LazyLock<PinyinMatchConfig> = std::sync::LazyLock::new(|| {
    PinyinMatchConfig::builder(
        PinyinNotation::Ascii
            | PinyinNotation::AsciiFirstLetter
            | PinyinNotation::DiletterMicrosoft,
    )
    .build()
});

static ROMAJI_MATCH_DATA: LazyLock<RomajiMatchConfig> =
    std::sync::LazyLock::new(|| RomajiMatchConfig::builder().build());

#[derive(Default)]
pub(crate) struct CommandPalette {
    visible: bool,
    query: String,
    selected_alternative: usize,
}

enum MatchedType {
    Route(RouteKey),
    Station(StationKey),
    Trip(TripKey),
    Tab(fn() -> MainTab),
}

impl CommandPalette {
    pub(crate) fn toggle(&mut self) {
        self.visible ^= true;
    }

    pub(crate) fn show(&mut self, ctx: &Context, app: &mut AppState) {
        self.visible &= !ctx.input_mut(|i| i.key_pressed(Key::Escape));
        if !self.visible {
            self.query.clear();
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
                egui::Frame {
                    inner_margin: 2.0.into(),
                    ..Default::default()
                }
                .show(ui, |ui| self.window_content_ui(ui, app));
            });
    }

    fn window_content_ui(&mut self, ui: &mut Ui, app: &mut AppState) {
        let enter_pressed = ui.input_mut(|i| i.consume_key(Default::default(), Key::Enter));
        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        text_response.request_focus();
        let scroll_to_selected_alternative = if text_response.changed() {
            self.selected_alternative = 0;
            true
        } else {
            false
        };

        let selected = egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.alternatives_ui(ui, enter_pressed, scroll_to_selected_alternative, app)
            })
            .inner;

        if selected {
            *self = Default::default();
        }
    }

    fn alternatives_ui(
        &mut self,
        ui: &mut Ui,
        enter_pressed: bool,
        mut scroll_to_selected_alternative: bool,
        app: &mut AppState,
    ) -> bool {
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowUp));
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowDown));

        let item_height = 16.0;

        let mut num_alternatives: usize = 0;
        let mut selected_and_determined = false;

        let build_matcher = || {
            IbMatcher::builder(self.query.as_str())
                .pinyin(PINYIN_MATCH_DATA.shallow_clone())
                .romaji(ROMAJI_MATCH_DATA.shallow_clone())
                .analyze(true)
                .build()
        };

        let mut matcher = build_matcher();
        let query_changed = self.query.is_empty() || !matcher.is_match("");

        // Build alternatives list
        let mut matched: Vec<(String, MatchedType)> = Vec::new();
        if query_changed {
            let panel_info: [(String, fn() -> MainTab); 4] = [
                (tr!("tab-start"), || MainTab::Start(StartTab::default())),
                (tr!("tab-settings"), || MainTab::Settings(SettingsTab)),
                (tr!("tab-classes"), || {
                    MainTab::Classes(ClassesTab::default())
                }),
                (tr!("tab-graph"), || MainTab::Graph(GraphTab::default())),
            ];
            for (name, fn_ptr) in panel_info.into_iter() {
                let match_string = format!("{} (Tab)", name);
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), MatchedType::Tab(fn_ptr)));
            }

            // Collect trips
            for (tk, name) in app.source.trips_iter() {
                let match_string = format!("{} trip", name);
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), MatchedType::Trip(tk)));
                if matched.len() >= 100 {
                    break;
                }
            }

            // Collect stations
            for (sk, name, _) in app.source.stations_iter() {
                let match_string = format!("{} station", name);
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), MatchedType::Station(sk)));
                if matched.len() >= 100 {
                    break;
                }
            }

            // Collect routes
            for (rk, name) in app.source.routes_iter() {
                let match_string = format!("{} route", name);
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), MatchedType::Route(rk)));
                if matched.len() >= 100 {
                    break;
                }
            }
        }

        for (i, (name, matched_type)) in matched.iter().enumerate() {
            let selected = i == self.selected_alternative;
            let response = ui.add_sized(
                egui::vec2(ui.available_width(), item_height),
                egui::Button::new(name).right_text(match matched_type {
                    MatchedType::Route(_) => "(Route)",
                    MatchedType::Station(_) => "(Station)",
                    MatchedType::Trip(_) => "(Trip)",
                    MatchedType::Tab(_) => "(Tab)",
                }),
            );
            if response.clicked() {
                selected_and_determined |= true;
            }
            if selected {
                ui.painter().rect_filled(
                    response.rect.expand(1.0),
                    2,
                    ui.visuals().selection.bg_fill.gamma_multiply(0.5),
                );

                if enter_pressed {
                    match matched_type {
                        MatchedType::Route(rk) => {
                            // Open diagram tab for route
                            // TODO: need a way to get/create diagram tab from route key
                        }
                        MatchedType::Station(sk) => {
                            app.main_ui.push_to_focused_leaf(MainTab::Station(
                                StationTab::new(*sk),
                            ));
                        }
                        MatchedType::Trip(tk) => {
                            app.main_ui.push_to_focused_leaf(MainTab::Trip(TripTab::new(*tk)));
                        }
                        MatchedType::Tab(f) => {
                            app.main_ui.push_to_focused_leaf(f());
                        }
                    }
                    selected_and_determined |= true;
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

        self.selected_alternative = self
            .selected_alternative
            .saturating_sub(ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp)));
        self.selected_alternative = self
            .selected_alternative
            .saturating_add(ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown)));

        self.selected_alternative = self
            .selected_alternative
            .clamp(0, num_alternatives.saturating_sub(1));

        selected_and_determined
    }
}
