use super::PageCache;
use crate::{
    intervals::Station,
    lines::DisplayedLine,
    units::time::{Duration, TimetableTime},
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui,
    Vec2,
    emath::{self, RectTransform},
    response, vec2,
};

// Time and time-canvas related constants
const SECONDS_PER_WORLD_UNIT: f64 = 1.0; // world units -> seconds
const TICKS_PER_SECOND: i64 = 100;
const TICKS_PER_WORLD_UNIT: f64 = SECONDS_PER_WORLD_UNIT * TICKS_PER_SECOND as f64;

pub struct DiagramPageCache {
    active_lines: Vec<(Vec<Vec<Pos2>>, Stroke)>,
    selected_line: Option<Entity>,
    stroke: Stroke,
    tick_offset: i64,
    vertical_offset: f32,
    heights: Option<Vec<(Entity, f32)>>,
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
            active_lines: Vec::new(),
            selected_line: None,
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
    vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: Query<&TimetableEntry>,
    station_names: Query<&Name, With<Station>>,
    mut page_cache: Local<PageCache<Entity, DiagramPageCache>>,
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
            let visible_stations = pc.get_visible_stations(vertical_visible.clone());
            draw_station_lines(
                pc.vertical_offset,
                pc.stroke,
                &mut painter,
                &response.rect,
                pc.zoom.y,
                visible_stations,
                ui.pixels_per_point(),
            );
            draw_time_lines(
                pc.tick_offset,
                pc.stroke,
                &mut painter,
                &response.rect,
                ticks_per_screen_unit,
                &horizontal_visible,
                ui.pixels_per_point(),
            );
            let mut active_lines = Vec::new();
            if let Some(children) = displayed_line.children.as_ref() {
                for (vehicle_entity, name, schedule) in
                    children.iter().filter_map(|e| vehicles.get(*e).ok())
                {
                    let Some(visible_sets) = schedule.get_entries_range(
                        TimetableTime((horizontal_visible.start / TICKS_PER_SECOND) as i32)
                            ..TimetableTime((horizontal_visible.end / TICKS_PER_SECOND) as i32),
                        &timetable_entries,
                    ) else {
                        continue;
                    };
                    for (initial_offset, set) in visible_sets {
                        let mut all_to_draw = Vec::new();
                        let mut current_group = (Vec::new(), Vec::new());
                        for (entry, timetable_entity) in set {
                            let (Some(ae), Some(de)) =
                                (entry.arrival_estimate, entry.departure_estimate)
                            else {
                                all_to_draw.push(std::mem::take(&mut current_group));
                                continue;
                            };
                            let Some((_, h)) =
                                visible_stations.iter().find(|(s, _)| *s == entry.station)
                            else {
                                all_to_draw.push(std::mem::take(&mut current_group));
                                continue;
                            };
                            let mut draw_height =
                                (*h - pc.vertical_offset) * pc.zoom.y + response.rect.top();
                            let start = Pos2 {
                                x: ticks_to_screen_x(
                                    (ae.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
                                    &response.rect,
                                    ticks_per_screen_unit,
                                    pc.tick_offset,
                                ),
                                y: draw_height,
                            };
                            let end = Pos2 {
                                x: ticks_to_screen_x(
                                    (de.0 + initial_offset.0) as i64 * TICKS_PER_SECOND,
                                    &response.rect,
                                    ticks_per_screen_unit,
                                    pc.tick_offset,
                                ),
                                y: draw_height,
                            };
                            current_group.0.push(start);
                            current_group.0.push(end);
                            current_group.1.push(ae);
                            current_group.1.push(de);
                        }
                        all_to_draw.push(current_group);
                        active_lines.push((all_to_draw, pc.stroke, vehicle_entity));
                    }
                }
            }
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let mut found = false;
                'check_selected: for (lines, _, vehicle_entity) in active_lines.iter() {
                    for (line, entries) in lines {
                        for w in line.windows(2) {
                            let [curr, next] = w else {
                                continue;
                            };
                            let a = pos.x - curr.x;
                            let b = pos.y - curr.y;
                            let c = next.x - curr.x;
                            let d = next.y - curr.y;
                            let dot = a * c + b * d;
                            let len_sq = c * c + d * d;
                            if len_sq == 0.0 {
                                continue;
                            }
                            let t = (dot / len_sq).clamp(0.0, 1.0);
                            let px = curr.x + t * c;
                            let py = curr.y + t * d;
                            let dx = pos.x - px;
                            let dy = pos.y - py;
                            // range is 5.0
                            if dx * dx + dy * dy < 25.0 {
                                pc.selected_line = Some(*vehicle_entity);
                                found = true;
                                break 'check_selected;
                            }
                        }
                    }
                }
                if !found {
                    pc.selected_line = None;
                }
            }
            for (lines, mut stroke, vehicle_entity) in active_lines {
                const SIGNAL_STROKE: Stroke = Stroke {
                    width: 2.0,
                    color: Color32::ORANGE,
                };
                if let Some(selected_entity) = pc.selected_line
                    && selected_entity == vehicle_entity
                {
                    stroke.width *= 5.0;
                    for (line, entries) in lines.iter() {
                        for idx in 0..line.len().saturating_sub(2) {
                            let mut curr_pos = line[idx];
                            let mut next_pos = line[idx + 1];
                            curr_pos.y += 5.0;
                            next_pos.y += 5.0;
                            SIGNAL_STROKE
                                .round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.x);
                            SIGNAL_STROKE
                                .round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.y);
                            SIGNAL_STROKE
                                .round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.x);
                            SIGNAL_STROKE
                                .round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.y);
                            let duration = entries[idx + 1] - entries[idx];
                            let points = if next_pos.y <= curr_pos.y {
                                vec![curr_pos, Pos2::new(next_pos.x, curr_pos.y), next_pos]
                            } else {
                                vec![curr_pos, Pos2::new(curr_pos.x, next_pos.y), next_pos]
                            };
                            painter.add(Shape::dashed_line(&points, SIGNAL_STROKE, 6.0, 3.0));
                            if duration == Duration(0) {
                                continue;
                            }
                            let duration_text = painter.layout_no_wrap(
                                {
                                    let time = duration.to_hms();
                                    format!("{}:{:02}", time.0 * 60 + time.1, time.2)
                                },
                                egui::FontId::monospace(15.0),
                                Color32::ORANGE,
                            );
                            painter.galley(
                                Pos2 {
                                    x: (curr_pos.x + next_pos.x - duration_text.size().x) / 2.0,
                                    y: curr_pos.y.max(next_pos.y) + 1.0,
                                },
                                duration_text,
                                Color32::ORANGE,
                            );
                        }
                    }
                };
                for (line, _) in lines {
                    painter.line(line, stroke);
                }
            }
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
                let world_height_before = response.rect.height() as f64 / old_zoom.y as f64;
                let world_height_after = response.rect.height() as f64 / new_zoom.y as f64;
                let world_pos_before_y =
                    pc.vertical_offset as f64 + rel_pos.y as f64 * world_height_before;
                let new_vertical_offset =
                    (world_pos_before_y - rel_pos.y as f64 * world_height_after) as f32;
                pc.zoom = new_zoom;
                pc.tick_offset = new_tick_offset;
                pc.vertical_offset = new_vertical_offset;
            }
            pc.tick_offset -= (ticks_per_screen_unit
                * (response.drag_delta().x + translation_delta.x) as f64)
                as i64;
            pc.vertical_offset -= (response.drag_delta().y + translation_delta.y) / pc.zoom.y;
            pc.tick_offset = pc.tick_offset.clamp(
                -366 * 86400 * TICKS_PER_SECOND,
                366 * 86400 * TICKS_PER_SECOND,
            );
            const TOP_BOTTOM_PADDING: f32 = 30.0;
            let max_height = pc
                .heights
                .as_ref()
                .unwrap()
                .last()
                .map(|(_, h)| *h)
                .unwrap_or(0.0);
            pc.vertical_offset = if response.rect.height() / pc.zoom.y
                > (max_height + TOP_BOTTOM_PADDING * 2.0 / pc.zoom.y)
            {
                (-response.rect.height() / pc.zoom.y + max_height) / 2.0
            } else {
                pc.vertical_offset.clamp(
                    -TOP_BOTTOM_PADDING / pc.zoom.y,
                    max_height - response.rect.height() / pc.zoom.y
                        + TOP_BOTTOM_PADDING / pc.zoom.y,
                )
            }
        });
}

fn draw_station_lines(
    vertical_offset: f32,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    zoom: f32,
    to_draw: &[(Entity, f32)],
    pixels_per_point: f32,
) {
    // Guard against invalid zoom
    for (entity, height) in to_draw {
        let mut draw_height = (*height - vertical_offset) * zoom + screen_rect.top();
        stroke.round_center_to_pixel(pixels_per_point, &mut draw_height);
        painter.hline(
            screen_rect.left()..=screen_rect.right(),
            draw_height,
            stroke,
        );
    }
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
fn draw_time_lines(
    tick_offset: i64,
    stroke: Stroke,
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
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start - visible_ticks.start.rem_euclid(*spacing) - spacing;
        let mut tick = first;
        let strength = (((*spacing as f64 / ticks_per_screen_unit * 1.5) - MIN_SCREEN_WIDTH)
            / (MAX_SCREEN_WIDTH - MIN_SCREEN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        let mut current_stroke = stroke;
        current_stroke.color = current_stroke.color.gamma_multiply(strength as f32);
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
