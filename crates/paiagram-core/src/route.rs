// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("route/doc.md")]

//! Implementation of the route progress model described above.

use ecow::EcoVec;
use petgraph::algo::bidirectional_dijkstra;
use petgraph::visit::EdgeRef;

use crate::{IntervalProgress, NodeKey, RouteInfo, StationRecord, WorldSnapshot};

impl RouteInfo {
    pub fn gen_progresses(
        &self,
        world: &WorldSnapshot,
    ) -> Vec<(
        Vec<(NodeKey, IntervalProgress)>,
        Vec<(NodeKey, IntervalProgress)>,
    )> {
        let mut ret = Vec::with_capacity(self.stations.len());
        let mut buf = Vec::new();
        let get_record_station_nodes = |rec: &StationRecord| -> EcoVec<NodeKey> {
            match rec {
                StationRecord::All(stn) => {
                    world.stations.query(*stn, |view| view.nodes.clone()).unwrap_or_default()
                }
                StationRecord::Some(nodes) => nodes.clone(),
            }
        };
        for [prev, curr] in self.stations.array_windows::<2>() {
            let record_stn_nodes = get_record_station_nodes(&curr.stn);
            buf.clear();
            buf.extend(curr.prev_curr_nodes.iter().map(|&start| {
                record_stn_nodes
                    .iter()
                    .map(|&goal| get_edge_cost(world, start, goal))
                    .min()
                    .unwrap_or_default()
            }));
            let max_len = buf.iter().max().copied().unwrap_or(i32::MAX);
        }
        ret
    }
}

fn get_edge_cost(world: &WorldSnapshot, start: NodeKey, goal: NodeKey) -> i32 {
    bidirectional_dijkstra(world, start, goal, |edge| {
        world
            .intervals
            .query((edge.source(), edge.target()), |view| view.length().0)
            .unwrap_or(i32::MAX)
    })
    .unwrap_or(i32::MAX)
}
