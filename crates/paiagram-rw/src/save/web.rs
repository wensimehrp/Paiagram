//! Autosave storage shared by the native and web builds.

use idb::{DatabaseEvent, Factory, ObjectStoreParams, TransactionMode};
use log::{error, info, warn};

use crate::save::MAX_AUTOSAVES;
use crate::{ExportObject, SaveCategory, paiagram_idb};

/// Save an [`ExportObject`] into IndexedDB.
/// Typically used to store an app save.
///
/// This function keeps the latest [`MAX_AUTOSAVES`] files.
pub fn save_to_indexed_db(mut data: impl ExportObject, category: SaveCategory) {
    // stupid tx rx since worker thread isn't automatically updated
    let (tx, rx) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let _ = match data.write_content(&mut buffer) {
            Ok(()) => tx.send(Some(buffer.into_inner())),
            Err(error) => {
                warn!("Failed to serialize autosave: {error}");
                tx.send(None)
            }
        };
    });
    wasm_bindgen_futures::spawn_local(async move {
        let Ok(Some(bytes)) = rx.await else { return };
        if let Err(error) = _save_to_indexed_db(bytes, category).await {
            error!("Failed to store autosave in IndexedDB: {error}");
        } else {
            info!("Autosaved on {}", jiff::Timestamp::now());
        }
    });
}

/// Save some bytes into IndexedDB.
/// Typically used to store an app save.
async fn _save_to_indexed_db(bytes: Vec<u8>, category: SaveCategory) -> Result<(), idb::Error> {
    let factory = Factory::new()?;
    let mut open_request = factory.open(paiagram_idb::DATABASE, Some(paiagram_idb::VERSION))?;
    open_request.on_upgrade_needed(move |event| {
        let Ok(db) = event.database() else { return };
        let _ = db.create_object_store(SaveCategory::Autosave.to_str(), ObjectStoreParams::new());
        let _ = db.create_object_store(SaveCategory::Save.to_str(), ObjectStoreParams::new());
    });

    let db = open_request.await?;

    let transaction = db.transaction(&[category.to_str()], TransactionMode::ReadWrite)?;
    let store = transaction.object_store(category.to_str())?;

    let timestamp_key = jiff::Timestamp::now().to_string();
    let key = web_sys::wasm_bindgen::JsValue::from_str(&timestamp_key);

    let js_array = web_sys::js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    js_array.copy_from(&bytes);
    let value: web_sys::wasm_bindgen::JsValue = js_array.into();

    store.put(&value, Some(&key))?.await?;

    if !matches!(category, SaveCategory::Autosave) {
        return Ok(());
    }

    let keys = store.get_all_keys(None, None)?.await?;

    if keys.len() > MAX_AUTOSAVES {
        let excess = keys.len() - MAX_AUTOSAVES;
        for old_key in keys.into_iter().take(excess) {
            store.delete(old_key)?;
        }
    }

    Ok(())
}
