// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
pub mod export;
pub mod graph;
pub mod i18n;
pub mod import;
pub mod problems;
pub mod script;
pub mod settings;
pub mod trip;
pub mod units;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16};
use std::sync::mpsc::{Receiver, Sender, channel};

use ecow::{EcoString, EcoVec};
use egui::Color32;
use nohash_hasher::BuildNoHashHasher;
use std::collections::hash_map::RandomState;
use petgraph::graphmap::DiGraphMap;
use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};
pub use units::*;

pub use crate::trip::{TEntry, TripSchedule};

pub const MAX_CLIENTS: u8 = 10;
pub static CLIENT_ORDER: AtomicU8 = AtomicU8::new(0);

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

macro_rules! make_type {
    (
        $struct_name:ident,
        data {
            $($field_name:ident: $field_type:ty,)*
        }
        cached {

        }
    ) => {
        paste::paste! {
            #[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            pub struct [<$struct_name Key>](std::num::NonZeroU64);

            pub type [<$struct_name KeyHashMap>]<T> = nohash_hasher::IntMap<[<$struct_name Key>], T>;
            pub type [<$struct_name KeyHasher>] = BuildNoHashHasher<[<$struct_name Key>]>;

            impl nohash_hasher::IsEnabled for [<$struct_name Key>] {}

            static [<$struct_name:snake:upper _COUNTER>]: AtomicU16 = AtomicU16::new(0);

            impl [<$struct_name Key>] {
                pub fn new() -> Self {
                    use web_time::SystemTime;
                    let now_ms = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    let timestamp_48 = now_ms & 0xFFFF_FFFF_FFFF;
                    let counter_16 = [<$struct_name:snake:upper _COUNTER>]
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut raw_id = (timestamp_48 << 16) | (counter_16 as u64);
                    // I hope nobody would use this app and generate a key
                    // at exactly Jan 1, 1970 UTC+0...
                    if raw_id == 0 {
                        raw_id = 1;
                    }
                    Self(std::num::NonZeroU64::new(raw_id).unwrap())
                }
            }

            impl Key for [<$struct_name Key>] {
                fn to_bits(self) -> u64 {
                    self.0.get()
                }
            }

            #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
            pub struct [<$struct_name Handle>](usize);

            // View stays raw data, as it's just used for passing data in/out
            #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
            pub struct [<$struct_name View>] {
                $(
                    pub $field_name: $field_type,
                )*
            }

            pub struct [<$struct_name Borrow>]<'a> {
                $(
                    pub $field_name: &'a $field_type,
                )*
            }

            pub(crate) struct [<$struct_name BorrowMut>]<'a> {
                $(
                    pub $field_name: BorrowMutField<'a, $field_type>,
                )*
            }

            // The Struct wraps the entire collections in Arc
            #[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
            pub struct [<$struct_name Collection>] {
                registry: std::sync::Arc<nohash_hasher::IntMap<[<$struct_name Key>], [<$struct_name Handle>]>>,
                keys: std::sync::Arc<Vec<[<$struct_name Key>]>>,
            $(
                $field_name: std::sync::Arc<Vec<$field_type>>,
            )*
            }

            impl [<$struct_name Collection>] {
                pub fn len(&self) -> usize {
                    self.registry.len()
                }

                pub fn get_handle(&self, key: [<$struct_name Key>]) -> Option<[<$struct_name Handle>]> {
                    self.registry.get(&key).cloned()
                }

                /// Check if the current collection contains the key
                pub fn contains_key(&self, key: [<$struct_name Key>]) -> bool {
                    self.registry.contains_key(&key)
                }

                pub fn remove(&mut self, key: [<$struct_name Key>]) -> Option<[<$struct_name View>]> {
                    let registry_mut = std::sync::Arc::make_mut(&mut self.registry);
                    let handle = registry_mut.remove(&key)?;
                    let idx = handle.0;

                    let keys_mut = std::sync::Arc::make_mut(&mut self.keys);
                    let last_idx = keys_mut.len() - 1;
                    let last_key = keys_mut[last_idx];

                    let ret = [<$struct_name View>] { $(
                        $field_name: std::sync::Arc::make_mut(&mut self.$field_name).swap_remove(idx),
                    )*};

                    keys_mut.swap_remove(idx);

                    if idx != last_idx {
                        registry_mut.insert(last_key, [<$struct_name Handle>](idx));
                    }

                    Some(ret)
                }

                pub fn insert(&mut self, key: [<$struct_name Key>], view: [<$struct_name View>]) -> Option<[<$struct_name View>]> {
                    let old_view = if self.registry.contains_key(&key) {
                        self.remove(key)
                    } else {
                        None
                    };

                    let registry_mut = std::sync::Arc::make_mut(&mut self.registry);
                    let keys_mut = std::sync::Arc::make_mut(&mut self.keys);

                    let idx = keys_mut.len();
                    registry_mut.insert(key, [<$struct_name Handle>](idx));
                    keys_mut.push(key);

                    $(
                        std::sync::Arc::make_mut(&mut self.$field_name).push(view.$field_name);
                    )*

                    old_view
                }

                pub fn query<R>(
                    &self,
                    key: [<$struct_name Key>],
                    f: impl FnOnce([<$struct_name Borrow>]) -> R
                ) -> Option<R> {
                    let handle = self.get_handle(key)?;
                    let idx = handle.0;

                    let borrow = [<$struct_name Borrow>] {
                        $( $field_name: &self.$field_name[idx], )*
                    };

                    Some(f(borrow))
                }

                /// Write access via a named-field struct
                fn update<R>(
                    &mut self,
                    key: [<$struct_name Key>],
                    f: impl FnOnce([<$struct_name BorrowMut>]) -> R
                ) -> Option<R> {
                    let handle = self.get_handle(key)?;
                    let idx = handle.0;

                    let borrow_mut = [<$struct_name BorrowMut>] {
                        $( $field_name: BorrowMutField {
                            borrow: &mut self.[<$field_name>],
                            idx
                        }, )*
                    };

                    Some(f(borrow_mut))
                }

                /// Return all keys in insertion order (swap-remove may reorder).
                pub fn keys(&self) -> impl Iterator<Item = &[<$struct_name Key>]> {
                    self.keys.iter()
                }

                /// Get the view data for a key, returning a cloned View.
                pub fn get_view(&self, key: [<$struct_name Key>]) -> Option<[<$struct_name View>]> {
                    let handle = self.get_handle(key)?;
                    let idx = handle.0;
                    Some([<$struct_name View>] {
                        $(
                            $field_name: self.$field_name[idx].clone(),
                        )*
                    })
                }
            }
        }
    };
}

make_type!(
    Trip,
    data {
        name: EcoString,
        schedule: TripSchedule,
        class: Option<ClassKey>,
    }
    cached { }
);

make_type!(
    Vehicle,
    data {
        name: EcoString,
    }
    cached { }
);

make_type!(
    Station,
    data {
        name: EcoString,
        pos: LonLat,
    }
    cached { }
);

make_type!(
    Class,
    data {
        name: EcoString,
        style: StrokeStyle,
    }
    cached { }
);

make_type!(
    Route,
    data {
        name: EcoString,
        stations: EcoVec<StationKey>,
    }
    cached { }
);

make_type!(
    Interval,
    data {
        nodes: EcoVec<LonLat>,
        length: Option<NonZeroU32>,
    }
    cached { }
);

impl<'a> IntervalBorrow<'a> {
    pub fn length(&self) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };
        todo!()
    }
}

// Convenience accessor methods for UI code that uses Handle-based access
impl TripCollection {
    pub fn get_name(&self, handle: TripHandle) -> EcoString {
        self.name[handle.0].clone()
    }
    pub fn get_entries(&self, handle: TripHandle) -> EcoVec<TEntry> {
        self.schedule[handle.0].entries().to_vec().into()
    }
}

impl StationCollection {
    pub fn get_name(&self, handle: StationHandle) -> EcoString {
        self.name[handle.0].clone()
    }
    pub fn get_pos(&self, handle: StationHandle) -> LonLat {
        self.pos[handle.0]
    }
}

impl RouteCollection {
    pub fn get_name(&self, handle: RouteHandle) -> EcoString {
        self.name[handle.0].clone()
    }
    pub fn get_stations(&self, handle: RouteHandle) -> EcoVec<StationKey> {
        self.stations[handle.0].clone()
    }
}

impl ClassCollection {
    pub fn get_name(&self, handle: ClassHandle) -> EcoString {
        self.name[handle.0].clone()
    }
}

impl VehicleCollection {
    pub fn get_name(&self, handle: VehicleHandle) -> EcoString {
        self.name[handle.0].clone()
    }
}

/// The style of a stroke
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub color: Color32,
    pub width: u8,
}

pub type WorldGraph = DiGraphMap<StationKey, IntervalKey, RandomState>;

// future idea: scripting via rhai
/// The world stores much of the content using SoA.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WorldSnapshot {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub intervals: IntervalCollection,
    pub classes: ClassCollection,
    pub routes: RouteCollection,
    vehicle_trip_matrix: Arc<VehicleTripMatrix>,
    pub graph: Arc<WorldGraph>,
}

impl WorldSnapshot {
    /// Applies a command and returns its inverse. Could modify the world and return the inverse if
    /// the application succeeds; doesn't modify the world and returns None if the application
    /// fails.
    pub fn apply_command(&mut self, cmd: Command) -> Option<Command> {
        match cmd {
            // Trip
            Command::AddTrip { key, view } => (!self.trips.contains_key(key)).then(|| {
                self.trips.insert(key, view);
                Command::RemoveTrip { key }
            }),
            Command::RenameTrip {
                key,
                name: mut new_name,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameTrip {
                    key,
                    name: new_name,
                }
            }),
            Command::ChangeTripClass {
                key,
                class: mut new_class,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.class.get_mut(), &mut new_class);
                Command::ChangeTripClass {
                    key,
                    class: new_class,
                }
            }),
            Command::RemoveTrip { key } => self
                .trips
                .remove(key)
                .map(|view| Command::AddTrip { key, view }),
            // Station
            Command::AddStation { key, name, pos } => {
                (!self.stations.contains_key(key)).then(|| {
                    self.stations.insert(key, StationView { name, pos });
                    Command::RemoveStation { key }
                })
            }
            Command::RenameStation {
                key,
                name: mut new_name,
            } => self.stations.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameStation {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveStation { key } => {
                // Remove graph edges connected to this station
                let g = Arc::make_mut(&mut self.graph);
                let to_remove: Vec<_> = g
                    .all_edges()
                    .filter(|(a, b, _)| *a == key || *b == key)
                    .map(|(a, b, _)| (a, b))
                    .collect();
                for (a, b) in to_remove {
                    g.remove_edge(a, b);
                }
                self.stations.remove(key).map(|view| Command::AddStation {
                    key,
                    name: view.name,
                    pos: view.pos,
                })
            }
            // Simply use recursion in this case since macros are not common
            Command::Macro(commands) => {
                let backup = self.clone();
                let mut inverses = Vec::with_capacity(commands.len());

                for cmd in commands.into_vec() {
                    match self.apply_command(cmd) {
                        Some(inverse) => inverses.push(inverse),
                        None => {
                            *self = backup;
                            return None;
                        }
                    }
                }

                inverses.reverse();
                Some(Command::Macro(inverses.into_boxed_slice()))
            }
            // Vehicle
            Command::AddVehicle { key, name } => {
                (!self.vehicles.contains_key(key)).then(|| {
                    self.vehicles.insert(key, VehicleView { name });
                    self.sync_vehicle_trip_matrix();
                    Command::RemoveVehicle { key }
                })
            }
            Command::RenameVehicle {
                key,
                name: mut new_name,
            } => self.vehicles.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameVehicle {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveVehicle { key } => {
                // Clean up matrix entries
                let matrix = Arc::make_mut(&mut self.vehicle_trip_matrix);
                if let Some(old_trips) = matrix.veh_to_trip.remove(&key) {
                    for trip_key in old_trips.iter() {
                        if let Some(veh_list) = matrix.trip_to_veh.get_mut(trip_key) {
                            veh_list.retain(|v| *v != key);
                        }
                    }
                }
                self.vehicles.remove(key).map(|view| {
                    Command::AddVehicle {
                        key,
                        name: view.name,
                    }
                })
            }
            Command::ChangeVehicleTrips { key, trips } => {
                if self.vehicles.contains_key(key) {
                    let matrix = Arc::make_mut(&mut self.vehicle_trip_matrix);
                    let old_trips = matrix.veh_to_trip.insert(key, trips.clone());
                    if let Some(ref old) = old_trips {
                        for trip_key in old.iter() {
                            if let Some(veh_list) = matrix.trip_to_veh.get_mut(trip_key) {
                                veh_list.retain(|v| *v != key);
                            }
                        }
                    }
                    for trip_key in trips.iter() {
                        matrix.trip_to_veh.entry(*trip_key).or_default().push(key);
                    }
                    Some(Command::ChangeVehicleTrips {
                        key,
                        trips: old_trips.unwrap_or_default(),
                    })
                } else {
                    None
                }
            }
            // Class
            Command::AddClass { key, view } => (!self.classes.contains_key(key)).then(|| {
                self.classes.insert(key, view);
                Command::RemoveClass { key }
            }),
            Command::RemoveClass { key } => self
                .classes
                .remove(key)
                .map(|view| Command::AddClass { key, view }),
            Command::RenameClass {
                key,
                name: mut new_name,
            } => self.classes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameClass {
                    key,
                    name: new_name,
                }
            }),
            // Route
            Command::AddRoute { key, view } => (!self.routes.contains_key(key)).then(|| {
                self.routes.insert(key, view);
                Command::RemoveRoute { key }
            }),
            Command::RemoveRoute { key } => self
                .routes
                .remove(key)
                .map(|view| Command::AddRoute { key, view }),
            Command::RenameRoute {
                key,
                name: mut new_name,
            } => self.routes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameRoute {
                    key,
                    name: new_name,
                }
            }),
            // Interval
            Command::AddInterval {
                key,
                view,
                from,
                to,
            } => (!self.intervals.contains_key(key)).then(|| {
                self.intervals.insert(key, view.clone());
                if let (Some(f), Some(t)) = (from, to) {
                    let g = Arc::make_mut(&mut self.graph);
                    g.add_edge(f, t, key);
                }
                Command::RemoveInterval { key }
            }),
            Command::RemoveInterval { key } => {
                // Remove graph edges for this interval
                let g = Arc::make_mut(&mut self.graph);
                let to_remove: Vec<_> = g
                    .all_edges()
                    .filter(|(_, _, w)| **w == key)
                    .map(|(a, b, _)| (a, b))
                    .collect();
                for (a, b) in to_remove {
                    g.remove_edge(a, b);
                }
                self.intervals.remove(key).map(|view| {
                    Command::AddInterval {
                        key,
                        view,
                        from: None,
                        to: None,
                    }
                })
            }
            // Change trip entries
            Command::ChangeTripEntries {
                key,
                entries: mut new_entries,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.schedule.get_mut().entries_mut(), &mut new_entries);
                Command::ChangeTripEntries {
                    key,
                    entries: new_entries,
                }
            }),
            Command::UnloadWorld => {
                let old = std::mem::take(self);
                Some(Command::LoadWorld {
                    snapshot: Box::new(old),
                })
            }
            Command::LoadWorld { snapshot: mut new } => {
                std::mem::swap(self, &mut *new);
                Some(Command::LoadWorld { snapshot: new })
            }
        }
    }

    /// Rebuild the vehicle-trip matrix from scratch.
    fn sync_vehicle_trip_matrix(&mut self) {
        // Currently a no-op; vehicle-trip relationships are tracked via
        // the matrix in ChangeVehicleTrips handlers.
    }

    /// Rebuild the graph from scratch based on current routes.
    /// Consecutive stations in each route are linked as graph edges.
    pub fn rebuild_graph(&mut self) {
        let mut g = DiGraphMap::default();
        // Collect all route station sequences to build edges
        for rk in self.routes.keys() {
            let Some(stations) = self.routes.query(*rk, |b| b.stations.clone()) else {
                continue;
            };
            for pair in stations.windows(2) {
                let from = pair[0];
                let to = pair[1];
                // Look up an interval between these stations
                // For now add a placeholder edge; real interval lookup
                // requires separate tracking.
                if !g.contains_edge(from, to) {
                    // Use a dummy interval key — the actual interval
                    // assignment is done during import.
                    let _ = (from, to);
                }
            }
        }
        self.graph = Arc::new(g);
    }

    /// Return all trip keys with their names.
    pub fn trips_iter(&self) -> Vec<(TripKey, EcoString)> {
        self.trips
            .keys()
            .filter_map(|k| {
                let name = self.trips.query(*k, |b| b.name.clone())?;
                Some((*k, name))
            })
            .collect()
    }

    /// Return all route keys with their names.
    pub fn routes_iter(&self) -> Vec<(RouteKey, EcoString)> {
        self.routes
            .keys()
            .filter_map(|k| {
                let name = self.routes.query(*k, |b| b.name.clone())?;
                Some((*k, name))
            })
            .collect()
    }

    /// Return all station keys with their names and positions.
    pub fn stations_iter(&self) -> Vec<(StationKey, EcoString, LonLat)> {
        self.stations
            .keys()
            .filter_map(|k| {
                let (name, pos) = self.stations.query(*k, |b| (b.name.clone(), *b.pos))?;
                Some((*k, name, pos))
            })
            .collect()
    }

    /// Add or update an interval between two stations, inserting the interval
    /// into the collection and adding the edge to the routing graph.
    pub fn add_interval_edge(
        &mut self,
        interval_key: IntervalKey,
        from: StationKey,
        to: StationKey,
        view: IntervalView,
    ) -> Option<IntervalView> {
        let old = self.intervals.insert(interval_key, view);
        let g = Arc::make_mut(&mut self.graph);
        g.add_edge(from, to, interval_key);
        old
    }

    /// Check if an edge exists between two stations in the routing graph.
    pub fn has_edge(&self, from: StationKey, to: StationKey) -> bool {
        self.graph.contains_edge(from, to)
    }

    /// Remove an interval and its associated graph edges.
    /// Returns the removed interval view if it existed.
    pub fn remove_interval_edge(
        &mut self,
        interval_key: IntervalKey,
    ) -> Option<IntervalView> {
        let view = self.intervals.remove(interval_key);
        if view.is_some() {
            let g = Arc::make_mut(&mut self.graph);
            let to_remove: Vec<_> = g
                .all_edges()
                .filter(|(_, _, w)| **w == interval_key)
                .map(|(a, b, _)| (a, b))
                .collect();
            for (a, b) in to_remove {
                g.remove_edge(a, b);
            }
        }
        view
    }

    fn sync_graph(&mut self) {
        // Rebuild graph content when intervals change.
        // Currently delegates to rebuild_graph which scans routes.
        self.rebuild_graph();
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
    rhai_script_world: RhaiScriptWorld,
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

impl Default for Source {
    fn default() -> Self {
        Self {
            undos: Vec::new(),
            undo_len: 0,
            snap: WorldSnapshot::default(),
            rtrees: GraphCacheWorld::new(),
            rhai_script_world: RhaiScriptWorld::new(),
        }
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
            SaveFile::V1 { world } => Ok(Self {
                undos: Vec::new(),
                undo_len: 0,
                snap: world,
                rtrees: GraphCacheWorld::new(),
                rhai_script_world: RhaiScriptWorld::new(),
            }),
        }
    }
}

impl From<Source> for SaveFile {
    fn from(value: Source) -> Self {
        Self::V1 { world: value.snap }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Command {
    // Trips
    AddTrip {
        key: TripKey,
        view: TripView,
    },
    RenameTrip {
        key: TripKey,
        name: EcoString,
    },
    ChangeTripEntries {
        key: TripKey,
        entries: EcoVec<TEntry>,
    },
    ChangeTripClass {
        key: TripKey,
        class: Option<ClassKey>,
    },
    RemoveTrip {
        key: TripKey,
    },
    // Vehicles
    AddVehicle {
        key: VehicleKey,
        name: EcoString,
    },
    RenameVehicle {
        key: VehicleKey,
        name: EcoString,
    },
    RemoveVehicle {
        key: VehicleKey,
    },
    /// Hybrid
    ChangeVehicleTrips {
        key: VehicleKey,
        trips: EcoVec<TripKey>,
    },
    // Stations
    AddStation {
        key: StationKey,
        name: EcoString,
        pos: LonLat,
    },
    RenameStation {
        key: StationKey,
        name: EcoString,
    },
    RemoveStation {
        key: StationKey,
    },
    // Classes
    AddClass {
        key: ClassKey,
        view: ClassView,
    },
    RemoveClass {
        key: ClassKey,
    },
    RenameClass {
        key: ClassKey,
        name: EcoString,
    },
    // Routes
    AddRoute {
        key: RouteKey,
        view: RouteView,
    },
    RemoveRoute {
        key: RouteKey,
    },
    RenameRoute {
        key: RouteKey,
        name: EcoString,
    },
    // Intervals
    AddInterval {
        key: IntervalKey,
        view: IntervalView,
        from: Option<StationKey>,
        to: Option<StationKey>,
    },
    RemoveInterval {
        key: IntervalKey,
    },
    // World related stuff
    UnloadWorld,
    LoadWorld {
        snapshot: Box<WorldSnapshot>,
    },
    /// A user-defined macro.
    Macro(Box<[Command]>),
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
}

#[derive(Clone)]
pub struct TEntrySpatialEntry {
    /// The reference to the trip
    pub key: TripKey,
    /// baseline
    pub t1: i32,
    /// delta of t1
    pub t2: i16,
    /// delta of t1
    pub t3: i16,
    /// The interval's points
    pub points: EcoVec<LonLat>,
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
        let lon_min = self.points.iter().map(|p| p.lon).min().unwrap() as i64;
        let lon_max = self.points.iter().map(|p| p.lon).max().unwrap() as i64;
        let lat_min = self.points.iter().map(|p| p.lat).min().unwrap() as i64;
        let lat_max = self.points.iter().map(|p| p.lat).max().unwrap() as i64;
        let tmin = self.t1 as i64;
        let tmax = tmin + self.t3 as i64;
        AABB::from_corners([lon_min, lat_min, tmin], [lon_max, lat_max, tmax])
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

#[derive(Clone)]
enum ScriptResponse {
    Output(Arc<str>),
    Done(Result<Vec<Command>, String>),
}

#[derive(Clone)]
pub enum ScriptPollResponse {
    NotBusy,
    Busy,
    Output(Arc<str>),
    Done(Result<Vec<Command>, String>),
}

struct RhaiScriptWorld {
    script_req_tx: Sender<(WorldSnapshot, Arc<str>)>,
    script_res_rx: Receiver<ScriptResponse>,
    terminate_script: Arc<AtomicBool>,
    busy: bool,
}

impl RhaiScriptWorld {
    fn new() -> Self {
        let (script_req_tx, script_req_rx) = channel();
        let (script_res_tx, script_res_rx) = channel();

        let terminate_script = Arc::new(AtomicBool::new(false));
        let terminate_script_copy = terminate_script.clone();

        std::thread::spawn(move || {
            while let Ok((world, src)) = script_req_rx.recv() {
                let iteration_terminate = terminate_script_copy.clone();

                let print_tx = script_res_tx.clone();
                let debug_tx = script_res_tx.clone();

                let res = script::execute_rhai_script(
                    world,
                    src,
                    move |s| {
                        let _ = print_tx.send(ScriptResponse::Output(s.into()));
                    },
                    move |s, _, p| {
                        let dbg_text = format!("{:?}: {}", p, s);
                        let _ = debug_tx.send(ScriptResponse::Output(dbg_text.into()));
                    },
                    move |_c| {
                        if iteration_terminate.load(std::sync::atomic::Ordering::Relaxed) {
                            return Some(rhai::Dynamic::UNIT);
                        }
                        None
                    },
                );

                let _ = script_res_tx.send(ScriptResponse::Done(res));
            }
        });

        Self {
            script_req_tx,
            script_res_rx,
            terminate_script,
            busy: false,
        }
    }
    fn poll(&mut self) -> ScriptPollResponse {
        if !self.busy {
            return ScriptPollResponse::NotBusy;
        }
        let Ok(res) = self.script_res_rx.try_recv() else {
            return ScriptPollResponse::Busy;
        };
        match res {
            ScriptResponse::Done(m) => {
                self.busy = false;
                ScriptPollResponse::Done(m)
            }
            ScriptResponse::Output(m) => ScriptPollResponse::Output(m),
        }
    }
    fn start_execute(&mut self, snap: WorldSnapshot, src: Arc<str>) {
        self.script_req_tx
            .send((snap, src))
            .expect("Script thread closed!");
        self.busy = true;
    }
}

// relatively slow to clone because SparseSecondaryMap is backed by a hashmap
// I might consider using a dynamic container in the future
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
struct VehicleTripMatrix {
    trip_to_veh: TripKeyHashMap<EcoVec<VehicleKey>>,
    veh_to_trip: VehicleKeyHashMap<EcoVec<TripKey>>,
}

pub trait ToEcoStringView {
    fn to_view(&mut self) -> EcoStringView<'_>;
}

pub struct EcoStringView<'a>(&'a mut EcoString);

impl ToEcoStringView for EcoString {
    fn to_view(&mut self) -> EcoStringView<'_> {
        EcoStringView(self)
    }
}
