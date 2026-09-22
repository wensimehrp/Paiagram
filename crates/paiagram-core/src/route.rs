// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/README.md")]

use ecow::EcoVec;
use pathfinding::prelude::dijkstra;
use serde::{Deserialize, Serialize};

use crate::time::TimetableTime;
use crate::{
    CanvasLength, Distance, IntervalCollection, NodeKey, NodeKeyHashMap, StationCollection,
    StationKey, TripKey, WorldSnapshot,
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
                    &snap.intervals,
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
    intervals: &IntervalCollection,
    curr_stn_nodes: &[NodeKey],
    interval_nodes: &[NodeKey],
    next_stn_nodes: &[NodeKey],
) -> Option<f32> {
    let is_part_of_interval = |key: &NodeKey| {
        curr_stn_nodes.contains(key) || interval_nodes.contains(key) || next_stn_nodes.contains(key)
    };
    // roughly the same as the implementation in graph.rs
    let successors = |source: &NodeKey| {
        source.outgoing_intervals(&intervals).filter_map(|((_, target), interval)| {
            is_part_of_interval(target).then_some((*target, interval.length()))
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
pub struct RouteCache(Vec<RouteCacheInner>);

struct RouteCacheInner {
    key: TripKey,
    slice: Box<[I]>,
}

type I = (TimetableTime, u32, f32);

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
        let node_lookup = self
            .progresses(snap)
            .enumerate()
            .flat_map(|(idx, it)| {
                it.filter_map(move |(key, distance)| Some((key, (idx as u32, distance?))))
            })
            .collect::<NodeKeyHashMap<(u32, f32)>>();
        for trip in self
            .0
            .iter()
            .flat_map(|route_interval| {
                std::iter::chain(
                    route_interval.station_record.nodes(&snap.stations),
                    &route_interval.nodes,
                )
            })
            .flat_map(|node_key| node_key.outgoing_intervals(&snap.intervals))
            .flat_map(|(_, wfc)| &wfc.cache.trips)
            .filter_map(|trip_key| snap.trips.get(trip_key))
        {
            trip.schedule.estimates(&snap.intervals, |entries| {})
        }
    }
}
