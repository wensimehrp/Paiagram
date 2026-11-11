use ecow::{EcoVec, eco_vec};
use rayon::join;
use rstar::{AABB, Envelope, RTree, RTreeObject};
use rustc_hash::FxHashMap;

use crate::graph::Graph;
use crate::time::TimetableTime;
use crate::{Coord, Interval, IntervalCache, IntervalKey, NodeKey, TripKey, Wfc, World, XyPos};

/// Spatial caches used for fast geographic lookups.
///
/// Every tree is derived from the rest of the world, so the cache is never
/// persisted. Instead it is rebuilt from scratch whenever the world changes.
///
/// The trees index projected positions ([`XyPos`]) rather than stored coordinates ([`Coord`]), so
/// they can be queried directly with positions obtained from the screen.
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
        let (intervals, cache) = IntervalsRTree::from_world(world.clone());
        let (nodes, trip_legs) = join(
            || NodesRTree::from_world(world.clone()),
            || TripLegsRTree::from_world_and_cache(world.clone(), cache),
        );
        Self { intervals, nodes, trip_legs }
    }
}

/// Normalise the two corners of a query and build the matching 2D envelope.
fn envelope_2d(xy_min: XyPos, xy_max: XyPos) -> AABB<[i64; 2]> {
    AABB::from_corners([xy_min.0.min(xy_max.0) as i64, xy_min.1.min(xy_max.1) as i64], [
        xy_min.0.max(xy_max.0) as i64,
        xy_min.1.max(xy_max.1) as i64,
    ])
}

/// Like [`envelope_2d`], but collapsed onto the given instant on the time axis.
fn envelope_3d(xy_min: XyPos, xy_max: XyPos, time: TimetableTime) -> AABB<[i64; 3]> {
    let t = i64::from(time.0);
    AABB::from_corners([xy_min.0.min(xy_max.0) as i64, xy_min.1.min(xy_max.1) as i64, t], [
        xy_min.0.max(xy_max.0) as i64,
        xy_min.1.max(xy_max.1) as i64,
        t,
    ])
}

#[derive(Clone, Debug)]
pub struct IntervalRTreeObject {
    pub interval_key: IntervalKey,
    pub nodes: EcoVec<(XyPos, f32)>,
}

impl RTreeObject for IntervalRTreeObject {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.nodes.iter().fold(AABB::new_empty(), |mut aabb, &(xy, _)| {
            aabb.merge(&AABB::from_point([i64::from(xy.0), i64::from(xy.1)]));
            aabb
        })
    }
}

/// Builds the progress and projected positions for an interval.
///
/// Distances are measured on the stored [`Coord`]s, since the projection may distort lengths;
/// the positions themselves are projected so the index can be queried with [`XyPos`].
fn build_progress(
    graph: &Graph,
    key: IntervalKey,
    interval: Option<&Wfc<Interval, IntervalCache>>,
) -> EcoVec<(XyPos, f32)> {
    // stupid path here...
    if let Some(interval) = interval.or(graph.intervals.get(&key)) {
        let mut positions: EcoVec<(XyPos, f32)> = EcoVec::new();
        let mut prev: Option<Coord> = None;
        let mut total_distance = 0.0f32;
        for coord in interval.positions(key, graph.nodes()) {
            if let Some(prev) = prev {
                total_distance += graph.coord_type.distance_to_meters(prev, coord) as f32;
            }
            prev = Some(coord);
            positions.push((graph.coord_type.project(coord), total_distance));
        }
        // normalize the processes
        for (_, progress) in positions.make_mut().iter_mut() {
            *progress = *progress / total_distance
        }
        positions
    } else {
        // Short path if the interval has not been created
        let hi_pos = graph.nodes().get(&key.hi).map_or_default(|wfc| wfc.pos);
        let lo_pos = graph.nodes().get(&key.lo).map_or_default(|wfc| wfc.pos);
        eco_vec![(graph.coord_type.project(hi_pos), 0.0), (graph.coord_type.project(lo_pos), 1.0)]
    }
}

#[derive(Clone, Debug, Copy)]
pub struct NodeRTreeObject {
    pub node_key: NodeKey,
    pub xy: XyPos,
}

impl RTreeObject for NodeRTreeObject {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([i64::from(self.xy.0), i64::from(self.xy.1)])
    }
}

#[derive(Clone, Debug)]
pub struct TripLegRTreeObject {
    /// shared progress in high-key to low-key order
    pub interval: EcoVec<(XyPos, f32)>,
    pub is_hi_to_lo: bool,
    pub trip_key: TripKey,
    pub curr_arr: TimetableTime,
    pub curr_dep: TimetableTime,
    pub next_arr: TimetableTime,
}

impl RTreeObject for TripLegRTreeObject {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.interval.iter().fold(AABB::new_empty(), |mut aabb, (xy, _)| {
            let x = i64::from(xy.0);
            let y = i64::from(xy.1);
            let p1 = [x, y, i64::from(self.curr_arr.0)];
            let p2 = [x, y, i64::from(self.curr_dep.0)];
            let p3 = [x, y, i64::from(self.next_arr.0)];
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
    fn from_world(world: World) -> (Self, FxHashMap<IntervalKey, EcoVec<(XyPos, f32)>>) {
        puffin::profile_function!();
        let mut cache = FxHashMap::default();
        let objects = world
            .graph
            .intervals
            .iter()
            .map(|(interval_key, interval)| IntervalRTreeObject {
                interval_key: *interval_key,
                nodes: {
                    let interval_progress =
                        build_progress(&world.graph, *interval_key, Some(interval));
                    cache.insert(*interval_key, interval_progress.clone());
                    interval_progress
                },
            })
            .collect();
        (Self(RTree::bulk_load(objects)), cache)
    }

    /// Every interval whose geometry intersects the given box.
    pub fn get(&self, xy_min: XyPos, xy_max: XyPos) -> impl Iterator<Item = &IntervalRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_2d(xy_min, xy_max))
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
                xy: world.graph.coord_type.project(node.pos),
            })
            .collect();
        Self(RTree::bulk_load(objects))
    }

    /// Every node inside the given box.
    pub fn get(&self, xy_min: XyPos, xy_max: XyPos) -> impl Iterator<Item = NodeRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_2d(xy_min, xy_max)).cloned()
    }
}

#[derive(Default, Clone, Debug)]
pub struct TripLegsRTree(RTree<TripLegRTreeObject>);

impl TripLegsRTree {
    /// Build the tree from the estimated legs of every trip.
    fn from_world_and_cache(
        world: World,
        progresses: FxHashMap<IntervalKey, EcoVec<(XyPos, f32)>>,
    ) -> Self {
        puffin::profile_function!();
        let mut objects = Vec::with_capacity(world.trips.len() * 10);
        for (trip_key, trip) in &world.trips {
            trip.schedule.estimates(&world.graph, |e| {
                for [(curr_est, curr), (next_est, next)] in e.array_windows() {
                    let (Some(curr_est), Some(next_est)) = (curr_est, next_est) else {
                        continue;
                    };
                    if curr.node_key() == next.node_key() {
                        continue;
                    }
                    let interval_key = IntervalKey::new(curr.node_key(), next.node_key());
                    let progresses = progresses
                        .get(&interval_key)
                        .cloned()
                        .unwrap_or_else(|| build_progress(&world.graph, interval_key, None));
                    objects.push(TripLegRTreeObject {
                        interval: progresses,
                        is_hi_to_lo: curr.node_key() == interval_key.hi,
                        trip_key: *trip_key,
                        curr_arr: curr_est.arr,
                        curr_dep: curr_est.dep,
                        next_arr: next_est.arr,
                    });
                }
            })
        }
        Self(RTree::bulk_load(objects))
    }

    /// Every trip leg whose geometry intersects the given box and whose time
    /// span contains the given instant.
    pub fn get(
        &self,
        xy_min: XyPos,
        xy_max: XyPos,
        time: TimetableTime,
    ) -> impl Iterator<Item = TripLegRTreeObject> {
        self.0.locate_in_envelope_intersecting(&envelope_3d(xy_min, xy_max, time)).cloned()
    }
}
