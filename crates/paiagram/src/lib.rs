// SPDX-License-Identifier: MPL-2.0
//! Definitions for the user interface.

mod config;
mod selection;
mod tabs;
mod timer;
mod widgets;

use egui::{Color32, Frame, OpenUrl, Panel, Response, RichText, ScrollArea, Stroke, Ui};
use egui_i18n::tr;
use egui_tiles::{Behavior, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse};
use paiagram_core::Source;
use paiagram_core::time::Tick;
use serde::{Deserialize, Serialize};
use tabs::all_tabs::*;
use tabs::{MainTab, Tab, for_all_tabs};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::timer::GlobalTimer;
use crate::widgets::TimeDragValue;
pub struct UiPlugin;

pub struct App {
    source: Source,
    timer: GlobalTimer,
    preferences: config::Preferences,
    settings: config::Settings,
}

impl App {
    pub fn new() -> Self {
        Self {
            source: Source::new(),
            timer: GlobalTimer::new(),
            preferences: config::Preferences::default(),
            settings: config::Settings::default(),
        }
    }
}

impl std::ops::Deref for App {
    type Target = Source;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl std::ops::DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.source
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MainUiState {
    tree: Tree<MainTab>,
    maximized: Option<TileId>,
}

impl Default for MainUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs("main", vec![MainTab::Start(StartTab::default())]),
            maximized: None,
        }
    }
}

struct MainTabViewer<'a> {
    app: &'a mut App,
}

impl<'a> MainTabViewer<'a> {
    fn add_popup(&mut self, ui: &mut Ui) {
        let tab_definitions: &[(&str, MainTab)] = &[
            // (&tr!("tab-start"), MainTab::Start(StartTab::default())),
            // (&tr!("tab-settings"), MainTab::Settings(SettingsTab)),
            // (&tr!("tab-classes"), MainTab::Classes(ClassesTab::default())),
            // (&tr!("tab-graph"), MainTab::Graph(GraphTab::default())),
        ];
        for (s, t) in tab_definitions {
            if ui.button(*s).clicked() {
                // TOOD: open tabs
                // self.world.write_message(OpenOrFocus(t));
                ui.close();
            }
        }
    }
}

impl<'w> Behavior<MainTab> for MainTabViewer<'w> {
    fn tab_title_for_pane(&mut self, pane: &MainTab) -> egui::WidgetText {
        for_all_tabs!(pane, p, p.title())
    }
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, tab: &mut MainTab) -> UiResponse {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        for_all_tabs!(tab, t, t.main_display(self.app, ui));

        Default::default()
    }
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: false,
            prune_single_child_containers: true,
            all_panes_must_have_tabs: true,
            join_nested_linear_containers: true,
        }
    }
    fn is_tab_closable(&self, tiles: &Tiles<MainTab>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            None => false,
            Some(Tile::Container(_)) => false,
            Some(Tile::Pane(MainTab::Start(_))) => false,
            Some(Tile::Pane(_)) => true,
        }
    }
    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> Stroke {
        Stroke::new(1.0, Color32::TRANSPARENT)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
enum AdditionalTab {
    Edit,
    Properties,
    Export,
}

#[derive(Serialize, Deserialize, Clone)]
struct AdditionalUiState {
    tree: Tree<AdditionalTab>,
    focused_id: Option<TileId>,
    expanded: bool,
}

impl Default for AdditionalUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs(
                "additional",
                vec![
                    AdditionalTab::Edit,
                    AdditionalTab::Properties,
                    AdditionalTab::Export,
                ],
            ),
            focused_id: None,
            expanded: true,
        }
    }
}

struct AdditionalTabViewer<'w> {
    world: &'w mut App,
    focused_tab: Option<&'w mut MainTab>,
}

impl<'w> egui_tiles::Behavior<AdditionalTab> for AdditionalTabViewer<'w> {
    fn tab_title_for_pane(&mut self, tab: &AdditionalTab) -> egui::WidgetText {
        match *tab {
            AdditionalTab::Edit => tr!("side-panel-edit"),
            AdditionalTab::Properties => tr!("side-panel-details"),
            AdditionalTab::Export => tr!("side-panel-export"),
        }
        .into()
    }
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        _tile_id: egui_tiles::TileId,
        tab: &mut AdditionalTab,
    ) -> egui_tiles::UiResponse {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
            let Some(ref mut focused) = self.focused_tab else {
                ui.label(tr!("menu-nothing-focused"));
                return;
            };
            match *tab {
                AdditionalTab::Edit => {
                    for_all_tabs!(focused, t, t.edit_display(self.world, ui));
                }
                AdditionalTab::Properties => {
                    for_all_tabs!(focused, t, t.display_display(self.world, ui));
                }
                AdditionalTab::Export => {
                    for_all_tabs!(focused, t, t.export_display(self.world, ui));
                }
            }
        });
        Default::default()
    }
}

/// WASM fullscreen toggle
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function toggle_fullscreen(id) {
    if (!document.fullscreenElement) {
        const el = document.getElementById(id);
        if (el?.requestFullscreen) {
            el.requestFullscreen().catch(err => {
                console.error(`Error attempting to enable full-screen mode: ${err.message}`);
            });
        }
    } else {
        if (document.exitFullscreen) {
            document.exitFullscreen();
        }
    }
}
"#)]
extern "C" {
    fn toggle_fullscreen(id: &str);
}

pub fn show_ui(ui: &mut Ui, app: &mut App, mus: &mut MainUiState) {
    Panel::top("top panel")
        .exact_size(32.0)
        .show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let res = ui.button("More...");
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Fullscreen").clicked() {
                    let is_fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                }
                #[cfg(target_arch = "wasm32")]
                if ui.button("Fullscreen").clicked() {
                    toggle_fullscreen("paiagram_canvas");
                }
                egui::Popup::menu(&res).show(|ui| {});
                let res = ui.button(tr!("menu-about"));
                egui::Popup::menu(&res).show(|ui| {
                    if ui.button(tr!("menu-documentation")).clicked() {
                        ui.ctx()
                            .open_url(OpenUrl::new_tab(if cfg!(target_arch = "wasm32") {
                                "/docs"
                            } else {
                                "https://paiagram.com/docs"
                            }));
                    }
                    if cfg!(target_arch = "wasm32") && ui.button(tr!("menu-legal")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("./license.html"));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut true, "");
                    const GIT_REV_SHORT: &str = git_version::git_version!(fallback = "unknown");
                    const GH_LINK: &str = git_version::git_version!(
                        args = ["--always", "--abbrev=40"],
                        prefix = "https://github.com/wensimehrp/Paiagram/commit/",
                        fallback = "https://github.com/wensimehrp/Paiagram/"
                    );
                    ui.hyperlink_to(GIT_REV_SHORT, GH_LINK);
                });
            })
        });
    Panel::bottom("bottom panel")
        .exact_size(24.0)
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let mut time = app.timer.ticks().to_timetable_time();
                ui.add_enabled(
                    !app.timer.sync_to_real_time,
                    egui::Checkbox::new(&mut app.timer.animation_playing, ""),
                );
                let time_response = ui.add(TimeDragValue(&mut time));
                ui.add_enabled(
                    !app.timer.sync_to_real_time,
                    egui::DragValue::new(&mut app.timer.animation_speed)
                        .fixed_decimals(1)
                        .suffix("×"),
                );
                egui::Popup::menu(&time_response).show(|ui| {
                    ui.checkbox(
                        &mut app.timer.sync_to_real_time,
                        tr!("menu-sync-system-clock"),
                    );
                });
                if !app.timer.sync_to_real_time
                    && time_response.dragged()
                    && let Some(key) = app.timer.try_lock()
                {
                    *app.timer.ticks_mut(&key) = Tick::from_timetable_time(time);
                    app.timer.unlock(key);
                }
                if app.timer.animation_playing {
                    ui.ctx().request_repaint();
                }
            })
        });
    egui::CentralPanel::default()
        .frame(Frame::default())
        .show_inside(ui, |ui| {
            let mut tab_viewer = MainTabViewer { app };
            mus.tree.ui(&mut tab_viewer, ui);
        });
}
