use pathfinding::prelude::dijkstra_bidirectional;

use crate::{Distance, NodeKey, NodeNeighbor, WorldSnapshot};

impl WorldSnapshot {
    pub fn dijkstra_bidirectional(
        &self,
        start: NodeKey,
        end: NodeKey,
    ) -> Option<(Vec<NodeKey>, Distance)> {
        let neighbors = |source: &NodeKey| {
            let neighbors_slice =
                self.nodes.get(source).map_or_default(|wfc| wfc.cache.neighbors.as_slice());
            let source = *source;
            neighbors_slice
                .iter()
                .filter_map(|neighbor| {
                    if let NodeNeighbor::Outgoing(target) = neighbor {
                        Some(*target)
                    } else {
                        None
                    }
                })
                .filter_map(move |target| {
                    self.intervals
                        .get(&(source, target))
                        .map(|interval| (target, interval.length()))
                })
        };
        dijkstra_bidirectional(&start, &end, neighbors, neighbors)
    }
}
