use bevy::{ecs::system::RunSystemOnce, prelude::*};
use petgraph::dot;

use crate::graph::Graph;

pub struct Graphviz<'a> {
    world: &'a mut World,
}

impl<'a> super::ExportObject for Graphviz<'a> {
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        self.world
            .run_system_once_with(make_dot_string, buffer)
            .unwrap();
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
