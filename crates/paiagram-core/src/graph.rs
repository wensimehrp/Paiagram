//! Definitions for the graph.
use super::{StationKey, WorldSnapshot};
use crate::Distance;

impl WorldSnapshot {
    /// Find a route between the source stop and the target stop.
    /// Returns [`None`] if no valid route exists.
    /// Returns the total length in i32 and the stations on the route if a valid route is found.
    pub fn route_between(
        &self,
        source: StationKey,
        target: StationKey,
    ) -> Option<(Distance, Vec<StationKey>)> {
        petgraph::algo::astar(
            &*self.graph,
            source,
            |node| node == target,
            |(_, _, interval_key)| {
                let Some(handle) = self.intervals.get_handle(*interval_key) else {
                    return i32::MAX;
                };
                self.intervals.length(handle).0
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
        let mut prev = points.next()?;
        let mut total_length = Distance(0);
        let mut passes = vec![prev];
        for curr in points {
            let (leg_length, leg_points) = self.route_between(prev, curr)?;
            total_length += leg_length;
            passes.extend_from_slice(&leg_points[1..]);
            prev = curr;
        }
        Some((total_length, passes))
    }
}
