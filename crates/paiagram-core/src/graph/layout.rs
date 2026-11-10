use std::collections::HashMap;

use fdg_sim::{ForceGraph, Node, Simulation, SimulationParameters};

use crate::{NodeKey, Source, XyPos};

impl Source {
    pub fn auto_layout(&mut self) {
        auto_layout(self);
    }
}

fn auto_layout(source: &mut Source) {
    puffin::profile_function!();
    let mut graph: ForceGraph<NodeKey, ()> = ForceGraph::default();
    let mut idx_map = HashMap::with_capacity(source.graph.nodes().len());
    for &node in source.graph.nodes().keys() {
        let idx = graph.add_node(Node::new("", node));
        idx_map.insert(node, idx);
    }
    for edge_key in source.graph.intervals().keys() {
        graph.add_edge(
            idx_map.get(&edge_key.hi).copied().unwrap(),
            idx_map.get(&edge_key.lo).copied().unwrap(),
            (),
        );
    }
    let mut simulation = Simulation::from_graph(graph, SimulationParameters::default());
    for _ in 0..1000 {
        simulation.update(0.1);
        let max_step = simulation
            .get_graph()
            .node_weights()
            .map(|node| node.location.distance(node.old_location))
            .fold(0.0_f32, f32::max);
        if max_step < 0.1 {
            break;
        }
    }
    let _ = source.mutate(|mut world| {
        for node in simulation.get_graph().node_weights() {
            let xy = XyPos(node.location.x.round() as i32, node.location.y.round() as i32);
            let new_coord = world.graph.coord_type.unproject(xy);
            world.graph.update_node_coordinate(node.data, new_coord);
        }
        Ok(world)
    });
}
