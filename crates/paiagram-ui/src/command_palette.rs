use super::MainTab;
use super::OpenOrFocus;
use crate::tabs::Tab;
use crate::tabs::all_tabs::*;
use bevy::prelude::*;
use egui::{Context, Key, NumExt, Ui};
use ib_matcher::matcher::{IbMatcher, PinyinMatchConfig, RomajiMatchConfig};
use ib_matcher::pinyin::PinyinNotation;
use paiagram_core::route::Route;
use paiagram_core::station::Station;
use paiagram_core::trip::Trip;
use std::sync::LazyLock;

// TODO: make this based on settings
// TODO: make this a resource instead?
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

#[derive(Resource, Default)]
pub struct CommandPalette {
    visible: bool,
    query: String,
    selected_alternative: usize,
}

enum MatchedType {
    Route(Entity),
    Station(Entity),
    Trip(Entity),
    Tab(fn() -> MainTab),
}

impl CommandPalette {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
    pub fn show(&mut self, ctx: &Context, world: &mut World) {
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
                // We need an extra egui frame here because we set clip_rect_margin to zero.
                egui::Frame {
                    inner_margin: 2.0.into(),
                    ..Default::default()
                }
                .show(ui, |ui| self.window_content_ui(ui, world));
            });
    }

    fn window_content_ui(&mut self, ui: &mut Ui, world: &mut World) {
        // query a bunch of stuff from the ECS, then throw them in
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
                world
                    .run_system_cached_with(
                        Self::alternatives_ui,
                        (
                            self,
                            ui,
                            enter_pressed,
                            scroll_to_selected_alternative,
                            text_response.changed(),
                        ),
                    )
                    .unwrap()
            })
            .inner;

        if selected {
            *self = Default::default();
        }
    }

    fn alternatives_ui(
        (
            InMut(panel),
            InMut(ui),
            In(enter_pressed),
            In(mut scroll_to_selected_alternative),
            In(query_changed),
        ): (InMut<Self>, InMut<Ui>, In<bool>, In<bool>, In<bool>),
        names: Query<(Entity, &Name, AnyOf<(&Trip, &Station, &Route)>)>,
        mut matched: Local<Vec<(String, MatchedType)>>,
        mut matcher: Local<Option<IbMatcher>>,
        mut commands: Commands,
    ) -> bool {
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowUp));
        scroll_to_selected_alternative |= ui.input(|i| i.key_pressed(Key::ArrowDown));

        let item_height = 16.0;

        let mut num_alternatives: usize = 0;
        let mut selected_and_determined = false;

        let build_matcher = || {
            IbMatcher::builder(panel.query.as_str())
                .pinyin(PINYIN_MATCH_DATA.shallow_clone())
                .romaji(ROMAJI_MATCH_DATA.shallow_clone())
                .analyze(true)
                .build()
        };

        let matcher = matcher.get_or_insert_with(build_matcher);

        if query_changed {
            *matcher = build_matcher();
            matched.clear();
            let mut match_string = String::new();
            const PANEL_INFO: &[(&str, fn() -> MainTab)] = &[
                (StartTab::NAME, || MainTab::Start(StartTab::default())),
                (SettingsTab::NAME, || MainTab::Settings(SettingsTab)),
                (InspectorTab::NAME, || {
                    MainTab::Inspector(InspectorTab::default())
                }),
                (ClassesTab::NAME, || MainTab::Classes(ClassesTab)),
            ];
            for (name, fn_ptr) in PANEL_INFO.iter().copied() {
                match_string.clear();
                match_string.push_str(name);
                match_string.push_str(" (Tab)");
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), MatchedType::Tab(fn_ptr)));
            }
            for (e, name, matched_type) in names {
                match_string.clear();
                let (matched_type, matched_str) = match matched_type {
                    (Some(_), _, _) => (MatchedType::Trip(e), "trip"),
                    (_, Some(_), _) => (MatchedType::Station(e), "station"),
                    (_, _, Some(_)) => (MatchedType::Route(e), "route"),
                    (None, None, None) => unreachable!(),
                };
                match_string.push_str(name.as_str());
                match_string.push_str(" ");
                match_string.push_str(matched_str);
                if !matcher.is_match(match_string.as_str()) {
                    continue;
                }
                matched.push((name.to_string(), matched_type));
                if matched.len() >= 100 {
                    break;
                }
            }
        }

        for (i, (name, matched_type)) in matched.iter().enumerate() {
            let selected = i == panel.selected_alternative;
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
                    commands.write_message(OpenOrFocus(match matched_type {
                        MatchedType::Route(e) => MainTab::Diagram(DiagramTab::new(*e)),
                        MatchedType::Station(e) => MainTab::Station(StationTab::new(*e)),
                        MatchedType::Trip(e) => MainTab::Trip(TripTab::new(*e)),
                        MatchedType::Tab(f) => f(),
                    }));
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

        panel.selected_alternative = panel.selected_alternative.saturating_sub(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp)),
        );
        panel.selected_alternative = panel.selected_alternative.saturating_add(
            ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown)),
        );

        panel.selected_alternative = panel
            .selected_alternative
            .clamp(0, num_alternatives.saturating_sub(1));

        selected_and_determined
    }
}
