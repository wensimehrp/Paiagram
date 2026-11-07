// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

mod app;
mod command_palette;
mod config;
mod distance_to_line;
mod emergency_save;
mod font;
mod human_time;
mod load;
mod tiles;
mod timer;
mod trip_schedule_visual;
mod ui_tiles;
mod widgets;

pub use app::App;
pub use config::AppLanguage;
use egui::*;
use egui_i18n::tr;
pub use emergency_save::{setup_emergency_save, update_emergency_save};
use log::info;
use paiagram_core::RouteKey;
use paiagram_core::import::{ImportMode, ImportType};
use paiagram_core::time::TimetableTime;
use paiagram_export::{ExportGtfs, ExportOuDia};
use paiagram_rw::ExportObject;
use rfd::AsyncFileDialog;
use tiles::{UiTile, for_all_tiles};
use ui_tiles::{TileViewer, UiTiles};
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;
pub use widgets::search::build_matcher;

use crate::command_palette::CommandPalette;

#[derive(Default)]
pub struct UiState {
    command_palette: CommandPalette,
    mus: UiTiles,
}

enum UiCommand {
    OpenOrFocus(UiTile),
}

pub fn show_ui(
    ui: &mut Ui,
    app: &mut App,
    ui_state: &mut UiState,
    delta_time: std::time::Duration,
) {
    profiling::finish_frame!();
    puffin::profile_function!();
    app.begin_frame(ui, &mut ui_state.mus);

    {
        puffin::profile_scope!("command_palette");
        ui_state.command_palette.show(ui.ctx(), app);
    }

    if ui.ctx().input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
        app.save_as();
    } else if ui.ctx().input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
        app.save();
    }

    Panel::top("top panel").exact_size(32.0).show(ui, |ui| {
        puffin::profile_scope!("top_panel");
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            Popup::menu(&ui.button("App")).show(|ui| {
                if ui.add(Button::new("Command Palette").shortcut_text("Ctrl+P")).clicked() {
                    ui_state.command_palette.visible ^= true;
                }
                if ui.button("Toggle Fullscreen").clicked() {
                    cfg_select! {
                        target_arch = "wasm32" => {
                            let document = web_sys::window().unwrap().document().unwrap();
                            if document.fullscreen_element().is_none() {
                                let _ = document
                                    .get_element_by_id("paiagram_canvas")
                                    .unwrap()
                                    .request_fullscreen();
                            } else {
                                document.exit_fullscreen();
                            }
                        }
                        _ => {
                            let is_fullscreen =
                                ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                            ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                        }
                    }
                }
            });
            Popup::menu(&ui.button("File")).show(|ui| {
                for (button_display, category, import_type) in [
                    ("Import OuDiaSecond", "OuDiaSecond", ImportType::OuDiaSecond),
                    ("Import OuDia", "OuDia", ImportType::OuDia),
                    ("Import qETRC/pyETRC", "pyetgr", ImportType::Pyetgr),
                    ("Import GTFS", "GTFS", ImportType::Gtfs),
                    ("Import .paia", "Paiagram", ImportType::PaiagramPaia),
                    #[cfg(debug_assertions)]
                    ("Import sample.oud2", "OuDiaSecond", ImportType::BuiltinOud2),
                ] {
                    ui.menu_button(button_display, |ui| {
                        for (label, import_mode) in [
                            ("Add to current world", ImportMode::Merge),
                            ("Replace current world", ImportMode::Replace),
                        ] {
                            if !ui.button(label).clicked() {
                                continue;
                            }
                            if matches!(import_mode, ImportMode::Replace) {
                                app.save_path = None;
                            }
                            info!("Trying to read {category}");
                            let dialog = AsyncFileDialog::new()
                                .set_title(button_display)
                                .add_filter(category, import_type.file_extensions());
                            load::load_file(
                                dialog,
                                import_type,
                                import_mode,
                                app.world_read_tx.clone(),
                            );
                            ui.close();
                        }
                    });
                }
                ui.separator();
                if ui.add(Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                    app.save();
                    ui.close();
                }
                if ui.add(Button::new("Save As...").shortcut_text("Ctrl+Shift+S")).clicked() {
                    app.save_as();
                    ui.close();
                }
                ui.separator();
                if ui.button("Export .oud").clicked() {
                    ExportOuDia {
                        world: app.world().clone(),
                        route: RouteKey::new(),
                        is_oudia_second: false,
                    }
                    .write_via_interface(app.file_write_tx.clone());
                }
                if ui.button("Export GTFS").clicked() {
                    ExportGtfs {
                        world: app.world().clone(),
                    }
                    .write_via_interface(app.file_write_tx.clone());
                }
            });
            Popup::menu(&ui.button(tr!("menu-about"))).show(|ui| {
                if ui.button(tr!("menu-documentation")).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(if cfg!(target_arch = "wasm32") {
                        "/docs"
                    } else {
                        "https://paiagram.com/docs"
                    }));
                }
                if cfg!(target_arch = "wasm32") && ui.button(tr!("menu-legal")).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab("./license.html"));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("History", |ui| {
                    ScrollArea::vertical().show_rows(
                        ui,
                        ui.spacing().interact_size.y,
                        app.history().len(),
                        |ui, range| {
                            for (idx, _) in app.history()[range].iter().enumerate() {
                                ui.button(idx.to_string());
                            }
                        },
                    )
                });
                if ui.add_enabled(app.redoable(), Button::new("Redo")).clicked() {
                    app.redo();
                }
                if ui.add_enabled(app.undoable(), Button::new("Undo")).clicked() {
                    app.undo();
                }
                const GIT_REV_SHORT: &str = git_version::git_version!(fallback = "unknown");
                const GH_LINK: &str = git_version::git_version!(
                    args = ["--always", "--abbrev=40"],
                    prefix = "https://github.com/wensimehrp/Paiagram/commit/",
                    fallback = "https://github.com/wensimehrp/Paiagram/"
                );
                ui.hyperlink_to(GIT_REV_SHORT, GH_LINK);
            });
        })
    });
    Panel::bottom("bottom panel").exact_size(24.0).show(ui, |ui| {
        puffin::profile_scope!("bottom_panel");
        ui.horizontal_centered(|ui| {
            let mut time = TimetableTime(app.timer.seconds() as i32);
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::Checkbox::new(&mut app.timer.animation_playing, ""),
            );
            let time_response = ui.add(DragValue::new(&mut time));
            ui.add_enabled(
                !app.timer.sync_to_real_time,
                egui::DragValue::new(&mut app.timer.animation_speed).fixed_decimals(1).suffix("×"),
            );
            egui::Popup::menu(&time_response).show(|ui| {
                ui.checkbox(
                    &mut app.timer.sync_to_real_time,
                    tr!("menu-sync-system-clock"),
                );
            });
            if !app.timer.sync_to_real_time
                && time_response.dragged()
                && let Some(key) = app.timer.try_lock()
            {
                app.timer.update_seconds(time.0 as f64, &key);
                app.timer.unlock(key);
            }
            if app.timer.animation_playing || app.timer.sync_to_real_time {
                ui.ctx().request_repaint();
            }
            app.timer.march(delta_time.as_secs_f64());
        })
    });
    egui::CentralPanel::default().frame(Frame::default()).show(ui, |ui| {
        puffin::profile_scope!("tiles");
        let mut tile_viewer = TileViewer { app };
        ui_state.mus.tree.ui(&mut tile_viewer, ui);
    });
}
