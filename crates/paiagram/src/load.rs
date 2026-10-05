use std::sync::Arc;

use paiagram_core::WorldSnapshot;
use paiagram_core::import::{ImportType, make_snapshot};
use parking_lot::Mutex;
use pollster::block_on;
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
    rayon::spawn(move || {
        *state.lock() = FileLoadState::Processing;
        let data = if cfg_select! {
            debug_assertions => matches!(import_type, ImportType::BuiltinOud2),
            _ => false,
        } {
            Vec::new()
        } else {
            let data = block_on(dialog.pick_file());
            let Some(data) = data else {
                *state.lock() = FileLoadState::Idle;
                return;
            };
            *state.lock() = FileLoadState::Processing;
            block_on(data.read())
        };
        let new_world = make_snapshot(&data, import_type).map_err(|e| e.to_string());
        *state.lock() = FileLoadState::Done(new_world);
        ctx.request_repaint();
    });
}
