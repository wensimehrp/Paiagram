//! This tab displays a message
//!
//! The message could the project's remarks or a customized message.
//! Additionally, this tab supports displaying commonmark strings.

//TODO: implement a way to despawn the message entity
//TODO: add message support for any components. Trips, entries, routes, etc.

use std::sync::LazyLock;

use super::Tab;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::{Component, Entity};
use egui::mutex::Mutex;
use egui::{Frame, ScrollArea, TextEdit};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use paiagram_core::settings::ProjectSettings;

#[derive(Component)]
pub(crate) struct TextMessage(pub String);

#[derive(MapEntities, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct TextTab {
    #[entities]
    pub entity: Option<Entity>,
    #[serde(skip, default)]
    editing: bool,
}

impl PartialEq for TextTab {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl TextTab {
    pub fn new(entity: Option<Entity>) -> Self {
        Self {
            entity,
            editing: false,
        }
    }
}

static CACHE: LazyLock<Mutex<CommonMarkCache>> =
    LazyLock::new(|| Mutex::new(CommonMarkCache::default()));

impl Tab for TextTab {
    const NAME: &'static str = "Text message";
    fn edit_display(&mut self, _world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        ui.label("You may use markdown here");
        self.editing ^= ui
            .button(if self.editing { "Finish edit" } else { "Edit" })
            .clicked();
    }
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        let mut show = |buf: &mut String| {
            egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    if self.editing {
                        ui.add_sized(
                            ui.available_size(),
                            TextEdit::multiline(buf)
                                .hint_text("Enter your message...")
                                .frame(Frame::new()),
                        );
                    } else {
                        let mut cache = CACHE.lock();
                        CommonMarkViewer::new().show(ui, &mut cache, buf);
                    }
                })
            });
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
