// SPDX-License-Identifier: MPL-2.0

//! Trip routing functions.
//!
//! Given a trip's nominal schedule (pinned stations) and the graph,
//! this module calculates derived entries (intermediate stations)
//! and time estimates for flexible entries.

use crate::units::time::TimetableTime;
use crate::trip::{TEntry, TripSchedule};
use crate::IntervalCollection;
use crate::{StationKey, WorldGraph, WorldSnapshot};

/// Result of expanding a trip's schedule: a flat list of entries with
/// calculated time estimates.
#[derive(Debug, Clone)]
pub struct ExpandedEntry {
    pub station: StationKey,
    pub arr: Option<TimetableTime>,
    pub dep: Option<TimetableTime>,
    pub is_derived: bool,
}

impl TripSchedule {
    /// Expand the trip's schedule by inserting derived entries between
    /// consecutive pinned entries. Returns the full station list.
    pub fn expand_schedule(
        &self,
        graph: &WorldGraph,
        intervals: &IntervalCollection,
    ) -> Vec<StationKey> {
        let mut result: Vec<StationKey> = Vec::new();
        let entries = self.entries();

        for window in entries.windows(2) {
            let (prev_stn, next_stn) = match (window[0].station_key(), window[1].station_key()) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };

            result.push(prev_stn);

            if prev_stn == next_stn {
                continue;
            }

            // If there's a direct graph edge, no derived stations needed
            if graph.contains_edge(prev_stn, next_stn) {
                continue;
            }

            // Find shortest path between the two stations
            if let Some((_, path)) = WorldSnapshot::route_between_static(
                graph, intervals, prev_stn, next_stn,
            ) {
                // Skip first (already pushed as prev_stn) and last
                // (will be pushed by next iteration).
                for &stn in &path[1..path.len().saturating_sub(1)] {
                    result.push(stn);
                }
            }
        }

        // Push the last station
        if let Some(last) = entries.last().and_then(|e| e.station_key()) {
            result.push(last);
        }

        result
    }
}

impl WorldSnapshot {
    /// Find a route between source and target stations using the graph.
    /// Returns total distance and the station path (inclusive).
    pub fn route_between_static(
        graph: &WorldGraph,
        intervals: &IntervalCollection,
        source: StationKey,
        target: StationKey,
    ) -> Option<(crate::Distance, Vec<StationKey>)> {
        petgraph::algo::astar(
            graph,
            source,
            |node| node == target,
            |(_, _, interval_key)| {
                intervals
                    .query(*interval_key, |view| view.length().0)
                    .unwrap_or(i32::MAX)
            },
            |_| 0,
        )
        .map(|(d, chain)| (crate::Distance(d), chain))
    }
}

impl TEntry {
    /// Return the station key of a pinned or derived entry, if available.
    pub fn station_key(&self) -> Option<StationKey> {
        match self {
            TEntry::Derived(stn)
            | TEntry::Pinned { stn, .. }
            | TEntry::PinnedNonStop { stn, .. }
            | TEntry::PinnedExternalNonStop { stn, .. } => Some(*stn),
            TEntry::PinnedExternal { .. } => None,
        }
    }
}
