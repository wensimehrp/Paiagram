// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

mod command_palette;
mod config;
mod emergency_save;
mod font;
mod human_time;
mod load;
mod tabs;
mod timer;
mod widgets;

use std::sync::{Arc, mpsc};

pub use config::AppLanguage;
use egui::{Button, DragValue, Frame, Id, OpenUrl, Panel, Popup, ScrollArea, Ui};
use egui_i18n::tr;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
pub use emergency_save::{setup_emergency_save, update_emergency_save};
use log::{error, info};
use paiagram_core::import::{ImportMode, ImportType};
use paiagram_core::time::TimetableTime;
use paiagram_core::{RouteKey, SaveData, Source};
use paiagram_export::{ExportGtfs, ExportOuDia};
use paiagram_rw::{ExportObject, WriteResult};
use paiagram_selection::SelectedItems;
use parking_lot::Mutex;
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use tabs::all_tabs::*;
use tabs::{MainTab, Tab, for_all_tabs};
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;
pub use widgets::search::build_matcher;

use crate::command_palette::CommandPalette;
use crate::load::FileLoadState;
use crate::timer::GlobalTimer;

pub struct App {
    source: Source,
    timer: GlobalTimer,
    preferences: config::Preferences,
    settings: config::Settings,
    ui_action_queue: Vec<UiCommand>,
    selected_items: SelectedItems,
    file_load_state: Arc<Mutex<FileLoadState>>,
    pub file_write_tx: mpsc::Sender<WriteResult>,
    file_write_rx: mpsc::Receiver<WriteResult>,
}

#[derive(Default)]
pub struct UiState {
    command_palette: CommandPalette,
    mus: MainUiState,
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        let (file_write_tx, file_write_rx) = mpsc::channel();
        Self {
            source: Source::new(),
            timer: GlobalTimer::new(),
            preferences: config::Preferences::new(ctx),
            settings: config::Settings::default(),
            ui_action_queue: Vec::with_capacity(100),
            selected_items: SelectedItems::None,
            file_load_state: Arc::new(Mutex::new(FileLoadState::Idle)),
            file_write_tx,
            file_write_rx,
        }
    }
    /// Initializes the custom WGPU resources used by the Marey tab's render callback.
    ///
    /// Call once at startup, once the WGPU backend exists. `msaa_samples` must match the render
    /// pass egui uses (eframe's `NativeOptions::multisampling`).
    pub fn init_gpu(&mut self, render_state: &eframe::egui_wgpu::RenderState, msaa_samples: u32) {
        crate::tabs::marey::gpu_draw::init(render_state, msaa_samples);
        crate::tabs::graph::gpu_draw::init(render_state, msaa_samples);
    }
    /// Apply UI commands and change the main ui state
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

#[derive(Serialize, Deserialize)]
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
                Id::unique("main"),
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

impl<'w> Behavior<MainTab> for MainTabViewer<'w> {
    fn tab_title_for_pane(&mut self, pane: &MainTab) -> egui::WidgetText {
        for_all_tabs!(pane, p, p.title())
    }
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, tab: &mut MainTab) -> UiResponse {
        puffin::profile_function!();
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
    fn tab_bar_trailing_ui(
        &mut self,
        _tiles: &Tiles<MainTab>,
        ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
    ) {
        // wtf???
        let mut ui_action_queue = std::mem::take(&mut self.app.ui_action_queue);
        ui.menu_button("+", |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                #[rustfmt::skip]
                let tabs: [(_, fn() -> MainTab); 6] = [
                    ("Intervals",       || MainTab::Intervals(IntervalsTab::default())),
                    ("Service Classes", || MainTab::ServiceClasses(ServiceClassesTab::default())),
                    ("Graph",           || MainTab::Graph(GraphTab::default())),
                    ("Extension",       || MainTab::Extension(ExtensionTab::default())),
                    ("Config",          || MainTab::Config(ConfigTab::default())),
                    ("Trip",            || MainTab::Trip(TripTab::default())),
                ];
                for (text, tab) in tabs {
                    if ui.button(text).clicked() {
                        ui_action_queue.push(UiCommand::OpenOrFocus(tab()));
                    }
                }
                if self.app.preferences.dev_mode && ui.button("Profiler").clicked() {
                    ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Profiler(
                        ProfilerTab::default(),
                    )));
                }
                ScrollArea::vertical().show_rows(
                    ui,
                    ui.spacing().interact_size.y,
                    self.app.world.routes.len(),
                    |ui, range| {
                        for (route_key, info) in
                            &self.app.world.routes.skip(range.start).take(range.count())
                        {
                            ui.menu_button(info.name.as_str(), |ui| {
                                if ui.button("Graph").clicked() {
                                    ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Marey(
                                        MareyTab::new(*route_key),
                                    )));
                                }
                                if ui.button("Route Timetable").clicked() {
                                    ui_action_queue.push(UiCommand::OpenOrFocus(
                                        MainTab::RouteTimetable(RouteTimetableTab::new(*route_key)),
                                    ));
                                }
                            });
                        }
                    },
                );
            })
        });
        self.app.ui_action_queue = ui_action_queue;
    }
}

pub fn show_ui(
    ui: &mut Ui,
    app: &mut App,
    ui_state: &mut UiState,
    delta_time: std::time::Duration,
) {
    puffin::GlobalProfiler::lock().new_frame();
    puffin::profile_function!();
    app.source.begin_frame();
    while let Ok(result) = app.file_write_rx.try_recv() {
        match result {
            WriteResult::Success(path) => info!("Saved `{}`", path.display()),
            WriteResult::Error(error) => {
                error!("Failed to write file: {error}");
            }
            WriteResult::Aborted => {}
        }
    }
    {
        puffin::profile_scope!("command_palette");
        ui_state.command_palette.show(ui.ctx(), app);
    }
    {
        puffin::profile_scope!("apply_ui_commands");
        app.apply_ui_commands(&mut ui_state.mus);
    }
    if let Some(mut lock) = app.file_load_state.try_lock() {
        puffin::profile_scope!("file_load");
        match &*lock {
            FileLoadState::Idle => {}
            FileLoadState::Processing => {
                egui::Modal::new(Id::unique("write modal")).show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Loading data...");
                        ui.spinner();
                    })
                });
            }
            FileLoadState::Done(_) => {
                let FileLoadState::Done(world) = std::mem::take(&mut *lock) else {
                    unreachable!();
                };
                match world {
                    Ok((new_world, mode)) => {
                        let _ = app.source.import(new_world, mode);
                    }
                    Err(s) => {}
                }
            }
        }
    }
    Panel::top("top panel").exact_size(32.0).show(ui, |ui| {
        puffin::profile_scope!("top_panel");
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
                    ("Import qETRC/pyETRC", "pyetgr", ImportType::Pyetgr),
                    ("Import GTFS", "GTFS", ImportType::Gtfs),
                    ("Import .paia", "Paiagram", ImportType::PaiagramPaia),
                    #[cfg(debug_assertions)]
                    ("Import sample.oud2", "OuDiaSecond", ImportType::BuiltinOud2),
                ] {
                    ui.menu_button(button_display, |ui| {
                        for (label, import_mode) in [
                            ("Add to current world", ImportMode::Merge),
                            ("Replace current world", ImportMode::Replace),
                        ] {
                            if !ui.button(label).clicked() {
                                continue;
                            }
                            info!("Trying to read {category}");
                            let dialog = AsyncFileDialog::new()
                                .set_title(button_display)
                                .add_filter(category, import_type.file_extensions());
                            load::load_file(
                                dialog,
                                import_type,
                                import_mode,
                                app.file_load_state.clone(),
                                ui.ctx().clone(),
                            );
                            ui.close();
                        }
                    });
                }
                ui.separator();
                if ui.button("Save .paia").clicked() {
                    let new_file = SaveData::from(app.world.clone());
                    new_file.write_via_interface(app.file_write_tx.clone());
                }
                ui.separator();
                if ui.button("Export .oud").clicked() {
                    ExportOuDia {
                        world: app.world.clone(),
                        route: RouteKey::new(),
                        is_oudia_second: false,
                    }
                    .write_via_interface(app.file_write_tx.clone());
                }
                if ui.button("Export GTFS").clicked() {
                    ExportGtfs {
                        world: app.world.clone(),
                    }
                    .write_via_interface(app.file_write_tx.clone());
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
                ui.menu_button("History", |ui| {
                    ScrollArea::vertical().show_rows(
                        ui,
                        ui.spacing().interact_size.y,
                        app.history().len(),
                        |ui, range| {
                            for (idx, _) in app.history()[range].iter().enumerate() {
                                ui.button(idx.to_string());
                            }
                        },
                    )
                });
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
        puffin::profile_scope!("bottom_panel");
        ui.horizontal_centered(|ui| {
            let mut time = TimetableTime(app.timer.seconds() as i32);
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::Checkbox::new(&mut app.timer.animation_playing, ""),
            );
            let time_response = ui.add(DragValue::new(&mut time));
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
                app.timer.update_seconds(time.0 as f64, &key);
                app.timer.unlock(key);
            }
            if app.timer.animation_playing || app.timer.sync_to_real_time {
                ui.ctx().request_repaint();
            }
            app.timer.march(delta_time.as_secs_f64());
        })
    });
    egui::CentralPanel::default().frame(Frame::default()).show(ui, |ui| {
        puffin::profile_scope!("main_tabs");
        let mut tab_viewer = MainTabViewer { app };
        ui_state.mus.tree.ui(&mut tab_viewer, ui);
    });
}
