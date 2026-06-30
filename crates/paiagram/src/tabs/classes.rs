use egui::{Ui, WidgetText};
use egui_i18n::tr;
use paiagram_core::{ClassKey, Source, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ClassesTab {
    #[serde(skip)]
    selected_class: Option<ClassKey>,
    #[serde(skip)]
    hovered_trip: Option<TripKey>,
}

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn title(&self) -> WidgetText {
        tr!("tab-classes").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        ui.label("Remaking...");
    }
}
