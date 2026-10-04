use ecow::{EcoVec, eco_vec};
use rayon::join;
use rstar::{AABB, Envelope, RTree, RTreeObject};

use crate::time::{TDuration, TimetableTime};
use crate::{IntervalKey, LonLat, NodeKey, TripKey, WorldSnapshot};

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
    pub fn build(snap: &WorldSnapshot) -> Self {
        // temp solution
        let (intervals, (nodes, trip_legs)) = join(
            || IntervalsRTree::from_world(snap.clone()),
            || {
                join(
                    || NodesRTree::from_world(snap.clone()),
                    || TripLegsRTree::from_world(snap.clone()),
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

#[derive(Clone, Debug)]
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
    pub interval: EcoVec<LonLat>,
    pub is_hi_to_lo: bool,
    pub trip_key: TripKey,
    pub curr_arr: TimetableTime,
    pub curr_dep: TimetableTime,
    pub next_arr: TimetableTime,
}

impl RTreeObject for TripLegRTreeObject {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.interval.iter().fold(AABB::new_empty(), |mut aabb, coor| {
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
    fn from_world(snap: WorldSnapshot) -> Self {
        let objects = snap
            .graph
            .intervals()
            .iter()
            .map(|(interval_key, interval)| IntervalRTreeObject {
                interval_key: *interval_key,
                nodes: interval.nodes.clone(),
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
    fn from_world(snap: WorldSnapshot) -> Self {
        let objects = snap
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
    fn from_world(snap: WorldSnapshot) -> Self {
        let mut objects = Vec::new();
        for (trip_key, trip) in snap.trips.iter() {
            trip.schedule.estimates(&snap.graph, |estimates| {
                for [(curr_est, curr), (next_est, next)] in estimates.array_windows() {
                    let (Some(curr_est), Some(next_est)) = (curr_est, next_est) else {
                        continue;
                    };
                    if curr.node_key() == next.node_key() {
                        continue;
                    }
                    let interval_key = IntervalKey::new(curr.node_key(), next.node_key());
                    let Some(interval) = snap.graph.intervals().get(&interval_key) else {
                        continue;
                    };
                    objects.push(TripLegRTreeObject {
                        interval: interval.nodes.clone(),
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
