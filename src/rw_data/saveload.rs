use bevy::prelude::*;
use moonshine_core::save::prelude::*;
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

#[cfg(not(target_arch = "wasm32"))]
pub fn autosave(mut commands: Commands) {
    let path = match autosave_file_location() {
        Ok(path) => path,
        Err(e) => {
            error!("Could not determine autosave file location, {:?}", e);
            return;
        }
    };
    commands.trigger_save(
        SaveWorld::default_into_file(path.clone())
            .include_resource::<crate::settings::ApplicationSettings>()
            .include_resource::<crate::interface::UiState>()
            .include_resource::<crate::graph::Graph>(),
    );
    info!("Triggered autosave to {:?}", path);
}

#[cfg(target_arch = "wasm32")]
pub fn autosave(mut commands: Commands) {
    error!("Autosave is not supported on wasm32 targets");
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
pub fn load_autosave(mut _commands: Commands) {
    error!("Autosave loading is not supported on wasm32 targets");
}

#[cfg(target_arch = "wasm32")]
pub fn load_save(_commands: &mut Commands, _path: PathBuf) {
    error!("Loading from file is not supported on wasm32 targets");
}
