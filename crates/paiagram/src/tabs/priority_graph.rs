use core::f32;

use egui::epaint::TextShape;
use egui::{Align2, Margin, Painter, Pos2, Rect, Sense, Stroke, Vec2, WidgetText, pos2};
use egui_i18n::tr;
use paiagram_core::colors::DisplayedColor;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::TimetableTime;
use paiagram_core::{RouteKey, TripKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{AppState, Navigatable, Tab};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PriorityGraphTab {
    route_key: RouteKey,
    navi: PriorityTabNavigation,
    #[serde(skip)]
    cached_lines: Option<Vec<Vec<(TripKey, TimetableTime)>>>,
}

impl PartialEq for PriorityGraphTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_key == other.route_key
    }
}

impl PriorityGraphTab {
    pub(crate) fn new(rk: RouteKey) -> Self {
        Self {
            route_key: rk,
            navi: PriorityTabNavigation::default(),
            cached_lines: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct PriorityTabNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: Vec2,
    visible_rect: Rect,
}

impl Default for PriorityTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: Vec2::splat(1.0),
            visible_rect: Rect::ZERO,
        }
    }
}

impl Navigatable for PriorityTabNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn allow_axis_zoom(&self) -> bool { true }
    fn zoom_x(&self) -> f32 { self.zoom.x }
    fn zoom_y(&self) -> f32 { self.zoom.y }
    fn offset_x(&self) -> f64 { self.x_offset }
    fn offset_y(&self) -> f64 { self.y_offset }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) { self.x_offset = offset_x; self.y_offset = offset_y; }
    fn visible_rect(&self) -> egui::Rect { self.visible_rect }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) { self.zoom.x = zoom_x; self.zoom.y = zoom_y; }
}

impl Tab for PriorityGraphTab {
    const NAME: &'static str = "Priority Graph";
    fn title(&self) -> WidgetText { tr!("tab-priority-graph").into() }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        self.calculate_priority(app);
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
                self.navi.visible_rect = response.rect;
                self.navi.handle_navigation(ui, &response);
                self.draw_lines(app, &painter, ui);
            });
    }
}

impl PriorityGraphTab {
    fn calculate_priority(&mut self, app: &mut AppState) {
        if self.cached_lines.is_some() { return; }
        let route_view = app.source.routes.get_view(self.route_key);
        let Some(ref route) = route_view else { return; };

        let mut maps: Vec<Vec<(TripKey, TimetableTime)>> = Vec::new();
        for sk in route.stations.iter() {
            let mut times: Vec<(TripKey, TimetableTime)> = Vec::new();
            for tk in app.source.trips.keys() {
                if let Some(view) = app.source.trips.get_view(*tk) {
                    for entry in view.schedule.entries() {
                        let entry_sk = match entry {
                            TEntry::Derived(s) => *s,
                            TEntry::Pinned { stn: s, .. } => *s,
                            TEntry::PinnedNonStop { stn: s, .. } => *s,
                            TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                            TEntry::PinnedExternal { .. } => continue,
                        };
                        if entry_sk != *sk { continue; }
                        let dep = match entry {
                            TEntry::Pinned { dep: TravelMode::At(t), .. } => *t,
                            TEntry::PinnedNonStop { pass: TravelMode::At(t), .. } => *t,
                            _ => continue,
                        };
                        times.push((*tk, dep));
                    }
                }
            }
            times.sort_unstable_by_key(|(_, t)| *t);
            maps.push(times);
        }
        self.cached_lines = Some(maps);
    }

    fn draw_lines(&self, app: &AppState, painter: &egui::Painter, ui: &egui::Ui) {
        // Draw station lines
        let route_view = app.source.routes.get_view(self.route_key);
        let Some(ref route) = route_view else { return; };
        let stroke = Stroke { width: 0.6, color: ui.visuals().window_stroke().color };
        let text_color = ui.visuals().text_color();

        for (idx, sk) in route.stations.iter().enumerate() {
            let pos = idx as f64 * 10.0;
            let screen_x = self.navi.xy_to_screen_pos(pos, 0.0).x;
            painter.vline(screen_x, self.navi.visible_rect.y_range(), stroke);
            let name = app.source.stations.query(*sk, |b| b.name.clone())
                .unwrap_or_default();
            let galley = painter.layout_no_wrap(name.to_string(), egui::FontId::proportional(13.0), text_color);
            let text_shape = TextShape::new(pos2(screen_x, self.navi.visible_rect.top()), galley, text_color)
                .with_angle_and_anchor(f32::consts::FRAC_PI_4, Align2::LEFT_BOTTOM);
            painter.add(text_shape);
        }

        // Draw trip lines
        let line_stroke = Stroke {
            width: 2.0,
            color: DisplayedColor::Predefined(paiagram_core::colors::PredefinedColor::Amber).get(false),
        };
        if let Some(ref maps) = self.cached_lines {
            let mut line_map: HashMap<TripKey, Vec<Pos2>> = HashMap::new();
            for (station_idx, map) in maps.iter().enumerate() {
                let x = station_idx as f64 * 10.0;
                for (priority, (tk, _)) in map.iter().enumerate() {
                    let pos = self.navi.xy_to_screen_pos(x, priority as f64 * 10.0);
                    line_map.entry(*tk).or_default().push(pos);
                }
            }
            for (_, points) in line_map {
                painter.line(points, line_stroke);
            }
        }
    }
}
