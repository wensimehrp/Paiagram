// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/README.md")]

use ecow::EcoVec;
use pathfinding::prelude::dijkstra_bidirectional;
use serde::{Deserialize, Serialize};

use crate::{CanvasLength, Distance, NodeKey, NodeNeighbor, Route, StationKey, WorldSnapshot};

/// What to account as a part of the station
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StationRecord {
    /// All platforms
    All(StationKey),
    /// Some platforms
    Some(EcoVec<NodeKey>),
}

impl StationRecord {
    fn nodes<'a>(&'a self, snap: &'a WorldSnapshot) -> &'a [NodeKey] {
        match *self {
            StationRecord::All(station_key) => {
                snap.stations.get(&station_key).map_or_default(|wfc| wfc.cache.nodes.as_slice())
            }
            StationRecord::Some(ref nodes) => nodes.as_slice(),
        }
    }
}

/// An interval on the route
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RouteInterval {
    pub station_record: StationRecord,
    /// Optional milestone to be displayed
    pub milestone: Option<Distance>,
    pub canvas_length: Option<CanvasLength>,
    pub nodes: EcoVec<NodeKey>,
}

impl RouteInterval {
    pub fn progresses<'a>(
        &'a self,
        snap: &'a WorldSnapshot,
        next_stn_nodes: &'a [NodeKey],
    ) -> impl Iterator<Item = (NodeKey, Option<f32>)> + 'a {
        self.nodes.iter().map(|node_key| {
            (
                *node_key,
                gen_progress(
                    node_key,
                    snap,
                    self.station_record.nodes(snap),
                    &self.nodes,
                    next_stn_nodes,
                ),
            )
        })
    }
}

// suboptimal implementation to generate progress
fn gen_progress(
    current_node: &NodeKey,
    snap: &WorldSnapshot,
    curr_stn_nodes: &[NodeKey],
    interval_nodes: &[NodeKey],
    next_stn_nodes: &[NodeKey],
) -> Option<f32> {
    let is_part_of_interval = |key: &NodeKey| {
        curr_stn_nodes.contains(key) || interval_nodes.contains(key) || next_stn_nodes.contains(key)
    };
    // roughly the same as the implementation in graph.rs
    let neighbors = |source: &NodeKey| {
        let neighbors_slice =
            snap.nodes.get(source).map_or_default(|wfc| wfc.cache.neighbors.as_slice());
        let source = *source;
        neighbors_slice
            .iter()
            .filter_map(|neighbor| {
                if let NodeNeighbor::Outgoing(target) = neighbor
                // checks if the node is a part of the interval.
                // typically enough for our case since nobody would tuck
                // 100000 nodes inside an interval.
                    && is_part_of_interval(target)
                {
                    Some(*target)
                } else {
                    None
                }
            })
            .filter_map(move |target| {
                snap.intervals.get(&(source, target)).map(|interval| (target, interval.length()))
            })
    };
    let distance_to_source = curr_stn_nodes
        .into_iter()
        .filter_map(|source| dijkstra_bidirectional(source, current_node, neighbors, neighbors))
        .map(|(_, distance)| distance)
        .min()?
        .0 as f32;
    let distance_to_target = next_stn_nodes
        .into_iter()
        .filter_map(|source| dijkstra_bidirectional(source, current_node, neighbors, neighbors))
        .map(|(_, distance)| distance)
        .min()?
        .0 as f32;
    Some(distance_to_source / (distance_to_source + distance_to_target))
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RouteIntervals(pub Vec<RouteInterval>);

impl RouteIntervals {
    pub fn progresses<'a>(
        &'a self,
        snap: &'a WorldSnapshot,
    ) -> impl Iterator<Item = impl Iterator<Item = (NodeKey, Option<f32>)>> + 'a {
        self.0.array_windows().map(|[curr, next]| {
            let next_stn_nodes = next.station_record.nodes(snap);
            curr.progresses(snap, next_stn_nodes)
        })
    }
}
