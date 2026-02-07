use crate::units::time::TimetableTime;

use super::TICKS_PER_SECOND;
use bevy::prelude::*;
use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke};

pub fn draw_station_lines<'a>(
    vertical_offset: f32,
    painter: &mut Painter,
    screen_rect: Rect,
    zoom: f32,
    to_draw: impl Iterator<Item = (Entity, f32, &'a str)>,
    pixels_per_point: f32,
) {
    // TODO: implement per-station stroke
    let stroke = Stroke {
        width: 0.6,
        color: Color32::GRAY,
    };
    // Guard against invalid zoom
    for (_, height, name) in to_draw {
        let mut draw_height = (height - vertical_offset) * zoom + screen_rect.top();
        stroke.round_center_to_pixel(pixels_per_point, &mut draw_height);
        painter.hline(
            screen_rect.left()..=screen_rect.right(),
            draw_height,
            stroke,
        );
        let layout = painter.layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );
        let layout_pos = Pos2 {
            x: screen_rect.left(),
            y: draw_height - layout.size().y,
        };
        painter.galley(layout_pos, layout, Color32::WHITE);
    }
}

pub(super) fn ticks_to_screen_x(
    ticks: i64,
    screen_rect: Rect,
    ticks_per_screen_unit: f64,
    offset_ticks: i64,
) -> f32 {
    let base = (ticks - offset_ticks) as f64 / ticks_per_screen_unit;
    screen_rect.left() + base as f32
}

/// Draw vertical time lines and labels
pub fn draw_time_lines(
    tick_offset: i64,
    painter: &mut Painter,
    screen_rect: Rect,
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
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start - visible_ticks.start.rem_euclid(*spacing) - spacing;
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
        while tick <= visible_ticks.end {
            tick += *spacing;
            if drawn.contains(&tick) {
                continue;
            }
            let mut x = ticks_to_screen_x(tick, screen_rect, ticks_per_screen_unit, tick_offset);
            current_stroke.round_center_to_pixel(pixels_per_point, &mut x);
            painter.vline(x, screen_rect.top()..=screen_rect.bottom(), current_stroke);
            drawn.push(tick);
            let time = TimetableTime((tick / 100) as i32);
            let text = match i + first_visible_position {
                0..=2 => time.to_hmsd().2.to_string(),
                3..=8 => format!("{}:{:02}", time.to_hmsd().0, time.to_hmsd().1),
                _ => time.to_string(),
            };
            let label = painter.layout_no_wrap(text, FontId::monospace(13.0), current_stroke.color);
            painter.galley(
                Pos2 {
                    x: x - label.size().x / 2.0,
                    y: screen_rect.top(),
                },
                label,
                current_stroke.color,
            );
        }
    }
}
