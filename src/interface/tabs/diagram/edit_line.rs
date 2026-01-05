use bevy::{
    ecs::{
        entity::Entity,
        name::Name,
        query::With,
        system::{In, InMut, Local, Query},
    },
    log::info,
};
use egui::{Color32, Popup, Pos2, ScrollArea, Sense, Stroke, Ui, UiBuilder, Vec2};
use moonshine_core::kind::Instance;

use crate::intervals::Station;

pub fn edit_line(
    (InMut(ui), In(displayed_line_entity)): (InMut<Ui>, In<Entity>),
    stations: Query<(Entity, &Name), With<Station>>,
    mut displayed_lines: Query<&mut super::DisplayedLine>,
) {
    let mut insertion: Option<(usize, Instance<Station>)> = None;
    let mut deletion: Option<usize> = None;
    let Ok(mut displayed_line) = displayed_lines.get_mut(displayed_line_entity) else {
        ui.label("Error: Displayed line not found.");
        return;
    };
    ui.spacing_mut().item_spacing.y = 0.0;
    let label_height = 20.0;
    let addition_button_height = 30.0;
    let addition_button_offset = 40.0;
    ui.painter().line_segment(
        [
            Pos2 {
                x: ui.max_rect().left() + addition_button_offset,
                y: ui.min_rect().bottom(),
            },
            Pos2 {
                x: ui.max_rect().left() + addition_button_offset,
                y: ui.min_rect().bottom()
                    + (label_height + addition_button_height)
                        * (displayed_line.stations().len() + 1) as f32,
            },
        ],
        ui.visuals().widgets.hovered.bg_stroke,
    );

    let mut add_station_between = |ui: &mut Ui, index: usize| {
        let (rect, resp) = ui.allocate_exact_size(
            Vec2 {
                x: ui.available_width(),
                y: addition_button_height,
            },
            Sense::click(),
        );
        let stroke = if resp.interact_pointer_pos().is_some() {
            ui.visuals().widgets.hovered.fg_stroke
        } else if resp.hovered() {
            ui.visuals().widgets.hovered.bg_stroke
        } else {
            Stroke::NONE
        };
        let fill = if resp.hovered() || resp.interact_pointer_pos().is_some() {
            ui.visuals().window_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .line_segment([rect.left_center(), rect.right_center()], stroke);
        ui.painter().circle(
            rect.left_center()
                + Vec2 {
                    x: addition_button_offset,
                    y: 0.0,
                },
            6.0,
            fill,
            stroke,
        );
        Popup::menu(&resp).show(|ui| {
            // TODO: improve station selection UI
            ScrollArea::vertical().show(ui, |ui| {
                for (entity, name) in stations.iter() {
                    if ui.button(name.as_str()).clicked() {
                        insertion = Some((index, unsafe { Instance::from_entity_unchecked(entity) }));
                    }
                }
            });
        });
    };

    let station_names = displayed_line
        .stations()
        .iter()
        .copied()
        .map(|(e, _)| stations.get(e.entity()).map_or("<Unknown>", |(_, n)| n.as_str()));
    add_station_between(ui, 0);
    for (i, name) in station_names.enumerate() {
        let (rect, resp) = ui.allocate_exact_size(
            Vec2 {
                x: ui.available_width(),
                y: label_height,
            },
            Sense::click(),
        );
        ui.scope_builder(UiBuilder::new().max_rect(rect).sense(resp.sense), |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(addition_button_offset + 8.0 + 10.0);
                ui.label(name)
            });
        });
        ui.painter().circle(
            rect.left_center()
                + Vec2 {
                    x: addition_button_offset,
                    y: 0.0,
                },
            8.0,
            ui.visuals().widgets.hovered.bg_fill,
            ui.visuals().widgets.hovered.bg_stroke,
        );
        add_station_between(ui, i + 1);
    }
    if let Some((index, entity)) = insertion {
        info!("Inserting station at index {index}");
        // TODO: fix the 10.0 thingy
        displayed_line.insert(index, (entity, 10.0));
    }
}
