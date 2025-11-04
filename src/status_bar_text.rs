use crate::interface::UiCommand;
use bevy::ecs::message::MessageWriter;
use bevy_egui::egui::{Rect, Response, Ui};

pub trait SetStatusBarText {
    fn set_status_bar_text(
        self,
        text: impl Into<String>,
        msg_writer: &mut MessageWriter<UiCommand>,
    ) -> Self;
}

impl SetStatusBarText for Response {
    fn set_status_bar_text(
        self,
        text: impl Into<String>,
        msg_writer: &mut MessageWriter<UiCommand>,
    ) -> Self {
        // set when hovering over the status bar itself
        if self.hovered() {
            msg_writer.write(UiCommand::SetStatusBarText(text.into()));
        }
        self
    }
}

impl SetStatusBarText for Ui {
    fn set_status_bar_text(
        self,
        text: impl Into<String>,
        msg_writer: &mut MessageWriter<UiCommand>,
    ) -> Self {
        // set when hovering over the status bar itself
        let rect = self.max_rect().expand2(self.style().spacing.item_spacing);
        if self.rect_contains_pointer(rect) {
            msg_writer.write(UiCommand::SetStatusBarText(text.into()));
        }
        self
    }
}

impl SetStatusBarText for (Rect, Response) {
    fn set_status_bar_text(
        self,
        text: impl Into<String>,
        msg_writer: &mut MessageWriter<UiCommand>,
    ) -> Self {
        let (_, response) = self.clone();
        if response.hovered() {
            msg_writer.write(UiCommand::SetStatusBarText(text.into()));
        }
        self
    }
}
