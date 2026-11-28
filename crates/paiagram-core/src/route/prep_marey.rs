use smallvec::SmallVec;

use crate::NodeKey;
use crate::route::TripBucket;
use crate::trip::{EstimateEntry, TEstimate};

pub(super) struct TripAllEdges<'a> {
    /// Active edges that may be expanded.
    pub active: TripBucket,
    /// Finalized edges that are either invalidated because of an invalid estimate or finalized
    /// because it's impossible to extend the edge any more
    pub finalized: &'a mut TripBucket,
}

/// Prepare the segments for a trip on a marey chart
pub(super) fn make_trip_segments<'a>(
    mut all_edges: TripAllEdges<'a>,
    (estimate, entry): &(Option<TEstimate>, EstimateEntry),
    mut node_lookup: impl FnMut(NodeKey) -> &'a [(u32, f32)],
) -> TripAllEdges<'a> {
    let &Some(estimate) = estimate else {
        // invalid estimate invalidates the working set
        all_edges.finalized.extend(all_edges.active.drain(..));
        return all_edges;
    };

    let curr_idxs = node_lookup(entry.node_key());
    let mut new_active = SmallVec::new();

    for &(curr_idx, curr_progress) in curr_idxs {
        if let Some(pos) = all_edges.active.iter().position(|edge| {
            let (_estimate, _node, station_idx, _progress) = edge.last().unwrap();
            station_idx.abs_diff(curr_idx) <= 1
        }) {
            let mut edge = all_edges.active.swap_remove(pos);
            edge.push((estimate, *entry, curr_idx, curr_progress));
            new_active.push(edge);
        } else {
            new_active.push(vec![(estimate, *entry, curr_idx, curr_progress)]);
        }
    }

    all_edges.finalized.extend(all_edges.active.drain(..));
    all_edges.active = new_active;
    all_edges
}
