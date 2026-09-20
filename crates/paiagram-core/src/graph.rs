//! Definitions for the graph.
use petgraph::Direction;
use petgraph::visit::EdgeRef;

use super::{StationKey, WorldSnapshot};
use crate::{Distance, Interval, IntervalKey, NodeInfo, NodeKey};

impl WorldSnapshot {
    /// Find a route between the source station and the target station.
    /// Returns [`None`] if no valid routes exist.
    /// Returns the total distance and the stations on the route if a valid route is found.
    // Since stations are just collections of nodes, we instead run an O(n*m*k) complexity
    // search where n, m := nodes of source and target and k := complexity of each astar
    // and I just realized that I might need two graph structures... So the question is how can I
    // maintain two graphs?? TBH I don't really want to :-( I'll leave it todo now...
    // TODO: station subgraph OR smart way to avoid that.
    pub fn route_between_stations(
        &self,
        source: StationKey,
        target: StationKey,
    ) -> Option<(Distance, Vec<StationKey>)> {
        if source == target {
            return Some((Distance::ZERO, Vec::new()));
        }
        // cheap clone
        let source_nodes = self.stations.query(source, |view| view.nodes.clone())?;
        let target_nodes = self.stations.query(target, |view| view.nodes.clone())?;
        for node in source_nodes {
            // a la la la la
            // la la land!
        }
        // petgraph::algo::astar(
        //     self,
        //     source,
        //     |node| node == target,
        //     |edge| {
        //         let mut query_key = edge.id();
        //         // discard the info on the other key
        //         if let Some(IntervalDirection::TwoWay(other)) =
        //             self.intervals.query(query_key, |interval| interval.direction)
        //         {
        //             query_key = std::cmp::min(other, query_key);
        //         }
        //         let Some(length) = self.intervals.query(query_key, |interval| interval.length())
        // else {             return i32::MAX;
        //         };
        //         length.0
        //     },
        //     |_| 0,
        // )
        // .map(|(d, chain)| (Distance(d), chain))
        todo!()
    }

    /// Find a route between the source node and the target node.
    /// Returns [`None`] if no valid routes exist.
    /// Returns the total distances and the nodes on the route if a valid route is found.
    pub fn route_between_nodes(
        &self,
        source: NodeKey,
        target: NodeKey,
    ) -> Option<(Distance, Vec<NodeKey>)> {
        petgraph::algo::astar(
            self,
            source,
            |node| node == target,
            |edge| {
                let Some(length) = self.intervals.query(edge.id(), |interval| interval.length())
                else {
                    return i32::MAX;
                };
                length.0
            },
            |_| 0,
        )
        .map(|(d, chain)| (Distance(d), chain))
    }

    /// Find a route given a set of stations that must be on the route.
    /// Returns [`None`] if no valid route exists.
    /// Returns the total length and the stations on the route if a valid route is found.
    pub fn route_between_source_waypoint_target(
        &self,
        mut points: impl Iterator<Item = StationKey>,
    ) -> Option<(Distance, Vec<StationKey>)> {
        // let mut prev = points.next()?;
        // let mut total_length = Distance(0);
        // let mut passes = vec![prev];
        // for curr in points {
        //     let (leg_length, leg_points) = self.route_between(prev, curr)?;
        //     total_length += leg_length;
        //     passes.extend_from_slice(&leg_points[1..]);
        //     prev = curr;
        // }
        // Some((total_length, passes))
        todo!()
    }
}

/// The neighbour iterator backing `IntoNeighbors`, `IntoEdges`, and their
/// directed variants.
fn node_neighbours<'a>(
    world: &'a WorldSnapshot,
    node: NodeKey,
    dir: Direction,
) -> std::slice::Iter<'a, NodeKey> {
    static EMPTY: [NodeKey; 0] = [];
    world
        .nodes
        .query(node, |view| match dir {
            Direction::Outgoing => view.outgoing.iter(),
            Direction::Incoming => view.incoming.iter(),
        })
        .unwrap_or_else(|| EMPTY.iter())
}

/// A copyable reference to an edge.
#[derive(Clone, Copy)]
pub struct WorldEdgeRef {
    key: IntervalKey,
}

impl petgraph::visit::EdgeRef for WorldEdgeRef {
    type NodeId = NodeKey;
    type EdgeId = IntervalKey;
    type Weight = IntervalKey;

    fn source(&self) -> Self::NodeId {
        self.key.0
    }

    fn target(&self) -> Self::NodeId {
        self.key.1
    }

    fn weight(&self) -> &Self::Weight {
        &self.key
    }

    fn id(&self) -> Self::EdgeId {
        self.key
    }
}

pub struct WorldEdges<'a> {
    node: NodeKey,
    dir: Direction,
    inner: std::slice::Iter<'a, NodeKey>,
}

impl<'a> Iterator for WorldEdges<'a> {
    type Item = WorldEdgeRef;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|&other| WorldEdgeRef {
            key: match self.dir {
                Direction::Outgoing => (self.node, other),
                Direction::Incoming => (other, self.node),
            },
        })
    }
}

pub struct WorldNeighbors<'a> {
    inner: std::slice::Iter<'a, NodeKey>,
}

impl<'a> Iterator for WorldNeighbors<'a> {
    type Item = NodeKey;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().copied()
    }
}

pub struct WorldEdgeReferences<'a> {
    inner: std::collections::hash_map::Keys<'a, IntervalKey, Interval>,
}

impl<'a> Iterator for WorldEdgeReferences<'a> {
    type Item = WorldEdgeRef;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|&key| WorldEdgeRef { key })
    }
}

impl petgraph::visit::GraphBase for WorldSnapshot {
    type NodeId = NodeKey;
    type EdgeId = IntervalKey;
}

impl petgraph::visit::Data for WorldSnapshot {
    type NodeWeight = NodeInfo;
    type EdgeWeight = IntervalKey;
}

impl<'a> petgraph::visit::IntoNeighbors for &'a WorldSnapshot {
    type Neighbors = WorldNeighbors<'a>;

    fn neighbors(self, node: Self::NodeId) -> Self::Neighbors {
        WorldNeighbors {
            inner: node_neighbours(self, node, Direction::Outgoing),
        }
    }
}

impl<'a> petgraph::visit::IntoEdgeReferences for &'a WorldSnapshot {
    type EdgeRef = WorldEdgeRef;
    type EdgeReferences = WorldEdgeReferences<'a>;

    fn edge_references(self) -> Self::EdgeReferences {
        WorldEdgeReferences {
            inner: self.intervals.keys(),
        }
    }
}

impl<'a> petgraph::visit::IntoEdges for &'a WorldSnapshot {
    type Edges = WorldEdges<'a>;

    fn edges(self, node: Self::NodeId) -> Self::Edges {
        WorldEdges {
            node,
            dir: Direction::Outgoing,
            inner: node_neighbours(self, node, Direction::Outgoing),
        }
    }
}

impl petgraph::visit::Visitable for WorldSnapshot {
    type Map = std::collections::HashSet<NodeKey>;

    fn visit_map(&self) -> Self::Map {
        std::collections::HashSet::with_capacity(self.nodes.len())
    }

    fn reset_map(&self, map: &mut Self::Map) {
        map.clear();
        map.reserve(self.nodes.len());
    }
}

impl<'a> petgraph::visit::IntoNeighborsDirected for &'a WorldSnapshot {
    type NeighborsDirected = WorldNeighbors<'a>;

    fn neighbors_directed(self, node: Self::NodeId, dir: Direction) -> Self::NeighborsDirected {
        WorldNeighbors {
            inner: node_neighbours(self, node, dir),
        }
    }
}

impl<'a> petgraph::visit::IntoEdgesDirected for &'a WorldSnapshot {
    type EdgesDirected = WorldEdges<'a>;

    fn edges_directed(self, node: Self::NodeId, dir: Direction) -> Self::EdgesDirected {
        WorldEdges {
            node,
            dir,
            inner: node_neighbours(self, node, dir),
        }
    }
}
