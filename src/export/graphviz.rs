use bevy::{ecs::system::RunSystemOnce, prelude::*};
use petgraph::dot;

use crate::graph::Graph;

#[derive(Event)]
pub struct Graphviz;

impl super::ExportObject for Graphviz {
    fn export_to_buffer(&mut self, world: &mut World, buffer: &mut Vec<u8>, _input: ()) {
        world.run_system_once_with(make_dot_string, buffer);
    }
    fn extension(&self) -> impl AsRef<str> {
        ".dot"
    }
    fn filename(&self) -> impl AsRef<str> {
        "diagram"
    }
}

fn make_dot_string(InMut(buffer): InMut<Vec<u8>>, graph: Res<Graph>, names: Query<&Name>) {
    let get_node_attr = |_, (_, entity): (_, &Entity)| {
        format!(
            r#"label = "{}""#,
            names
                .get(entity.entity())
                .map_or("<Unknown>".to_string(), |name| name.to_string())
        )
    };
    let get_edge_attr = |_, _| String::new();
    let dot_string = dot::Dot::with_attr_getters(
        &graph.map,
        &[dot::Config::EdgeNoLabel, dot::Config::NodeNoLabel],
        &get_edge_attr,
        &get_node_attr,
    );
    buffer.clear();
    buffer.extend_from_slice(&format!("{:?}", dot_string).into_bytes());
}
