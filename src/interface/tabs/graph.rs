use crate::intervals::{Graph, UiGraph};
use bevy::prelude::*;
use egui::Rect;
use egui_graphs::{DefaultEdgeShape, DefaultGraphView, DefaultNodeShape, GraphView, to_graph};

use super::Tab;

#[derive(Debug, Clone)]
pub struct GraphTab;

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        if let Err(e) = world.run_system_cached_with(show_graph, ui) {
            bevy::log::error!("UI Error while displaying graph page: {}", e)
        }
    }
}

// ...existing code...
fn show_graph(InMut(ui): InMut<egui::Ui>, mut ui_graph: ResMut<UiGraph>) {
    type L = egui_graphs::LayoutHierarchical;
    type S = egui_graphs::LayoutStateHierarchical;

    // 1. Create the view
    let mut view = egui_graphs::GraphView::<_, _, _, _, _, _, S, L>::new(&mut ui_graph)
        // 2. Enable Zoom and Pan (Navigation)
        .with_navigations(
            &egui_graphs::SettingsNavigation::default()
                .with_zoom_and_pan_enabled(true)
                .with_fit_to_screen_enabled(false),
        ) // Set to false to allow free movement
        // 3. Enable Node Dragging (Interaction)
        .with_interactions(
            &egui_graphs::SettingsInteraction::default()
                .with_dragging_enabled(true)
                .with_edge_selection_enabled(true)
                .with_node_selection_enabled(true),
        )
        .with_styles(&egui_graphs::SettingsStyle::default().with_labels_always(true))
        // 4. Provide a unique ID (Crucial for persisting zoom/pan state in tabs)
        .with_id(Some("main_transport_graph".to_string()));

    // 5. Just add it to the UI. It will automatically fill the available space.
    ui.add(&mut view);
}
