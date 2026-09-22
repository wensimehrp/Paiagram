use pathfinding::prelude::dijkstra;

use crate::{Distance, IntervalCollection, NodeKey};

pub trait Routefinding {
    fn dijkstra(&self, start: NodeKey, end: NodeKey) -> Option<(Vec<NodeKey>, Distance)>;
}

impl Routefinding for IntervalCollection {
    fn dijkstra(&self, start: NodeKey, end: NodeKey) -> Option<(Vec<NodeKey>, Distance)> {
        let successors = |source: &NodeKey| {
            source
                .outgoing_intervals(&self)
                .map(|((_, target), interval)| (*target, interval.length()))
        };
        dijkstra(&start, successors, |node| *node == end)
    }
}
