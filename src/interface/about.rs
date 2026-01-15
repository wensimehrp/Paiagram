use crate::interface::UiCommand;
use crate::interface::tabs::all_tabs::MinesweeperTab;
use crate::rw_data::ModifyData;
use bevy::prelude::*;
use egui::{Id, Modal, OpenUrl};
use rfd::AsyncFileDialog;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

static FILE_IMPORT_QUEUE: OnceLock<Arc<Mutex<VecDeque<ModifyData>>>> = OnceLock::new();

fn shared_queue() -> Arc<Mutex<VecDeque<ModifyData>>> {
    FILE_IMPORT_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(VecDeque::new())))
        .clone()
}

fn show_import_button(
    ui: &mut egui::Ui,
    label: &'static str,
    filter_label: &'static str,
    extensions: &'static [&'static str],
    constructor: fn(String) -> ModifyData,
) {
    if ui.button(label).clicked() {
        start_file_import(filter_label, extensions, constructor);
    }
}

fn start_file_import(
    filter_label: &'static str,
    extensions: &'static [&'static str],
    constructor: fn(String) -> ModifyData,
) {
    let queue = shared_queue();
    let task = async move {
        if let Some(content) = pick_file_contents(filter_label, extensions).await {
            if let Ok(mut guard) = queue.lock() {
                guard.push_back(constructor(content));
            }
        }
    };

    bevy::tasks::IoTaskPool::get().spawn(task).detach();
}

async fn pick_file_contents(
    filter_label: &'static str,
    extensions: &'static [&'static str],
) -> Option<String> {
    let file = AsyncFileDialog::new()
        .add_filter(filter_label, extensions)
        .pick_file()
        .await?;
    let bytes = file.read().await;
    String::from_utf8(bytes).ok()
}

pub fn show_about(
    (InMut(ui), InMut(modal_open)): (InMut<egui::Ui>, InMut<bool>),
    mut msg_read_file: MessageWriter<ModifyData>,
    mut msg_open_tab: MessageWriter<UiCommand>,
) {
    let queue = shared_queue();
    let pending: Vec<_> = {
        let mut guard = queue.lock().unwrap();
        guard.drain(..).collect()
    };
    drop(queue);
    for message in pending {
        msg_read_file.write(message);
    }

    if ui.button("⚙").clicked() {
        msg_open_tab.write(UiCommand::OpenOrFocusTab(super::AppTab::Settings(
            super::tabs::settings::SettingsTab,
        )));
    }

    if ui.button("G").clicked() {
        msg_open_tab.write(UiCommand::OpenOrFocusTab(super::AppTab::Graph(
            super::tabs::graph::GraphTab::default(),
        )));
    }

    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Read...").clicked() {
                // TODO
            }
            if ui.button("Save As...").clicked() {
                // TODO
            }
            ui.separator();
            show_import_button(
                ui,
                "Import qETRC...",
                "qETRC Files",
                &["pyetgr", "json"],
                ModifyData::LoadQETRC,
            );
            show_import_button(
                ui,
                "Import OuDiaSecond...",
                "OuDiaSecond Files",
                &["oud2"],
                ModifyData::LoadOuDiaSecond,
            );
            // FIXME: THIS IS NOT CUSTOM AT ALL
            show_import_button(
                ui,
                "Import Custom...",
                "Custom Files",
                &["json"],
                ModifyData::LoadCustom,
            );
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Minesweeper").clicked() {
                msg_open_tab.write(UiCommand::OpenOrFocusTab(super::AppTab::Minesweeper(
                    MinesweeperTab,
                )));
            }
            if ui.button("Check for Updates").clicked() {
                // TODO
            }
            if ui.button("Report Issue").clicked() {
                ui.ctx().open_url(OpenUrl {
                    url: "https://github.com/WenSimEHRP/Paiagram/issues".into(),
                    new_tab: true,
                });
            };
            if ui.button("Help Translate").clicked() {
                ui.ctx().open_url(OpenUrl {
                    // TODO: add a real URL here
                    url: "https://github.com/WenSimEHRP/Paiagram/issues".into(),
                    new_tab: true,
                });
            };
            ui.separator();
            if ui.button("Documentation").clicked() {
                ui.ctx().open_url(OpenUrl {
                    // TODO: add a real URL here
                    url: "https://github.com/WenSimEHRP/Paiagram/issues".into(),
                    new_tab: true,
                });
            };
            if ui.button("About Paiagram").clicked() {
                *modal_open = true;
            };
        });
    });
    if *modal_open {
        let modal = Modal::new(Id::new("Modal B")).show(ui.ctx(), |ui| {
            ui.set_width(500.0);
            ui.heading("About");
            ui.label(format!("Paiagram ({})", git_version::git_version!()));
            ui.label("© 2025 Jeremy Gao");
            ui.label("Paiagram is a free and open-source application licensed under the AGPL v3.0 license.");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width().max(0.0));
                ui.monospace(include_str!("../../LICENSE.md"));
            });
        });

        if modal.should_close() {
            *modal_open = false;
        }
    }
}
