use egui::{Rect, Response, Ui};
use std::fmt::{Display, Write};

pub trait SetStatusBarText {
    fn set_status_bar_text(self, text: impl Display, target: &mut String) -> Self;
}

impl SetStatusBarText for Response {
    fn set_status_bar_text(self, text: impl Display, target: &mut String) -> Self {
        // set when hovering over the status bar itself
        if self.hovered() {
            target.clear();
            write!(target, "{}", text);
        }
        self
    }
}

impl SetStatusBarText for Ui {
    fn set_status_bar_text(self, text: impl Display, target: &mut String) -> Self {
        // set when hovering over the status bar itself
        let rect = self.max_rect().expand2(self.style().spacing.item_spacing);
        if self.rect_contains_pointer(rect) {
            target.clear();
            write!(target, "{}", text);
        }
        self
    }
}

impl SetStatusBarText for (Rect, Response) {
    fn set_status_bar_text(self, text: impl Display, target: &mut String) -> Self {
        let (_, response) = self.clone();
        if response.hovered() {
            target.clear();
            write!(target, "{}", text);
        }
        self
    }
}
