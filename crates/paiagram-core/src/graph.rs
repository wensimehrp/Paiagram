use std::num::NonZeroU64;

use imbl::OrdSet;
use pathfinding::prelude::dijkstra;
use serde::{Deserialize, Serialize};

use crate::{
    Distance, Interval, IntervalCache, IntervalCollection, IntervalKey, LonLat, Node,
    NodeCollection, NodeKey, Wfc,
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
            let is_lo_key = node_key < interval_key.hi;
            match (wfc.direction, is_lo_key) {
                (IntervalDirection::Both, _) => true,
                (IntervalDirection::LoToHi, true) => true,
                (IntervalDirection::HiToLo, false) => true,
                _ => false,
            }
        })
    }

    /// The returning vector contains both starting and ending nodes
    pub fn dijkstra(&self, start: NodeKey, end: NodeKey) -> Option<(Vec<NodeKey>, Distance)> {
        let successors = |source: &NodeKey| {
            let source = *source;
            self.outgoing_intervals(source).map(move |(interval_key, interval)| {
                (interval_key.other(source), interval.length())
            })
        };
        dijkstra(&start, successors, |node| *node == end)
    }

    pub fn insert_node(&mut self, node_key: NodeKey, node: Node) {
        self.nodes.insert(node_key, Wfc::new(node));
    }

    pub fn insert_interval(&mut self, (a, b): (NodeKey, NodeKey), interval: Interval) {
        self.intervals.insert(IntervalKey::new(a, b), Wfc::new(interval));
        self.adjacency.insert((a, b));
        self.adjacency.insert((b, a));
    }

    pub fn remove_interval(
        &mut self,
        (a, b): (NodeKey, NodeKey),
    ) -> Option<Wfc<Interval, IntervalCache>> {
        self.adjacency.remove(&(a, b))?;
        self.adjacency.remove(&(b, a))?;
        self.intervals.remove(&IntervalKey::new(a, b))
    }

    pub fn update_node_coordinate(
        &mut self,
        node_key: NodeKey,
        new_coordinate: LonLat,
    ) -> Option<LonLat> {
        let wfc = self.nodes.get_mut(&node_key)?;
        let old = std::mem::replace(&mut wfc.pos, new_coordinate);
        let sorted_keys: Vec<_> =
            self.neighbor_intervals(node_key).map(|(sorted_key, _)| sorted_key).collect();
        for key in sorted_keys.into_iter() {
            let interval = self.intervals.get_mut(&key).unwrap();
            if node_key == key.hi {
                if interval.data.nodes.is_empty() {
                    interval.data.nodes.push(new_coordinate);
                } else {
                    *interval.data.nodes.make_mut().first_mut().unwrap() = new_coordinate;
                }
            } else {
                match interval.data.nodes.len() {
                    0 => {
                        interval.data.nodes.push(new_coordinate);
                        interval.data.nodes.push(new_coordinate);
                    }
                    1 => {
                        interval.data.nodes.push(new_coordinate);
                    }
                    _ => {
                        *interval.data.nodes.make_mut().last_mut().unwrap() = new_coordinate;
                    }
                }
            }
        }
        Some(old)
    }
}
