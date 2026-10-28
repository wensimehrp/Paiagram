use std::sync::mpsc;

use egui::*;
use egui_i18n::tr;
use log::error;
use paiagram_rw::list_files::{self, Result};
use paiagram_rw::{AvailableSaves, SaveCategory};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
use crate::widgets::{LOGO_COORDINATES, LogoStroke};

#[derive(Serialize, Deserialize)]
#[serde(into = "StartTabNoCache", from = "StartTabNoCache")]
pub(crate) struct StartTab {
    available_files: Option<AvailableSaves>,
    available_file_tx: mpsc::Sender<Result<AvailableSaves>>,
    available_file_rx: mpsc::Receiver<Result<AvailableSaves>>,
}

impl Clone for StartTab {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Default for StartTab {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            available_files: None,
            available_file_rx: rx,
            available_file_tx: tx,
        }
    }
}

impl PartialEq for StartTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct StartTabNoCache;

impl From<StartTab> for StartTabNoCache {
    fn from(_value: StartTab) -> Self {
        Self
    }
}

impl From<StartTabNoCache> for StartTab {
    fn from(_value: StartTabNoCache) -> Self {
        Self::default()
    }
}

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        puffin::profile_function!();

        let logo_stroke = Stroke::new(25.0, ui.visuals().window_stroke.color);
        draw_logo(ui.painter(), logo_stroke, ui.clip_rect());

        let refresh = if self.available_files.is_none() {
            self.available_files = Some(AvailableSaves::default());
            true
        } else {
            false
        } || ui.button("Refresh").clicked();

        if refresh {
            #[cfg(target_arch = "wasm32")]
            list_files::list_all_saves(self.available_file_tx.clone());
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = eframe::storage_dir("paiagram") {
                list_files::list_all_saves(self.available_file_tx.clone(), path);
            }
        }

        if let Ok(files) = self.available_file_rx.try_recv() {
            match files {
                Ok(files) => self.available_files = Some(files),
                Err(e) => error!("Failed to scan available files: {e}"),
            }
        }

        if let Some(available_files) = &self.available_files
            && let Some((category, key)) = show_available_save(ui, available_files)
        {}
    }
}

fn show_available_save(ui: &mut Ui, available: &AvailableSaves) -> Option<(SaveCategory, String)> {
    let mut selected = None;
    ScrollArea::vertical().show(ui, |ui| {
        for (title, category, keys) in [
            (
                "Autosaves",
                SaveCategory::Autosave,
                available.autosaves.as_slice(),
            ),
            ("Saves", SaveCategory::Save, available.saves.as_slice()),
        ] {
            if keys.is_empty() {
                continue;
            }
            ui.heading(title);
            for key in keys {
                if ui.button(key).clicked() {
                    selected = Some((category, key.clone()));
                }
            }
            ui.add_space(8.0);
        }
    });
    selected
}

fn draw_logo(painter: &Painter, stroke: Stroke, rect: Rect) {
    let origin = rect.min;
    let to_screen = |pos: Vec2| origin + pos * 20.0;
    let to_local = |pos: Pos2| (pos - origin) / 20.0;

    for coord in LOGO_COORDINATES {
        match *coord {
            LogoStroke::_2(extend1, vec1, vec2, extend2) => {
                let vec1: Vec2 = vec1.into();
                let vec2: Vec2 = vec2.into();
                let vec1 = if extend1 {
                    to_local(extend_to_clip_edge(to_screen(vec1), to_screen(vec2), rect))
                } else {
                    vec1
                };
                let vec2 = if extend2 {
                    to_local(extend_to_clip_edge(to_screen(vec2), to_screen(vec1), rect))
                } else {
                    vec2
                };
                let vec1 = vec1 + (vec1 - vec2).normalized() * 0.5;
                let vec2 = vec2 + (vec2 - vec1).normalized() * 0.5;
                painter.line_segment([to_screen(vec1), to_screen(vec2)], stroke)
            }
            LogoStroke::_3(extend1, vec1, vec2, vec3, extend2) => {
                let vec1: Vec2 = vec1.into();
                let vec2: Vec2 = vec2.into();
                let vec3: Vec2 = vec3.into();
                let vec1 = if extend1 {
                    to_local(extend_to_clip_edge(to_screen(vec1), to_screen(vec2), rect))
                } else {
                    vec1
                };
                let vec3 = if extend2 {
                    to_local(extend_to_clip_edge(to_screen(vec3), to_screen(vec2), rect))
                } else {
                    vec3
                };
                let vec1 = vec1 + (vec1 - vec2).normalized() * 0.5;
                let vec3 = vec3 + (vec3 - vec2).normalized() * 0.5;
                painter.line(
                    vec![to_screen(vec1), to_screen(vec2), to_screen(vec3)],
                    stroke,
                )
            }
        };
    }
}

/// Extends `start` in the direction away from `other` until it reaches the
/// boundary of `rect`. Returns the new position (in the same space as the
/// inputs, i.e. screen space).
fn extend_to_clip_edge(start: Pos2, other: Pos2, rect: egui::Rect) -> Pos2 {
    // Direction pointing from `other` towards `start` is the extension direction.
    let dir = start - other;
    if dir.x.abs() < f32::EPSILON && dir.y.abs() < f32::EPSILON {
        return start;
    }

    // Intersect the ray with each of the four edges and pick the closest
    // intersection in the forward direction.
    let mut best: Option<(f32, Pos2)> = None;
    let mut consider = |t: f32, p: Pos2| {
        if t >= 0.0 {
            if let Some((bt, _)) = best {
                if t < bt {
                    best = Some((t, p));
                }
            } else {
                best = Some((t, p));
            }
        }
    };

    // Left edge (x = rect.min.x)
    if dir.x != 0.0 {
        let t = (rect.min.x - start.x) / dir.x;
        let y = start.y + t * dir.y;
        consider(t, Pos2::new(rect.min.x, y));
    }
    // Right edge (x = rect.max.x)
    if dir.x != 0.0 {
        let t = (rect.max.x - start.x) / dir.x;
        let y = start.y + t * dir.y;
        consider(t, Pos2::new(rect.max.x, y));
    }
    // Top edge (y = rect.min.y)
    if dir.y != 0.0 {
        let t = (rect.min.y - start.y) / dir.y;
        let x = start.x + t * dir.x;
        consider(t, Pos2::new(x, rect.min.y));
    }
    // Bottom edge (y = rect.max.y)
    if dir.y != 0.0 {
        let t = (rect.max.y - start.y) / dir.y;
        let x = start.x + t * dir.x;
        consider(t, Pos2::new(x, rect.max.y));
    }

    best.map(|(_, p)| p).unwrap_or(start)
}
