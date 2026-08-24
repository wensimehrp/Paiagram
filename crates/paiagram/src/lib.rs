// SPDX-License-Identifier: MPL-2.0
//! Definitions for the user interface.

mod command_palette;
mod config;
mod font;
mod selection;
mod tabs;
mod timer;
mod widgets;

pub use config::AppLanguage;
use egui::{Color32, Frame, OpenUrl, Panel, Stroke, Ui};
use egui_i18n::tr;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
use futures_lite::future::block_on;
use log::{info, warn};
use paiagram_core::Source;
use paiagram_core::import::{ImportType, generate_commands};
use paiagram_core::time::Tick;
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use tabs::all_tabs::*;
use tabs::{MainTab, Tab, for_all_tabs};

use crate::selection::SelectedItems;
use crate::timer::GlobalTimer;
use crate::widgets::TimeDragValue;
pub struct UiPlugin;

pub struct App {
    source: Source,
    timer: GlobalTimer,
    preferences: config::Preferences,
    settings: config::Settings,
    ui_action_queue: Vec<UiCommand>,
    selected_items: SelectedItems,
}

#[derive(Default)]
pub struct UiState {
    command_palette: command_palette::CommandPalette,
    mus: MainUiState,
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        Self {
            source: Source::new(),
            timer: GlobalTimer::new(),
            preferences: config::Preferences::new(ctx),
            settings: config::Settings::default(),
            ui_action_queue: Vec::default(),
            selected_items: SelectedItems::None,
        }
    }
    fn apply_ui_commands(&mut self, mus: &mut MainUiState) {
        for cmd in self.ui_action_queue.drain(..) {
            match cmd {
                UiCommand::OpenOrFocus(tab) => mus.open_or_focus(tab),
            }
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

enum UiCommand {
    OpenOrFocus(MainTab),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MainUiState {
    tree: Tree<MainTab>,
    maximized: Option<TileId>,
}

impl MainUiState {
    fn open_or_focus(&mut self, tab: MainTab) {
        let focused_id = if let Some(tile_id) = self.tree.tiles.find_pane(&tab) {
            // Already exists → just focus it
            self.tree.make_active(|id, _| id == tile_id);
            self.tree.set_visible(tile_id, true);
            tile_id
        } else {
            // New pane → add it to the currently focused container
            self.push_to_focused_leaf(tab)
        };
    }
    fn push_to_focused_leaf(&mut self, new_pane: MainTab) -> TileId {
        let new_id = self.tree.tiles.insert_pane(new_pane);

        // Try to add it to the same Tabs container that is currently focused
        if let Some(&active_id) = self.tree.active_tiles().last()
            && let Some(parent_id) = self.tree.tiles.parent_of(active_id)
            && let Some(Tile::Container(container)) = self.tree.tiles.get_mut(parent_id)
            && container.kind() == ContainerKind::Tabs
        {
            container.add_child(new_id);
            self.tree.make_active(|id, _| id == new_id);
            return new_id;
        }

        // Fallback: create a new top-level Tabs container
        let old_root = self.tree.root;
        let tabs_id = if let Some(old_root) = old_root {
            self.tree.tiles.insert_tab_tile(vec![old_root, new_id])
        } else {
            self.tree.tiles.insert_tab_tile(vec![new_id])
        };
        self.tree.root = Some(tabs_id);
        self.tree.make_active(|id, _| id == new_id);
        new_id
    }
}

impl Default for MainUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs(
                "main",
                vec![
                    MainTab::Start(StartTab::default()),
                    MainTab::Config(ConfigTab::default()),
                ],
            ),
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
        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
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
            flatten_tabs_in_tabs: true,
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

pub fn show_ui(
    ui: &mut Ui,
    app: &mut App,
    ui_state: &mut UiState,
    delta_time: std::time::Duration,
) {
    ui_state.command_palette.show(ui.ctx(), app);
    app.apply_ui_commands(&mut ui_state.mus);
    Panel::top("top panel").exact_size(32.0).show(ui, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let res = ui.button("File");
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button("Fullscreen").clicked() {
                let is_fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
            }
            #[cfg(target_arch = "wasm32")]
            if ui.button("Fullscreen").clicked() {
                toggle_fullscreen("paiagram_canvas");
            }
            egui::Popup::menu(&res).show(|ui| {
                if ui.button("Import OuDia(Second)").clicked() {
                    info!("trying to read file");
                    let Some(file) = block_on(
                        AsyncFileDialog::new()
                            .add_filter("OuDiaSecond", ImportType::OuDiaSecond.file_extensions())
                            .add_filter("OuDia", ImportType::OuDia.file_extensions())
                            .pick_file(),
                    ) else {
                        warn!("File not picked");
                        return;
                    };
                    info!("File picked");
                    let data = block_on(file.read());
                    info!("Data read");
                    let command = generate_commands(
                        &data,
                        if file.file_name().ends_with("oud2") {
                            ImportType::OuDiaSecond
                        } else {
                            ImportType::OuDia
                        },
                    );
                    if let Err(e) = match command {
                        Ok(cmd) => {
                            if app.apply_command(cmd) {
                                Ok(())
                            } else {
                                Err("Failed to apply stuff".to_string())
                            }
                        }
                        Err(e) => Err(format!("Error while loading command: {:?}", e)),
                    } {
                        warn!("{e}");
                    }
                };
            });
            let res = ui.button(tr!("menu-about"));
            egui::Popup::menu(&res).show(|ui| {
                if ui.button(tr!("menu-documentation")).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(if cfg!(target_arch = "wasm32") {
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
    Panel::bottom("bottom panel").exact_size(24.0).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            let mut time = app.timer.ticks().to_timetable_time();
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::Checkbox::new(&mut app.timer.animation_playing, ""),
            );
            let time_response = ui.add(TimeDragValue(&mut time));
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::DragValue::new(&mut app.timer.animation_speed).fixed_decimals(1).suffix("×"),
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
            app.timer.march(delta_time.as_secs_f64());
        })
    });
    egui::CentralPanel::default().frame(Frame::default()).show(ui, |ui| {
        let mut tab_viewer = MainTabViewer { app };
        ui_state.mus.tree.ui(&mut tab_viewer, ui);
    });
}
