use egui::{Pos2, Stroke, Ui};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
use crate::tabs::trip::TripTab;
use crate::widgets::{LOGO_COORDINATES, LogoStroke};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub(crate) struct StartTab;

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        let painter = ui.painter();
        let stroke = Stroke::new(20.0, ui.visuals().window_stroke.color);
        for coord in LOGO_COORDINATES {
            match *coord {
                LogoStroke::_2(extend1, pos1, pos2, extend2) => {
                    let pos1: Pos2 = pos1.into();
                    let pos2: Pos2 = pos2.into();
                    let pos1 = if extend1 {
                        extend_to_clip_edge(pos1 * 20.0, pos2 * 20.0, ui.clip_rect()) / 20.0
                    } else {
                        pos1
                    };
                    let pos2 = if extend2 {
                        extend_to_clip_edge(pos2 * 20.0, pos1 * 20.0, ui.clip_rect()) / 20.0
                    } else {
                        pos2
                    };
                    let pos1 = pos1 + (pos1 - pos2).normalized() * 0.5;
                    let pos2 = pos2 + (pos2 - pos1).normalized() * 0.5;
                    painter.line_segment([pos1 * 20.0, pos2 * 20.0], stroke)
                }
                LogoStroke::_3(extend1, pos1, pos2, pos3, extend2) => {
                    let pos1: Pos2 = pos1.into();
                    let pos2: Pos2 = pos2.into();
                    let pos3: Pos2 = pos3.into();
                    let pos1 = if extend1 {
                        extend_to_clip_edge(pos1 * 20.0, pos2 * 20.0, ui.clip_rect()) / 20.0
                    } else {
                        pos1
                    };
                    let pos3 = if extend2 {
                        extend_to_clip_edge(pos3 * 20.0, pos2 * 20.0, ui.clip_rect()) / 20.0
                    } else {
                        pos3
                    };
                    let pos1 = pos1 + (pos1 - pos2).normalized() * 0.5;
                    let pos3 = pos3 + (pos3 - pos2).normalized() * 0.5;
                    painter.line(vec![pos1 * 20.0, pos2 * 20.0, pos3 * 20.0], stroke)
                }
            };
        }
        ui.vertical_centered_justified(|ui| {
            ui.set_max_width((300.0_f32.min(ui.available_width()) - 10.0).max(1.0));
            ui.horizontal_centered(|ui| {
                ui.heading(tr!("program-name"));
                egui::Grid::new("start info grid").num_columns(2).show(ui, |ui| {
                    ui.label(tr!("tab-start-amount-vehicles"));
                    ui.label(app.vehicles.len().to_string());
                    ui.end_row();
                    ui.label(tr!("tab-start-amount-trips"));
                    ui.label(app.trips.len().to_string());
                    ui.end_row();
                    ui.label(tr!("tab-start-amount-stations"));
                    ui.label(app.stations.len().to_string());
                    ui.end_row();
                    ui.label(tr!("tab-start-amount-intervals"));
                    ui.label(app.intervals.len().to_string());
                    ui.end_row();
                });
            })
        });
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
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
