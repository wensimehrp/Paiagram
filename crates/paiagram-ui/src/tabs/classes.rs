use crate::{OpenOrFocus, tabs::trip::TripTab};

use super::Tab;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use egui::{Button, CollapsingHeader, ScrollArea, Ui};
use paiagram_core::trip::class::{Class, DisplayedStroke};
use serde::{Deserialize, Serialize};

#[derive(Default, PartialEq, Clone, Serialize, Deserialize, MapEntities)]
pub struct ClassesTab;

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        world.run_system_cached_with(list_classes, ui).unwrap();
    }
}

fn list_classes(
    InMut(ui): InMut<Ui>,
    mut class_q: Query<(Entity, &Class, &Name, &mut DisplayedStroke)>,
    entity_name_q: Query<(Entity, &Name)>,
    mut commands: Commands,
) {
    let mut itoa_buffer = itoa::Buffer::new();
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("class grid").num_columns(3).show(ui, |ui| {
            ui.label("Class name");
            ui.label("Count");
            ui.label("Color");
            ui.end_row();
            for (class_entity, class, name, mut stroke) in class_q.iter_mut() {
                CollapsingHeader::new(name.as_str())
                    .id_salt(class_entity)
                    .show(ui, |ui| {
                        for (trip_entity, name) in
                            entity_name_q.iter_many(class.as_trips().iter().copied())
                        {
                            if ui
                                .add_sized(ui.available_size(), Button::new(name.as_str()))
                                .clicked()
                            {
                                commands.write_message(OpenOrFocus(crate::MainTab::Trip(
                                    TripTab::new(trip_entity),
                                )));
                            }
                        }
                    });
                let printed = itoa_buffer.format(class.as_trips().len());
                ui.label(printed);
                ui.add(&mut stroke.color);
                ui.end_row();
            }
        });
    });
}
