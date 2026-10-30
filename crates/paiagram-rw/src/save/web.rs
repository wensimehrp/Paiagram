//! IndexedDB storage for the web build.

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::Context;
use idb::{DatabaseEvent, Factory, ObjectStoreParams, TransactionMode};
use log::{info, warn};
use web_sys::js_sys::{Array, Uint8Array};
use web_sys::wasm_bindgen::{JsCast, JsValue};
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

use crate::save::MAX_AUTOSAVES;
use crate::{ExportObject, SaveCategory, WriteResult, paiagram_idb};

impl SaveCategory {
    /// Saves `data` into this category's default storage (IndexedDB).
    ///
    /// Only the latest [`MAX_AUTOSAVES`] autosaves are kept.
    pub fn save(self, mut data: impl ExportObject, tx: mpsc::Sender<WriteResult>) {
        let (inner_tx, inner_rx) = futures_channel::oneshot::channel();
        rayon::spawn(move || {
            let _ = inner_tx.send(data.write_to_vec(data.default_buf()));
        });
        wasm_bindgen_futures::spawn_local(async move {
            let result = async || -> anyhow::Result<_> {
                let bytes = inner_rx
                    .await
                    .context("Failed to receive data from channel")?
                    .context("Failed to serialize data to buffer")?;
                save_bytes_to_indexed_db(self, bytes)
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))
                    .context("foo bar")
            };
            let _ = tx.send(match result().await {
                Ok(path) => WriteResult::Success(path),
                Err(e) => WriteResult::Error(e),
            });
        });
    }
}

async fn save_bytes_to_indexed_db(
    category: SaveCategory,
    bytes: Vec<u8>,
) -> Result<PathBuf, idb::Error> {
    let factory = Factory::new()?;
    let mut open_request = factory.open(paiagram_idb::DATABASE, Some(paiagram_idb::VERSION))?;
    open_request.on_upgrade_needed(move |event| {
        let Ok(db) = event.database() else { return };
        let _ = db.create_object_store(
            SaveCategory::Autosave.category_str(),
            ObjectStoreParams::new(),
        );
        let _ = db.create_object_store(
            SaveCategory::Save(None).category_str(),
            ObjectStoreParams::new(),
        );
    });

    let db = open_request.await?;

    let transaction = db.transaction(&[category.category_str()], TransactionMode::ReadWrite)?;
    let store = transaction.object_store(category.category_str())?;
    let key = &category.filename();
    let js_key = JsValue::from_str(key);

    let js_array: Uint8Array = Uint8Array::new_with_length(bytes.len() as u32);
    js_array.copy_from(&bytes);
    let value: JsValue = js_array.into();

    store.put(&value, Some(&js_key))?.await?;

    if matches!(category, SaveCategory::Autosave) {
        let keys = store.get_all_keys(None, None)?.await?;
        if keys.len() > MAX_AUTOSAVES {
            let excess = keys.len() - MAX_AUTOSAVES;
            for old_key in keys.into_iter().take(excess) {
                store.delete(old_key)?;
            }
        }
    }

    Ok(PathBuf::from(key.to_string()))
}

/// Downloads the file in the browser.
///
/// This function must be run in the main thread. As a result, almost all file operations in
/// Paiagram must be completed on the main thread.
pub fn download_file(filename: &str, content: &[u8]) -> Result<(), JsValue> {
    info!("Downloading `{}` ({} bytes)", filename, content.len());
    let anchor: HtmlAnchorElement = {
        let window = web_sys::window().expect("window is not available");
        let document = window.document().expect("document is not available");
        document.create_element("a").unwrap().dyn_into::<HtmlAnchorElement>().unwrap()
    };
    let blob = Blob::new_with_u8_array_sequence_and_options(
        &Array::of1(&Uint8Array::from(&content[..])),
        &{
            let bag = BlobPropertyBag::new();
            bag.set_type("application/octet-stream");
            bag
        },
    )?;

    let url = Url::create_object_url_with_blob(&blob)?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    if let Err(e) = Url::revoke_object_url(&url) {
        warn!("Failed to revoke object URL: {:?}", e);
    }
    Ok(())
}
