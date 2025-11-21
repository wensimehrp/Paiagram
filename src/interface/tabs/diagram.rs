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

pub fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    mut displayed_lines: Populated<&mut DisplayedLine>,
    vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: Query<&TimetableEntry>,
    station_names: Query<&Name, With<Station>>,
    mut lines: Local<Vec<Vec<Pos2>>>,
    mut stroke: Local<Stroke>,
    mut view_offset: Local<Vec2>,
) {
    let Ok(displayed_line) = displayed_lines.get(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };
    ui.horizontal(|ui| {
        ui.label("Stroke:");
        ui.add(&mut *stroke);
        ui.separator();
        if ui.button("Clear Painting").clicked() {
            lines.clear();
        }
    });
    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;
    Frame::canvas(ui.style()).show(ui, |ui| {
        let (mut response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

        *view_offset -= response.drag_delta();
        let vlines = (0..=24).map(|t| {
            egui::Shape::vline(
                response.rect.left() + 100.0 * t as f32 - view_offset.x,
                response.rect.top()..=response.rect.bottom(),
                *stroke,
            )
        });
        painter.extend(vlines);
        let mut current_height = response.rect.top();
        let mut heights: Vec<(Entity, f32)> = Vec::new();
        let station_lines = displayed_line.0.iter().map(|l| {
            current_height += l.1.0.log2().max(1.0) * 15f32;
            heights.push((l.0, current_height));
            egui::Shape::hline(
                response.rect.left()..=response.rect.right(),
                current_height - view_offset.y,
                *stroke,
            )
        });
        painter.extend(station_lines);
        for (entity, name, schedule) in vehicles {
            let mut points = Vec::new();
            for (a, d, s) in schedule
                .get_entries_range(TimetableTime(0)..TimetableTime(1), &timetable_entries)
                .iter()
                .filter_map(|e| {
                    let (Some(a), Some(d)) = (e.arrival_estimate, e.departure_estimate) else {
                        return None;
                    };
                    Some((a, d, e.station))
                })
            {
                let Some((_, h)) = heights.iter().find(|(e, _)| *e == s) else {
                    painter.line(points.drain(..).collect(), *stroke);
                    continue;
                };
                points.push(Pos2::new(
                    a.0 as f32 / 36f32 - view_offset.x,
                    *h - view_offset.y,
                ));
            }
            painter.line(points, *stroke);
        }
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
                    h - view_offset.y - galley.size().y / 2.0 - 2.0,
                ),
                galley.size() + vec2(8.0, 4.0),
            );

            painter.rect_filled(rect, CornerRadius::same(2), bg_color);
            painter.galley(rect.min + vec2(4.0, 2.0), galley, text_color);
        }
        for i in 0..=24 {
            let center_pos = response.rect.min + vec2(100.0 * i as f32 - view_offset.x, 5.0);
            let galley = painter.layout_no_wrap(i.to_string(), font_id.clone(), text_color);
            let rect = Rect::from_center_size(center_pos, galley.size() + vec2(8.0, 4.0));

            painter.rect_filled(rect, CornerRadius::same(2), bg_color);
            painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);
        }
        response
    });
}
