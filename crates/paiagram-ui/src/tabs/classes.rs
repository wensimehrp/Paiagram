use super::Tab;
use bevy::prelude::*;
use egui::Ui;
use moonshine_core::prelude::MapEntities;
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

fn list_classes(InMut(ui): InMut<Ui>, mut class_q: Query<(&Class, &Name, &mut DisplayedStroke)>) {
    egui::Grid::new("class grid").num_columns(2).show(ui, |ui| {
        for (class, name, mut stroke) in class_q.iter_mut() {
            ui.label(name.as_str());
            ui.add(&mut stroke.color);
            ui.end_row();
        }
    });
}
