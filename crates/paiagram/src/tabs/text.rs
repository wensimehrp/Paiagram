//! This tab displays a message
//!
//! The message could be the project's remarks or a customized message.
//! Additionally, this tab supports displaying commonmark strings.

use std::sync::LazyLock;

use egui::mutex::Mutex;
use egui::{Frame, ScrollArea, TextEdit, WidgetText};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct TextTab {
    #[serde(skip, default)]
    editing: bool,
}

impl Default for TextTab {
    fn default() -> Self {
        Self { editing: false }
    }
}

impl PartialEq for TextTab {
    fn eq(&self, other: &Self) -> bool {
        self.editing == other.editing
    }
}

static CACHE: LazyLock<Mutex<CommonMarkCache>> =
    LazyLock::new(|| Mutex::new(CommonMarkCache::default()));

impl Tab for TextTab {
    const NAME: &'static str = "Text message";
    fn title(&self) -> WidgetText {
        tr!("tab-text").into()
    }
    fn edit_display(&mut self, _app: &mut App, ui: &mut egui::Ui) {
        ui.label(tr!("text-markdown-hint"));
        self.editing ^= ui
            .button(if self.editing { "Finish edit" } else { "Edit" })
            .clicked();
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
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
        show(&mut app.project_settings.remarks);
    }
}
