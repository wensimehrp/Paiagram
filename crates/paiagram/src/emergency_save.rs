// SPDX-License-Identifier: MPL-2.0
use std::panic::{self, PanicHookInfo};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use arc_swap::ArcSwapOption;
use paiagram_core::{SaveData, World};
use paiagram_rw::{SaveCategory, WriteResult};

/// A backup for the current world. The content stored in this variable would be serialized and
/// stored upon a panic.
static WORLD_BACKUP: ArcSwapOption<World> = ArcSwapOption::const_empty();

pub fn update_emergency_save(new_world: World) {
    WORLD_BACKUP.store(Some(Arc::new(new_world)));
}

pub fn setup_emergency_save() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        _setup_emergency_save(info);
        default_hook(info);
    }));
}

fn _setup_emergency_save(panic_info: &PanicHookInfo<'_>) {
    eprintln!("An unexpected error occurred. The app has panicked and crashed.");
    eprintln!("Panic info: {:?}", panic_info);

    // Lock-free load from global backup
    let Some(world_guard) = WORLD_BACKUP.load_full() else {
        eprintln!("Failed to create backup: no world stored in backup");
        return;
    };

    let world: &World = &world_guard;
    let now = format!("{}_emergency_save", jiff::Timestamp::now());
    eprintln!("Attempting to create save `{now}`...");

    let save = SaveData::from((*world).clone());
    let (tx, rx) = mpsc::channel();
    SaveCategory::Emergency.save(save, tx);
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(result) => match result {
            WriteResult::Aborted => eprintln!("Write was aborted"),
            WriteResult::Error(e) => eprintln!("Error while writing file: {}", e),
            WriteResult::Success(path) => {
                eprintln!("Successfully wrote to `{}`", path.display())
            }
        },
        Err(e) => {
            eprintln!("Failed to save file: {}", e)
        }
    };
}
