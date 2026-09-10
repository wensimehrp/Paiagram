//! Integer spatial indexes with copy-on-write polylines.
//!
//! Stations and intervals retain the snapshot's i32 longitude/latitude coordinates.
//! Trip geometry is projected once per interval to i32 XY coordinates with u32 progress,
//! then cheaply cloned into every trip using that interval. R-tree envelopes widen to i64.
use rstar::{AABB, RTree, RTreeObject};

use crate::*;

#[derive(Clone, Debug)]
pub struct MapPoint<K> {
    pub key: K,
    pub point: LonLat,
}

impl<K> RTreeObject for MapPoint<K> {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([i64::from(self.point.lon), i64::from(self.point.lat)])
    }
}

#[derive(Clone, Debug)]
pub struct IntervalSpatialEntry {
    pub key: IntervalKey,
    /// All vertices, including both endpoints. Shares storage with Interval::nodes
    /// when its endpoint positions already agree with the authoritative nodes.
    pub points: EcoVec<LonLat>,
    /// Projected once, then shared by every timetable entry traversing this interval.
    pub projected: EcoVec<(u32, XyPos)>,
}
impl RTreeObject for IntervalSpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        point_bounds(self.points.iter().map(|p| [p.lon, p.lat]))
    }
}

#[derive(Clone, Debug)]
pub struct TEntrySpatialEntry {
    pub trip: TripKey,
    /// A complete polyline with cumulative progress, never a flattened pair of endpoints.
    pub points: EcoVec<(u32, XyPos)>,
    /// Absolute departure (or dwell start) and next arrival (or dwell end), in seconds.
    pub from: i32,
    pub until: i32,
}
impl RTreeObject for TEntrySpatialEntry {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        let xy = point_bounds(self.points.iter().map(|(_, p)| [p.x, p.y]));
        let lo = xy.lower();
        let hi = xy.upper();
        AABB::from_corners(
            [lo[0], lo[1], i64::from(self.from)],
            [hi[0], hi[1], i64::from(self.until)],
        )
    }
}
impl TEntrySpatialEntry {
    /// Interpolate within the enclosing pair of vertices, including its local heading.
    /// Floating point is only used for this calculation, not cached coordinates/envelopes.
    pub fn position_and_angle(&self, time: f64) -> Option<(XyPos, f64)> {
        let &(_, first) = self.points.first()?;
        if self.points.len() == 1 || self.until <= self.from {
            return Some((first, 0.0));
        }
        let fraction = ((time - f64::from(self.from))
            / (f64::from(self.until) - f64::from(self.from)))
        .clamp(0.0, 1.0);
        let progress = (fraction * f64::from(u32::MAX)).round() as u32;
        // The upper bound skips coincident vertices. At the endpoint, use the final
        // nonzero segment so a duplicate final vertex does not erase the heading.
        let (index, next) = if progress == u32::MAX {
            let last = self.points.len() - 1;
            let index = self.points[..last].iter().rposition(|(p, _)| *p < u32::MAX).unwrap_or(0);
            (index, last)
        } else {
            let next =
                self.points.partition_point(|(p, _)| *p <= progress).min(self.points.len() - 1);
            (next.saturating_sub(1), next)
        };
        let (p0, a) = self.points[index];
        let (p1, b) = self.points[next];
        let local = if p1 > p0 {
            f64::from(progress.saturating_sub(p0)) / f64::from(p1 - p0)
        } else {
            0.0
        };
        // Widen BEFORE subtraction: a single segment can span both sides of the world.
        let dx = i64::from(b.x) - i64::from(a.x);
        let dy = i64::from(b.y) - i64::from(a.y);
        let pos = XyPos {
            x: (f64::from(a.x) + local * dx as f64).round() as i32,
            y: (f64::from(a.y) + local * dy as f64).round() as i32,
        };
        Some((pos, (dy as f64).atan2(dx as f64)))
    }
    pub fn get_pos_angle_at(&self, time_secs: i32) -> Option<(XyPos, f64)> {
        self.position_and_angle(f64::from(time_secs))
    }
}

#[derive(Default)]
pub struct SpatialCache {
    stations: RTree<MapPoint<StationKey>>,
    nodes: RTree<MapPoint<NodeKey>>,
    intervals: RTree<IntervalSpatialEntry>,
    trips: RTree<TEntrySpatialEntry>,
    time_range: Option<(i32, i32)>,
}

/// Display-space metres; deliberately separate from the integer cache representation.
pub fn project(pos: LonLat) -> [f64; 2] {
    let p = XyPosF64::from(Wgs84LonLat::from(pos));
    [p.x, p.y]
}

/// Convert the screen's metre bounds to conservative longitude/latitude index bounds.
pub fn geographic_bounds(min: [f64; 2], max: [f64; 2]) -> ([i32; 2], [i32; 2]) {
    let a = Wgs84LonLat::from(XyPosF64::new(min[0], min[1]));
    let b = Wgs84LonLat::from(XyPosF64::new(max[0], max[1]));
    // Geographic coordinates are degrees * 10^7. Latitude reverses the screen's Y axis.
    let lower = [
        (a.lon.min(b.lon).clamp(-180.0, 180.0) * 10_000_000.0).floor() as i32,
        (a.lat.min(b.lat).clamp(-90.0, 90.0) * 10_000_000.0).floor() as i32,
    ];
    let upper = [
        (a.lon.max(b.lon).clamp(-180.0, 180.0) * 10_000_000.0).ceil() as i32,
        (a.lat.max(b.lat).clamp(-90.0, 90.0) * 10_000_000.0).ceil() as i32,
    ];
    (lower, upper)
}
/// Convert metre bounds to fixed-point XY, rounding outwards to avoid dropping edge hits.
pub fn projected_bounds(min: [f64; 2], max: [f64; 2]) -> ([i32; 2], [i32; 2]) {
    (
        std::array::from_fn(|i| (min[i].min(max[i]) * XyPos::UNITS_PER_METRE).floor() as i32),
        std::array::from_fn(|i| (min[i].max(max[i]) * XyPos::UNITS_PER_METRE).ceil() as i32),
    )
}
fn bounds(a: [i32; 2], b: [i32; 2]) -> AABB<[i64; 2]> {
    AABB::from_corners(
        std::array::from_fn(|i| i64::from(a[i].min(b[i]))),
        std::array::from_fn(|i| i64::from(a[i].max(b[i]))),
    )
}
fn point_bounds(mut points: impl Iterator<Item = [i32; 2]>) -> AABB<[i64; 2]> {
    let first = points.next().unwrap_or([0, 0]);
    let mut lo = first;
    let mut hi = first;
    for p in points {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    bounds(lo, hi)
}
fn project_polyline(points: &[LonLat]) -> EcoVec<(u32, XyPos)> {
    let mut projected: EcoVec<_> =
        points.iter().map(|p| (0, XyPos::from(XyPosF64::from(Wgs84LonLat::from(*p))))).collect();
    let distance = |a: XyPos, b: XyPos| {
        ((i64::from(b.x) - i64::from(a.x)) as f64).hypot((i64::from(b.y) - i64::from(a.y)) as f64)
    };
    let total: f64 = projected.windows(2).map(|p| distance(p[0].1, p[1].1)).sum();
    let mut cumulative = 0.0;
    let vertices = projected.make_mut();
    for i in 1..vertices.len() {
        cumulative += distance(vertices[i - 1].1, vertices[i].1);
        vertices[i].0 = if total > 0.0 {
            (cumulative / total * f64::from(u32::MAX)).round() as u32
        } else {
            0
        };
    }
    if vertices.len() > 1 {
        vertices.last_mut().unwrap().0 = u32::MAX;
    }
    projected
}
fn interval_entry(world: &WorldSnapshot, key: IntervalKey) -> Option<IntervalSpatialEntry> {
    let a = world.nodes.query(key.0, |n| *n.pos)?;
    let b = world.nodes.query(key.1, |n| *n.pos)?;
    let mut points = world.intervals.get(key)?.nodes.clone();
    if points.len() < 2 {
        points = [a, b].into_iter().collect();
    } else if points[0] != a || *points.last()? != b {
        let points = points.make_mut();
        points[0] = a;
        *points.last_mut()? = b;
    }
    let projected = project_polyline(&points);
    Some(IntervalSpatialEntry {
        key,
        points,
        projected,
    })
}
impl SpatialCache {
    pub fn build(world: &WorldSnapshot) -> Self {
        // Bulk loading requires Vecs of index entries; the geometry inside each entry is EcoVec.
        let stations = RTree::bulk_load(
            world
                .stations
                .iter()
                .map(|v| MapPoint {
                    key: v.key,
                    point: *v.pos,
                })
                .collect(),
        );
        let nodes = RTree::bulk_load(
            world
                .nodes
                .iter()
                .map(|v| MapPoint {
                    key: v.key,
                    point: *v.pos,
                })
                .collect(),
        );
        let intervals: FxHashMap<_, _> = world
            .intervals
            .keys()
            .filter_map(|&key| Some((key, interval_entry(world, key)?)))
            .collect();
        let mut samples = Vec::new();
        for trip in world.trips.iter() {
            trip.schedule.estimates(&world.intervals, |estimates| {
                for (estimate, entry) in estimates {
                    if entry.is_external() {
                        continue;
                    }
                    if let Some(estimate) = estimate
                        && estimate.dep.0 >= estimate.arr.0
                        && let Some(point) = world.nodes.query(entry.node_key(), |v| {
                            XyPos::from(XyPosF64::from(Wgs84LonLat::from(*v.pos)))
                        })
                    {
                        samples.push(TEntrySpatialEntry {
                            trip: trip.key,
                            points: [(0, point)].into_iter().collect(),
                            from: estimate.arr.0,
                            until: estimate.dep.0,
                        });
                    }
                }
                let mut previous: Option<(Option<trip::TEstimate>, trip::TEntry)> = None;
                for &(estimate, entry) in estimates {
                    if entry.is_external() {
                        continue;
                    }
                    if let Some((Some(a), ae)) = previous
                        && let Some(b) = estimate
                        && b.arr.0 > a.dep.0
                    {
                        let key = (ae.node_key(), entry.node_key());
                        if let Some(interval) = intervals.get(&key) {
                            samples.push(TEntrySpatialEntry {
                                trip: trip.key,
                                points: interval.projected.clone(),
                                from: a.dep.0,
                                until: b.arr.0,
                            });
                        }
                    }
                    previous = Some((estimate, entry));
                }
            });
        }
        let time_range =
            samples.iter().map(|s| (s.from, s.until)).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        Self {
            stations,
            nodes,
            intervals: RTree::bulk_load(intervals.into_values().collect()),
            trips: RTree::bulk_load(samples),
            time_range,
        }
    }
    /// Bounds are i32 longitude/latitude units, matching LonLat.
    pub fn stations(
        &self,
        min: [i32; 2],
        max: [i32; 2],
    ) -> impl Iterator<Item = &MapPoint<StationKey>> {
        self.stations.locate_in_envelope_intersecting(&bounds(min, max))
    }
    /// Bounds are i32 longitude/latitude units, matching LonLat.
    pub fn nodes(&self, min: [i32; 2], max: [i32; 2]) -> impl Iterator<Item = &MapPoint<NodeKey>> {
        self.nodes.locate_in_envelope_intersecting(&bounds(min, max))
    }
    /// One hit per interval, with every vertex retained. Bounds use LonLat units.
    pub fn intervals(
        &self,
        min: [i32; 2],
        max: [i32; 2],
    ) -> impl Iterator<Item = &IntervalSpatialEntry> {
        self.intervals.locate_in_envelope_intersecting(&bounds(min, max))
    }
    /// Bounds use i32 XyPos units. Absolute times allow overnight/repeated services.
    pub fn trips(
        &self,
        min: [i32; 2],
        max: [i32; 2],
        time: f64,
        period: f64,
    ) -> Vec<(&TEntrySpatialEntry, f64)> {
        let Some((start, end)) = self.time_range else {
            return Vec::new();
        };
        if !time.is_finite() || !period.is_finite() {
            return Vec::new();
        }
        let (first, last) = if period > 0.0 {
            (
                ((f64::from(start) - time) / period).ceil() as i64,
                ((f64::from(end) - time) / period).floor() as i64,
            )
        } else {
            (0, 0)
        };
        let xy = bounds(min, max);
        let lo = xy.lower();
        let hi = xy.upper();
        let mut result = Vec::new();
        for cycle in first..=last {
            let t = time + cycle as f64 * period.max(0.0);
            let envelope = AABB::from_corners(
                [lo[0], lo[1], t.floor() as i64],
                [hi[0], hi[1], t.ceil() as i64],
            );
            result.extend(
                self.trips
                    .locate_in_envelope_intersecting(&envelope)
                    .filter(|s| f64::from(s.from) <= t && t <= f64::from(s.until))
                    .map(|s| (s, t)),
            );
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimetableTime;
    use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};

    fn curved_world() -> (WorldSnapshot, IntervalKey, [TripKey; 2]) {
        let mut world = WorldSnapshot::default();
        let (a, b) = (NodeKey::new(), NodeKey::new());
        let coordinates: EcoVec<LonLat> = [
            (110.0, 35.0),
            (110.01, 35.0),
            (110.01, 35.02),
            (110.03, 35.02),
            (110.04, 35.01),
        ]
        .into_iter()
        .map(|(lon, lat)| Wgs84LonLat::new(lon, lat).into())
        .collect();
        for (node, pos) in [(a, coordinates[0]), (b, *coordinates.last().unwrap())] {
            let station = StationKey::new();
            world
                .apply_command(Command::AddStation {
                    key: station,
                    info: StationInfo {
                        name: "S".into(),
                        pos,
                    },
                })
                .unwrap();
            world
                .apply_command(Command::AddNode {
                    key: node,
                    info: NodeInfo {
                        name: "1".into(),
                        parent: station,
                        pos,
                        is_platform: true,
                    },
                })
                .unwrap();
        }
        world
            .apply_command(Command::AddInterval {
                key: (a, b),
                info: Interval {
                    nodes: coordinates,
                    length: None,
                    trips: EcoVec::new(),
                },
            })
            .unwrap();
        let trips = [TripKey::new(), TripKey::new()];
        for key in trips {
            let entries = [(a, 100), (b, 200)]
                .into_iter()
                .map(|(node, t)| TEntry::PinnedNonStop {
                    node,
                    pass: TravelMode::At(TimetableTime(t)),
                    external: false,
                    id: TEntryId::new(),
                })
                .collect();
            world
                .apply_command(Command::AddTrip {
                    key,
                    info: TripInfo {
                        name: "T".into(),
                        schedule: TripSchedule::new(entries),
                        service_class: None,
                        vehicles: Default::default(),
                    },
                })
                .unwrap();
        }
        (world, (a, b), trips)
    }

    #[test]
    fn intervals_keep_all_vertices_and_trip_entries_share_projected_storage() {
        let (world, key, trips) = curved_world();
        let cache = SpatialCache::build(&world);
        let all = [i32::MIN; 2];
        let high = [i32::MAX; 2];
        let entries: Vec<_> = cache.intervals(all, high).collect();
        assert_eq!(entries.len(), 1);
        let interval = entries[0];
        assert_eq!(interval.points.len(), 5);
        assert_eq!(interval.projected.len(), 5);
        // EcoVec must actually share its backing storage, not merely wrap freshly copied Vecs.
        assert_eq!(
            interval.points.as_ptr(),
            world.intervals.get(key).unwrap().nodes.as_ptr()
        );
        let samples = cache.trips(all, high, 150.0, 0.0);
        assert_eq!(samples.len(), 2);
        for (sample, _) in &samples {
            assert!(trips.contains(&sample.trip));
            assert_eq!(sample.points.len(), 5);
            assert_eq!(sample.points.as_ptr(), interval.projected.as_ptr());
        }
        let mut edited = samples[0].0.clone();
        let saved = edited.points[2];
        edited.points.make_mut()[2].1.x += 123;
        assert_ne!(edited.points.as_ptr(), samples[0].0.points.as_ptr());
        assert_eq!(samples[0].0.points[2], saved);
        assert_eq!(samples[1].0.points[2], saved);
        assert_eq!(interval.projected[2], saved);
        let bend = interval.points[2];
        assert_eq!(
            cache.intervals([bend.lon, bend.lat], [bend.lon, bend.lat]).count(),
            1
        );
        let (progress, xy) = interval.projected[2];
        let t = 100.0 + f64::from(progress) / f64::from(u32::MAX) * 100.0;
        assert_eq!(cache.trips([xy.x, xy.y], [xy.x, xy.y], t, 0.0).len(), 2);
    }

    #[test]
    fn interpolation_follows_each_leg_and_returns_its_heading() {
        let sample = TEntrySpatialEntry {
            trip: TripKey::new(),
            from: 100,
            until: 200,
            points: [
                (0, XyPos { x: 0, y: 0 }),
                (u32::MAX / 2, XyPos { x: 1000, y: 0 }),
                (u32::MAX, XyPos { x: 1000, y: 1000 }),
            ]
            .into_iter()
            .collect(),
        };
        let (p, a) = sample.get_pos_angle_at(125).unwrap();
        assert_eq!(p, XyPos { x: 500, y: 0 });
        assert_eq!(a, 0.0);
        let (p, a) = sample.get_pos_angle_at(175).unwrap();
        assert_eq!(p, XyPos { x: 1000, y: 500 });
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(sample.get_pos_angle_at(0).unwrap().0, XyPos { x: 0, y: 0 });
        assert_eq!(
            sample.get_pos_angle_at(300).unwrap().0,
            XyPos { x: 1000, y: 1000 }
        );
    }

    #[test]
    fn repeated_vertices_and_stationary_entries_are_finite() {
        let points = [LonLat::ZERO; 4];
        let projected = project_polyline(&points);
        assert_eq!(projected.len(), 4);
        assert_eq!(projected[0].0, 0);
        assert_eq!(projected[3].0, u32::MAX);
        let sample = TEntrySpatialEntry {
            trip: TripKey::new(),
            from: 0,
            until: 10,
            points: projected,
        };
        assert_eq!(
            sample.get_pos_angle_at(5).unwrap(),
            (XyPos { x: 0, y: 0 }, 0.0)
        );
        let sample = TEntrySpatialEntry {
            points: [(0, XyPos { x: 8, y: 9 })].into_iter().collect(),
            from: 1,
            until: 1,
            ..sample
        };
        assert_eq!(
            sample.get_pos_angle_at(1).unwrap(),
            (XyPos { x: 8, y: 9 }, 0.0)
        );
        let sample = TEntrySpatialEntry {
            points: EcoVec::new(),
            ..sample
        };
        assert_eq!(sample.get_pos_angle_at(1), None);
    }

    #[test]
    fn projection_fits_the_world_without_saturating_and_roundtrips_to_centimetres() {
        for (lon, lat) in [
            (-180.0, -85.051128),
            (180.0, 85.051128),
            (110.0, 35.0),
            (-114.0, 53.5),
        ] {
            let metres = XyPosF64::from(Wgs84LonLat::new(lon, lat));
            let fixed = XyPos::from(metres);
            assert!(fixed.x > i32::MIN && fixed.x < i32::MAX);
            assert!(fixed.y > i32::MIN && fixed.y < i32::MAX);
            let roundtrip = XyPosF64::from(fixed);
            assert!((roundtrip.x - metres.x).abs() <= 0.00501);
            assert!((roundtrip.y - metres.y).abs() <= 0.00501);
        }
    }

    #[test]
    fn integer_envelopes_cover_full_range_and_fractional_time_queries_are_exact() {
        let sample = TEntrySpatialEntry {
            trip: TripKey::new(),
            from: -2_000_000_000,
            until: 2_000_000_000,
            points: [
                (
                    0,
                    XyPos {
                        x: i32::MIN,
                        y: i32::MIN,
                    },
                ),
                (
                    u32::MAX,
                    XyPos {
                        x: i32::MAX,
                        y: i32::MAX,
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };
        let bbox: AABB<[i64; 3]> = sample.envelope();
        assert_eq!(
            bbox.lower(),
            [i64::from(i32::MIN), i64::from(i32::MIN), -2_000_000_000]
        );
        assert_eq!(
            bbox.upper(),
            [i64::from(i32::MAX), i64::from(i32::MAX), 2_000_000_000]
        );
        let (point, angle) = sample.get_pos_angle_at(0).unwrap();
        assert!(point.x.abs() <= 1 && point.y.abs() <= 1);
        assert!(angle.is_finite());
        let (world, _, _) = curved_world();
        let cache = SpatialCache::build(&world);
        assert!(cache.trips([i32::MIN; 2], [i32::MAX; 2], 99.5, 0.0).is_empty());
        assert!(cache.trips([i32::MIN; 2], [i32::MAX; 2], 200.5, 0.0).is_empty());
        assert_eq!(
            cache.trips([i32::MIN; 2], [i32::MAX; 2], 150.5, 0.0).len(),
            2
        );
    }

    #[test]
    fn moving_an_endpoint_preserves_bends_and_undo_restores_every_vertex() {
        let (world, key, _) = curved_world();
        let mut source = Source::new();
        assert!(source.apply_command(Command::LoadWorld {
            snapshot: Box::new(world)
        }));
        let old = source.intervals.get(key).unwrap().nodes.clone();
        let info = source
            .nodes
            .query(key.0, |n| NodeInfo {
                name: n.name.clone(),
                parent: *n.parent,
                pos: Wgs84LonLat::new(109.9, 35.0).into(),
                is_platform: *n.is_platform,
            })
            .unwrap();
        assert!(source.apply_command(Command::ChangeNode { key: key.0, info }));
        let graph_cache = source.graph_cache();
        let entry = graph_cache.intervals([i32::MIN; 2], [i32::MAX; 2]).next().unwrap();
        assert_eq!(entry.points.len(), 5);
        assert_eq!(&entry.points[1..], &old[1..]);
        assert!(source.undo());
        assert_eq!(
            source.graph_cache().intervals([i32::MIN; 2], [i32::MAX; 2]).next().unwrap().points,
            old
        );
    }
}
