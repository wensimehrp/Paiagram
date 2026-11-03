use std::sync::{Arc, mpsc};

use egui::*;
use paiagram_extensions::config_ui::{
    ConfigUi, ConfigUiList, ReturnValueMap, default_return_values,
};
use paiagram_extensions::{
    ByteCodes, ConfigRunningStatus, JsDataExporter, JsIo, JsRuntimeData, ScriptRunningStatus,
    compile_script, eval_config, run_script,
};
use paiagram_rw::ExportObject;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::Tile;

#[derive(Serialize, Deserialize)]
#[serde(into = "ExtensionTileNoCache", from = "ExtensionTileNoCache")]
pub struct ExtensionTile {
    script_text: String,
    // wasm only allows saving file on the main thread
    bytecodes: Option<paiagram_extensions::Result<ByteCodes>>,
    file_data_rx: mpsc::Receiver<JsDataExporter>,
    file_data_tx: mpsc::Sender<JsDataExporter>,
    bytecodes_rx: mpsc::Receiver<paiagram_extensions::Result<ByteCodes>>,
    bytecodes_tx: mpsc::Sender<paiagram_extensions::Result<ByteCodes>>,
    edit_mode: bool,
    config_running_status: Arc<Mutex<ConfigRunningStatus>>,
    script_running_status: Arc<Mutex<ScriptRunningStatus>>,
    config_ui: Option<Result<Vec<ConfigUi>, String>>,
    return_value_map: ReturnValueMap,
    script_eval_err: Option<String>,
}

impl ExtensionTile {
    pub fn new(script_text: String) -> Self {
        let (file_data_tx, file_data_rx) = mpsc::channel();
        let (bytecodes_tx, bytecodes_rx) = mpsc::channel();
        compile_script(script_text.as_str().into(), bytecodes_tx.clone());
        Self {
            script_text,
            file_data_rx,
            file_data_tx,
            bytecodes: None,
            bytecodes_rx,
            bytecodes_tx,
            edit_mode: false,
            config_running_status: Default::default(),
            script_running_status: Default::default(),
            config_ui: None,
            return_value_map: Default::default(),
            script_eval_err: None,
        }
    }
}

impl Clone for ExtensionTile {
    fn clone(&self) -> Self {
        ExtensionTileNoCache {
            script_text: self.script_text.clone(),
        }
        .into()
    }
}

impl Default for ExtensionTile {
    fn default() -> Self {
        Self::new(include_str!("../../../../extensions/export_focused.js").into())
    }
}

#[derive(Serialize, Deserialize)]
struct ExtensionTileNoCache {
    script_text: String,
}

impl From<ExtensionTile> for ExtensionTileNoCache {
    fn from(value: ExtensionTile) -> Self {
        Self {
            script_text: value.script_text,
        }
    }
}

impl From<ExtensionTileNoCache> for ExtensionTile {
    fn from(value: ExtensionTileNoCache) -> Self {
        Self::new(value.script_text)
    }
}

impl PartialEq for ExtensionTile {
    fn eq(&self, other: &Self) -> bool {
        let (Some(Ok(self_bc)), Some(Ok(other_bc))) = (&self.bytecodes, &other.bytecodes) else {
            return false;
        };
        self_bc.metadata.id == other_bc.metadata.id
    }
}

impl Tile for ExtensionTile {
    const NAME: &'static str = "Extension";
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        puffin::profile_function!();
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            main_display(self, app, ui);
        });
    }
}

fn main_display(tile: &mut ExtensionTile, app: &mut crate::App, ui: &mut Ui) {
    puffin::profile_function!();
    if let Ok(data) = tile.file_data_rx.try_recv() {
        data.write_via_interface(app.file_write_tx.clone());
    }
    if let Ok(bytecodes) = tile.bytecodes_rx.try_recv() {
        tile.bytecodes = Some(bytecodes);
        tile.config_ui = None;
    }
    let edit_changed = ui.checkbox(&mut tile.edit_mode, "Edit mode").changed();
    if tile.edit_mode {
        let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut layout_job = egui_extras::syntax_highlighting::highlight(
                ui.ctx(),
                ui.style(),
                &theme,
                buf.as_str(),
                "js",
            );
            layout_job.wrap.max_width = wrap_width;
            ui.fonts_mut(|f| f.layout_job(layout_job))
        };
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let res = ui.add(
                TextEdit::multiline(&mut tile.script_text)
                    .code_editor()
                    .layouter(&mut layouter)
                    .desired_width(f32::INFINITY),
            );
            if res.changed() {
                tile.bytecodes = None;
                tile.config_ui = None;
            }
        });
        return;
    }
    if edit_changed && tile.bytecodes.is_none() {
        compile_script(tile.script_text.as_str().into(), tile.bytecodes_tx.clone());
    }
    let Some(bytecodes) = &tile.bytecodes else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Compiling script...");
        });
        return;
    };
    let bytecodes = match bytecodes {
        Ok(bytecodes) => bytecodes,
        Err(err) => {
            ui.label("Error while compiling script:");
            ui.label(err.to_string());
            return;
        }
    };
    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked()
            || (tile.config_ui.is_none()
                && matches!(
                    tile.config_running_status.try_lock(),
                    Some(inner) if matches!(*inner, ConfigRunningStatus::Idle)
                ))
        {
            eval_config(
                JsRuntimeData {
                    world: app.world.clone(),
                    selected_items: app.selected_items.clone(),
                },
                tile.config_running_status.clone(),
                bytecodes.clone(),
            );
        }
        if ui.button("Run script").clicked() {
            run_script(
                JsRuntimeData {
                    world: app.world.clone(),
                    selected_items: app.selected_items.clone(),
                },
                JsIo {
                    file_data_tx: tile.file_data_tx.clone(),
                },
                tile.return_value_map.clone(),
                tile.script_running_status.clone(),
                bytecodes.clone(),
            );
        }
        if let Some(mut result) = tile.config_running_status.try_lock() {
            if matches!(*result, ConfigRunningStatus::Running) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Evaluating config...");
                });
            } else if matches!(*result, ConfigRunningStatus::Finished(..))
                && let ConfigRunningStatus::Finished(result) = std::mem::take(&mut *result)
            {
                if let Ok(result) = &result {
                    tile.return_value_map = default_return_values(result);
                };
                tile.config_ui = Some(result.map_err(|e| e.to_string()));
            }
        }
        if let Some(mut result) = tile.script_running_status.try_lock() {
            if matches!(*result, ScriptRunningStatus::Running) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Executing script...");
                });
            } else if matches!(*result, ScriptRunningStatus::Finished(..))
                && let ScriptRunningStatus::Finished(result) = std::mem::take(&mut *result)
            {
                match result {
                    Ok(world) => tile.script_eval_err = None,
                    Err(e) => tile.script_eval_err = Some(e.to_string()),
                }
            }
        }
    });
    if let Some(err) = &tile.script_eval_err {
        ui.label(err);
    }
    let Some(config_ui) = &tile.config_ui else {
        return;
    };
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match config_ui {
        Ok(config_ui) => ConfigUiList {
            ui_definition: config_ui,
            return_value: &mut tile.return_value_map,
        }
        .show_ui(ui),
        Err(msg) => {
            ui.label("Error while showing config:");
            ui.label(msg);
        }
    });
}
