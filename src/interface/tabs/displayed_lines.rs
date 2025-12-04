use bevy::ecs::{name::Name, query::With, system::{InMut, Populated}};

use crate::{intervals::Station, lines::DisplayedLine};

pub fn list_displayed_lines(
    InMut(Ui): InMut<egui::Ui>,
    mut displayed_lines: Populated<&mut DisplayedLine>,
    station_names: Populated<&Name, With<Station>>
) {
    for line in displayed_lines {
        todo!("display all of those lines")
    }
}
