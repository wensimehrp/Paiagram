use std::sync::mpsc;

use idb::{Database, DatabaseEvent, Factory, ObjectStoreParams, TransactionMode};
use web_sys::js_sys::Uint8Array;
use web_sys::wasm_bindgen::JsValue;

use crate::{AvailableSaves, SaveCategory, paiagram_idb};

pub type Result<T> = core::result::Result<T, idb::Error>;

async fn open_database() -> Result<Database> {
    let factory = Factory::new()?;
    let mut request = factory.open(paiagram_idb::DATABASE, Some(paiagram_idb::VERSION))?;
    request.on_upgrade_needed(|event| {
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
    request.await
}

impl SaveCategory {
    /// Lists the saves of this category.
    pub async fn list_files(&self) -> Result<Vec<String>> {
        let db = open_database().await?;

        let transaction = db.transaction(&[self.category_str()], TransactionMode::ReadOnly)?;
        let store = transaction.object_store(self.category_str())?;

        let keys = store.get_all_keys(None, None)?.await?;

        Ok(keys.into_iter().filter_map(|key| key.as_string()).collect())
    }
    /// Read this save from this category
    pub async fn load_file(&self, filename: &str) -> Result<Vec<u8>> {
        let db = open_database().await?;

        let transaction = db.transaction(&[self.category_str()], TransactionMode::ReadOnly)?;
        let store = transaction.object_store(self.category_str())?;

        let target_key = JsValue::from_str(filename);

        Ok(store
            .get(target_key)?
            .await?
            .map(Uint8Array::from)
            .map(|arr| arr.to_vec())
            .unwrap_or_default())
    }
}

/// Lists the saves of both categories.
pub fn list_all_saves(tx: mpsc::Sender<Result<AvailableSaves>>) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = async move {
            Ok(AvailableSaves {
                autosaves: SaveCategory::Autosave.list_files().await?,
                saves: SaveCategory::Save(None).list_files().await?,
            })
        };
        let _ = tx.send(result.await);
    });
}
