use std::sync::Arc;

use egui::Ui;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct TextMessage(pub(crate) String);

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TextTab {
    message: Option<Arc<TextMessage>>,
    // If message is None, show project remarks
}

impl TextTab {
    pub(crate) fn new(message_id: Option<usize>) -> Self {
        // message_id was Entity in old code; now we just pass a placeholder
        Self { message: None }
    }
}

impl Default for TextTab {
    fn default() -> Self {
        Self { message: None }
    }
}

impl Tab for TextTab {
    const NAME: &'static str = "Text";
    fn title(&self) -> egui::WidgetText {
        "Text".into()
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.message.is_some() {
                // TODO: show the message
                ui.label("Message placeholder");
            } else {
                // Show project remarks
                ui.heading("Project Remarks");
                ui.text_edit_multiline(&mut app.project_settings.remarks);
            }
        });
    }
}
