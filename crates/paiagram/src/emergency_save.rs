// SPDX-License-Identifier: MPL-2.0
use std::panic::{self, PanicHookInfo};
use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwapOption;
use log::error;
use paiagram_core::{SaveData, World};

/// A backup for the current world. The content stored in this variable would be serialized and
/// stored upon a panic.
static WORLD_BACKUP: ArcSwapOption<World> = ArcSwapOption::const_empty();

/// Updates the backup emergency save at [`WORLD_BACKUP`].
pub fn update_emergency_save(new_world: World) {
    WORLD_BACKUP.store(Some(Arc::new(new_world)));
}

/// Set up the emergency save pipeline
pub fn setup_emergency_save() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        _setup_emergency_save(info);
        default_hook(info);
    }));
}

fn _setup_emergency_save(panic_info: &PanicHookInfo<'_>) {
    error!("An unexpected error occurred. The app has panicked and crashed.");
    error!("Panic info: {:?}", panic_info);

    let Some(world_guard) = WORLD_BACKUP.load_full() else {
        error!("Failed to create backup: no world stored in backup");
        return;
    };

    let world: &World = &world_guard;
    error!("Attempting to create emergency save...");

    #[allow(unused_mut)]
    let mut save = SaveData::from(world.clone());

    #[cfg(target_arch = "wasm32")]
    {
        use paiagram_rw::ExportObject;
        let filename = format!(
            "paiagram-emergency-{}.{}",
            jiff::Timestamp::now().to_string().replace(':', "-"),
            save.extension(),
        );
        match save.write_to_vec(save.default_buf()) {
            Ok(bytes) => match paiagram_rw::save::download_file(&filename, &bytes) {
                Ok(()) => error!("Emergency save downloaded as `{filename}`"),
                Err(e) => error!("Failed to download emergency save: {e:?}"),
            },
            Err(e) => error!("Failed to serialize emergency save: {e}"),
        }
        show_dialog(Path::new(&filename));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use paiagram_rw::{SaveCategory, WriteResult};
        let (tx, rx) = std::sync::mpsc::channel();

        SaveCategory::Emergency.save(save, tx);
        match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(result) => match result {
                WriteResult::Aborted => error!("Write was aborted"),
                WriteResult::Error(e) => error!("Error while writing file: {}", e),
                WriteResult::Success(path) => {
                    error!("Successfully wrote to `{}`", path.display());
                    show_dialog(&path);
                }
            },
            Err(e) => {
                error!("Failed to save file: {}", e)
            }
        }
    }
}

fn show_dialog(path: &Path) {
    let desc = format!(
        "The app has crashed. An emergency save at {} has been created.",
        path.display()
    );
    let title = "Emergency Save";
    cfg_select! {
        target_arch = "wasm32" => wasm_bindgen_futures::spawn_local(async move {
            rfd::AsyncMessageDialog::new()
                .set_title(title)
                .set_description(desc)
                .set_level(rfd::MessageLevel::Error)
                .show()
                .await;
        }),
        _ => {
            rfd::MessageDialog::new()
                .set_title(title)
                .set_description(desc)
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
    }
}
