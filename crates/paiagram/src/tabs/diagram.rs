use egui::*;
use paiagram_core::route::DiagramCache;
use paiagram_core::time::{Tick, TimetableTime};
use paiagram_core::{CanvasLength, RouteKey};
use serde::{Deserialize, Serialize};

pub(crate) mod gpu_draw;
mod gpu_trip;

use super::{Navigatable, Tab};
use crate::App;

#[derive(Clone, Serialize, Deserialize)]
#[serde(into = "DiagramTabNoCache", from = "DiagramTabNoCache")]
pub(crate) struct DiagramTab {
    key: RouteKey,
    navi: DiagramTabNavigation,
    cache: DiagramCache,
    callback: gpu_draw::DiagramCallback,
}

/// [`DiagramTab`] w/o cache for serde
#[derive(Clone, Serialize, Deserialize)]
struct DiagramTabNoCache {
    key: RouteKey,
    navi: DiagramTabNavigation,
}

impl From<DiagramTab> for DiagramTabNoCache {
    fn from(DiagramTab { key, navi, .. }: DiagramTab) -> Self {
        Self { key, navi }
    }
}

impl From<DiagramTabNoCache> for DiagramTab {
    fn from(DiagramTabNoCache { key, navi }: DiagramTabNoCache) -> Self {
        Self {
            key,
            navi,
            cache: DiagramCache::default(),
            callback: gpu_draw::DiagramCallback::new(key),
        }
    }
}

impl DiagramTab {
    pub fn new(key: RouteKey) -> Self {
        Self {
            key,
            navi: DiagramTabNavigation::default(),
            cache: DiagramCache::default(),
            callback: gpu_draw::DiagramCallback::new(key),
        }
    }
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct DiagramTabNavigation {
    x_offset: Tick,
    y_offset: CanvasLength,
    zoom: Vec2,
    #[serde(skip, default = "default_visible_rect")]
    visible_rect: Rect,
    max_height: CanvasLength,
}

impl Default for DiagramTabNavigation {
    fn default() -> Self {
        // Default horizontal scale: five minutes of time span one centimetre of canvas.
        let five_minutes_in_ticks = TimetableTime::from_hms(0, 5, 0).to_ticks().0 as f32;
        let one_cm_in_pts = CanvasLength::from_cm(1.0).to_egui_pts();
        Self {
            x_offset: Tick(0),
            y_offset: CanvasLength(0.0),
            zoom: vec2(one_cm_in_pts / five_minutes_in_ticks, 1.0),
            visible_rect: Rect::NOTHING,
            max_height: CanvasLength(0.0),
        }
    }
}

fn default_visible_rect() -> Rect {
    Rect::NOTHING
}

impl Navigatable for DiagramTabNavigation {
    type XOffset = Tick;
    type YOffset = CanvasLength;

    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y * CanvasLength::egui_pts_per_mm() as f32
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom = Vec2::new(zoom_x, zoom_y / CanvasLength::egui_pts_per_mm() as f32);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset.0 as f64
    }
    fn offset_y(&self) -> f64 {
        self.y_offset.0
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = Tick(offset_x.round() as i64);
        self.y_offset = CanvasLength(offset_y);
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
    fn x_per_screen_unit(&self) -> Self::XOffset {
        Tick((1.0 / self.zoom_x().max(f32::EPSILON) as f64) as i64)
    }
    fn visible_x(&self) -> std::ops::Range<Self::XOffset> {
        let width = self.visible_rect().width() as f64;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let start = self.x_offset;
        let end = Tick(start.0 + (width * ticks_per_screen_unit).ceil() as i64);
        start..end
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let height = self.visible_rect.height() as f64;
        let start = self.offset_y();
        let end = start + height / self.zoom_y().max(f32::EPSILON) as f64;
        CanvasLength(start)..CanvasLength(end)
    }
    fn y_per_screen_unit(&self) -> Self::YOffset {
        CanvasLength(1.0 / self.zoom_y().max(f32::EPSILON) as f64)
    }
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        let ppmm = CanvasLength::egui_pts_per_mm() as f32;
        (
            zoom_x.clamp(0.00005, 0.4),
            zoom_y.clamp(ppmm * 0.1, ppmm * 2048.0),
        )
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        let max_tick = Tick::from_timetable_time(TimetableTime(366 * 86400)).0;
        self.x_offset = Tick(self.x_offset.0.clamp(
            -max_tick,
            max_tick - (response.rect.width() as f64 / self.zoom.x as f64) as i64,
        ));
        const TOP_BOTTOM_PADDING: f32 = 30.0;
        let v = if response.rect.height() / self.zoom_y()
            > (self.max_height.0 as f32 + TOP_BOTTOM_PADDING * 2.0 / self.zoom_y())
        {
            ((-response.rect.height() / self.zoom_y() + self.max_height.0 as f32) / 2.0) as f64
        } else {
            self.y_offset.0.clamp(
                (-TOP_BOTTOM_PADDING / self.zoom_y()) as f64,
                (self.max_height.0 as f32 - response.rect.height() / self.zoom_y()
                    + TOP_BOTTOM_PADDING / self.zoom_y()) as f64,
            )
        };
        self.y_offset.0 = v;
    }
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn title(&self) -> WidgetText {
        "Diagram".into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        if ui.button("Update cache").clicked() {
            if let Some(route) = app.routes.get(&self.key) {
                route.intervals.populate_trips(&app.snap, &mut self.cache);
            } else {
                self.cache.0.clear();
            }
            self.callback.populate(&self.cache);
        }
        Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                main_display(self, app, ui);
            });
    }
}

fn draw_time_lines(painter: &mut Painter, navi: &DiagramTabNavigation) {
    const MAX_SCREEN_WIDTH: f64 = 64.0;
    const MIN_SCREEN_WIDTH: f64 = 32.0;
    let sizes = [
        Tick::from_timetable_time(TimetableTime(1)).0, // 1 second
        Tick::from_timetable_time(TimetableTime(10)).0, // 10 seconds
        Tick::from_timetable_time(TimetableTime(30)).0, // 30 seconds
        Tick::from_timetable_time(TimetableTime(60)).0, // 1 minute
        Tick::from_timetable_time(TimetableTime(60 * 5)).0, // 5 minutes
        Tick::from_timetable_time(TimetableTime(60 * 10)).0, // 10 minutes
        Tick::from_timetable_time(TimetableTime(60 * 30)).0, // 30 minutes
        Tick::from_timetable_time(TimetableTime(60 * 60)).0, // 1 hour
        Tick::from_timetable_time(TimetableTime(60 * 60 * 4)).0, // 4 hours
        Tick::from_timetable_time(TimetableTime(60 * 60 * 24)).0, // 1 day
    ];
    let visible_ticks = navi.visible_x();
    let ticks_per_screen_unit = navi.x_per_screen_unit_f64();
    let screen_rect = navi.visible_rect();
    let pixels_per_point = painter.pixels_per_point();
    let mut drawn: Vec<i64> = Vec::with_capacity(30);

    // align the first tick to a spacing boundary that is <= visible start.
    let first_visible_position = sizes
        .iter()
        .position(|s| *s as f64 / ticks_per_screen_unit * 1.5 > MIN_SCREEN_WIDTH)
        .unwrap_or(0);
    let visible = &sizes[first_visible_position..];
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start.0 - visible_ticks.start.0.rem_euclid(*spacing) - spacing;
        let mut tick = first;
        let strength = (((*spacing as f64 / ticks_per_screen_unit * 1.5) - MIN_SCREEN_WIDTH)
            / (MAX_SCREEN_WIDTH - MIN_SCREEN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        // TODO: make stroke depend on current theme
        let mut current_stroke = Stroke {
            width: 0.6,
            color: Color32::GRAY,
        };
        if strength.is_finite() {
            // strange bug here
            current_stroke.color = current_stroke.color.gamma_multiply(strength as f32);
        }
        current_stroke.width = 0.5;
        while tick <= visible_ticks.end.0 {
            tick += *spacing;
            if drawn.contains(&tick) {
                continue;
            }
            let mut x = navi.logical_x_to_screen_x(Tick(tick));
            current_stroke.round_center_to_pixel(pixels_per_point, &mut x);
            painter.vline(x, screen_rect.top()..=screen_rect.bottom(), current_stroke);
            drawn.push(tick);
            let time = Tick(tick).to_timetable_time();
            let mut offset = screen_rect.top();
            let text = match i + first_visible_position {
                0..=2 => time.to_hmsd().2.to_string(),
                3..=8 => format!("{}:{:02}", time.to_hmsd().0, time.to_hmsd().1),
                9 => {
                    offset += 13.0;
                    time.to_string()
                }
                _ => unreachable!(),
            };
            let label = painter.layout_no_wrap(
                text,
                FontId::new(13.0, egui::FontFamily::Proportional),
                current_stroke.color,
            );
            painter.galley(
                Pos2 {
                    x: x - label.size().x / 2.0,
                    y: offset,
                },
                label,
                current_stroke.color,
            );
        }
    }
}

fn main_display(tab: &mut DiagramTab, app: &mut App, ui: &mut Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible_rect = response.rect;
    tab.navi.max_height = CanvasLength::from_cm(100.0);
    tab.navi.handle_navigation(ui, &response);
    draw_time_lines(&mut painter, &tab.navi);

    // paint the lines
    tab.callback.populate_uniforms(&tab.navi, response.rect);
    painter.add(tab.callback.clone().paint_callback(response.rect));

    // interactive zone
}
