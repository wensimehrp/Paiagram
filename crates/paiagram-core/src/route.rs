// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/README.md")]

use std::iter;

use ecow::EcoVec;
use itertools::{Either, Itertools};
use pathfinding::prelude::dijkstra;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

mod prep_marey;
mod prep_route_timetable;

pub use prep_route_timetable::{
    RouteTimetable, RouteTimetableEntry, RouteTimetableExport, StationDisplayMode,
};

use crate::graph::Graph;
use crate::trip::{EstimateEntry, TEstimate, TripSchedule};
use crate::{
    CanvasLength, Distance, NodeCollection, NodeKey, NodeKeyHashMap, StationCollection, StationKey,
    TripKey, TripKeyHashMap, World,
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
    pub fn nodes<'a>(&'a self, stations: &'a StationCollection) -> &'a [NodeKey] {
        match *self {
            StationRecord::All(station_key) => {
                stations.get(&station_key).map_or_default(|wfc| wfc.cache.nodes.as_slice())
            }
            StationRecord::Some(ref nodes) => nodes.as_slice(),
        }
    }
    pub fn station_key<'a>(
        &'a self,
        node_collection: &'a NodeCollection,
    ) -> Option<&'a StationKey> {
        match self {
            StationRecord::All(station_key) => Some(station_key),
            StationRecord::Some(nodes) => {
                nodes.first().and_then(|key| node_collection.get(key)).map(|wfc| &wfc.parent)
            }
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
    /// Generates the progresses of each node in the interval between the current station record and
    /// the next station record.
    ///
    /// Each item in the output iterator is sorted.
    fn progresses<'a>(
        &'a self,
        world: &'a World,
        next_stn_nodes: &'a [NodeKey],
    ) -> impl Iterator<Item = (NodeKey, Option<f32>)> + 'a {
        let curr_stn_nodes = self.station_record.nodes(&world.stations);
        iter::chain(&self.nodes, curr_stn_nodes)
            .map(|node_key| {
                (
                    *node_key,
                    gen_progress(
                        node_key,
                        &world.graph,
                        &self.nodes,
                        curr_stn_nodes,
                        next_stn_nodes,
                    ),
                )
            })
            .sorted_by(|&(_, a), (_, b)| match (a, b) {
                (Some(x), Some(y)) => x.total_cmp(y),
                _ => a.is_some().cmp(&b.is_some()),
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
            is_part_of_interval(&target).then_some((target, interval.length(interval_key, graph)))
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
    let ratio = distance_to_source / (distance_to_source + distance_to_target);
    Some(if ratio.is_nan() { 0.0 } else { ratio })
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RouteIntervals(pub EcoVec<RouteInterval>);

pub type BucketPoint = (TEstimate, EstimateEntry, u32, f32);
pub type TripBucket = SmallVec<[Vec<BucketPoint>; 1]>;

#[derive(Clone, Default, Debug, Serialize)]
pub struct MareyCache {
    pub map: TripKeyHashMap<TripBucket>,
}

impl RouteIntervals {
    /// Returns an iterator that outputs the progresses of all nodes in a station interval,
    /// including the starting nodes of an interval.
    fn progresses<'a>(
        &'a self,
        world: &'a World,
    ) -> impl Iterator<Item = impl Iterator<Item = (NodeKey, Option<f32>)> + 'a> + 'a {
        let records = &self.0;
        let intermediate_iter = records.array_windows().map(|[curr, next]| {
            let next_stn_nodes = next.station_record.nodes(&world.stations);
            Either::Left(curr.progresses(world, next_stn_nodes))
        });
        let last_iter = records
            .last()
            .into_iter()
            .map(|last| Either::Right(last.nodes.iter().map(|&node| (node, Some(0.0)))));
        iter::chain(intermediate_iter, last_iter)
    }

    /// Prepare a lookup for the
    fn prepare_lookup(&self, world: &World) -> NodeKeyHashMap<SmallVec<[(u32, f32); 1]>> {
        puffin::profile_function!();
        self.progresses(world)
            .enumerate()
            .flat_map(|(idx, it)| {
                it.filter_map(move |(key, progress)| Some((key, (idx as u32, progress?))))
            })
            .fold(NodeKeyHashMap::default(), |mut node_lookup, (key, progress)| {
                let key_indices = node_lookup.entry(key).or_default();
                key_indices.push(progress);
                node_lookup
            })
    }

    pub fn all_passing_trips(&self, world: &World) -> Vec<TripKey> {
        puffin::profile_function!();
        let mut trip_keys: Vec<TripKey> = self
            .0
            .iter()
            .flat_map(|route_interval| {
                iter::chain(
                    route_interval.station_record.nodes(&world.stations),
                    &route_interval.nodes,
                )
            })
            .flat_map(|node_key| world.graph.outgoing_intervals(*node_key))
            .flat_map(|(_, wfc)| &wfc.cache.trips)
            .copied()
            .collect();
        trip_keys.sort_unstable();
        trip_keys.dedup();
        trip_keys
    }

    /// Populates trips into a Marey cache
    pub fn populate_trips(
        &self,
        world: &World,
        cache: &mut MareyCache,
        trips_to_rebuild: impl Iterator<Item = TripKey>,
    ) {
        puffin::profile_function!();
        let node_lookup = self.prepare_lookup(world);

        for trip_key in trips_to_rebuild {
            let Some(trip) = world.trips.get(&trip_key) else {
                continue;
            };
            let mut bucket = cache.map.remove(&trip_key).unwrap_or_default();
            bucket.clear();
            trip.schedule.estimates(&world.graph, |est| {
                let update = |a, b| {
                    prep_marey::make_trip_segments(a, b, |node_key| {
                        node_lookup.get(&node_key).map_or(&[], SmallVec::as_slice)
                    })
                };
                let mut all_edges = est.iter().fold(
                    prep_marey::TripAllEdges { active: TripBucket::new(), finalized: &mut bucket },
                    update,
                );
                all_edges.finalized.extend(all_edges.active.drain(..));
            });
            if !bucket.is_empty() {
                cache.map.insert(trip_key, bucket);
            }
        }
    }

    /// Populates a single schedule into the Marey cache
    pub fn populate_schedule(&self, world: &World, schedule: &TripSchedule) -> Option<TripBucket> {
        let node_lookup = self.prepare_lookup(world);
        let mut bucket = TripBucket::new();
        schedule.estimates(&world.graph, |est| {
            let update = |a, b| {
                prep_marey::make_trip_segments(a, b, |node_key| {
                    node_lookup.get(&node_key).map_or(&[], SmallVec::as_slice)
                })
            };
            let mut all_edges = est.iter().fold(
                prep_marey::TripAllEdges { active: TripBucket::new(), finalized: &mut bucket },
                update,
            );
            all_edges.finalized.extend(all_edges.active.drain(..));
        });
        Some(bucket)
    }

    pub fn populate_route_timetable(&self, world: &World) -> RouteTimetable {
        puffin::profile_function!();
        let mut marey_cache = MareyCache::default();
        self.populate_trips(world, &mut marey_cache, self.all_passing_trips(world).into_iter());
        RouteTimetable::new(marey_cache, self.0.iter().map(|entry| &entry.station_record))
    }
}
