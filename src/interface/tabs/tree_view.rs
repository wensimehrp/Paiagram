use crate::interface::AppTab;
use crate::interface::tabs::diagram::DiagramTab;
use crate::interface::tabs::{displayed_lines, vehicle};
use crate::vehicles::Vehicle;
use crate::vehicles::vehicle_set::VehicleSet;
use crate::{interface::UiCommand, lines::DisplayedLine};
use bevy::prelude::*;
use egui::Id;
use egui_ltreeview::{Action, TreeView};

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
                msg_open_tab.write(UiCommand::OpenOrFocusTab(AppTab::Diagram(DiagramTab {
                    displayed_line_entity: entity,
                })));
            }
        }
    });
    let (response, actions) = TreeView::new(Id::new("tree view")).show(ui, |builder| {
        for (set_entity, set_name, children) in vehicle_sets {
            builder.dir(TreeViewItem::VehicleSet(set_entity), set_name.to_string());
            for child in children {
                if let Ok((vehicle_entity, vehicle_name)) = vehicles.get(*child) {
                    builder.leaf(
                        TreeViewItem::Vehicle(vehicle_entity),
                        vehicle_name.to_string(),
                    );
                }
            }
            builder.close_dir();
        }
    });
    for action in actions {
        match action {
            Action::Activate(entries) => {
                for item in entries.selected {
                    match item {
                        TreeViewItem::Vehicle(entity) => {
                            msg_open_tab.write(UiCommand::OpenOrFocusTab(
                                crate::interface::AppTab::Vehicle(vehicle::VehicleTab(entity)),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
