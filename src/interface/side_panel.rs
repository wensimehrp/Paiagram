use bevy::ecs::system::{InMut, Local};
use egui::{CornerRadius, RichText, Ui, vec2};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub mod vehicle_stats;
pub mod station_stats;
pub mod interval_stats;

#[derive(PartialEq, Eq, Clone, Copy, Default, EnumIter)]
pub enum CurrentTab {
    #[default]
    Edit,
    Details,
}

impl CurrentTab {
    pub fn name(self) -> &'static str {
        match self {
            CurrentTab::Edit => "Edit",
            CurrentTab::Details => "Details",
        }
    }
}

pub fn show_side_panel(ui: &mut Ui, selected_tab: &mut CurrentTab) {
    // Segmented tab buttons (egui-style): selectable widgets with fixed width.
    const SEGMENT_SPACING: f32 = 5.0;
    let segment_width = (ui.available_width() - SEGMENT_SPACING) / CurrentTab::iter().len() as f32;
    let segment_size = vec2(segment_width, 30.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SEGMENT_SPACING;
        for tab in CurrentTab::iter() {
            let is_selected = *selected_tab == tab;
            let resp = ui.add_sized(
                segment_size,
                egui::Button::selectable(is_selected, RichText::new(tab.name())),
            );
            if resp.clicked() {
                *selected_tab = tab;
            }
        }
    });
}
