use crate::{
    lines::DisplayedLine,
    vehicles::entries::{TimetableEntry, VehicleSchedule},
};
use bevy::{prelude::*};
use egui::{
    Color32, Context, CornerRadius, Frame, Painter, Pos2, Rect, Sense, Stroke, Ui, Window, emath,
    vec2,
};

pub fn show_diagram(
    (InMut(ui), In(displayed_line_entity)): (InMut<egui::Ui>, In<Entity>),
    mut displayed_lines: Populated<&mut DisplayedLine>,
    vehicles: Populated<(Entity, &Name, &VehicleSchedule)>,
    timetable_entries: Populated<&TimetableEntry>,
    mut lines: Local<Vec<Vec<Pos2>>>,
    mut stroke: Local<Stroke>,
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
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::drag());

        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.square_proportions()),
            response.rect,
        );
        let from_screen = to_screen.inverse();

        if lines.is_empty() {
            lines.push(vec![]);
        }

        let current_line = lines.last_mut().unwrap();

        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let canvas_pos = from_screen * pointer_pos;
            if current_line.last() != Some(&canvas_pos) {
                current_line.push(canvas_pos);
                response.mark_changed();
            }
        } else if !current_line.is_empty() {
            lines.push(vec![]);
            response.mark_changed();
        }

        let shapes = lines.iter().filter(|line| line.len() >= 2).map(|line| {
            let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
            egui::Shape::line(points, *stroke)
        });

        painter.extend(shapes);
        response
    });
}
