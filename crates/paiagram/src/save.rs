// SPDX-License-Identifier: MPL-2.0
//! Save/load functionality using the core's serialization.

use paiagram_core::{SaveFile, Source, WorldSnapshot};

pub(crate) fn save(app: &super::tabs::AppState, filename: String) {
    // Source implements Deref<Target = WorldSnapshot>, clone the snapshot for saving
    let world: WorldSnapshot = (&*app.source).clone();
    paiagram_rw::save::serialize_compressed_cbor(SaveFile::V1 { world }, filename);
}

pub(crate) fn save_ron(app: &super::tabs::AppState, filename: String) {
    let world: WorldSnapshot = (&*app.source).clone();
    paiagram_rw::save::serialize_ron(SaveFile::V1 { world }, filename);
}

pub(crate) fn apply_loaded_scene(app: &mut super::tabs::AppState, bytes: &[u8]) -> Result<(), String> {
    let save: SaveFile = cbor4ii::serde::from_slice(bytes).map_err(|e| format!("CBOR decode error: {e}"))?;
    let source = Source::try_from(save).map_err(|e| format!("Save format error: {e}"))?;
    app.source = source;
    Ok(())
}

pub(crate) fn apply_loaded_scene_ron(app: &mut super::tabs::AppState, content: &str) -> Result<(), String> {
    let save: SaveFile = ron::from_str(content).map_err(|e| format!("RON decode error: {e}"))?;
    let source = Source::try_from(save).map_err(|e| format!("Save format error: {e}"))?;
    app.source = source;
    Ok(())
}
