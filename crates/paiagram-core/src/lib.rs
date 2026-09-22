// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
mod graph;
pub mod import;
mod interval;
mod make_type;
pub mod problems;
pub mod route;
pub mod trip;
pub mod units;
mod world;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU16;

use ecow::{EcoString, EcoVec};
use egui::Color32;
use make_type::make_type;
use nohash_hasher::BuildNoHashHasher;
use paiagram_rw::ExportObject;
use route::RouteIntervals;
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

type Wfc<D, C> = WorldFieldContainer<D, C>;

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
        /// The name of the node, e.g. I, II, III and 1, 2, 3 for China Railway
        name: EcoString,
        /// The parent station of the node
        parent: StationKey,
        /// The position of the node
        pos: LonLat,
        /// If the station is a platform
        is_platform: bool,
    }
    cache { }
}

impl NodeKey {
    const MIN: Self = Self(std::num::NonZeroU64::new(1).unwrap());
    const MAX: Self = Self(std::num::NonZeroU64::new(u64::MAX).unwrap());
    pub fn outgoing_intervals<'a>(
        self,
        intervals: &'a IntervalCollection,
    ) -> impl Iterator<Item = (&'a IntervalKey, &'a Wfc<Interval, IntervalCache>)> + 'a {
        let min = (self, NodeKey::MIN);
        let max = (self, NodeKey::MAX);
        intervals.range(min..=max)
    }
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
    key = (NodeKey, NodeKey),
    data {
        /// The nodes at and between the two nodes this interval connects.
        /// This includes the starting and ending nodes.
        /// Thus it must have at least two elements and it is safe to call
        /// `.unwrap()` on `.first()` and `.last()`.
        nodes: EcoVec<LonLat>,
        /// The length of the interval. If the length is None, then it is calculated from nodes.
        length: Option<NonZeroU32>,
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
    /// Updates the world and makes a backup
    pub fn update(
        &mut self,
        f: impl FnOnce(WorldSnapshot) -> Result<WorldSnapshot, String>,
    ) -> Result<(), String> {
        let new_snap = f(self.snap.clone())?;

        if self.redo_len > 0 {
            let active_len = self.history.len() - self.redo_len;
            self.history.truncate(active_len);
            self.redo_len = 0;
        }

        let old = std::mem::replace(&mut self.snap, new_snap);
        self.history.push(old);
        self.revision += 1;
        Ok(())
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
    fn extension(&self) -> impl AsRef<str> {
        ".paia"
    }
    fn write_content<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
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
