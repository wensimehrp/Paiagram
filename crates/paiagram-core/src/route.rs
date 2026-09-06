// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/doc.md")]

//! Implementation of the route progress model described above.

use petgraph::algo::bidirectional_dijkstra;
use petgraph::visit::EdgeRef;

use crate::{IntervalProgress, NodeKey, RouteInfo, StationRecord, WorldSnapshot};

impl RouteInfo {
    /// Suboptimal implementation to generate the progress
    pub fn gen_progresses(
        &self,
        world: &WorldSnapshot,
    ) -> Vec<(Vec<Option<IntervalProgress>>, Vec<Option<IntervalProgress>>)> {
        let mut ret = Vec::new();
        for [prev_stn, curr_stn] in self.stations.array_windows::<2>() {
            let curr_nodes = match &curr_stn.stn {
                StationRecord::All(key) => {
                    world.stations.query(*key, |stn| stn.nodes.clone()).unwrap_or_default()
                }
                StationRecord::Some(v) => v.clone(),
            };
            let prev_nodes = match &prev_stn.stn {
                StationRecord::All(key) => {
                    world.stations.query(*key, |stn| stn.nodes.clone()).unwrap_or_default()
                }
                StationRecord::Some(v) => v.clone(),
            };
            let prev_curr_prog: Vec<_> = curr_stn
                .prev_curr_nodes
                .iter()
                .map(|&node| {
                    let (ds, dt) = calc_node_min_distance_batch(
                        world,
                        node,
                        prev_nodes.iter().copied(),
                        curr_nodes.iter().copied(),
                        &curr_stn.prev_curr_nodes,
                    );
                    let ds = ds? as f64;
                    let dt = dt? as f64;
                    let progress = u16::MAX as f64 * (ds / (ds + dt));
                    Some(IntervalProgress(progress as u16))
                })
                .collect();
            let curr_prev_prog: Vec<_> = curr_stn
                .curr_prev_nodes
                .iter()
                .map(|&node| {
                    let (ds, dt) = calc_node_min_distance_batch(
                        world,
                        node,
                        curr_nodes.iter().copied(),
                        prev_nodes.iter().copied(),
                        &curr_stn.curr_prev_nodes,
                    );
                    let ds = ds? as f64;
                    let dt = dt? as f64;
                    let progress = u16::MAX as f64 * (ds / (ds + dt));
                    Some(IntervalProgress(progress as u16))
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
    sources: impl Iterator<Item = NodeKey>,
    targets: impl Iterator<Item = NodeKey>,
    subgraph: &[NodeKey],
) -> (Option<i32>, Option<i32>) {
    let ds = sources
        .map(|source| {
            bidirectional_dijkstra(world, source, node, |e| {
                [e.source(), e.target()]
                    .into_iter()
                    .all(|nd| subgraph.contains(&nd))
                    .then(|| world.intervals.query((e.source(), e.target()), |int| int.length().0))
                    .flatten()
                    .unwrap_or(i32::MAX)
            })
        })
        .min()
        .flatten();
    let dt = targets
        .map(|target| {
            bidirectional_dijkstra(world, target, node, |e| {
                [e.source(), e.target()]
                    .into_iter()
                    .all(|nd| subgraph.contains(&nd))
                    .then(|| world.intervals.query((e.source(), e.target()), |int| int.length().0))
                    .flatten()
                    .unwrap_or(i32::MAX)
            })
        })
        .min()
        .flatten();
    (ds, dt)
}
