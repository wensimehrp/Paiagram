use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::*;
use paiagram_extensions::config_ui::{
    ConfigUi, ConfigUiList, ReturnValueMap, default_return_values,
};
use paiagram_extensions::{
    ConfigRunningStatus, JsDataExporter, JsRuntimeData, ScriptRunningStatus, eval_config,
    run_script,
};
use paiagram_rw::ExportObject;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::Tab;

#[derive(Serialize, Deserialize)]
#[serde(into = "ExtensionTabNoCache", from = "ExtensionTabNoCache")]
pub struct ExtensionTab {
    script_text: String,
    // wasm only allows saving file on the main thread
    file_receiver: Receiver<JsDataExporter>,
    file_sender: Sender<JsDataExporter>,
    edit_mode: bool,
    config_running_status: Arc<Mutex<ConfigRunningStatus>>,
    script_running_status: Arc<Mutex<ScriptRunningStatus>>,
    config_ui: Option<Result<Vec<ConfigUi>, String>>,
    return_value_map: ReturnValueMap,
    script_eval_err: Option<String>,
}

impl Clone for ExtensionTab {
    fn clone(&self) -> Self {
        ExtensionTabNoCache {
            script_text: self.script_text.clone(),
        }
        .into()
    }
}

impl Default for ExtensionTab {
    fn default() -> Self {
        let (file_sender, file_receiver) = channel();
        Self {
            script_text: include_str!("../../../paiagram-extensions/scripts/ui_sample.js").into(),
            file_receiver,
            file_sender,
            edit_mode: false,
            config_running_status: Default::default(),
            script_running_status: Default::default(),
            config_ui: None,
            return_value_map: Default::default(),
            script_eval_err: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ExtensionTabNoCache {
    script_text: String,
}

impl From<ExtensionTab> for ExtensionTabNoCache {
    fn from(value: ExtensionTab) -> Self {
        Self {
            script_text: value.script_text,
        }
    }
}

impl From<ExtensionTabNoCache> for ExtensionTab {
    fn from(value: ExtensionTabNoCache) -> Self {
        Self {
            script_text: value.script_text,
            ..Default::default()
        }
    }
}

impl PartialEq for ExtensionTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Tab for ExtensionTab {
    const NAME: &'static str = "Extension";
    fn title(&self) -> egui::WidgetText {
        Self::NAME.into()
    }
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            main_display(self, app, ui);
        });
    }
}

fn main_display(tab: &mut ExtensionTab, app: &mut crate::App, ui: &mut Ui) {
    if let Ok(data) = tab.file_receiver.try_recv() {
        data.write_to_file::<false>(Default::default());
    }
    ui.checkbox(&mut tab.edit_mode, "Edit mode");
    if tab.edit_mode {
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut tab.script_text)
                    .code_editor()
                    .desired_width(f32::INFINITY),
            );
        });
        return;
    }
    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked()
            || (tab.config_ui.is_none()
                && matches!(
                    tab.config_running_status.try_lock(),
                    Some(inner) if matches!(*inner, ConfigRunningStatus::Idle)
                ))
        {
            eval_config(
                JsRuntimeData {
                    snap: app.snap.clone(),
                    selected_items: app.selected_items.clone(),
                    file_sender: tab.file_sender.clone(),
                },
                tab.config_running_status.clone(),
                tab.script_text.as_str().into(),
            );
        }
        if ui.button("Run script").clicked() {
            run_script(
                JsRuntimeData {
                    snap: app.snap.clone(),
                    selected_items: app.selected_items.clone(),
                    file_sender: tab.file_sender.clone(),
                },
                tab.return_value_map.clone(),
                tab.script_running_status.clone(),
                tab.script_text.as_str().into(),
            );
        }
        if let Some(mut result) = tab.config_running_status.try_lock() {
            if matches!(*result, ConfigRunningStatus::Running) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Evaluating config...");
                });
            } else if matches!(*result, ConfigRunningStatus::Finished(..))
                && let ConfigRunningStatus::Finished(result) = std::mem::take(&mut *result)
            {
                if let Ok(result) = &result {
                    tab.return_value_map = default_return_values(result);
                };
                tab.config_ui = Some(result.map_err(|e| e.to_string()));
            }
        }
        if let Some(mut result) = tab.script_running_status.try_lock() {
            if matches!(*result, ScriptRunningStatus::Running) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Executing script...");
                });
            } else if matches!(*result, ScriptRunningStatus::Finished(..))
                && let ScriptRunningStatus::Finished(result) = std::mem::take(&mut *result)
            {
                match result {
                    Ok(world) => tab.script_eval_err = None,
                    Err(e) => tab.script_eval_err = Some(e.to_string()),
                }
            }
        }
    });
    if let Some(err) = &tab.script_eval_err {
        ui.label(err);
    }
    let Some(config_ui) = &tab.config_ui else {
        return;
    };
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match config_ui {
        Ok(config_ui) => ConfigUiList {
            ui_definition: config_ui,
            return_value: &mut tab.return_value_map,
        }
        .show_ui(ui),
        Err(msg) => {
            ui.label("Error while showing config:");
            ui.label(msg);
        }
    });
}
