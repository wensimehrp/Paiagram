use std::sync::LazyLock;

use egui::mutex::Mutex;
use egui::{Frame, ScrollArea, TextEdit, WidgetText};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TextTab {
    /// None = project remarks, Some(i) = index into text_messages
    pub(crate) message_idx: Option<usize>,
    #[serde(skip)]
    editing: bool,
}

impl PartialEq for TextTab {
    fn eq(&self, other: &Self) -> bool {
        self.message_idx == other.message_idx
    }
}

impl TextTab {
    pub(crate) fn new(message_idx: Option<usize>) -> Self {
        Self { message_idx, editing: false }
    }
}

impl Default for TextTab {
    fn default() -> Self {
        Self { message_idx: None, editing: false }
    }
}

static CACHE: LazyLock<Mutex<CommonMarkCache>> =
    LazyLock::new(|| Mutex::new(CommonMarkCache::default()));

impl Tab for TextTab {
    const NAME: &'static str = "Text message";
    fn title(&self) -> WidgetText {
        tr!("tab-text").into()
    }
    fn edit_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        ui.label(tr!("text-markdown-hint"));
        self.editing ^= ui
            .button(if self.editing { "Finish edit" } else { "Edit" })
            .clicked();
        // Edit message name
        if let Some(idx) = self.message_idx {
            if let Some(msg) = app.text_messages.get_mut(idx) {
                ui.separator();
                ui.label("Name:");
                ui.text_edit_singleline(&mut msg.name);
            }
        }
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
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
        match self.message_idx {
            None => show(&mut app.project_settings.remarks),
            Some(idx) => {
                if let Some(msg) = app.text_messages.get_mut(idx) {
                    show(&mut msg.content);
                } else {
                    ui.label("Message not found");
                }
            }
        }
    }
}


