use egui::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub(crate) struct ChangelogTab;
impl super::Tab for ChangelogTab {
    const NAME: &'static str = "Changelog";
    fn title(&self) -> egui::WidgetText {
        Self::NAME.into()
    }
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        ui.label("This is the changelog tab");
    }
}
