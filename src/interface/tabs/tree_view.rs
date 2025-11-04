use crate::interface::UiCommand;
use crate::vehicle_set::VehicleSet;
use crate::vehicles::Vehicle;
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
    vehicles: Query<(Entity, &Name), With<Vehicle>>,
    mut msg_open_tab: MessageWriter<UiCommand>,
) {
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
                            msg_open_tab.write(UiCommand::OpenOrFocusVehicleTab(
                                crate::interface::AppTab::Vehicle(entity),
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
