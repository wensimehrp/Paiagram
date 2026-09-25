// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/README.md")]

use ecow::EcoVec;
use pathfinding::prelude::dijkstra;
use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::time::TimetableTime;
use crate::{
    CanvasLength, Distance, NodeKey, NodeKeyHashMap, StationCollection, StationKey, TripKeyHashMap,
    WorldSnapshot,
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
                    curr_stn_nodes,
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

#[derive(Default)]
pub struct RouteCache(TripKeyHashMap<Vec<(TimetableTime, u32, f32)>>);

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
    pub fn populate_trips(&self, snap: &WorldSnapshot, cache: &mut RouteCache) {
        // suboptimal implementation but cache is cached anyways
        let mut node_lookup: NodeKeyHashMap<Vec<(u32, f32)>> = NodeKeyHashMap::default();
        for (key, progress) in self.progresses(snap).enumerate().flat_map(|(idx, it)| {
            it.filter_map(move |(key, distance)| Some((key, (idx as u32, distance?))))
        }) {
            match node_lookup.get_mut(&key) {
                Some(entries) => entries.push(progress),
                None => {
                    node_lookup.insert(key, vec![progress]);
                }
            }
        }
        for (trip, trip_key) in self
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
            .filter_map(|trip_key| snap.trips.get(trip_key).map(|trip| (trip, trip_key)))
        {
            trip.schedule.estimates(&snap.graph, |entries| {
                for [(curr_estimate, curr_entry), (next_estimate, next_entry)] in
                    entries.array_windows()
                {}
            })
        }
    }
}
