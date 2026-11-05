use std::ops;
use std::path::PathBuf;
use std::sync::mpsc;

use egui::Ui;
use log::{error, info, warn};
use paiagram_core::import::ImportMode;
use paiagram_core::{SaveData, Source, World};
#[cfg(target_arch = "wasm32")]
use paiagram_rw::SaveCategory;
use paiagram_rw::{ExportObject, LoadResult, WriteResult};
use paiagram_selection::SelectedItems;

use crate::config::{Preferences, Settings};
use crate::timer::GlobalTimer;
use crate::ui_tiles::UiTiles;
use crate::{UiCommand, UiTile};

pub struct App {
    source: Source,
    pub(crate) timer: GlobalTimer,
    pub(crate) preferences: Preferences,
    pub(crate) settings: Settings,
    pub(crate) selected_items: SelectedItems,
    pub(crate) save_path: Option<PathBuf>,

    // action and file operation transmitters and receivers
    ui_command_rx: mpsc::Receiver<UiCommand>,
    world_read_rx: mpsc::Receiver<LoadResult<(World, ImportMode)>>,
    file_write_rx: mpsc::Receiver<WriteResult>,
    save_write_rx: mpsc::Receiver<WriteResult>,
    ui_command_tx: mpsc::Sender<UiCommand>,
    pub world_read_tx: mpsc::Sender<LoadResult<(World, ImportMode)>>,
    pub file_write_tx: mpsc::Sender<WriteResult>,
    save_write_tx: mpsc::Sender<WriteResult>,
}

/// Short definition for accessing the WORLD and SOURCE's data.
impl ops::Deref for App {
    type Target = Source;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

/// Short definition for accessing the SOURCE's data. Namely the [`Source::mutate`] method.
impl ops::DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.source
    }
}

impl App {
    /// Creates a new app instance
    pub fn new(ctx: &egui::Context) -> Self {
        let (ui_command_tx, ui_command_rx) = mpsc::channel();
        let (file_write_tx, file_write_rx) = mpsc::channel();
        let (save_write_tx, save_write_rx) = mpsc::channel();
        let (world_read_tx, world_read_rx) = mpsc::channel();
        Self {
            source: Source::new(),
            timer: GlobalTimer::new(),
            preferences: Preferences::new(ctx),
            settings: Settings::default(),
            selected_items: SelectedItems::None,
            save_path: None,
            ui_command_tx,
            ui_command_rx,
            world_read_tx,
            world_read_rx,
            file_write_tx,
            file_write_rx,
            save_write_tx,
            save_write_rx,
        }
    }

    /// Gives the underlying world
    pub(crate) fn world(&self) -> &World {
        self.source.world()
    }

    /// Method to prevent directly calling .clone() on app
    #[deprecated(note = "Use app.world().clone() instead")]
    pub(crate) fn clone(&self) -> ! {
        unreachable!()
    }

    /// Saves the current world to the stored path, or prompts for a location if none is set.
    pub(crate) fn save(&mut self) {
        let save_data = SaveData::from(self.world().clone());
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.save_path.clone() {
            save_data.write_to_path(path, self.save_write_tx.clone());
        } else {
            save_data.write_via_interface(self.save_write_tx.clone());
        }
        #[cfg(target_arch = "wasm32")]
        {
            let name = self.save_path.as_ref().and_then(|p| p.to_str()).map(str::to_owned);
            SaveCategory::Save(name).save(save_data, self.save_write_tx.clone());
        }
    }
    /// Saves the current world to a new location.
    pub(crate) fn save_as(&mut self) {
        let save_data = SaveData::from(self.world().clone());
        #[cfg(not(target_arch = "wasm32"))]
        save_data.write_via_interface(self.save_write_tx.clone());
        #[cfg(target_arch = "wasm32")]
        save_data.write_via_interface(self.file_write_tx.clone());
    }
    /// Initializes the custom WGPU resources used by the Marey tile's render callback.
    ///
    /// Call once at startup, once the WGPU backend exists. `msaa_samples` must match the render
    /// pass egui uses (eframe's `NativeOptions::multisampling`).
    pub fn init_gpu(&mut self, render_state: &eframe::egui_wgpu::RenderState, msaa_samples: u32) {
        crate::tiles::marey::gpu_draw::init(render_state, msaa_samples);
        crate::tiles::graph::gpu_draw::init(render_state, msaa_samples);
    }
    /// Apply UI commands and change the main ui state
    pub(super) fn begin_frame(&mut self, ui: &mut Ui, mus: &mut UiTiles) {
        puffin::profile_function!();
        ui.set_theme(self.preferences.theme_preference);
        self.source.begin_frame();
        for cmd in self.ui_command_rx.try_iter() {
            match cmd {
                UiCommand::OpenOrFocus(tile) => mus.open_or_focus(tile),
            }
        }
        for result in self.file_write_rx.try_iter() {
            ui.request_repaint();
            match result {
                WriteResult::Success(path) => info!("Saved `{}`", path.display()),
                WriteResult::Error(error) => {
                    error!("Failed to write file: {error:#}");
                }
                WriteResult::Aborted => {
                    warn!("Write operation aborted")
                }
            }
        }
        for result in self.save_write_rx.try_iter() {
            ui.request_repaint();
            match result {
                WriteResult::Success(path) => {
                    info!("Saved `{}`", path.display());
                    self.save_path = Some(path);
                }
                WriteResult::Error(error) => {
                    error!("Failed to write file: {error:#}");
                }
                WriteResult::Aborted => {
                    warn!("Write operation aborted")
                }
            }
        }
        for result in self.world_read_rx.try_iter() {
            ui.request_repaint();
            puffin::profile_scope!("file_load");
            match result {
                LoadResult::Aborted => {
                    info!("Load aborted");
                }
                LoadResult::Success((new_world, mode)) => {
                    let _ = self.source.import(new_world, mode);
                }
                LoadResult::Failed(e) => {
                    error!("Failed to load world: {e}");
                }
            }
        }
    }
    /// Send a UI command to the UI command queue.
    fn send_ui_cmd(&self, cmd: UiCommand) {
        self.ui_command_tx
            .send(cmd)
            .map_err(|e| format!("Failed to send UI message to app: {e}"))
            .unwrap();
    }
    /// Open or focus a tile. Shortcut for most tile-opening operations
    pub(crate) fn open_or_focus(&self, tile: impl Into<UiTile>) {
        self.send_ui_cmd(UiCommand::OpenOrFocus(tile.into()));
    }
}
