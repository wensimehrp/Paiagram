// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/README.md")]

use ecow::EcoVec;
use pathfinding::prelude::dijkstra;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

mod prep_segments;

use crate::graph::Graph;
use crate::trip::{EstimateEntry, TEstimate};
use crate::{
    CanvasLength, Distance, NodeKey, NodeKeyHashMap, StationCollection, StationKey, TripKey,
    TripKeyHashMap, WorldSnapshot,
};

/// What to account as a part of the station
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StationRecord {
    /// All platforms
    All(StationKey),
    /// Some platforms
    Some(EcoVec<NodeKey>),
}

impl StationRecord {
    fn nodes<'a>(&'a self, stations: &'a StationCollection) -> &'a [NodeKey] {
        match *self {
            StationRecord::All(station_key) => {
                stations.get(&station_key).map_or_default(|wfc| wfc.cache.nodes.as_slice())
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
    fn progresses<'a>(
        &'a self,
        snap: &'a WorldSnapshot,
        next_stn_nodes: &'a [NodeKey],
    ) -> impl Iterator<Item = (NodeKey, Option<f32>)> + 'a {
        let curr_stn_nodes = self.station_record.nodes(&snap.stations);
        std::iter::chain(&self.nodes, curr_stn_nodes).map(|node_key| {
            (
                *node_key,
                gen_progress(
                    node_key,
                    &snap.graph,
                    &self.nodes,
                    curr_stn_nodes,
                    next_stn_nodes,
                ),
            )
        })
    }
}

// suboptimal implementation to generate progress
fn gen_progress(
    current_node: &NodeKey,
    graph: &Graph,
    interval_nodes: &[NodeKey],
    curr_stn_nodes: &[NodeKey],
    next_stn_nodes: &[NodeKey],
) -> Option<f32> {
    let is_part_of_interval = |key: &NodeKey| {
        curr_stn_nodes.contains(key) || interval_nodes.contains(key) || next_stn_nodes.contains(key)
    };
    // roughly the same as the implementation in graph.rs
    let successors = |source: &NodeKey| {
        let source = *source;
        graph.outgoing_intervals(source).filter_map(move |(interval_key, interval)| {
            let target = interval_key.other(source);
            is_part_of_interval(&target).then_some((target, interval.length()))
        })
    };
    let distance_to_source = curr_stn_nodes
        .into_iter()
        .filter_map(|source| dijkstra(source, successors, |node| *node == *current_node))
        .map(|(_, distance)| distance)
        .min()?
        .0 as f32;
    let distance_to_target = next_stn_nodes
        .into_iter()
        .filter_map(|source| dijkstra(source, successors, |node| *node == *current_node))
        .map(|(_, distance)| distance)
        .min()?
        .0 as f32;
    Some(distance_to_source / (distance_to_source + distance_to_target))
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RouteIntervals(pub Vec<RouteInterval>);

#[derive(Clone, Default, Debug)]
pub struct DiagramCache {
    pub map: TripKeyHashMap<SmallVec<[Vec<(TEstimate, EstimateEntry, u32, f32)>; 1]>>,
}

impl RouteIntervals {
    fn progresses<'a>(
        &'a self,
        snap: &'a WorldSnapshot,
    ) -> impl Iterator<Item = impl Iterator<Item = (NodeKey, Option<f32>)>> + 'a {
        self.0.array_windows().map(|[curr, next]| {
            let next_stn_nodes = next.station_record.nodes(&snap.stations);
            curr.progresses(snap, next_stn_nodes)
        })
    }
    pub fn populate_trips(&self, snap: &WorldSnapshot, cache: &mut DiagramCache) {
        // Where each graph node sits on the diagram: every `(line index, progress)` placement.
        let mut node_lookup: NodeKeyHashMap<Vec<(u32, f32)>> = NodeKeyHashMap::default();
        for (key, progress) in self.progresses(snap).enumerate().flat_map(|(idx, it)| {
            it.filter_map(move |(key, distance)| Some((key, (idx as u32, distance?))))
        }) {
            match node_lookup.get_mut(&key) {
                Some(entries) => {
                    if !entries.contains(&progress) {
                        entries.push(progress);
                    }
                }
                None => {
                    node_lookup.insert(key, vec![progress]);
                }
            }
        }
        // The same trip is reachable through several route nodes/intervals, so collect and
        // deduplicate the trip keys first; otherwise every segment would be pushed once per
        // discovery.
        let mut trip_keys: Vec<TripKey> = self
            .0
            .iter()
            .flat_map(|route_interval| {
                std::iter::chain(
                    route_interval.station_record.nodes(&snap.stations),
                    &route_interval.nodes,
                )
            })
            .flat_map(|node_key| snap.graph.outgoing_intervals(*node_key))
            .flat_map(|(_, wfc)| &wfc.cache.trips)
            .copied()
            .collect();
        trip_keys.sort_unstable();
        trip_keys.dedup();

        cache.map.clear();
        prep_segments::calc(cache, trip_keys.into_iter(), snap, &node_lookup);
    }
}
