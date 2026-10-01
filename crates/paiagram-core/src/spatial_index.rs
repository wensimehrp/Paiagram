use ecow::EcoVec;
use rstar::{AABB, Envelope, RTree, RTreeObject};

use crate::time::TimetableTime;
use crate::{IntervalKey, LonLat, NodeKey, TripKey};

pub struct SpatialCache {
    pub intervals: IntervalsRTree,
    pub nodes: NodesRTree,
    pub trip_legs: TripLegsRTree,
}

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

pub struct TripLegRTreeObject {
    pub interval: EcoVec<LonLat>,
    pub trip_key: TripKey,
    pub estimate_dep: TimetableTime,
    pub estimate_arr: TimetableTime,
}

impl RTreeObject for TripLegRTreeObject {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.interval.iter().fold(AABB::new_empty(), |mut aabb, coor| {
            let p1 = [coor.lon as i64, coor.lat as i64, self.estimate_dep.0 as i64];
            let p2 = [coor.lon as i64, coor.lat as i64, self.estimate_arr.0 as i64];
            aabb.merge(&AABB::from_point(p1));
            aabb.merge(&AABB::from_point(p2));
            aabb
        })
    }
}

pub struct IntervalsRTree(RTree<IntervalRTreeObject>);

impl IntervalsRTree {
    pub fn get(
        &self,
        coor_min: LonLat,
        coor_max: LonLat,
    ) -> impl Iterator<Item = &IntervalRTreeObject> {
        [].into_iter()
    }
}

pub struct NodesRTree(RTree<NodeRTreeObject>);

impl NodesRTree {
    pub fn get(&self, coor_min: LonLat, coor_max: LonLat) -> impl Iterator<Item = NodeRTreeObject> {
        [].into_iter()
    }
}

pub struct TripLegsRTree(RTree<TripLegRTreeObject>);

impl TripLegsRTree {
    pub fn get(
        &self,
        coor_min: LonLat,
        coor_max: LonLat,
        time: TimetableTime,
    ) -> impl Iterator<Item = TripLegRTreeObject> {
        [].into_iter()
    }
}
