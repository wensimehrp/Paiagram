use egui::*;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ProfilerTab;

impl Tab for ProfilerTab {
    const NAME: &'static str = "Profiler";

    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Frame::new().inner_margin(6).show(ui, |ui| {
            puffin::profile_function!();

            let mut enabled = puffin::are_scopes_on();
            ui.add_enabled(
                app.preferences.dev_mode,
                Checkbox::new(&mut enabled, "Enable profiling"),
            )
            .on_disabled_hover_text("Enable developer mode to enable profiling");
            puffin::set_scopes_on(enabled && app.preferences.dev_mode);

            ui.separator();
            puffin_egui::profiler_ui(ui);
        });
    }
}
