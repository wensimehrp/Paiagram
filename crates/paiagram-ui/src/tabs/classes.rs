use crate::{OpenOrFocus, tabs::trip::TripTab};

use super::Tab;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use egui::{Button, Panel, ScrollArea, Ui};
use paiagram_core::trip::class::{Class, DisplayedStroke};
use serde::{Deserialize, Serialize};

#[derive(Default, PartialEq, Clone, Serialize, Deserialize, MapEntities)]
pub(crate) struct ClassesTab {
    #[serde(skip)]
    selected_class: Option<Entity>,
    #[serde(skip)]
    hovered_trip: Option<Entity>,
}

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        world
            .run_system_cached_with(list_classes, (ui, self))
            .unwrap();
    }
}

fn list_classes(
    (InMut(ui), InMut(tab)): (InMut<Ui>, InMut<ClassesTab>),
    mut class_q: Query<(Entity, &Class, &Name, &mut DisplayedStroke)>,
    entity_name_q: Query<(Entity, &Name)>,
    mut commands: Commands,
) {
    Panel::right(ui.id().with("first"))
        .exact_size(ui.available_width() / 3.0)
        .resizable(false)
        .show_inside(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                let Some(class_entity) = tab.selected_class else {
                    return;
                };
                let Ok((_, class, _, _)) = class_q.get(class_entity) else {
                    return;
                };
                let mut hovered = false;
                for (trip_entity, name) in entity_name_q.iter_many(class.as_trips().iter().copied())
                {
                    let res =
                        ui.add_sized([ui.available_width(), 24.0], Button::new(name.as_str()));
                    if res.hovered() {
                        hovered = true;
                        tab.hovered_trip = Some(trip_entity);
                    }
                    if res.clicked() {
                        commands.write_message(OpenOrFocus(crate::MainTab::Trip(TripTab::new(
                            trip_entity,
                        ))));
                    }
                }
                if !hovered {
                    tab.hovered_trip = None;
                }
            });
        });

    let mut itoa_buffer = itoa::Buffer::new();
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("class grid").num_columns(3).show(ui, |ui| {
            ui.label("Class name");
            ui.label("Count");
            ui.label("Color");
            ui.end_row();
            for (class_entity, class, class_name, mut stroke) in class_q.iter_mut() {
                ui.selectable_value(
                    &mut tab.selected_class,
                    Some(class_entity),
                    class_name.as_str(),
                );
                let printed = itoa_buffer.format(class.as_trips().len());
                ui.label(printed);
                ui.add(&mut stroke.color);
                ui.end_row();
            }
        });
    });

    ScrollArea::vertical().id_salt("third").show(ui, |ui| {
        let Some(trip_entity) = tab.hovered_trip else {
            return;
        };
        ui.label(trip_entity.to_string());
    });
}
