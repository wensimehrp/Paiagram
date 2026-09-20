// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
mod commands;
pub mod diagram;
pub mod graph;
pub mod import;
mod make_type;
pub mod problems;
pub mod route;
pub mod spatial;
pub mod trip;
pub mod units;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;

use arc_swap::ArcSwap;
pub use commands::Command;
use ecow::{EcoString, EcoVec};
use egui::Color32;
use imbl::OrdMap;
use make_type::make_type;
use nohash_hasher::BuildNoHashHasher;
use paiagram_rw::ExportObject;
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
    pub stn: StationRecord,
    pub milestone: Option<Distance>,
    pub canvas_length: Option<CanvasLength>,
    pub prev_curr_nodes: EcoVec<NodeKey>,
    pub curr_prev_nodes: EcoVec<NodeKey>,
}

/// The progress of a node within a single interval of a route.
///
/// `0` marks the start of the interval and [`u16::MAX`] marks its end.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntervalProgress(u16);

impl IntervalProgress {
    /// Create a progress from a ratio in `0.0..=1.0`. Values outside the range
    /// are clamped.
    pub fn from_ratio(ratio: f64) -> Self {
        let clamped = ratio.clamp(0.0, 1.0);
        Self((clamped * u16::MAX as f64).round() as u16)
    }

    /// The progress as a ratio in `0.0..=1.0`.
    pub fn to_ratio(self) -> f64 {
        self.0 as f64 / u16::MAX as f64
    }

    /// The raw progress value in `0..=u16::MAX`.
    pub fn get(self) -> u16 {
        self.0
    }

    /// Whether this marks the start of an interval.
    pub fn is_start(self) -> bool {
        self.0 == 0
    }

    /// Whether this marks the end of an interval.
    pub fn is_end(self) -> bool {
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
    cache {}
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
    /// The length of the interval in metres.
    ///
    /// Uses the explicitly-stored length when present; otherwise sums the
    /// great-circle (Haversine) distances between consecutive points in
    /// [`Interval::nodes`].
    pub fn length(&self) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };
        let total: f64 = self
            .nodes
            .windows(2)
            .map(|w| Wgs84LonLat::from(w[0]).distance_to_meters(Wgs84LonLat::from(w[1])))
            .sum();
        Distance(total.round() as i32)
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
    pub color: Color32,
    pub width: u8,
}

// future idea: scripting via rhai
/// The world stores much of the content using SoA.
#[derive(Serialize, Default, Clone, Debug)]
pub struct WorldSnapshot {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub intervals: IntervalCollection,
    pub service_classes: ServiceClassCollection,
    pub routes: RouteCollection,
    pub nodes: NodeCollection,
}

impl<'de> Deserialize<'de> for WorldSnapshot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Data {
            trips: TripCollection,
            vehicles: VehicleCollection,
            stations: StationCollection,
            intervals: IntervalCollection,
            service_classes: ServiceClassCollection,
            routes: RouteCollection,
            nodes: NodeCollection,
        }
        let d = Data::deserialize(deserializer)?;
        let mut world = Self {
            trips: d.trips,
            vehicles: d.vehicles,
            stations: d.stations,
            intervals: d.intervals,
            service_classes: d.service_classes,
            routes: d.routes,
            nodes: d.nodes,
        };
        world.rebuild_caches();
        Ok(world)
    }
}

impl WorldSnapshot {
    /// Restore derived relationships from authoritative fields. Called once per command batch.
    pub fn rebuild_caches(&mut self) {
        self.rebuild_vehicle_trip_cache();
        self.rebuild_node_edge_cache();
        for nodes in Arc::make_mut(&mut self.stations.nodes) {
            nodes.clear();
        }
        for node in self.nodes.iter() {
            self.stations.update(*node.parent, |mut station| {
                station.nodes.get_mut().push(node.key)
            });
        }
        for interval in self.intervals.map.values_mut() {
            interval.trips.clear();
        }
        for trip in self.trips.iter() {
            let entries: Vec<_> =
                trip.schedule.entries().iter().filter(|e| !e.is_external()).collect();
            for pair in entries.windows(2) {
                if let Some(interval) =
                    self.intervals.map.get_mut(&(pair[0].node_key(), pair[1].node_key()))
                {
                    if !interval.trips.contains(&trip.key) {
                        interval.trips.push(trip.key);
                    }
                }
            }
        }
    }

    /// Add `trip` to the cache of every vehicle in `vehicles`.
    fn cache_trip(&mut self, trip: TripKey, vehicles: &[VehicleKey]) {
        for vehicle in vehicles {
            self.vehicles.update(*vehicle, |mut view| {
                if !view.trips.get().contains(&trip) {
                    view.trips.get_mut().push(trip);
                }
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
    revision: u64,
    // TODO: add generation
    rtrees: Arc<ArcSwap<spatial::SpatialCache>>,
}

impl Source {
    /// Changes after every successful command, undo, and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn graph_cache(&self) -> Arc<spatial::SpatialCache> {
        self.rtrees.load().clone()
    }

    pub fn snap(&self) -> &WorldSnapshot {
        &self.snap
    }
}

impl Source {
    pub fn new() -> Self {
        Self {
            undos: Vec::new(),
            undo_len: 0,
            revision: 0,
            snap: WorldSnapshot::default(),
            rtrees: Arc::new(ArcSwap::from_pointee(spatial::SpatialCache::default())),
        }
    }
}

impl std::ops::Deref for Source {
    type Target = WorldSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.snap
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
        self.revision = self.revision.wrapping_add(1);
        self.refresh_rtrees();
        self.undos.truncate(self.undo_len);
        self.undos.push(inverse);
        self.undo_len = self.undos.len();

        true
    }

    fn refresh_rtrees(&self) {
        let rtree = self.rtrees.clone();
        let snap = self.snap.clone();
        rayon::spawn(move || {
            let new_cache = spatial::SpatialCache::build(&snap);
            rtree.swap(Arc::new(new_cache));
        });
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
        self.revision = self.revision.wrapping_add(1);
        self.refresh_rtrees();
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
        self.revision = self.revision.wrapping_add(1);
        self.refresh_rtrees();
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

impl ExportObject for SaveFile {
    fn extension(&self) -> impl AsRef<str> {
        ".paia"
    }
    fn write_content<W: std::io::prelude::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        cbor4ii::serde::to_writer(writer, &self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl TryFrom<SaveFile> for Source {
    type Error = &'static str;
    fn try_from(value: SaveFile) -> Result<Self, Self::Error> {
        match value {
            SaveFile::V1 { world } => {
                let mut snap = world;
                snap.rebuild_caches();
                let rtrees = spatial::SpatialCache::build(&snap);
                Ok(Self {
                    undos: Vec::new(),
                    undo_len: 0,
                    revision: 0,
                    snap,
                    rtrees: Arc::new(ArcSwap::from_pointee(rtrees)),
                    // rhai_script_world: RhaiScriptWorld::new(),
                })
            }
        }
    }
}

impl From<WorldSnapshot> for SaveFile {
    fn from(world: WorldSnapshot) -> Self {
        Self::V1 { world }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Train {
    name: EcoString,
    class: Option<u32>,
    vehicles: Vec<u32>,
    schedule: Vec<(u32, u32, u32)>,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorldFieldContainer<D, C> {
    #[serde(flatten)]
    data: D,
    #[serde(skip)]
    cache: C,
}

#[derive(Clone, Serialize, Deserialize)]
struct World {
    trains: OrdMap<u32, WorldFieldContainer<Train, ()>>,
}

struct SSSSource {
    world: World,
    history: Vec<World>,
}
