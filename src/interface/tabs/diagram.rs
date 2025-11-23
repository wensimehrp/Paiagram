use super::PageCache;
use crate::{
    intervals::Station,
    lines::DisplayedLine,
    units::time::TimetableTime,
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::prelude::*;
use egui::{
    Color32, Context, CornerRadius, Frame, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2, Window,
    emath, vec2,
};

pub struct DiagramPageCache {
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
    view_offset: Vec2,
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
        }
    }
}

pub fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    mut displayed_lines: Populated<&mut DisplayedLine>,
    vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: Query<&TimetableEntry>,
    station_names: Query<&Name, With<Station>>,
    mut page_cache: Local<PageCache<DiagramPageCache>>,
    // required for animations
    time: Res<Time>,
) {
    let Ok(displayed_line) = displayed_lines.get(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };
    let page_cache =
        page_cache.get_mut_or_insert_with(displayed_line_entity, DiagramPageCache::default);
    ui.horizontal(|ui| {
        ui.add(&mut page_cache.stroke);
    });
    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;
    Frame::canvas(ui.style()).show(ui, |ui| {
        let (mut response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

        page_cache.view_offset -= response.drag_delta();
        page_cache.view_offset.y = page_cache.view_offset.y.clamp(-10.0, f32::MAX);
        page_cache.view_offset.x = page_cache.view_offset.x.clamp(-1000.0, f32::MAX);
        if page_cache.view_offset.x < -100.0 && response.total_drag_delta().is_none() {
            let target = -100.0;
            let speed = 8.0;
            let t = (1.0 - (-speed * time.delta_secs()).exp()).clamp(0.0, 1.0);
            page_cache.view_offset.x += (target - page_cache.view_offset.x) * t;
            ui.ctx().request_repaint();
        }
        for t in 0..=((response.rect.right() - response.rect.left()) / 100.0) as i32 + 1 {
            painter.vline(
                response.rect.left() + 100.0 * t as f32 - page_cache.view_offset.x % 100.0,
                response.rect.top()..=response.rect.bottom(),
                page_cache.stroke,
            );
        }
        let mut current_height = response.rect.top();
        let mut heights: Vec<(Entity, f32)> = Vec::new();
        let station_lines = displayed_line.0.iter().map(|l| {
            current_height += l.1.0.log2().max(1.0) * 15f32;
            heights.push((l.0, current_height));
            egui::Shape::hline(
                response.rect.left()..=response.rect.right(),
                current_height - page_cache.view_offset.y,
                page_cache.stroke,
            )
        });
        painter.extend(station_lines);
        for (entity, name, schedule) in vehicles {}
        let font_id = egui::FontId::default();
        let text_color = ui.visuals().text_color();
        let bg_color = ui.visuals().window_fill;

        for (name, h) in heights.iter().filter_map(|(s, h)| {
            let Ok(name) = station_names.get(*s) else {
                return None;
            };
            Some((name.as_str(), h))
        }) {
            let galley = painter.layout_no_wrap(name.to_string(), font_id.clone(), text_color);
            let rect = Rect::from_min_size(
                Pos2::new(
                    response.rect.left(),
                    h - page_cache.view_offset.y - galley.size().y / 2.0 - 2.0,
                ),
                galley.size() + vec2(8.0, 4.0),
            );

            painter.rect_filled(rect, CornerRadius::same(2), bg_color);
            painter.galley(rect.min + vec2(4.0, 2.0), galley, text_color);
        }
        let time_range = {
            let time_min = (page_cache.view_offset.x / 100.0) as i32;
            let time_max = (response.rect.right() / 100.0) as i32 + time_min;
            time_min..=time_max
        };
        for offset in 0..=((response.rect.right() - response.rect.left()) / 100.0) as i32 + 1 {
            let i = ((page_cache.view_offset.x) / 100.0) as i32 + offset;
            if i < 0 {
                continue;
            }
            let galley = painter.layout_no_wrap(i.to_string(), font_id.clone(), text_color);
            let center_pos = response.rect.min
                + vec2(
                    100.0 * offset as f32 - page_cache.view_offset.x % 100.0,
                    galley.size().y / 2.0 + 2.0,
                );
            let rect = Rect::from_center_size(center_pos, galley.size() + vec2(8.0, 4.0));

            painter.rect_filled(rect, CornerRadius::same(2), bg_color);
            painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);
        }
        response
    });
}
