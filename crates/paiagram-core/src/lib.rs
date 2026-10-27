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
use std::sync::mpsc;

use ecow::{EcoString, EcoVec};
use egui::Color32;
pub use graph::IntervalDirection;
use itertools::chain;
pub use key_gen::{MAX_CLIENT_ID, client_id, set_client_id};
use make_type::make_type;
use nohash_hasher::BuildNoHashHasher;
use paiagram_rw::ExportObject;
use route::RouteIntervals;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use spatial_index::SpatialCache;
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
    pub fn new(data: D) -> Self
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
        /// The nodes between the two nodes this interval connects.
        /// Stored in high -> low order
        intermediate_nodes: EcoVec<LonLat>,
        /// The length of the interval. If the length is None, then it is calculated from
        /// the intermediate nodes and the endpoint node positions.
        length: Option<NonZeroU32>,
        direction: graph::IntervalDirection,
    }
    cache {
        /// trips passing this interval
        trips: EcoVec<TripKey>,
    }
}

impl Interval {
    /// The full polyline of the interval, from its high-key endpoint through the
    /// intermediate positions to its low-key endpoint.
    pub fn positions(
        &self,
        IntervalKey { hi, lo }: IntervalKey,
        nodes: &NodeCollection,
    ) -> impl Iterator<Item = LonLat> {
        chain!(
            nodes.get(&hi).map(|wfc| wfc.pos),
            self.intermediate_nodes.iter().cloned(),
            nodes.get(&lo).map(|wfc| wfc.pos)
        )
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

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct World {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub graph: graph::Graph,
    pub service_classes: ServiceClassCollection,
    pub routes: RouteCollection,
}

impl World {
    /// Insert every entity from `other`, keeping the entities already present in `self`.
    fn merged(mut self, other: &Self) -> Self {
        puffin::profile_function!();
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
            self.graph.insert_interval(*key, value.clone());
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
        self
    }
}

/// The truth of the application. This structure holds a write-only log and a set of undos and
/// redos, as well as the current world.
///
/// The source is not clonable, and should not be cloned anyways.
pub struct Source {
    /// The current world
    pub world: World,
    prev_frame_world: World,
    history: Vec<World>,
    /// the amount of redos available at this point
    redo_len: usize,
    revision: u64,
    spatial: SpatialCache,
    spatial_tx: mpsc::Sender<(u64, SpatialCache)>,
    spatial_rx: mpsc::Receiver<(u64, SpatialCache)>,
}

impl Source {
    /// Changes after every successful command, undo, and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The app's spatial cache
    pub fn spatial(&self) -> &SpatialCache {
        &self.spatial
    }

    /// The world from the previous frame.
    pub fn prev_frame_world(&self) -> &World {
        &self.prev_frame_world
    }

    /// Available history
    pub fn history(&self) -> &[World] {
        &self.history
    }

    /// Saves the current world as the previous frame's. Call once at the start of each frame.
    pub fn begin_frame(&mut self) {
        puffin::profile_function!();
        self.prev_frame_world = self.world.clone();
        if let Ok((build_idx, new_spatial)) = self.spatial_rx.try_recv() {
            if build_idx == self.revision() {
                self.spatial = new_spatial;
            } else {
                self.spawn_spatial_build();
            }
        }
    }

    /// Build the spatial cache on the rayon pool.
    fn spawn_spatial_build(&self) {
        let revision = self.revision;
        let world = self.world.clone();
        let sender = self.spatial_tx.clone();
        rayon::spawn(move || {
            puffin::profile_scope!("spatial_cache_build");
            let cache = SpatialCache::build(&world);
            let _ = sender.send((revision, cache));
        });
    }

    /// Mutates the world and makes a backup
    pub fn mutate(&mut self, f: impl FnOnce(World) -> Result<World, String>) -> Result<(), String> {
        puffin::profile_function!();
        let new_world = f(self.world.clone())?;

        if self.redo_len > 0 {
            let active_len = self.history.len() - self.redo_len;
            self.history.truncate(active_len);
            self.redo_len = 0;
        }

        let old = self.world.clone();
        self.history.push(old);
        let spatial_dirty = self.world.update(|_old_world| new_world);
        self.revision += 1;
        if spatial_dirty {
            self.spawn_spatial_build();
        }
        Ok(())
    }

    /// Applies an imported world, either on top of the current one or replacing it.
    pub fn import(&mut self, world: World, mode: ImportMode) -> Result<(), String> {
        puffin::profile_function!();
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
        puffin::profile_function!();
        if !self.undoable() {
            return Err("No undos available".into());
        }
        let target_idx = self.history.len() - 1 - self.redo_len;
        std::mem::swap(&mut self.world, &mut self.history[target_idx]);
        self.redo_len += 1;
        self.revision += 1;
        self.spawn_spatial_build();
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        puffin::profile_function!();
        if !self.redoable() {
            return Err("No redos available".into());
        }
        self.redo_len -= 1;
        let target_idx = self.history.len() - 1 - self.redo_len;
        std::mem::swap(&mut self.world, &mut self.history[target_idx]);
        self.revision += 1;
        self.spawn_spatial_build();
        Ok(())
    }
}

impl Source {
    pub fn new() -> Self {
        let (spatial_tx, spatial_rx) = mpsc::channel();
        Self {
            world: World::default(),
            prev_frame_world: World::default(),
            history: Vec::new(),
            redo_len: 0,
            revision: 0,
            spatial: SpatialCache::default(),
            spatial_tx,
            spatial_rx,
        }
    }
}

impl std::ops::Deref for Source {
    type Target = World;
    fn deref(&self) -> &Self::Target {
        &self.world
    }
}

/// The save file format.
#[derive(Serialize, Deserialize, Clone)]
pub enum SaveFile {
    V1(World),
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
            SaveFile::V1(world) => {
                let (spatial_tx, spatial_rx) = mpsc::channel();
                let source = Self {
                    prev_frame_world: world.clone(),
                    history: Vec::new(),
                    redo_len: 0,
                    revision: 0,
                    world,
                    spatial: SpatialCache::default(),
                    spatial_tx,
                    spatial_rx,
                };
                source.spawn_spatial_build();
                Ok(source)
            }
        }
    }
}

impl From<World> for SaveFile {
    fn from(value: World) -> Self {
        Self::V1(value)
    }
}
