use std::sync::mpsc;

use paiagram_core::World;
use paiagram_core::import::{ImportMode, ImportType, make_world};
use paiagram_rw::LoadResult;
use rfd::AsyncFileDialog;

pub(crate) fn load_file(
    dialog: AsyncFileDialog,
    import_type: ImportType,
    import_mode: ImportMode,
    world_tx: mpsc::Sender<LoadResult<(World, ImportMode)>>,
) {
    let pick_data = async {
        let handle = dialog.pick_file().await;
        if let Some(handle) = handle {
            Some(handle.read().await)
        } else {
            None
        }
    };
    let process = async move {
        #[cfg(debug_assertions)]
        let data = if matches!(import_type, ImportType::BuiltinOud2) {
            Some(Vec::new())
        } else {
            pick_data.await
        };
        #[cfg(not(debug_assertions))]
        let data = pick_data.await;
        let Some(data) = data else {
            let _ = world_tx.send(LoadResult::Aborted);
            return;
        };
        // for some reason egui's Context doesn't implement Send on wasm32. This means it can't be
        // send to the rayon thread.
        // Use a tx rx pair from futures_channel instead.
        rayon::spawn(move || {
            let _ = match make_world(&data, import_type) {
                Ok(world) => world_tx.send(LoadResult::Success((world, import_mode))),
                Err(e) => world_tx.send(LoadResult::Failed(anyhow::anyhow!("{e}"))),
            };
        });
    };
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(process);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = rayon::spawn(move || pollster::block_on(process));
    }
}
