use std::collections::HashMap;

use ecow::{EcoVec, eco_vec};
use rayon::join;
use rstar::{AABB, Envelope, RTree, RTreeObject};

use crate::graph::Graph;
use crate::time::{TDuration, TimetableTime};
use crate::{Distance, IntervalKey, LonLat, NodeKey, NodeKeyHashMap, TripKey, Wgs84LonLat, World};

/// Spatial caches used for fast geographic lookups.
///
/// Every tree is derived from the rest of the world, so the cache is never
/// persisted. Instead it is rebuilt from scratch whenever the world changes.
#[derive(Default, Clone, Debug)]
pub struct SpatialCache {
    pub intervals: IntervalsRTree,
    pub nodes: NodesRTree,
    pub trip_legs: TripLegsRTree,
}

impl SpatialCache {
    pub fn build(world: &World) -> Self {
        puffin::profile_function!();
        // temp solution
        let (intervals, (nodes, trip_legs)) = join(
            || IntervalsRTree::from_world(world.clone()),
            || {
                join(
                    || NodesRTree::from_world(world.clone()),
                    || TripLegsRTree::from_world(world.clone()),
                )
            },
        );
        Self {
            intervals,
            nodes,
            trip_legs,
        }
    }
}

/// Normalise the two corners of a query and build the matching 2D envelope.
fn envelope_2d(coor_min: LonLat, coor_max: LonLat) -> AABB<[i64; 2]> {
    AABB::from_corners(
        [
            coor_min.lon.min(coor_max.lon) as i64,
            coor_min.lat.min(coor_max.lat) as i64,
        ],
        [
            coor_min.lon.max(coor_max.lon) as i64,
            coor_min.lat.max(coor_max.lat) as i64,
        ],
    )
}

/// Like [`envelope_2d`], but collapsed onto the given instant on the time axis.
fn envelope_3d(coor_min: LonLat, coor_max: LonLat, time: TimetableTime) -> AABB<[i64; 3]> {
    let t = time.0 as i64;
    AABB::from_corners(
        [
            coor_min.lon.min(coor_max.lon) as i64,
            coor_min.lat.min(coor_max.lat) as i64,
            t,
        ],
        [
            coor_min.lon.max(coor_max.lon) as i64,
            coor_min.lat.max(coor_max.lat) as i64,
            t,
        ],
    )
}

#[derive(Clone, Debug)]
pub struct IntervalRTreeObject {
    pub interval_key: IntervalKey,
    pub nodes: EcoVec<LonLat>,
}

impl RTreeObject for IntervalRTreeObject {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.nodes.iter().fold(AABB::new_empty(), |mut aabb, coor| {
            let point = [coor.lon as i64, coor.lat as i64];
            aabb.merge(&AABB::from_point(point));
            aabb
        })
    }
}

#[derive(Clone, Debug, Copy)]
pub struct NodeRTreeObject {
    pub node_key: NodeKey,
    pub coor: LonLat,
}

impl RTreeObject for NodeRTreeObject {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.coor.lon as i64, self.coor.lat as i64])
    }
}

#[derive(Clone, Debug)]
pub struct TripLegRTreeObject {
    /// shared progress in high-key to low-key order
    pub interval: EcoVec<(LonLat, f32)>,
    pub is_hi_to_lo: bool,
    pub trip_key: TripKey,
    pub curr_arr: TimetableTime,
    pub curr_dep: TimetableTime,
    pub next_arr: TimetableTime,
}

impl RTreeObject for TripLegRTreeObject {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.interval.iter().fold(AABB::new_empty(), |mut aabb, (coor, _)| {
            let p1 = [coor.lon as i64, coor.lat as i64, self.curr_arr.0 as i64];
            let p2 = [coor.lon as i64, coor.lat as i64, self.curr_dep.0 as i64];
            let p3 = [coor.lon as i64, coor.lat as i64, self.next_arr.0 as i64];
            aabb.merge(&AABB::from_point(p1));
            aabb.merge(&AABB::from_point(p2));
            aabb.merge(&AABB::from_point(p3));
            aabb
        })
    }
}

#[derive(Default, Clone, Debug)]
pub struct IntervalsRTree(RTree<IntervalRTreeObject>);

impl IntervalsRTree {
    /// Build the tree from every interval in the world's graph.
    fn from_world(world: World) -> Self {
        puffin::profile_function!();
        let objects = world
            .graph
            .intervals()
            .iter()
            .map(|(interval_key, interval)| IntervalRTreeObject {
                interval_key: *interval_key,
                nodes: interval.positions(*interval_key, world.graph.nodes()).collect(),
            })
            .collect();
        Self(RTree::bulk_load(objects))
    }

    /// Every interval whose geometry intersects the given box.
    pub fn get(
        &self,
        coor_min: LonLat,
        coor_max: LonLat,
    ) -> impl Iterator<Item = &IntervalRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_2d(coor_min, coor_max))
    }
}

#[derive(Default, Clone, Debug)]
pub struct NodesRTree(RTree<NodeRTreeObject>);

impl NodesRTree {
    /// Build the tree from every node in the world's graph.
    fn from_world(world: World) -> Self {
        puffin::profile_function!();
        let objects = world
            .graph
            .nodes()
            .iter()
            .map(|(node_key, node)| NodeRTreeObject {
                node_key: *node_key,
                coor: node.pos,
            })
            .collect();
        Self(RTree::bulk_load(objects))
    }

    /// Every node inside the given box.
    pub fn get(&self, coor_min: LonLat, coor_max: LonLat) -> impl Iterator<Item = NodeRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_2d(coor_min, coor_max)).cloned()
    }
}

#[derive(Default, Clone, Debug)]
pub struct TripLegsRTree(RTree<TripLegRTreeObject>);

impl TripLegsRTree {
    /// Build the tree from the estimated legs of every trip.
    fn from_world(world: World) -> Self {
        puffin::profile_function!();
        let mut progresses = HashMap::with_capacity(world.graph.intervals.len());
        let mut objects = Vec::new();
        for (trip_key, trip) in &world.trips {
            trip.schedule.estimates(&world.graph, |estimates| {
                for [(curr_est, curr), (next_est, next)] in estimates.array_windows() {
                    let (Some(curr_est), Some(next_est)) = (curr_est, next_est) else {
                        continue;
                    };
                    if curr.node_key() == next.node_key() {
                        continue;
                    }
                    let interval_key = IntervalKey::new(curr.node_key(), next.node_key());
                    let progresses = progresses
                        .entry(interval_key)
                        .or_insert_with(|| Self::build_progress(&world.graph, interval_key));
                    objects.push(TripLegRTreeObject {
                        interval: progresses.clone(),
                        is_hi_to_lo: curr.node_key() == interval_key.hi,
                        trip_key: *trip_key,
                        curr_arr: curr_est.arr,
                        curr_dep: curr_est.dep,
                        next_arr: next_est.arr,
                    });
                }
            });
        }
        Self(RTree::bulk_load(objects))
    }

    /// Builds the progress and coordinates for an interval
    fn build_progress(graph: &Graph, key: IntervalKey) -> EcoVec<(LonLat, f32)> {
        if let Some(interval) = graph.intervals.get(&key) {
            let mut positions: EcoVec<(LonLat, f32)> =
                interval.positions(key, graph.nodes()).fold(EcoVec::new(), |mut acc, curr| {
                    let segment_distance = if let Some(&(prev, prev_distance)) = acc.last() {
                        let prev: Wgs84LonLat = prev.into();
                        let curr: Wgs84LonLat = curr.into();
                        prev_distance + prev.distance_to_meters(curr) as f32
                    } else {
                        0.0f32
                    };
                    acc.push((curr, segment_distance));
                    acc
                });
            // normalize the processes
            let &(_, total_distance) = positions.last().unwrap();
            // reuse the buffer
            for (_, progress) in positions.make_mut().iter_mut() {
                *progress = *progress / total_distance
            }
            positions
        } else {
            // Short path if the interval has not been created
            let hi_pos = graph.nodes().get(&key.hi).map_or_default(|wfc| wfc.pos);
            let lo_pos = graph.nodes().get(&key.lo).map_or_default(|wfc| wfc.pos);
            eco_vec![(hi_pos, 0.0), (lo_pos, 1.0)]
        }
    }

    /// Every trip leg whose geometry intersects the given box and whose time
    /// span contains the given instant.
    pub fn get(
        &self,
        coor_min: LonLat,
        coor_max: LonLat,
        time: TimetableTime,
    ) -> impl Iterator<Item = TripLegRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_3d(coor_min, coor_max, time)).cloned()
    }
}
