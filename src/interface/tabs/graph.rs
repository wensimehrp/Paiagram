use crate::intervals::{Graph, UiGraph};
use crate::rw_data::write::write_text_file;
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
    fn export_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        // export the current graph as a .dot file
        let mut buffer = String::with_capacity(512);
        if ui.button("Export Graph as DOT file").clicked() {
            if let Err(e) = world.run_system_cached_with(make_dot_string, &mut buffer) {
                bevy::log::error!("Error while generating DOT string: {}", e);
                return;
            }
            if let Err(e) = write_text_file(&buffer, "transport_graph.dot") {
                bevy::log::error!("Failed to export graph: {:?}", e);
            }
        }
    }
}

fn make_dot_string(InMut(buffer): InMut<String>, graph: Res<Graph>, names: Query<&Name>) {
    let get_node_attr = |_, (_, entity): (_, &Entity)| {
        format!(
            r#"label = {}"#,
            names
                .get(*entity)
                .map_or("<Unknown>".to_string(), |name| name.to_string())
        )
    };
    let dot_string = petgraph::dot::Dot::with_attr_getters(
        &graph.inner,
        &[],
        &|_, _| String::new(),
        &get_node_attr,
    );
    buffer.clear();
    buffer.push_str(&format!("{:?}", dot_string));
}

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
