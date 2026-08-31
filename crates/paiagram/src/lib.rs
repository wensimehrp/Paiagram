// SPDX-License-Identifier: MPL-2.0
//! Definitions for the user interface.

mod command_palette;
mod config;
mod font;
mod selection;
mod tabs;
mod timer;
mod widgets;

use std::sync::{Arc, Mutex};

pub use config::AppLanguage;
use egui::{Button, Color32, Frame, OpenUrl, Panel, Popup, Stroke, Ui};
use egui_i18n::tr;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
use log::{info, warn};
use paiagram_core::import::{ImportType, generate_commands};
use paiagram_core::time::Tick;
use paiagram_core::{Command, SaveFile, Source};
use paiagram_rw::{ExportObject, FileWriteState};
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use tabs::all_tabs::*;
use tabs::{MainTab, Tab, for_all_tabs};
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

use crate::selection::SelectedItems;
use crate::timer::GlobalTimer;
use crate::widgets::TimeDragValue;
pub struct UiPlugin;

fn load_file(dialog: AsyncFileDialog, import_type: ImportType, state: Arc<Mutex<FileLoadState>>) {
    *state.lock().unwrap() = FileLoadState::Reading { progress: None };
    let process = async move {
        let data = dialog.pick_file().await;
        let Some(data) = data else {
            *state.lock().unwrap() = FileLoadState::Idle;
            return;
        };
        *state.lock().unwrap() = FileLoadState::Processing { progress: None };
        let data = data.read().await;
        rayon::spawn(move || {
            let commands = generate_commands(&data, import_type).map_err(|e| e.to_string());
            *state.lock().unwrap() = FileLoadState::Done(commands)
        });
    };
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(process);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = std::thread::spawn(move || pollster::block_on(process));
    }
}

#[derive(Clone)]
enum FileLoadState {
    Idle,
    Reading { progress: Option<f32> },
    Processing { progress: Option<f32> },
    Done(Result<Command, String>),
}

pub struct App {
    source: Source,
    timer: GlobalTimer,
    preferences: config::Preferences,
    settings: config::Settings,
    ui_action_queue: Vec<UiCommand>,
    command_queue: Vec<Command>,
    selected_items: SelectedItems,
    file_load_state: Arc<Mutex<FileLoadState>>,
    file_write_state: Arc<Mutex<FileWriteState>>,
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
            ui_action_queue: Vec::with_capacity(100),
            command_queue: Vec::with_capacity(100),
            selected_items: SelectedItems::None,
            file_load_state: Arc::new(Mutex::new(FileLoadState::Idle)),
            file_write_state: Arc::new(Mutex::new(FileWriteState::Idle)),
        }
    }
    /// Apply UI commands and change the main ui state
    fn apply_ui_commands(&mut self, mus: &mut MainUiState) {
        for cmd in self.ui_action_queue.drain(..) {
            match cmd {
                UiCommand::OpenOrFocus(tab) => mus.open_or_focus(tab),
            }
        }
    }
    /// Clear the command queue and apply queued commands to the source
    fn apply_commands(&mut self) {
        for cmd in self.command_queue.drain(..) {
            // TODO: warn about fails in GUI;
            if !self.source.apply_command(cmd.clone()) {
                warn!("Failed to apply command {:?}", cmd);
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

pub fn show_ui(
    ui: &mut Ui,
    app: &mut App,
    ui_state: &mut UiState,
    delta_time: std::time::Duration,
) {
    ui_state.command_palette.show(ui.ctx(), app);
    app.apply_ui_commands(&mut ui_state.mus);
    app.apply_commands();
    if let Ok(mut cmd) = app.file_load_state.try_lock()
        && matches!(*cmd, FileLoadState::Done(..))
        && let FileLoadState::Done(res) = std::mem::replace(&mut *cmd, FileLoadState::Idle)
    {
        let _result = match res {
            Ok(cmd) => app.source.apply_command(cmd),
            Err(s) => {
                warn!("{s}");
                false
            }
        };
    }
    Panel::top("top panel").exact_size(32.0).show(ui, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            Popup::menu(&ui.button("App")).show(|ui| {
                if ui.add(Button::new("Command Palette").shortcut_text("Ctrl+P")).clicked() {
                    ui_state.command_palette.visible ^= true;
                }
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Toggle Fullscreen").clicked() {
                    let is_fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                }
                #[cfg(target_arch = "wasm32")]
                if ui.button("Toggle Fullscreen").clicked() {
                    let document = web_sys::window().unwrap().document().unwrap();
                    if document.fullscreen_element().is_none() {
                        let _ = document
                            .get_element_by_id("paiagram_canvas")
                            .unwrap()
                            .request_fullscreen();
                    } else {
                        document.exit_fullscreen();
                    }
                }
            });
            Popup::menu(&ui.button("File")).show(|ui| {
                for (button_display, category, import_type) in [
                    ("Import OuDiaSecond", "OuDiaSecond", ImportType::OuDiaSecond),
                    ("Import OuDia", "OuDia", ImportType::OuDia),
                ] {
                    if !ui.button(button_display).clicked() {
                        continue;
                    }
                    info!("Trying to read {category}");
                    let dialog = AsyncFileDialog::new()
                        .set_title(button_display)
                        .add_filter(category, import_type.file_extensions());
                    load_file(dialog, import_type, app.file_load_state.clone());
                }
                if ui.button("Save .paia").clicked() {
                    let new_file: SaveFile = app.source.snap().clone().into();
                    new_file.write_to_file::<true>(app.file_write_state.clone());
                }
            });
            Popup::menu(&ui.button(tr!("menu-about"))).show(|ui| {
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
                if ui.add_enabled(app.source.redoable(), Button::new("Redo")).clicked() {
                    app.source.redo();
                }
                if ui.add_enabled(app.source.undoable(), Button::new("Undo")).clicked() {
                    app.source.undo();
                }
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
            let time = app.timer.ticks().to_timetable_time();
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::Checkbox::new(&mut app.timer.animation_playing, ""),
            );
            let time_response = ui.add(TimeDragValue(time, &mut None));
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
            if app.timer.animation_playing || app.timer.sync_to_real_time {
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
