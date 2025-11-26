use super::PageCache;
use crate::{
    intervals::Station,
    lines::DisplayedLine,
    units::time::TimetableTime,
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, Frame, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
    emath::{self, RectTransform},
    vec2,
};

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
    Frame::canvas(ui.style()).show(ui, |ui| {
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
        draw_station_lines(
            &mut painter,
            &pc,
            &world_rect,
            &world_to_screen,
            ui,
            visible_stations,
        );
        draw_time_lines(&mut painter, &pc, &world_rect, &world_to_screen, ui);

        if let Some(children) = &displayed_line.children {
            for (entity, name, schedule) in children.iter().filter_map(|e| vehicles.get(*e).ok()) {
                let Some(visible_sets) = schedule.get_entries_range(
                    TimetableTime((world_rect.left() * 36.0) as i32)
                        ..TimetableTime((world_rect.right() * 36.0) as i32),
                    &timetable_entries,
                ) else {
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
                            * Pos2::new((initial_offset.0 + ae.0) as f32 / 36.0, *h);
                        let end = world_to_screen
                            * Pos2::new((initial_offset.0 + de.0) as f32 / 36.0, *h);
                        to_draw.push(start);
                        to_draw.push(end);
                    }
                    painter.line(to_draw, pc.stroke);
                }
            }
        }

        ui.input(|input| {
            // Zooming: keep the world point under the cursor pinned to the same
            // screen position. Convert the cursor pos into world coords to
            // compute a new view_offset after applying the zoom delta.
            let delta = input.zoom_delta_2d();
            if let Some(pos) = input.pointer.hover_pos() {
                let world_pos_before = (screen_to_world * pos).to_vec2();

                let new_zoom = pc.zoom * delta;
                // clamp to reasonable bounds to avoid division by zero or extreme zoom
                pc.zoom.x = new_zoom.x.clamp(0.025, 128.0);
                pc.zoom.y = new_zoom.y.clamp(0.025, 128.0);

                let new_world_size = response.rect.size() / pc.zoom;
                let screen_t = (pos - response.rect.min) / response.rect.size();
                // make view_offset such that world_pos_before remains at 'pos'
                pc.view_offset = world_pos_before - screen_t * new_world_size;
            }
            // Panning: translation_delta is in screen pixels; convert to world by
            // dividing by zoom. If zoom is per-axis, handle component-wise.
            let t_delta = input.translation_delta();
            if t_delta != Vec2::ZERO {
                pc.view_offset -= t_delta / pc.zoom;
            }
        });
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
        pc.stroke.round_center_to_pixel(ppp, &mut left.x);
        pc.stroke.round_center_to_pixel(ppp, &mut right.x);
        painter.line(vec![left, right], pc.stroke);
    }
}

fn draw_time_lines(
    painter: &mut Painter,
    pc: &DiagramPageCache,
    world_rect: &Rect,
    to_screen: &RectTransform,
    ui: &Ui,
) {
    let grid_world = Vec2::splat(100.0);
    let x_start = (world_rect.left() / grid_world.x).floor() as i32 - 1;
    let x_end = (world_rect.right() / grid_world.x).ceil() as i32 + 1;
    let ppp = ui.pixels_per_point();
    for i in x_start..=x_end {
        let world_x = i as f32 * grid_world.x;
        let mut top = to_screen * Pos2::new(world_x, world_rect.top());
        let mut bot = to_screen * Pos2::new(world_x, world_rect.bottom());
        pc.stroke.round_center_to_pixel(ppp, &mut top.x);
        pc.stroke.round_center_to_pixel(ppp, &mut bot.x);
        painter.line(vec![top, bot], pc.stroke);
    }
}
