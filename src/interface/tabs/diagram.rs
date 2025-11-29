use super::PageCache;
use crate::{
    intervals::Station,
    lines::DisplayedLine,
    units::time::TimetableTime,
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    emath::{self, RectTransform},
    vec2,
};

// Time and time-canvas related constants
const SECONDS_PER_WORLD_UNIT: f64 = 1.0; // world units -> seconds
const TICKS_PER_SECOND: i64 = 100;
const TICKS_PER_WORLD_UNIT: f64 = SECONDS_PER_WORLD_UNIT * TICKS_PER_SECOND as f64;

pub struct DiagramPageCache {
    stroke: Stroke,
    tick_offset: i64,
    vertical_offset: f32,
    heights: Option<Vec<(Entity, f32)>>,
    // trackpad, mobile inputs, and scroll wheel
    zoom: Vec2,
}

impl DiagramPageCache {
    fn get_visible_stations(&self, range: std::ops::Range<f32>) -> &[(Entity, f32)] {
        let Some(heights) = &self.heights else {
            return &[];
        };
        let first_visible = heights.iter().position(|(_, h)| *h > range.start);
        let last_visible = heights.iter().rposition(|(_, h)| *h < range.end);
        if let (Some(mut first_visible), Some(mut last_visible)) = (first_visible, last_visible) {
            first_visible = first_visible.saturating_sub(1);
            last_visible = (last_visible + 1).min(heights.len() - 1);
            &heights[first_visible..=last_visible]
        } else {
            &[]
        }
    }
}

impl Default for DiagramPageCache {
    fn default() -> Self {
        Self {
            tick_offset: 0,
            vertical_offset: 0.0,
            stroke: Stroke {
                width: 1.0,
                color: Color32::BLACK,
            },
            heights: None,
            zoom: vec2(0.0005, 1.0),
        }
    }
}

pub fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    mut displayed_lines: Populated<&mut DisplayedLine>,
    // vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    // timetable_entries: Query<&TimetableEntry>,
    // station_names: Query<&Name, With<Station>>,
    mut page_cache: Local<PageCache<Entity, DiagramPageCache>>,
    // required for animations
) {
    let Ok(mut displayed_line) = displayed_lines.get_mut(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };
    let pc = page_cache.get_mut_or_insert_with(displayed_line_entity, DiagramPageCache::default);
    ui.horizontal(|ui| {
        ui.add(&mut pc.stroke);
    });
    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;
    if pc.heights.is_none() {
        let mut current_height = 0.0;
        let mut heights = Vec::new();
        for (station, distance) in &displayed_line.stations {
            current_height += distance.abs().log2().max(1.0) * 15f32;
            heights.push((*station, current_height))
        }
        pc.heights = Some(heights);
    }
    Frame::canvas(ui.style())
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            let (mut response, mut painter) =
                ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
            let vertical_visible =
                pc.vertical_offset..response.rect.height() / pc.zoom.y + pc.vertical_offset;
            let horizontal_visible = pc.tick_offset
                ..pc.tick_offset + (response.rect.width() as f64 / pc.zoom.x as f64) as i64;
            let ticks_per_screen_unit = (horizontal_visible.end - horizontal_visible.start) as f64
                / response.rect.width() as f64;
            draw_time_lines(
                &pc,
                &mut painter,
                &response.rect,
                ticks_per_screen_unit,
                &horizontal_visible,
                ui.pixels_per_point(),
            );
            // capture movements
            let mut zoom_delta: Vec2 = Vec2::default();
            let mut translation_delta: Vec2 = Vec2::default();
            ui.input(|input| {
                zoom_delta = input.zoom_delta_2d();
                translation_delta = input.translation_delta();
            });
            if let Some(pos) = response.hover_pos() {
                let old_zoom = pc.zoom;
                let mut new_zoom = pc.zoom * zoom_delta;
                new_zoom.x = new_zoom.x.clamp(0.00001, 0.4);
                new_zoom.y = new_zoom.y.clamp(0.025, 2048.0);
                let rel_pos = (pos - response.rect.min) / response.rect.size();
                let world_width_before = response.rect.width() as f64 / old_zoom.x as f64;
                let world_width_after = response.rect.width() as f64 / new_zoom.x as f64;
                let world_pos_before_x =
                    pc.tick_offset as f64 + rel_pos.x as f64 * world_width_before;
                let new_tick_offset =
                    (world_pos_before_x - rel_pos.x as f64 * world_width_after).round() as i64;
                pc.zoom = new_zoom;
                pc.tick_offset = new_tick_offset;
            }
            pc.tick_offset -= (ticks_per_screen_unit
                * (response.drag_delta().x + translation_delta.x) as f64)
                as i64;
            info!(?pc.tick_offset, ?pc.zoom.x);
        });
}

fn draw_station_lines(
    painter: &mut Painter,
    pc: &DiagramPageCache,
    screen_rect: &Rect,
    to_draw: &[(Entity, f32)],
) {
}

fn ticks_to_screen_x(
    ticks: i64,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    offset_ticks: i64,
) -> f32 {
    let base = (ticks - offset_ticks) as f64 / ticks_per_screen_unit;
    screen_rect.left() + base as f32
}

/// Draw vertical time lines and labels
pub fn draw_time_lines(
    pc: &DiagramPageCache,
    painter: &mut Painter,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    visible_ticks: &std::ops::Range<i64>,
    pixels_per_point: f32,
) {
    const MAX_SCREEN_WIDTH: f64 = 64.0;
    const MIN_SCREEN_WIDTH: f64 = 32.0;
    const SIZES: &[i64] = &[
        TICKS_PER_SECOND * 1,            // 1 second
        TICKS_PER_SECOND * 10,           // 10 seconds
        TICKS_PER_SECOND * 30,           // 30 seconds
        TICKS_PER_SECOND * 60,           // 1 minute
        TICKS_PER_SECOND * 60 * 5,       // 5 minutes
        TICKS_PER_SECOND * 60 * 10,      // 10 minutes
        TICKS_PER_SECOND * 60 * 30,      // 30 minutes
        TICKS_PER_SECOND * 60 * 60,      // 1 hour
        TICKS_PER_SECOND * 60 * 60 * 4,  // 4 hours
        TICKS_PER_SECOND * 60 * 60 * 24, // 1 day
    ];
    let mut drawn: Vec<i64> = Vec::with_capacity(30);

    // align the first tick to a spacing boundary that is <= visible start.
    let first_visible_position = SIZES
        .iter()
        .position(|s| *s as f64 / ticks_per_screen_unit * 1.5 > MIN_SCREEN_WIDTH)
        .unwrap_or(0);
    let visible = &SIZES[first_visible_position..];
    info!(?visible);
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start - visible_ticks.start.rem_euclid(*spacing) - spacing;
        let mut tick = first;
        let strength = (((*spacing as f64 / ticks_per_screen_unit * 1.5) - MIN_SCREEN_WIDTH)
            / (MAX_SCREEN_WIDTH - MIN_SCREEN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        let mut stroke = pc.stroke;
        stroke.color = stroke.color.gamma_multiply(strength as f32);
        while tick <= visible_ticks.end {
            tick += *spacing;
            if drawn.contains(&tick) {
                continue;
            }
            let mut x = ticks_to_screen_x(tick, screen_rect, ticks_per_screen_unit, pc.tick_offset);
            stroke.round_center_to_pixel(pixels_per_point, &mut x);
            painter.vline(x, screen_rect.top()..=screen_rect.bottom(), stroke);
            drawn.push(tick);
            let time = TimetableTime((tick / 100) as i32);
            let text = match i + first_visible_position {
                0..=2 => time.to_hmsd().2.to_string(),
                _ => time.to_string(),
            };
            let label = painter.layout_no_wrap(text, FontId::monospace(13.0), stroke.color);
            painter.galley(
                Pos2 {
                    x: x - label.size().x / 2.0,
                    y: screen_rect.top() + label.size().y / 2.0,
                },
                label,
                stroke.color,
            );
        }
    }
}
