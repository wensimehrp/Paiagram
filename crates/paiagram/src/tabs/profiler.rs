use egui::*;
// use paiagram_rw::ExportObject;
// use puffin::FrameView;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ProfilerTab;

// TODO: implement profile data dumping
// related issue: https://github.com/EmbarkStudios/puffin/issues/290
// struct ProfiledData(Vec<u8>);
//
// impl ExportObject for ProfiledData {
//     fn filename(&self) -> String {
//         let time_str = jiff::Timestamp::now().to_string();
//         format!("{time_str}.paiagram.puffin")
//     }
//     fn write_content<W: std::io::Write + std::io::Seek>(
//         &mut self,
//         writer: &mut W,
//     ) -> std::io::Result<()> {
//         writer.write_all(&self.0)
//     }
// }

impl Tab for ProfilerTab {
    const NAME: &'static str = "Profiler";

    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Frame::new().inner_margin(6).show(ui, |ui| {
            puffin::profile_function!();

            let mut enabled = puffin::are_scopes_on();
            // if ui.button("Dump profile data").clicked() {
            //     FrameView::write()
            // }

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
