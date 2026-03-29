use bevy::prelude::*;
use egui::{Color32, FontId, Painter, Pos2, Stroke, Visuals};
use paiagram_core::units::time::{Tick, TimetableTime};

use crate::tabs::{Navigatable, diagram::DiagramTabNavigation};

pub fn draw_station_lines(
    painter: &mut Painter,
    navi: &DiagramTabNavigation,
    to_draw: impl Iterator<Item = (Entity, f32)>,
    visuals: &Visuals,
    world: &World,
) {
    // TODO: implement per-station stroke
    let stroke = Stroke {
        width: 0.6,
        color: visuals.window_stroke().color,
    };
    for (station_entity, raw_height) in to_draw {
        let mut height = navi.logical_y_to_screen_y(raw_height as f64);
        stroke.round_center_to_pixel(painter.pixels_per_point(), &mut height);
        painter.hline(
            navi.visible_rect.left()..=navi.visible_rect.right(),
            height,
            stroke,
        );
        let layout = painter.layout_no_wrap(
            world
                .get::<Name>(station_entity)
                .map_or("<Unknown>".to_string(), Name::to_string),
            egui::FontId::proportional(13.0),
            visuals.text_color(),
        );
        let layout_pos = Pos2 {
            x: navi.visible_rect.left(),
            y: height - layout.size().y,
        };
        painter.galley(layout_pos, layout, visuals.text_color());
    }
}

/// Draw vertical time lines and labels
pub fn draw_time_lines(painter: &mut Painter, navi: &DiagramTabNavigation) {
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
