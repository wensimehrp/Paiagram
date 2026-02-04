use crate::interface::AppTab;
use crate::interface::tabs::diagram::DiagramTab;
use crate::interface::tabs::{displayed_lines, vehicle};
use crate::vehicles::Vehicle;
use crate::vehicles::vehicle_set::VehicleSet;
use crate::{interface::UiCommand, lines::DisplayedLine};
use bevy::prelude::*;
use egui::Id;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
enum TreeViewItem {
    VehicleSet(Entity),
    Vehicle(Entity),
}

pub fn show_tree_view(
    InMut(ui): InMut<egui::Ui>,
    vehicle_sets: Query<(Entity, &Name, &Children), With<VehicleSet>>,
    displayed_lines: Query<(Entity, &Name), With<DisplayedLine>>,
    vehicles: Query<(Entity, &Name), With<Vehicle>>,
    mut msg_open_tab: MessageWriter<UiCommand>,
) {
    ui.vertical(|ui| {
        if ui.button("All displayed lines").clicked() {
            msg_open_tab.write(UiCommand::OpenOrFocusTab(AppTab::DisplayedLines(
                displayed_lines::DisplayedLinesTab,
            )));
        }
        for (entity, name) in displayed_lines {
            if ui.button(name.as_str()).clicked() {
                msg_open_tab.write(UiCommand::OpenOrFocusTab(AppTab::Diagram(DiagramTab::new(
                    entity,
                ))));
            }
        }
    });
    for (set_entity, set_name, set) in vehicle_sets {
        ui.label(set_name.as_str());
        for vehicle in set.into_iter().copied() {
            let Ok((entity, name)) = vehicles.get(vehicle) else {
                continue;
            };
            if ui.button(name.as_str()).clicked() {
                msg_open_tab.write(UiCommand::OpenOrFocusTab(AppTab::Vehicle(
                    vehicle::VehicleTab(entity),
                )));
            }
        }
    }
}
