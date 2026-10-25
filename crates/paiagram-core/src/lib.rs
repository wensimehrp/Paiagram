// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
mod graph;
pub mod import;
mod interval;
mod key_gen;
mod make_type;
pub mod problems;
pub mod route;
pub mod spatial_index;
pub mod trip;
pub mod units;
mod world;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};

use ecow::{EcoString, EcoVec};
use egui::Color32;
pub use graph::IntervalDirection;
pub use key_gen::{MAX_CLIENT_ID, client_id, set_client_id};
use make_type::make_type;
use nohash_hasher::BuildNoHashHasher;
use paiagram_rw::ExportObject;
use route::RouteIntervals;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
pub use units::*;

use crate::import::ImportMode;
use crate::trip::TripSchedule;

pub trait Key: Clone + Copy {
    /// Return the key in bits
    fn to_bits(self) -> u64;
    /// Return the creation time of the key
    fn creation_time(self) -> std::time::SystemTime {
        crate::key_gen::creation_time(self.to_bits())
    }
    /// Return the id of the client that allocated the key
    fn client(self) -> u8 {
        crate::key_gen::client(self.to_bits())
    }
    /// Return the generation
    fn generation(self) -> u16 {
        crate::key_gen::generation(self.to_bits())
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct WorldFieldContainer<D, C> {
    #[serde(flatten)]
    pub data: D,
    #[serde(skip)]
    pub cache: C,
}

impl<D, C> Deref for WorldFieldContainer<D, C> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D, C> DerefMut for WorldFieldContainer<D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<D, C> WorldFieldContainer<D, C> {
    fn new(data: D) -> Self
    where
        C: Default,
    {
        Self {
            data,
            cache: C::default(),
        }
    }
    fn new_with_cache(data: D, cache: C) -> Self {
        Self { data, cache }
    }
}

pub type Wfc<D, C> = WorldFieldContainer<D, C>;

make_type! {
    Trip,
    data {
        name: EcoString,
        schedule: TripSchedule,
        service_class: Option<ServiceClassKey>,
        /// The vehicles that serve this trip. Most trips have a single vehicle.
        vehicles: SmallVec<[VehicleKey; 1]>,
    }
    cache { }
}

make_type! {
    Vehicle,
    data {
        name: EcoString,
    }
    cache {
        /// The trips served by this vehicle.
        trips: EcoVec<TripKey>,
    }
}

make_type! {
    Station,
    data {
        name: EcoString,
        pos: LonLat,
    }
    cache {
        nodes: EcoVec<NodeKey>,
    }
}

// better make some type-level guarantee that this
make_type! {
    /// A node in the network. A node can be either of two types:
    /// A _platform_, which represents a position where the vehicle can stop,
    /// or pass, or a _switch_, which is a railway switch or traffic junction.
    Node,
    data {
        /// The position of the node
        name: EcoString,
        parent: StationKey,
        pos: LonLat,
    }
    cache { }
}

make_type! {
    /// The service class of the vehicle
    ServiceClass,
    data {
        /// The name of the service class
        name: EcoString,
        /// The stroke style displayed on the diagram.
        style: StrokeStyle,
    }
    cache { }
}

make_type! {
    /// The route.
    Route,
    data {
        /// The name of the route.
        name: EcoString,
        intervals: RouteIntervals,
    }
    cache { }
}

make_type! {
    /// An interval is a directed edge, so the ordered pair of
    /// its endpoints, [`IntervalKey`], uniquely identifies it. Parallel edges are not allowed.
    Interval,
    key = graph::SortedKey,
    data {
        /// The nodes at and between the two nodes this interval connects.
        /// This includes the starting and ending nodes.
        /// Thus it must have at least two elements and it is safe to call
        /// `.unwrap()` on `.first()` and `.last()`.
        nodes: EcoVec<LonLat>,
        /// The length of the interval. If the length is None, then it is calculated from nodes.
        length: Option<NonZeroU32>,
        direction: graph::IntervalDirection,
    }
    cache {
        /// trips passing this interval
        trips: EcoVec<TripKey>,
    }
}

/// The style of a stroke
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub color: Color32,
    pub width: u8,
}

impl StrokeStyle {
    pub fn to_egui_stroke(self) -> egui::Stroke {
        egui::Stroke::new(self.width as f32, self.color)
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: Color32::GRAY,
            width: 1,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WorldSnapshot {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub graph: graph::Graph,
    pub service_classes: ServiceClassCollection,
    pub routes: RouteCollection,
    /// Spatial r-trees derived from the rest of the world. Not persisted; rebuilt
    /// whenever [`spatial_dirty`](Self::spatial_dirty) is set.
    #[serde(skip)]
    pub spatial: spatial_index::SpatialCache,
    /// Whether [`spatial`](Self::spatial) is out of date and must be rebuilt.
    #[serde(skip, default = "default_true")]
    pub spatial_dirty: bool,
}

impl WorldSnapshot {
    /// Insert every entity from `other`, keeping the entities already present in `self`.
    fn merged(mut self, other: &Self) -> Self {
        for (key, value) in &other.vehicles {
            self.vehicles.insert(*key, value.clone());
        }
        for (key, value) in &other.stations {
            self.stations.insert(*key, value.clone());
        }
        for (key, value) in &other.graph.nodes {
            self.graph.nodes.insert(*key, value.clone());
        }
        for (key, value) in &other.graph.intervals {
            self.graph.insert_interval_entry(*key, value.clone());
        }
        for (key, value) in &other.trips {
            self.trips.insert(*key, value.clone());
        }
        for (key, value) in &other.service_classes {
            self.service_classes.insert(*key, value.clone());
        }
        for (key, value) in &other.routes {
            self.routes.insert(*key, value.clone());
        }
        self.spatial_dirty = true;
        self
    }
}

/// The truth of the application. This structure holds a write-only log and a set of undos and
/// redos, as well as the world's current snapshot.
///
/// The source is not clonable, and should not be cloned anyways.
pub struct Source {
    /// The current active world snapshot
    pub snap: WorldSnapshot,
    history: Vec<WorldSnapshot>,
    /// the amount of redos available at this point
    redo_len: usize,
    revision: u64,
}

impl Source {
    /// Changes after every successful command, undo, and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Mutates the world and makes a backup
    pub fn mutate(
        &mut self,
        f: impl FnOnce(WorldSnapshot) -> Result<WorldSnapshot, String>,
    ) -> Result<(), String> {
        let new_world = f(self.snap.clone())?;

        if self.redo_len > 0 {
            let active_len = self.history.len() - self.redo_len;
            self.history.truncate(active_len);
            self.redo_len = 0;
        }

        let old = self.snap.clone();
        self.history.push(old);
        self.snap.update_snapshot(|_old_world| new_world);
        self.revision += 1;
        Ok(())
    }

    /// Applies an imported world, either on top of the current one or replacing it.
    pub fn import(&mut self, world: WorldSnapshot, mode: ImportMode) -> Result<(), String> {
        self.mutate(move |existing| {
            Ok(match mode {
                ImportMode::Merge => existing.merged(&world),
                ImportMode::Replace => world,
            })
        })
    }

    pub fn undoable(&self) -> bool {
        self.history.len() > self.redo_len
    }
    pub fn redoable(&self) -> bool {
        self.redo_len > 0
    }

    pub fn undo(&mut self) -> Result<(), String> {
        if !self.undoable() {
            return Err("No undos available".into());
        }
        let target_idx = self.history.len() - 1 - self.redo_len;
        std::mem::swap(&mut self.snap, &mut self.history[target_idx]);
        self.redo_len += 1;
        self.revision += 1;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        if !self.redoable() {
            return Err("No redos available".into());
        }
        self.redo_len -= 1;
        let target_idx = self.history.len() - 1 - self.redo_len;
        std::mem::swap(&mut self.snap, &mut self.history[target_idx]);
        self.revision += 1;
        Ok(())
    }
}

impl Source {
    pub fn new() -> Self {
        Self {
            snap: WorldSnapshot::default(),
            history: Vec::new(),
            redo_len: 0,
            revision: 0,
        }
    }
}

impl std::ops::Deref for Source {
    type Target = WorldSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.snap
    }
}

/// The save file format.
#[derive(Serialize, Deserialize, Clone)]
pub enum SaveFile {
    V1(WorldSnapshot),
}

impl ExportObject for SaveFile {
    fn filename(&self) -> String {
        "exported.paia".into()
    }
    fn write_content<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let writer = zrip::FrameEncoder::new(writer, 4).unwrap();
        cbor4ii::serde::to_writer(writer, &self).map_err(std::io::Error::other)
    }
}

impl TryFrom<SaveFile> for Source {
    type Error = &'static str;
    fn try_from(value: SaveFile) -> Result<Self, Self::Error> {
        match value {
            SaveFile::V1(snap) => Ok(Self {
                history: Vec::new(),
                redo_len: 0,
                revision: 0,
                snap,
            }),
        }
    }
}

impl From<WorldSnapshot> for SaveFile {
    fn from(value: WorldSnapshot) -> Self {
        Self::V1(value)
    }
}
