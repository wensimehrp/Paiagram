//! Definitions for the graph.
use super::{StationKey, WorldSnapshot};
use crate::{Distance, IntervalDirection, NodeKey};

impl WorldSnapshot {
    /// Find a route between the source station and the target station.
    /// Returns [`None`] if no valid routes exist.
    /// Returns the total distance and the stations on the route if a valid route is found.
    // Since stations are just collections of nodes, we instead run an O(n*m*k) complexity
    // search where n, m := nodes of source and target and k := complexity of each astar
    // and I just realized that I might need two graph structures... So the question is how can I
    // maintain two graphs?? TBH I don't really want to :-( I'll leave it todo now...
    // TODO: station subgraph OR smart way to avoid that.
    pub fn route_between_stations(
        &self,
        source: StationKey,
        target: StationKey,
    ) -> Option<(Distance, Vec<StationKey>)> {
        if source == target {
            return Some((Distance::ZERO, Vec::new()));
        }
        // cheap clone
        let source_nodes = self.stations.query(source, |view| view.nodes.clone())?;
        let target_nodes = self.stations.query(target, |view| view.nodes.clone())?;
        for node in source_nodes {
            // a la la la la
            // la la land!
        }
        // petgraph::algo::astar(
        //     &*self.graph,
        //     source,
        //     |node| node == target,
        //     |(_, _, interval_key)| {
        //         let mut query_key = *interval_key;
        //         // discard the info on the other key
        //         if let Some(IntervalDirection::TwoWay(other)) =
        //             self.intervals.query(query_key, |view| *view.direction)
        //         {
        //             query_key = std::cmp::min(other, query_key);
        //         }
        //         let Some(length) = self.intervals.query(query_key, |view| view.length()) else {
        //             return i32::MAX;
        //         };
        //         length.0
        //     },
        //     |_| 0,
        // )
        // .map(|(d, chain)| (Distance(d), chain))
        todo!()
    }

    /// Find a route between the source node and the target node.
    /// Returns [`None`] if no valid routes exist.
    /// Returns the total distances and the nodes on the route if a valid route is found.
    pub fn route_between_nodes(
        &self,
        source: NodeKey,
        target: NodeKey,
    ) -> Option<(Distance, Vec<NodeKey>)> {
        petgraph::algo::astar(
            &*self.graph,
            source,
            |node| node == target,
            |(_, _, interval_key)| {
                let mut query_key = *interval_key;
                // discard the info on the other key
                if let Some(IntervalDirection::TwoWay(other)) =
                    self.intervals.query(query_key, |view| *view.direction)
                {
                    query_key = std::cmp::min(other, query_key);
                }
                let Some(length) = self.intervals.query(query_key, |view| view.length()) else {
                    return i32::MAX;
                };
                length.0
            },
            |_| 0,
        )
        .map(|(d, chain)| (Distance(d), chain))
    }

    /// Find a route given a set of stations that must be on the route.
    /// Returns [`None`] if no valid route exists.
    /// Returns the total length and the stations on the route if a valid route is found.
    pub fn route_between_source_waypoint_target(
        &self,
        mut points: impl Iterator<Item = StationKey>,
    ) -> Option<(Distance, Vec<StationKey>)> {
        // let mut prev = points.next()?;
        // let mut total_length = Distance(0);
        // let mut passes = vec![prev];
        // for curr in points {
        //     let (leg_length, leg_points) = self.route_between(prev, curr)?;
        //     total_length += leg_length;
        //     passes.extend_from_slice(&leg_points[1..]);
        //     prev = curr;
        // }
        // Some((total_length, passes))
        todo!()
    }
}
