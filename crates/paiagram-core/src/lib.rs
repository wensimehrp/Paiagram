// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
pub mod export;
pub mod graph;
pub mod import;
pub mod problems;
// pub mod script;
mod commands;
mod make_type;
pub mod trip;
pub mod units;
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;

pub use commands::Command;
use ecow::{EcoString, EcoVec};
use egui::emath::inverse_lerp;
use egui::{Color32, remap};
use make_type::make_type;
use nohash_hasher::BuildNoHashHasher;
use rstar::{AABB, RTree, RTreeObject};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
pub use units::*;

use crate::trip::TripSchedule;

pub trait Key: Clone + Copy {
    /// Return the key in bits
    fn to_bits(self) -> u64;
    /// Return the creation time of the key
    fn creation_time(self) -> std::time::SystemTime {
        let ms = self.to_bits() >> 16;
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms)
    }
    /// Return the generation
    fn generation(self) -> u16 {
        self.to_bits() as u16
    }
}

pub(crate) struct BorrowMutField<'a, T: Clone> {
    borrow: &'a mut Arc<Vec<T>>,
    idx: usize,
}

impl<'a, T: Clone> BorrowMutField<'a, T> {
    fn get(&self) -> &T {
        &self.borrow[self.idx]
    }
    fn get_mut(&mut self) -> &mut T {
        let m = Arc::make_mut(&mut self.borrow);
        &mut m[self.idx]
    }
}

/// An iterator that yields items borrowing from the iterator itself, so at most one item can be
/// alive at a time. The standard [`Iterator`] trait cannot express this, which is what makes it
/// impossible to hand out `BorrowMut` views through a plain `IntoIterator` implementation.
///
/// Because each item borrows from `&mut self`, this must be consumed with
/// `while let Some(item) = iter.next() { ... }` instead of a `for` loop.
pub(crate) trait LendingIterator {
    type Item<'a>
    where
        Self: 'a;

    fn next(&mut self) -> Option<Self::Item<'_>>;
}

make_type!(
    Trip,
    data {
        name: EcoString,
        schedule: TripSchedule,
        service_class: Option<ServiceClassKey>,
        /// The vehicles that serve this trip. Most trips have a single vehicle.
        vehicles: SmallVec<[VehicleKey; 1]>,
    }
    cache { }
);

make_type!(
    Vehicle,
    data {
        name: EcoString,
    }
    cache {
        /// The trips served by this vehicle.
        trips: EcoVec<TripKey>,
    }
);

make_type!(
    Station,
    data {
        name: EcoString,
        pos: LonLat,
    }
    cache {
        nodes: EcoVec<NodeKey>,
    }
);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NeighbourDirection {
    Incoming,
    Outgoing,
}

// better make some type-level guarantee that this
make_type!(
    /// A node in the network. A node can be either of two types:
    /// A _platform_, which represents a position where the vehicle can stop,
    /// or pass, or a _switch_, which is a railway switch or traffic junction.
    Node,
    data {
        /// The name of the node, e.g. I, II, III and 1, 2, 3 for China Railway
        name: EcoString,
        /// The parent station of the node
        parent: StationKey,
        /// The position of the node
        pos: LonLat,
        /// If the station is a platform
        is_platform: bool,
    }
    cache {
        /// Outgoing neighbours of this node.
        outgoing: SmallVec<[NodeKey; 1]>,
        /// Incoming neighbours of this node.
        incoming: SmallVec<[NodeKey; 1]>,
    }
);

make_type!(
    /// The service class of the vehicle
    ServiceClass,
    data {
        /// The name of the service class
        name: EcoString,
        /// The stroke style displayed on the diagram.
        style: StrokeStyle,
    }
    cache { }
);

/// What to include in this case
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum StationRecord {
    All(StationKey),
    Some(EcoVec<NodeKey>),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RouteStationRecord {
    stn: StationRecord,
    nominal_distance: Option<Distance>,
    canvas_length: Option<CanvasLength>,
    nodes: EcoVec<NodeKey>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervalProgress(u16);

impl IntervalProgress {
    fn is_start(self) -> bool {
        self.0 == 0
    }
    fn is_end(self) -> bool {
        self.0 == u16::MAX
    }
}

make_type!(
    /// A route. A route must be strictly linear, with the only exception being
    /// the first station can also be the last station. A route contains multiple
    /// entries, and each entry contains either all platforms in the station, or
    /// a subset of platforms in the station.
    Route,
    data {
        /// The name of the route.
        name: EcoString,
        /// List of stations in the route.
        stations: EcoVec<RouteStationRecord>,
    }
    cache {
        /// The routes from one station to another forms a tree structure.
        nodes: EcoVec<EcoVec<IntervalProgress>>,
    }
);

/// The key of an interval. An interval is a directed edge, so the ordered pair of
/// its endpoints uniquely identifies it. Parallel edges are not allowed.
pub type IntervalKey = (NodeKey, NodeKey);

/// An interval is a directed edge between two nodes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Interval {
    /// The nodes at and between the two nodes this interval connects.
    /// This includes the starting and ending nodes.
    /// Thus it must have at least two elements and it is safe to call
    /// `.unwrap()` on `.first()` and `.last()`.
    pub nodes: EcoVec<LonLat>,
    /// The length of the interval. If the length is None, then it is calculated from nodes.
    pub length: Option<NonZeroU32>,
    /// trips passing this interval
    #[serde(skip)]
    pub trips: EcoVec<TripKey>,
}

impl Interval {
    /// The length of the interval
    pub fn length(&self) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };
        // TODO: compute the length from `self.nodes`.
        todo!()
    }
}

/// Intervals are edges, keyed by the ordered pair of their endpoints.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct IntervalCollection {
    map: FxHashMap<IntervalKey, Interval>,
}

impl IntervalCollection {
    /// How many intervals currently exist in the world.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check whether an interval exists between `(source, target)`.
    pub fn contains_key(&self, key: IntervalKey) -> bool {
        self.map.contains_key(&key)
    }

    /// Insert an interval, returning the replaced interval if the key already existed.
    pub fn insert(&mut self, key: IntervalKey, interval: Interval) -> Option<Interval> {
        self.map.insert(key, interval)
    }

    /// Remove an interval, returning it if it existed.
    pub fn remove(&mut self, key: IntervalKey) -> Option<Interval> {
        self.map.remove(&key)
    }

    /// Borrow an interval if it exists.
    pub fn get(&self, key: IntervalKey) -> Option<&Interval> {
        self.map.get(&key)
    }

    /// Read access to an interval.
    pub fn query<R>(&self, key: IntervalKey, f: impl FnOnce(&Interval) -> R) -> Option<R> {
        self.map.get(&key).map(f)
    }

    /// Iterate over all intervals and their keys.
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, IntervalKey, Interval> {
        self.map.iter()
    }

    /// Iterate over all interval keys.
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, IntervalKey, Interval> {
        self.map.keys()
    }
}

/// The style of a stroke
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    color: Color32,
    width: u8,
}

// future idea: scripting via rhai
/// The world stores much of the content using SoA.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WorldSnapshot {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub intervals: IntervalCollection,
    pub service_classes: ServiceClassCollection,
    pub routes: RouteCollection,
    pub nodes: NodeCollection,
}

impl WorldSnapshot {
    /// Add `trip` to the cache of every vehicle in `vehicles`.
    fn cache_trip(&mut self, trip: TripKey, vehicles: &[VehicleKey]) {
        for vehicle in vehicles {
            self.vehicles.update(*vehicle, |mut view| {
                view.trips.get_mut().push(trip);
            });
        }
    }

    /// Remove `trip` from the cache of every vehicle in `vehicles`.
    fn uncache_trip(&mut self, trip: TripKey, vehicles: &[VehicleKey]) {
        for vehicle in vehicles {
            self.vehicles.update(*vehicle, |mut view| {
                view.trips.get_mut().retain(|t| *t != trip);
            });
        }
    }

    /// Rebuild every vehicle's trip cache from the trips' authoritative vehicle lists.
    pub fn rebuild_vehicle_trip_cache(&mut self) {
        let vehicles: Vec<VehicleKey> = self.vehicles.keys().collect();
        for vehicle in vehicles {
            self.vehicles.update(vehicle, |mut view| {
                view.trips.get_mut().clear();
            });
        }
        let trips: Vec<TripKey> = self.trips.keys().collect();
        for trip in trips {
            let trip_vehicles =
                self.trips.query(trip, |view| view.vehicles.clone()).unwrap_or_default();
            self.cache_trip(trip, &trip_vehicles);
        }
    }

    /// Rebuild every node's outgoing-edge cache from the authoritative intervals.
    pub fn rebuild_node_edge_cache(&mut self) {
        let nodes: Vec<NodeKey> = self.nodes.keys().collect();
        for node in nodes {
            self.nodes.update(node, |mut view| {
                view.outgoing.get_mut().clear();
                view.incoming.get_mut().clear();
            });
        }

        let intervals: Vec<IntervalKey> = self.intervals.keys().copied().collect();
        for (source, target) in intervals {
            self.nodes.update(source, |mut view| {
                view.outgoing.get_mut().push(target);
            });
            self.nodes.update(target, |mut view| {
                view.incoming.get_mut().push(source);
            });
        }
    }
}

/// The truth of the application. This structure holds a write-only log and a set of undos and
/// redos, as well as the world's current snapshot.
///
/// The source is not clonable, and should not be cloned.
pub struct Source {
    undos: Vec<Command>,
    /// The length or the amount of available undo commands.
    /// A value of 0 means no more undos available.
    undo_len: usize,
    snap: WorldSnapshot,
    rtrees: GraphCacheWorld,
    // rhai_script_world: RhaiScriptWorld,
}

impl Source {
    pub fn new() -> Self {
        Self {
            undos: Vec::new(),
            undo_len: 0,
            snap: WorldSnapshot::default(),
            rtrees: GraphCacheWorld::new(),
        }
    }
}

impl std::ops::Deref for Source {
    type Target = WorldSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.snap
    }
}

impl std::ops::DerefMut for Source {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snap
    }
}

impl Source {
    /// Applies a command on the source. Returns true if the application succeeds and false if
    /// it fails.
    ///
    /// The inverse of the command would be written to the history.
    #[must_use]
    pub fn apply_command(&mut self, cmd: Command) -> bool {
        let Some(inverse) = self.snap.apply_command(cmd.clone()) else {
            return false;
        };
        self.undos.truncate(self.undo_len);
        self.undos.push(inverse);
        self.undo_len = self.undos.len();

        true
    }

    /// Tells if the current history undo_idx is at 0.
    #[must_use]
    pub fn undoable(&mut self) -> bool {
        self.undo_len > 0
    }

    /// Undo a command.
    ///
    /// Returns false in case if an undo fails.
    #[must_use]
    pub fn undo(&mut self) -> bool {
        if !self.undoable() {
            return false;
        }

        let cmd = self.undos[self.undo_len - 1].clone();
        // writes the inverse back to the undo stack if undo works
        let Some(redo_cmd) = self.snap.apply_command(cmd.clone()) else {
            return false;
        };
        self.undos[self.undo_len - 1] = redo_cmd;
        self.undo_len -= 1;

        true
    }

    #[must_use]
    pub fn redoable(&self) -> bool {
        self.undo_len < self.undos.len()
    }

    #[must_use]
    pub fn redo(&mut self) -> bool {
        if !self.redoable() {
            return false;
        }

        let cmd = self.undos[self.undo_len].clone();
        let Some(undo_cmd) = self.snap.apply_command(cmd.clone()) else {
            return false;
        };
        self.undos[self.undo_len] = undo_cmd;
        self.undo_len += 1;

        true
    }
}

/// The save file format.
#[derive(Serialize, Deserialize, Clone)]
pub enum SaveFile {
    V1 { world: WorldSnapshot },
}

impl TryFrom<SaveFile> for Source {
    type Error = &'static str;
    fn try_from(value: SaveFile) -> Result<Self, Self::Error> {
        match value {
            SaveFile::V1 { world } => {
                let mut snap = world;
                snap.rebuild_vehicle_trip_cache();
                snap.rebuild_node_edge_cache();
                Ok(Self {
                    undos: Vec::new(),
                    undo_len: 0,
                    snap,
                    rtrees: GraphCacheWorld::new(),
                    // rhai_script_world: RhaiScriptWorld::new(),
                })
            }
        }
    }
}

impl From<Source> for SaveFile {
    fn from(value: Source) -> Self {
        Self::V1 { world: value.snap }
    }
}

/// The graph cache world
pub struct GraphCacheWorld {
    entry_rtree: RTree<TEntrySpatialEntry>,
    station_rtree: RTree<StationSpatialEntry>,
    interval_rtree: RTree<IntervalSpatialEntry>,
}

// TODO: find a way to let it work on wasm
// On wasm this should use something like gloo-worker
// TODO: add generation counter to avoid desync
impl GraphCacheWorld {
    fn new() -> Self {
        Self {
            entry_rtree: RTree::default(),
            station_rtree: RTree::default(),
            interval_rtree: RTree::default(),
        }
    }
    fn get_entries(
        &self,
        x_range: RangeInclusive<i32>,
        y_range: RangeInclusive<i32>,
        time: i32,
    ) -> impl Iterator<Item = &TEntrySpatialEntry> {
        let time = time as i64;
        let x_min = (*x_range.start()).min(*x_range.end()) as i64;
        let x_max = (*x_range.start()).max(*x_range.end()) as i64;
        let y_min = (*y_range.start()).min(*y_range.end()) as i64;
        let y_max = (*y_range.start()).max(*y_range.end()) as i64;

        let envelope = AABB::from_corners([x_min, y_min, time], [x_max, y_max, time]);
        self.entry_rtree.locate_in_envelope_intersecting(&envelope)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PredefinedTEntryIcon {
    Bus,
    Metro,
    Train,
    Tram,
    Trolleybus,
    Ferry,
}

impl PredefinedTEntryIcon {
    fn get_icon(self) -> egui::ColorImage {
        todo!()
    }
}

#[derive(Clone, Copy)]
pub enum TEntrySpatialEntryEnd {
    Start,
    Intermediate,
    End([u8; 2]),
    StartEnd([u8; 2]),
}

#[derive(Clone)]
pub struct TEntrySpatialEntry {
    /// The reference to the trip
    pub key: TripKey,
    /// baseline
    t1: i32,
    /// departure time
    t2: i32,
    /// arrival time of next station
    t3: i32,
    /// The interval's points premapped to XY position
    /// with progress stored as u32.
    pub points: EcoVec<(u32, XyPos)>,
}

impl TEntrySpatialEntry {
    pub fn get_pos_angle_at(self, time_secs: i32) -> Option<(XyPos, f64)> {
        if self.points.is_empty() {
            return None;
        };
        if self.points.len() == 1 {
            let (_, single_point) = self.points[0];
            return Some((single_point, 0.0));
        }
        let travel_secs_min = self.t1 as f64 + self.t2 as f64;
        let travel_secs_max = self.t3 as f64;
        let travel_range = travel_secs_min..=travel_secs_max;
        let time_secs = (time_secs as f64).clamp(travel_secs_min, travel_secs_max);
        let progress = inverse_lerp(travel_range, time_secs)?;
        let progress_u32 = (progress * u32::MAX as f64) as u32;

        let idx = self.points.binary_search_by_key(&progress_u32, |it| it.0);
        let idx = match idx {
            Ok(i) => i,
            Err(i) => i,
        };

        let idx = if idx >= self.points.len() - 1 {
            self.points.len() - 2
        } else {
            idx
        };

        let [(prev_prog, prev), (curr_prog, curr)] = [self.points[idx], self.points[idx + 1]];
        let pos = if prev_prog == curr_prog {
            prev
        } else {
            let local_progress = (prev_prog as f64)..=(curr_prog as f64);
            let current_x = progress_u32 as f64; // Aligns perfectly with local_progress domain

            let x = remap(
                current_x,
                local_progress.clone(),
                (prev.x as f64)..=(curr.x as f64),
            );
            let y = remap(current_x, local_progress, (prev.y as f64)..=(curr.y as f64));
            XyPos {
                x: x as i32,
                y: y as i32,
            }
        };

        let angle = ((curr.y - prev.y) as f64).atan2((curr.x - prev.x) as f64);
        Some((pos, angle))
    }
}

#[derive(Clone, Copy)]
pub struct StationSpatialEntry {
    pub key: StationKey,
    pub point: LonLat,
}

#[derive(Clone)]
pub struct IntervalSpatialEntry {
    pub key: IntervalKey,
    pub points: EcoVec<LonLat>,
}

impl RTreeObject for TEntrySpatialEntry {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        let x_min = self.points.iter().map(|p| p.1.x).min().unwrap() as i64;
        let x_max = self.points.iter().map(|p| p.1.x).max().unwrap() as i64;
        let y_min = self.points.iter().map(|p| p.1.y).min().unwrap() as i64;
        let y_max = self.points.iter().map(|p| p.1.y).max().unwrap() as i64;
        let tmin = self.t1 as i64;
        let tmax = tmin + self.t3 as i64;
        AABB::from_corners([x_min, y_min, tmin], [x_max, y_max, tmax])
    }
}

impl RTreeObject for StationSpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.point.lon as i64, self.point.lat as i64])
    }
}

impl RTreeObject for IntervalSpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        let lon_min = self.points.iter().map(|p| p.lon).min().unwrap() as i64;
        let lon_max = self.points.iter().map(|p| p.lon).max().unwrap() as i64;
        let lat_min = self.points.iter().map(|p| p.lat).min().unwrap() as i64;
        let lat_max = self.points.iter().map(|p| p.lat).max().unwrap() as i64;
        AABB::from_corners([lon_min, lat_min], [lon_max, lat_max])
    }
}

#[cfg(test)]
mod test;
