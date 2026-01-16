use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use moonshine_core::load::LoadInput;
use moonshine_core::save::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

// output a fixed autosave location, platform dependent
#[cfg(not(target_arch = "wasm32"))]
pub fn storage_file_location(app_id: &str, name: &str) -> Option<PathBuf> {
    eframe::storage_dir(app_id)?.join(name).into()
}

const APP_ID: &str = "paiagramdrawer";
const AUTOSAVE_FILE_NAME: &str = "autosave.paiagram";
#[cfg(not(target_arch = "wasm32"))]
fn autosave_file_location() -> Result<PathBuf, std::io::Error> {
    storage_file_location(APP_ID, AUTOSAVE_FILE_NAME).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Autosave file location not found or could not be created",
        )
    })
}

/// The function to trigger the save, with filters built in.
fn trigger_save(commands: &mut Commands, loader: SaveWorld) {
    let event = loader
        .include_resource::<crate::settings::ApplicationSettings>()
        .include_resource::<crate::interface::UiState>()
        .include_resource::<crate::graph::Graph>();
    commands.trigger_save(event)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn autosave(mut commands: Commands) {
    let path = match autosave_file_location() {
        Ok(path) => path,
        Err(e) => {
            error!("Could not determine autosave file location, {:?}", e);
            return;
        }
    };
    trigger_save(&mut commands, SaveWorld::default_into_file(path.clone()));
    info!("Triggered autosave to {:?}", path);
}

#[cfg(target_arch = "wasm32")]
pub fn autosave(mut commands: Commands) {
    use moonshine_core::save::DefaultSaveFilter;
    trigger_save(
        &mut commands,
        SaveWorld::<DefaultSaveFilter>::new(SaveOutput::Drop),
    );
    info!("Triggered autosave.");
}

// TODO: distinguish between different autosave slots
// TODO: old autosave cleanup and manual saves
#[cfg(target_arch = "wasm32")]
pub fn observe_autosave(save_reader: On<Saved>, registry: Res<AppTypeRegistry>) {
    // serialize the scene to a string
    let registry = registry.read();
    let result = match save_reader.scene.serialize(&registry) {
        Ok(data) => data,
        Err(e) => {
            error!("Could not serialize scene for autosave: {:?}", e);
            return;
        }
    };
    let size = result.len();
    let compressed: Vec<u8> = lz4_flex::block::compress_prepend_size(result.as_bytes());
    info!(
        "Autosave serialized ({} bytes) and compressed to {} bytes ({:2}% of original size)",
        size,
        compressed.len(),
        (compressed.len() as f32 / size as f32) * 100.0
    );
    // store in IndexedDB as blob
    // TODO: handle errors properly
    let task = async move {
        use idb::DatabaseEvent;
        use js_sys::Uint8Array;
        let factory = idb::Factory::new()?;
        let mut open_request = factory.open(APP_ID, Some(1))?;

        open_request.on_upgrade_needed(|event| {
            let Ok(db) = event.database() else {
                error!("Failed to get IndexedDB database during upgrade");
                return;
            };
            if !db.store_names().contains(&AUTOSAVE_FILE_NAME.to_string()) {
                if let Err(e) =
                    db.create_object_store(AUTOSAVE_FILE_NAME, idb::ObjectStoreParams::new())
                {
                    error!("Failed to create object store for autosave: {:?}", e);
                }
            }
        });

        let db = open_request.await?;

        let tx = db.transaction(&[AUTOSAVE_FILE_NAME], idb::TransactionMode::ReadWrite)?;
        let store = tx.object_store(AUTOSAVE_FILE_NAME)?;
        store.put(
            &Uint8Array::from(compressed.as_slice()).into(),
            Some(&wasm_bindgen::JsValue::from_str("latest")),
        )?;
        tx.commit()?.await?;
        info!("Autosave successfully stored in IndexedDB");
        Ok::<(), idb::Error>(())
    };
    bevy::tasks::IoTaskPool::get().spawn(task).detach();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_autosave(mut commands: Commands) {
    match autosave_file_location() {
        Ok(path) => load_save(&mut commands, path),
        Err(e) => {
            error!("Could not determine autosave file location: {:?}", e);
            return;
        }
    };
    info!("Triggered loading of autosave file");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_save(commands: &mut Commands, path: PathBuf) {
    commands.trigger_load(LoadWorld::default_from_file(path.clone()));
    info!("Triggered loading from {:?}", path);
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
pub struct AutosaveLoadTask {
    task: bevy::tasks::Task<Option<Vec<u8>>>,
}

#[cfg(target_arch = "wasm32")]
pub fn load_autosave(mut commands: Commands, task: Option<Res<AutosaveLoadTask>>) {
    if task.is_some() {
        info!("Autosave load already in progress");
        return;
    }

    let task = async move {
        use idb::DatabaseEvent;
        let factory = idb::Factory::new()?;
        let mut open_request = factory.open(APP_ID, Some(1))?;

        open_request.on_upgrade_needed(|event| {
            let Ok(db) = event.database() else {
                error!("Failed to get IndexedDB database during upgrade");
                return;
            };
            if !db.store_names().contains(&AUTOSAVE_FILE_NAME.to_string()) {
                if let Err(e) =
                    db.create_object_store(AUTOSAVE_FILE_NAME, idb::ObjectStoreParams::new())
                {
                    error!("Failed to create object store for autosave: {:?}", e);
                }
            }
        });

        let db = open_request.await?;
        let tx = db.transaction(&[AUTOSAVE_FILE_NAME], idb::TransactionMode::ReadOnly)?;
        let store = tx.object_store(AUTOSAVE_FILE_NAME)?;
        let value = store
            .get(wasm_bindgen::JsValue::from_str("latest"))?
            .await?;
        tx.await?;

        let bytes = value.map(|value| js_sys::Uint8Array::new(&value).to_vec());
        Ok::<Option<Vec<u8>>, idb::Error>(bytes)
    };

    let task = bevy::tasks::IoTaskPool::get().spawn(async move {
        match task.await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to load autosave data from IndexedDB: {:?}", e);
                None
            }
        }
    });
    commands.insert_resource(AutosaveLoadTask { task });
    info!("Triggered loading of autosave from IndexedDB");
}

#[cfg(target_arch = "wasm32")]
pub fn consume_autosave_load_task(
    mut commands: Commands,
    registry: Res<AppTypeRegistry>,
    mut task: ResMut<AutosaveLoadTask>,
) {
    let Some(result) =
        bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task.task))
    else {
        return;
    };

    commands.remove_resource::<AutosaveLoadTask>();

    let Some(bytes) = result else {
        info!("No autosave data found in IndexedDB");
        return;
    };

    let decompressed = match lz4_flex::block::decompress_size_prepended(&bytes) {
        Ok(data) => data,
        Err(err) => {
            error!("Could not decompress autosave data: {:?}", err);
            return;
        }
    };
    let scene_str = match String::from_utf8(decompressed) {
        Ok(s) => s,
        Err(err) => {
            error!("Could not convert autosave data to UTF-8 string: {:?}", err);
            return;
        }
    };
    let registry = registry.read();
    use bevy::scene::serde::SceneDeserializer;
    use ron::de::Deserializer;
    use serde::de::DeserializeSeed;

    let mut de = match Deserializer::from_str(&scene_str) {
        Ok(de) => de,
        Err(err) => {
            error!(
                "Could not create RON deserializer for autosave data: {:?}",
                err
            );
            return;
        }
    };

    let scene = match (SceneDeserializer {
        type_registry: &registry,
    }
    .deserialize(&mut de))
    {
        Ok(scene) => scene,
        Err(err) => {
            error!("Could not deserialize autosave scene: {:?}", err);
            return;
        }
    };

    let mut loader = LoadWorld::default_from_file("");
    loader.input = LoadInput::Scene(scene);
    commands.trigger_load(loader);
    info!("Triggered loading autosave from DynamicScene");
}
