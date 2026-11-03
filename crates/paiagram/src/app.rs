use std::ops;
use std::sync::mpsc;

use egui::Ui;
use log::{error, info, warn};
use paiagram_core::import::ImportMode;
use paiagram_core::{Source, World};
use paiagram_rw::{LoadResult, WriteResult};
use paiagram_selection::SelectedItems;

use crate::config::{Preferences, Settings};
use crate::timer::GlobalTimer;
use crate::ui_tiles::UiTiles;
use crate::{UiCommand, UiTile};

pub struct App {
    pub(crate) source: Source,
    pub(crate) timer: GlobalTimer,
    pub(crate) preferences: Preferences,
    pub(crate) settings: Settings,
    pub(crate) selected_items: SelectedItems,

    // action and file operation transmitters and receivers
    ui_command_rx: mpsc::Receiver<UiCommand>,
    world_read_rx: mpsc::Receiver<LoadResult<(World, ImportMode)>>,
    file_write_rx: mpsc::Receiver<WriteResult>,
    ui_command_tx: mpsc::Sender<UiCommand>,
    pub world_read_tx: mpsc::Sender<LoadResult<(World, ImportMode)>>,
    pub file_write_tx: mpsc::Sender<WriteResult>,
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
        let (world_read_tx, world_read_rx) = mpsc::channel();
        Self {
            source: Source::new(),
            timer: GlobalTimer::new(),
            preferences: Preferences::new(ctx),
            settings: Settings::default(),
            selected_items: SelectedItems::None,
            ui_command_tx,
            ui_command_rx,
            world_read_tx,
            world_read_rx,
            file_write_tx,
            file_write_rx,
        }
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
                    error!("Failed to write file: {error}");
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
