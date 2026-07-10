// SPDX-License-Identifier: MPL-2.0
//! Save/load functionality using the core's serialization.

use std::io::Read;
use std::sync::Mutex;

use paiagram_core::{SaveFile, Source, WorldSnapshot};
use serde::{Deserialize, Serialize};

use crate::MainUiState;
use crate::tabs::AppState;

/// Complete save data including UI state.
#[derive(Serialize, Deserialize)]
pub(crate) struct SaveData {
    pub(crate) world: SaveFile,
    pub(crate) main_ui: MainUiState,
    pub(crate) project_settings: paiagram_core::settings::ProjectSettings,
}

pub(crate) fn save(app: &AppState, filename: String) {
    let world: WorldSnapshot = (&*app.source).clone();
    let data = SaveData {
        world: SaveFile::V1 { world },
        main_ui: app.main_ui.clone(),
        project_settings: app.project_settings.clone(),
    };
    paiagram_rw::save::serialize_compressed_cbor(data, filename);
}

pub(crate) fn save_ron(app: &AppState, filename: String) {
    let world: WorldSnapshot = (&*app.source).clone();
    let data = SaveData {
        world: SaveFile::V1 { world },
        main_ui: app.main_ui.clone(),
        project_settings: app.project_settings.clone(),
    };
    paiagram_rw::save::serialize_ron(data, filename);
}

/// Load a .paia file from the given bytes (LZ4-compressed CBOR).
pub(crate) fn apply_loaded_scene(app: &mut AppState, bytes: &[u8]) -> Result<(), String> {
    let mut decoder = lz4_flex::frame::FrameDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|e| format!("LZ4 decompress error: {e}"))?;
    let data: SaveData = cbor4ii::serde::from_slice(&decompressed).map_err(|e| format!("CBOR decode error: {e}"))?;
    // Restore world
    let source = Source::try_from(data.world).map_err(|e| format!("Save format error: {e}"))?;
    app.source = source;
    app.main_ui = data.main_ui;
    app.project_settings = data.project_settings;
    Ok(())
}

/// Thread-safe storage for a loaded scene file.
static LOADED_FILE: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Spawn a thread to pick and read a .paia file.
pub(crate) fn spawn_load_thread() {
    std::thread::spawn(|| {
        let future = rfd::AsyncFileDialog::new()
            .add_filter("Paiagram Savefiles", &["paia"])
            .pick_file();
        let file = match futures_lite::future::block_on(future) {
            Some(f) => f,
            None => return,
        };
        let bytes = futures_lite::future::block_on(file.read());
        *LOADED_FILE.lock().unwrap() = Some(bytes);
    });
}

/// Check if a file has been loaded by the thread. Called from process_pending_tabs.
pub(crate) fn take_loaded_file() -> Option<Vec<u8>> {
    LOADED_FILE.lock().unwrap().take()
}
