//! This tab displays a message
//!
//! The message could the project's remarks or a customized message.
//!

//TODO: implement a way to despawn the message entity
//TODO: add message support for any components. Trips, entries, routes, etc.

use super::Tab;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::{Component, Entity};
use egui::{Frame, ScrollArea, TextEdit};
use paiagram_core::settings::ProjectSettings;

#[derive(Component)]
pub(crate) struct TextMessage(pub String);

#[derive(MapEntities, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub(crate) struct TextTab {
    #[entities]
    pub entity: Option<Entity>,
}

impl Tab for TextTab {
    const NAME: &'static str = "Text message";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        let mut show = |buf| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    TextEdit::multiline(buf)
                        .hint_text("Enter your message...")
                        .frame(Frame::new()),
                );
            })
        };
        match self.entity {
            None => {
                let mut settings = world.resource_mut::<ProjectSettings>();
                show(&mut settings.remarks);
            }
            Some(e) if let Some(mut text) = world.get_mut::<TextMessage>(e) => {
                show(&mut text.0);
            }
            Some(_) => {
                ui.label("???");
            }
        };
    }
}
