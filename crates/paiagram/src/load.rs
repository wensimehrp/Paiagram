use std::sync::Arc;

use paiagram_core::WorldSnapshot;
use paiagram_core::import::{ImportType, make_snapshot};
use parking_lot::Mutex;
use rfd::AsyncFileDialog;

#[derive(Clone, Default)]
pub(crate) enum FileLoadState {
    #[default]
    Idle,
    Processing,
    Done(Result<WorldSnapshot, String>),
}

pub(crate) fn load_file(
    dialog: AsyncFileDialog,
    import_type: ImportType,
    state: Arc<Mutex<FileLoadState>>,
    ctx: egui::Context,
) {
    *state.lock() = FileLoadState::Processing;
    let process = async move {
        let data = if cfg_select! {
            debug_assertions => matches!(import_type, ImportType::BuiltinOud2),
            _ => false,
        } {
            Vec::new()
        } else {
            let data = dialog.pick_file().await;
            let Some(data) = data else {
                *state.lock() = FileLoadState::Idle;
                return;
            };
            *state.lock() = FileLoadState::Processing;
            data.read().await
        };
        let (tx, rx) = futures_channel::oneshot::channel();
        // for some reason egui's Context doesn't implement Send on wasm32. This means it can't be
        // send to the rayon thread.
        // Use a tx rx pair from futures_channel instead.
        rayon::spawn(move || {
            let commands = make_snapshot(&data, import_type).map_err(|e| e.to_string());
            *state.lock() = FileLoadState::Done(commands);
            let _ = tx.send(());
        });
        let _ = rx.await;
        ctx.request_repaint();
    };
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(process);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = std::thread::spawn(move || pollster::block_on(process));
    }
}
