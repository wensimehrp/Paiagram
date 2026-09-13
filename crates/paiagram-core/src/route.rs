// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/doc.md")]

//! Implementation of the route progress model described above.

use petgraph::algo::bidirectional_dijkstra;
use petgraph::visit::EdgeRef;

use crate::{IntervalProgress, NodeKey, RouteInfo, StationRecord, WorldSnapshot};

impl RouteInfo {
    /// Suboptimal implementation to generate the progress
    pub(crate) fn gen_progresses(
        &self,
        world: &WorldSnapshot,
    ) -> Vec<(Vec<Option<IntervalProgress>>, Vec<Option<IntervalProgress>>)> {
        let mut ret = Vec::new();
        for [prev_stn, curr_stn] in self.stations.array_windows::<2>() {
            let curr_nodes = match &curr_stn.stn {
                StationRecord::All(key) => world
                    .stations
                    .query(*key, |stn| {
                        stn.nodes
                            .iter()
                            .copied()
                            .filter(|k| world.nodes.query(*k, |n| *n.is_platform).unwrap_or(false))
                            .collect::<ecow::EcoVec<_>>()
                    })
                    .unwrap_or_default(),
                StationRecord::Some(v) => v.clone(),
            };
            let prev_nodes = match &prev_stn.stn {
                StationRecord::All(key) => world
                    .stations
                    .query(*key, |stn| {
                        stn.nodes
                            .iter()
                            .copied()
                            .filter(|k| world.nodes.query(*k, |n| *n.is_platform).unwrap_or(false))
                            .collect::<ecow::EcoVec<_>>()
                    })
                    .unwrap_or_default(),
                StationRecord::Some(v) => v.clone(),
            };
            let prev_curr_prog: Vec<_> = curr_stn
                .prev_curr_nodes
                .iter()
                .map(|&node| {
                    let (ds, dt) = calc_node_min_distance_batch(
                        world,
                        node,
                        &prev_nodes,
                        &curr_nodes,
                        &curr_stn.prev_curr_nodes,
                    );
                    let ds = ds? as f64;
                    let dt = dt? as f64;
                    Some(IntervalProgress::from_ratio(if ds + dt == 0.0 {
                        0.0
                    } else {
                        ds / (ds + dt)
                    }))
                })
                .collect();
            let curr_prev_prog: Vec<_> = curr_stn
                .curr_prev_nodes
                .iter()
                .map(|&node| {
                    let (ds, dt) = calc_node_min_distance_batch(
                        world,
                        node,
                        &curr_nodes,
                        &prev_nodes,
                        &curr_stn.curr_prev_nodes,
                    );
                    let ds = ds? as f64;
                    let dt = dt? as f64;
                    Some(IntervalProgress::from_ratio(if ds + dt == 0.0 {
                        0.0
                    } else {
                        ds / (ds + dt)
                    }))
                })
                .collect();
            ret.push((prev_curr_prog, curr_prev_prog));
        }
        ret
    }
}

fn calc_node_min_distance_batch(
    world: &WorldSnapshot,
    node: NodeKey,
    sources: &[NodeKey],
    targets: &[NodeKey],
    subgraph: &[NodeKey],
) -> (Option<u64>, Option<u64>) {
    let subgraph_contains_node =
        |nd: NodeKey| subgraph.contains(&nd) || sources.contains(&nd) || targets.contains(&nd);
    let graph = petgraph::visit::EdgeFiltered::from_fn(world, |e| {
        subgraph_contains_node(e.source()) && subgraph_contains_node(e.target())
    });
    let ds = sources
        .iter()
        .filter_map(|&source| {
            bidirectional_dijkstra(&graph, source, node, |e| {
                world.intervals.get((e.source(), e.target())).unwrap().length().0.max(0) as u64
            })
        })
        .min();
    let dt = targets
        .iter()
        .filter_map(|&target| {
            bidirectional_dijkstra(&graph, node, target, |e| {
                world.intervals.get((e.source(), e.target())).unwrap().length().0.max(0) as u64
            })
        })
        .min();
    (ds, dt)
}

impl crate::RouteStationRecord {
    /// Construct an all-platform record, with directional shortest paths from the preceding
    /// station.
    pub fn for_station(
        world: &WorldSnapshot,
        station: crate::StationKey,
        previous: Option<crate::StationKey>,
    ) -> Self {
        let platforms = |key| {
            world
                .stations
                .query(key, |s| {
                    s.nodes
                        .iter()
                        .copied()
                        .filter(|n| world.nodes.query(*n, |n| *n.is_platform).unwrap_or(false))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let mut record = Self {
            stn: StationRecord::All(station),
            milestone: None,
            canvas_length: None,
            prev_curr_nodes: Default::default(),
            curr_prev_nodes: Default::default(),
        };
        if let Some(previous) = previous {
            let a = platforms(previous);
            let b = platforms(station);
            for (sources, targets, output) in [
                (&a, &b, &mut record.prev_curr_nodes),
                (&b, &a, &mut record.curr_prev_nodes),
            ] {
                for &source in sources {
                    for &target in targets {
                        if let Some((_, path)) = petgraph::algo::astar(
                            world,
                            source,
                            |n| n == target,
                            |e| world.intervals.get(*e.weight()).unwrap().length().0.max(0) as u64,
                            |_| 0,
                        ) {
                            for node in path {
                                if !sources.contains(&node)
                                    && !targets.contains(&node)
                                    && !output.contains(&node)
                                {
                                    output.push(node);
                                }
                            }
                        }
                    }
                }
            }
        }
        record
    }
}
