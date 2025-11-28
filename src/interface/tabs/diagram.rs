use super::PageCache;
use crate::{
    intervals::Station,
    lines::DisplayedLine,
    units::time::TimetableTime,
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, Frame, Margin, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    emath::{self, RectTransform},
    vec2,
};

// Time and time-canvas related constants
const SECONDS_PER_WORLD_UNIT: f32 = 36.0; // world units -> seconds

pub struct DiagramPageCache {
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
    view_offset: Vec2,
    heights: Option<Vec<(Entity, f32)>>,
    // trackpad, mobile inputs, and scroll wheel
    zoom: Vec2,
    is_log: bool,
    last_interact_time: f64,
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
            lines: Vec::new(),
            stroke: Stroke {
                width: 1.0,
                color: Color32::BLACK,
            },
            view_offset: Vec2::default(),
            heights: None,
            last_interact_time: 0.0,
            is_log: true,
            zoom: vec2(1.0, 1.0),
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
    // required for animations
    time: Res<Time>,
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
            // Compute world rect visible on the canvas using pc.view_offset and pc.zoom.
            // pc.view_offset is the world position at the top-left of the canvas.
            let world_size = response.rect.size() / pc.zoom;
            let world_rect =
                Rect::from_min_size(Pos2::new(pc.view_offset.x, pc.view_offset.y), world_size);
            // Build transforms between world and screen coordinates. Use them to draw
            // everything consistently in world space so zoom & pan keep items aligned.
            let world_to_screen = emath::RectTransform::from_to(world_rect, response.rect);
            let screen_to_world = world_to_screen.inverse();
            let visible_stations = pc.get_visible_stations(world_rect.top()..world_rect.bottom());
            let visible_time_range =
                TimetableTime((world_rect.left() * SECONDS_PER_WORLD_UNIT) as i32)
                    ..TimetableTime((world_rect.right() * SECONDS_PER_WORLD_UNIT) as i32);
            let sec_per_pt = (visible_time_range.end - visible_time_range.start).0 as f32
                / response.rect.width();
            draw_station_lines(
                &mut painter,
                &pc,
                &world_rect,
                &world_to_screen,
                ui,
                visible_stations,
            );
            draw_time_lines(
                &mut painter,
                &pc,
                &world_rect,
                &world_to_screen,
                ui,
                sec_per_pt,
                SECONDS_PER_WORLD_UNIT,
            );
            if let Some(children) = &displayed_line.children {
                for (entity, name, schedule) in
                    children.iter().filter_map(|e| vehicles.get(*e).ok())
                {
                    let Some(visible_sets) =
                        schedule.get_entries_range(visible_time_range.clone(), &timetable_entries)
                    else {
                        continue;
                    };
                    for (initial_offset, set) in visible_sets {
                        let mut to_draw = Vec::with_capacity(set.len());
                        for (entry, timetable_entity) in set {
                            let (Some(ae), Some(de)) =
                                (entry.arrival_estimate, entry.departure_estimate)
                            else {
                                painter.line(to_draw.drain(..).collect(), pc.stroke);
                                continue;
                            };
                            let Some((_, h)) =
                                visible_stations.iter().find(|(s, _)| *s == entry.station)
                            else {
                                painter.line(to_draw.drain(..).collect(), pc.stroke);
                                continue;
                            };
                            let start = world_to_screen
                                * Pos2::new(
                                    (initial_offset.0 + ae.0) as f32 / SECONDS_PER_WORLD_UNIT,
                                    *h,
                                );
                            let end = world_to_screen
                                * Pos2::new(
                                    (initial_offset.0 + de.0) as f32 / SECONDS_PER_WORLD_UNIT,
                                    *h,
                                );
                            to_draw.push(start);
                            to_draw.push(end);
                        }
                        painter.line(to_draw, pc.stroke);
                    }
                }
            }

            let mut zoom_delta: Vec2 = Vec2::default();
            let mut translation_delta: Vec2 = Vec2::default();
            ui.input(|input| {
                zoom_delta = input.zoom_delta_2d();
                translation_delta = input.translation_delta();
            });
            if let Some(pos) = response.hover_pos() {
                let world_pos_before = (screen_to_world * pos).to_vec2();
                let new_zoom = pc.zoom * zoom_delta;
                pc.zoom.x = new_zoom.x.clamp(0.025, 2048.0);
                pc.zoom.y = new_zoom.y.clamp(0.025, 2048.0);
                let new_world_size = response.rect.size() / pc.zoom;
                let screen_t = (pos - response.rect.min) / response.rect.size();
                pc.view_offset = world_pos_before - screen_t * new_world_size;
            }
            pc.view_offset -= translation_delta / pc.zoom;
            pc.view_offset -= response.drag_delta() / pc.zoom;
            pc.view_offset.x = pc.view_offset.x.clamp(
                -366.0 * 86400.0 / SECONDS_PER_WORLD_UNIT,
                366.0 * 86400.0 / SECONDS_PER_WORLD_UNIT - response.rect.width() / pc.zoom.x,
            );
            // SAFETY: heights is guaranteed to be initialized
            let max_height = pc
                .heights
                .as_ref()
                .unwrap()
                .last()
                .map(|(_, h)| *h)
                .unwrap_or(0.0);
            pc.view_offset.y =
                if response.rect.height() / pc.zoom.y > (max_height + 200.0 / pc.zoom.y) {
                    (-response.rect.height() / pc.zoom.y + max_height) / 2.0
                } else {
                    pc.view_offset.y.clamp(
                        -100.0 / pc.zoom.y,
                        max_height - response.rect.height() / pc.zoom.y + 100.0 / pc.zoom.y,
                    )
                }
        });
}

fn draw_station_lines(
    painter: &mut Painter,
    pc: &DiagramPageCache,
    world_rect: &Rect,
    to_screen: &RectTransform,
    ui: &Ui,
    to_draw: &[(Entity, f32)],
) {
    let ppp = ui.pixels_per_point();
    for (station, height) in to_draw {
        let world_y = *height;
        let mut left = to_screen * Pos2::new(world_rect.left(), world_y);
        let mut right = to_screen * Pos2::new(world_rect.right(), world_y);
        pc.stroke.round_center_to_pixel(ppp, &mut left.y);
        right.y = left.y;
        painter.line(vec![left, right], pc.stroke);
    }
}

/// Draw vertical time lines and labels
pub fn draw_time_lines(
    painter: &mut Painter,
    pc: &DiagramPageCache,
    world_rect: &Rect,
    to_screen: &RectTransform,
    ui: &Ui,
    spp: f32,
    seconds_per_world_unit: f32,
) {
    const MIN_SPACING_PX: f32 = 48.0;
    const MAX_SPACING_PX: f32 = 256.0;
    const ELEMENT_DISPLAY_THRESHOLD: f32 = 0.08;
    let time_sizes: &[(
        i32,
        std::ops::Range<f32>,
        Box<dyn Fn(TimetableTime) -> String>,
    )] = &[
        // (size_in_seconds, min_spp .. max_spp)
        (
            3600 * 24,
            (3600.0 * 24.0) / MAX_SPACING_PX..f32::MAX,
            Box::new(|t| format!("{}", t)),
        ),
        (
            3600 * 4,
            (3600.0 * 4.0) / MAX_SPACING_PX..(3600.0 * 14.0) / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            3600,
            3600.0 / MAX_SPACING_PX..(3600.0 * 2.5) / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            1800,
            1800.0 / MAX_SPACING_PX..2100.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            0600,
            0600.0 / MAX_SPACING_PX..1200.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            0300,
            0300.0 / MAX_SPACING_PX..0450.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            0060,
            0060.0 / MAX_SPACING_PX..0180.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}:{:02}", t.to_hmsd().0, t.to_hmsd().1)),
        ),
        (
            0030,
            0030.0 / MAX_SPACING_PX..0045.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}", t.to_hmsd().2)),
        ),
        (
            0010,
            0010.0 / MAX_SPACING_PX..0020.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{:02}", t.to_hmsd().2)),
        ),
        (
            0001,
            0001.0 / MAX_SPACING_PX..0005.0 / MIN_SPACING_PX,
            Box::new(|t| format!("{}", t.to_hmsd().2 % 10)),
        ),
    ];

    // --- Drawing Logic ---

    let ppp = ui.pixels_per_point();
    let mut drawn = Vec::new();

    // Iterate over all time sizes that are visible at the current zoom (spp)
    for (size_in_seconds, spp_range, format_fn) in
        time_sizes.iter().filter(|(_, range, _)| range.end > spp)
    {
        let world_length = *size_in_seconds as f32 / seconds_per_world_unit;
        let x_start = (world_rect.left() / world_length) as i32;
        let x_end = (world_rect.right() / world_length) as i32 + 1;
        if x_start - x_end == 0 {
            continue;
        }
        let line_strength =
            1.0 - ((spp - spp_range.start) / (spp_range.end - spp_range.start)).clamp(0.0, 1.0);
        if line_strength < ELEMENT_DISPLAY_THRESHOLD {
            continue;
        }
        let text_strength = line_strength.powi(5);
        let mut stroke = pc.stroke;
        let text_color = stroke.color.gamma_multiply(text_strength);
        stroke.color = stroke.color.gamma_multiply(line_strength);
        for x_idx in x_start..=x_end {
            let world_x = x_idx as f32 * world_length;
            let current_time = size_in_seconds * x_idx;
            if drawn.contains(&current_time) {
                continue;
            }
            let mut top = to_screen * Pos2::new(world_x, world_rect.top());
            let mut bot = to_screen * Pos2::new(world_x, world_rect.bottom());
            stroke.round_center_to_pixel(ppp, &mut top.x);
            bot.x = top.x;
            // draw the label
            if text_strength >= ELEMENT_DISPLAY_THRESHOLD {
                let label = painter.layout_no_wrap(
                    format_fn(TimetableTime(current_time)),
                    egui::FontId::monospace(13.0),
                    text_color,
                );
                painter.galley(
                    top - Vec2::new(label.size().x / 2.0, 0.0),
                    label.clone(),
                    Color32::BLACK,
                );
                painter.galley(
                    bot + Vec2::new(label.size().x / 2.0, 0.0),
                    label,
                    Color32::BLACK,
                );
            }
            painter.line_segment([top, bot], stroke);
            drawn.push(current_time);
        }
    }
}
