use std::sync::mpsc;

use idb::{Database, DatabaseEvent, Factory, ObjectStoreParams, TransactionMode};

use crate::{AvailableSaves, SaveCategory, paiagram_idb};

pub type Result<T> = core::result::Result<T, idb::Error>;

async fn open_database() -> Result<Database> {
    let factory = Factory::new()?;
    let mut request = factory.open(paiagram_idb::DATABASE, Some(paiagram_idb::VERSION))?;
    request.on_upgrade_needed(|event| {
        let Ok(db) = event.database() else { return };
        let _ = db.create_object_store(SaveCategory::Autosave.to_str(), ObjectStoreParams::new());
        let _ = db.create_object_store(SaveCategory::Save.to_str(), ObjectStoreParams::new());
    });
    request.await
}

pub async fn list_saves(category: SaveCategory) -> Result<Vec<String>> {
    let db = open_database().await?;

    let transaction = db.transaction(&[category.to_str()], TransactionMode::ReadOnly)?;
    let store = transaction.object_store(category.to_str())?;

    let keys = store.get_all_keys(None, None)?.await?;

    Ok(keys.into_iter().filter_map(|key| key.as_string()).collect())
}

/// Lists the saves of both categories.
pub fn list_all_saves(tx: mpsc::Sender<Result<AvailableSaves>>) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = async move {
            Ok(AvailableSaves {
                autosaves: list_saves(SaveCategory::Autosave).await?,
                saves: list_saves(SaveCategory::Save).await?,
            })
        };
        let _ = tx.send(result.await);
    });
}

pub async fn load_save(category: SaveCategory, key_str: Option<&str>) -> Result<Option<Vec<u8>>> {
    let db = open_database().await?;

    let transaction = db.transaction(&[category.to_str()], TransactionMode::ReadOnly)?;
    let store = transaction.object_store(category.to_str())?;

    let target_key = match key_str {
        Some(k) => web_sys::wasm_bindgen::JsValue::from_str(k),
        None => {
            let keys = store.get_all_keys(None, None)?.await?;
            let Some(last_key) = keys.into_iter().last() else {
                return Ok(None);
            };
            last_key
        }
    };

    let Some(js_value) = store.get(target_key)?.await? else {
        return Ok(None);
    };

    let uint8_array = web_sys::js_sys::Uint8Array::from(js_value);
    Ok(Some(uint8_array.to_vec()))
}
