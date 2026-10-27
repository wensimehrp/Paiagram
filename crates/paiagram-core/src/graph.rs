use std::num::NonZeroU64;

use ecow::EcoVec;
use imbl::OrdSet;
use pathfinding::prelude::astar;
use serde::{Deserialize, Serialize};

use crate::{
    Distance, Interval, IntervalCache, IntervalCollection, IntervalKey, LonLat, Node,
    NodeCollection, NodeKey, Wfc, Wgs84LonLat,
};

impl NodeKey {
    const MIN: Self = NodeKey(NonZeroU64::MIN);
    const MAX: Self = NodeKey(NonZeroU64::MAX);
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SortedKey {
    pub hi: NodeKey,
    pub lo: NodeKey,
}

impl SortedKey {
    pub fn other(&self, node_key: NodeKey) -> NodeKey {
        if node_key == self.hi {
            self.lo
        } else if node_key == self.lo {
            self.hi
        } else {
            panic!("{node_key:?} is not an endpoint of interval {self:?}");
        }
    }
    pub fn new(v1: NodeKey, v2: NodeKey) -> Self {
        Self {
            hi: std::cmp::max(v1, v2),
            lo: std::cmp::min(v1, v2),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug, Default)]
pub enum IntervalDirection {
    HiToLo,
    LoToHi,
    #[default]
    Both,
}

// I hate caching
/// The graph in Paiagram.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(into = "GraphNoCache", from = "GraphNoCache")]
pub struct Graph {
    pub(crate) nodes: NodeCollection,
    pub(crate) intervals: IntervalCollection,
    adjacency: OrdSet<(NodeKey, NodeKey)>,
}

#[derive(Serialize, Deserialize)]
struct GraphNoCache {
    nodes: NodeCollection,
    intervals: IntervalCollection,
}

impl From<GraphNoCache> for Graph {
    fn from(GraphNoCache { nodes, intervals }: GraphNoCache) -> Self {
        let adjacency = intervals.iter().flat_map(|(k, _)| [(k.hi, k.lo), (k.lo, k.hi)]).collect();
        Self {
            nodes,
            intervals,
            adjacency,
        }
    }
}

impl From<Graph> for GraphNoCache {
    fn from(
        Graph {
            nodes, intervals, ..
        }: Graph,
    ) -> Self {
        Self { nodes, intervals }
    }
}

/// Whether an interval can be traversed from the given endpoint.
///
/// `is_lo_key` is true when the endpoint is the lower key of the interval.
fn traversable(direction: IntervalDirection, is_lo_key: bool) -> bool {
    match (direction, is_lo_key) {
        (IntervalDirection::Both, _) => true,
        (IntervalDirection::LoToHi, true) => true,
        (IntervalDirection::HiToLo, false) => true,
        _ => false,
    }
}

impl Graph {
    pub fn nodes(&self) -> &NodeCollection {
        &self.nodes
    }

    pub fn intervals(&self) -> &IntervalCollection {
        &self.intervals
    }

    pub fn neighbor_intervals<'a>(
        &'a self,
        node_key: NodeKey,
    ) -> impl Iterator<Item = (IntervalKey, &'a Wfc<Interval, IntervalCache>)> + 'a {
        let min_key = (node_key, NodeKey::MIN);
        let max_key = (node_key, NodeKey::MAX);
        self.adjacency.range(min_key..=max_key).copied().filter_map(|(source, target)| {
            let key = SortedKey::new(source, target);
            self.intervals.get(&key).map(|interval| (key, interval))
        })
    }

    pub fn outgoing_intervals<'a>(
        &'a self,
        node_key: NodeKey,
    ) -> impl Iterator<Item = (IntervalKey, &'a Wfc<Interval, IntervalCache>)> + 'a {
        self.neighbor_intervals(node_key).filter(move |(interval_key, wfc)| {
            traversable(wfc.direction, node_key < interval_key.hi)
        })
    }

    /// The great-circle distance between two nodes, used as the A* heuristic.
    ///
    /// This is admissible whenever interval lengths are at least the straight-line
    /// distance between their endpoints, which holds for lengths derived from geometry.
    fn heuristic(&self, from: NodeKey, to: NodeKey) -> Distance {
        let (Some(from), Some(to)) = (self.nodes.get(&from), self.nodes.get(&to)) else {
            return Distance::ZERO;
        };
        Distance(
            Wgs84LonLat::from(from.pos).distance_to_meters(Wgs84LonLat::from(to.pos)).round()
                as i32,
        )
    }

    /// Finds a shortest path with A*.
    ///
    /// The returned vector contains both the starting and ending nodes.
    pub fn shortest_path(&self, start: NodeKey, end: NodeKey) -> Option<(Vec<NodeKey>, Distance)> {
        puffin::profile_function!();
        astar(
            &start,
            |node| {
                let node = *node;
                self.outgoing_intervals(node).map(move |(interval_key, interval)| {
                    (
                        interval_key.other(node),
                        interval.length(interval_key, &self.nodes),
                    )
                })
            },
            |node| self.heuristic(*node, end),
            |node| *node == end,
        )
    }

    pub fn insert_node(&mut self, node_key: NodeKey, node: Node) {
        self.nodes.insert(node_key, Wfc::new(node));
    }

    /// Moves a node, returning its previous position.
    ///
    /// Intervals do not store their endpoints, so only the node itself needs to change.
    pub fn update_node_coordinate(
        &mut self,
        node_key: NodeKey,
        new_coordinate: LonLat,
    ) -> Option<LonLat> {
        let node = self.nodes.get_mut(&node_key)?;
        Some(std::mem::replace(&mut node.pos, new_coordinate))
    }

    pub fn insert_interval(
        &mut self,
        key: IntervalKey,
        new_interval: Wfc<Interval, IntervalCache>,
    ) {
        self.adjacency.insert((key.hi, key.lo));
        self.adjacency.insert((key.lo, key.hi));
        self.intervals.insert(key, new_interval);
    }

    pub fn connect_nodes(&mut self, (from, to): (NodeKey, NodeKey)) {
        if from == to {
            return;
        }
        let key = SortedKey::new(from, to);
        if self.intervals.contains_key(&key) {
            return;
        }
        self.insert_interval(
            key,
            Wfc::new(Interval {
                intermediate_nodes: EcoVec::new(),
                length: None,
                direction: IntervalDirection::Both,
            }),
        );
    }

    pub(crate) fn remove_interval(
        &mut self,
        key: &IntervalKey,
    ) -> Option<Wfc<Interval, IntervalCache>> {
        self.adjacency.remove(&(key.hi, key.lo));
        self.adjacency.remove(&(key.lo, key.hi));
        self.intervals.remove(key)
    }
}
