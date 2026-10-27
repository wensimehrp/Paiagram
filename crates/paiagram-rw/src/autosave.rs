//! Autosave storage shared by the native and web builds.

use log::{info, warn};

/// Creates a timestamped `.paia` file in `dir`, lets `write` fill it,
/// and keeps only the newest ten files.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_to_dir(
    dir: impl AsRef<std::path::Path>,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    const MAX_AUTOSAVES: usize = 10;
    let dir = dir.as_ref();

    std::fs::create_dir_all(dir)?;
    let timestamp = jiff::Timestamp::now();
    let filename = format!("{}.paia", timestamp.strftime("%Y%m%dT%H%M%S%3fZ"));
    let path = dir.join(filename.clone());
    let mut file = std::fs::File::create(&path)?;
    if let Err(error) = write(&mut file) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    file.sync_all()?;

    info!("Autosaved {}", path.display());

    // Timestamped file names sort chronologically, so the first entries are the oldest.
    let mut autosaves: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
        .collect();
    autosaves.sort();
    let excess = autosaves.len().saturating_sub(MAX_AUTOSAVES);
    for path in autosaves.into_iter().take(excess) {
        if let Err(error) = std::fs::remove_file(&path) {
            warn!(
                "Failed to remove old autosave `{}`: {error}",
                path.display()
            );
        }
    }
    Ok(())
}

/// Stores `bytes` as the latest autosave in IndexedDB.
///
/// IndexedDB is available in web workers, so this can be called from a rayon thread.
#[cfg(target_arch = "wasm32")]
pub fn save_to_indexed_db(bytes: Vec<u8>) {
    use idb::{DatabaseEvent, Factory, ObjectStoreParams, Request};

    const DATABASE: &str = "paiagram";
    const VERSION: u32 = 1;
    const STORE: &str = "autosaves";

    let result = (|| -> Result<(), idb::Error> {
        let mut request = Factory::new()?.open(DATABASE, Some(VERSION))?;

        request.on_upgrade_needed(|event| {
            let Ok(database) = event.database() else {
                return;
            };
            let _ = database.create_object_store(STORE, ObjectStoreParams::new());
        });

        request.on_success(move |event| {
            let Ok(database) = event.database() else {
                log::warn!("IndexedDB open request succeeded without a database");
                return;
            };
            if let Err(error) = put(&database, &bytes) {
                log::warn!("Failed to store autosave in IndexedDB: {error}");
            }
        });

        request.on_error(|_| log::warn!("Failed to open the IndexedDB database"));

        // The request wrapper owns the callbacks and is dropped when we return.
        request.forget_callbacks();
        Ok(())
    })();

    if let Err(error) = result {
        log::warn!("Failed to store autosave in IndexedDB: {error}");
    }
}

#[cfg(target_arch = "wasm32")]
fn put(database: &idb::Database, bytes: &[u8]) -> Result<(), idb::Error> {
    use idb::{Request, StoreEvent, TransactionMode};

    const STORE: &str = "autosaves";
    const KEY: &str = "latest";

    let transaction = database.transaction(&[STORE], TransactionMode::ReadWrite)?;
    let store = transaction.object_store(STORE)?;
    let value: web_sys::wasm_bindgen::JsValue = web_sys::js_sys::Uint8Array::from(bytes).into();
    let key = web_sys::wasm_bindgen::JsValue::from_str(KEY);
    let mut request = store.put(&value, Some(&key))?;
    request.on_error(|event| {
        log::warn!("Failed to store autosave in IndexedDB: {}", event.error());
    });
    request.forget_callbacks();
    Ok(())
}
