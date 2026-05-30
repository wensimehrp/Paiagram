use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use egui::{Button, Layout, Panel, ScrollArea, Ui, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::trip::class::{Class, DisplayedStroke};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::OpenOrFocus;
use crate::tabs::trip::TripTab;

#[derive(Default, PartialEq, Clone, Serialize, Deserialize, MapEntities)]
pub(crate) struct ClassesTab {
    #[serde(skip)]
    selected_class: Option<Entity>,
    #[serde(skip)]
    hovered_trip: Option<Entity>,
}

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn title(&self) -> WidgetText {
        tr!("tab-classes").into()
    }
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
        .default_size(ui.available_width() / 3.0)
        .resizable(true)
        .show_inside(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.label("Trips");
                let Some(class_entity) = tab.selected_class else {
                    return;
                };
                let Ok((_, class, _, _)) = class_q.get(class_entity) else {
                    return;
                };
                let mut hovered = false;
                ui.with_layout(Layout::default().with_cross_justify(true), |ui| {
                    for (trip_entity, name) in
                        entity_name_q.iter_many(class.as_trips().iter().copied())
                    {
                        let res = ui.button(name.as_str());
                        if res.hovered() {
                            hovered = true;
                            tab.hovered_trip = Some(trip_entity);
                        }
                        if res.clicked() {
                            commands.write_message(OpenOrFocus(crate::MainTab::Trip(
                                TripTab::new(trip_entity),
                            )));
                        }
                    }
                });
                if !hovered {
                    tab.hovered_trip = None;
                }
            });
        });

    let mut itoa_buffer = itoa::Buffer::new();
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("class grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr!("classes-name"));
                ui.label(tr!("classes-count"));
                ui.label(tr!("classes-color"));
                ui.end_row();
                for (class_entity, class, class_name, mut stroke) in class_q.iter_mut() {
                    ui.allocate_ui_with_layout(
                        vec2(200.0, 24.0),
                        Layout::default().with_cross_justify(true),
                        |ui| {
                            ui.selectable_value(
                                &mut tab.selected_class,
                                Some(class_entity),
                                class_name.as_str(),
                            );
                        },
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
