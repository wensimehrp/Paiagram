// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
pub mod export;
pub mod graph;
pub mod import;
pub mod problems;
// pub mod script;
pub mod trip;
pub mod units;

use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;

use ecow::{EcoString, EcoVec};
use egui::emath::inverse_lerp;
use egui::{Color32, remap};
use nohash_hasher::BuildNoHashHasher;
use petgraph::graphmap::DiGraphMap;
use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};
pub use units::*;

use crate::trip::{TEntry, TripSchedule};

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
        $(#[$struct_attr:meta])*
        $struct_name:ident,
        data { $(
            $(#[$field_attr:meta])*
            $field_name:ident: $field_type:ty,
        )* }
        cache { $(
            $(#[$cache_attr:meta])*
            $cache_name:ident: $cache_type:ty,
        )* }
    ) => {
        paste::paste! {
            #[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            $(#[$struct_attr])*
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
            struct [<$struct_name Handle>](usize);

            // View stays raw data, as it's just used for passing data in/out
            #[derive(Clone, Debug, PartialEq)]
            pub struct [<$struct_name View>] {
                $(
                    $(#[$field_attr])*
                    pub $field_name: $field_type,
                )*
                $(
                    $(#[$cache_attr])*
                    pub $cache_name: $cache_type,
                )*
            }

            #[derive(Serialize, Deserialize, Clone, Debug)]
            pub struct [<$struct_name Info>] {
                $( pub $field_name: $field_type, )*
            }

            pub struct [<$struct_name Borrow>]<'a> {
                $( pub $field_name: &'a $field_type, )*
                $( pub $cache_name: &'a $cache_type, )*
            }

            pub(crate) struct [<$struct_name BorrowMut>]<'a> {
                $( pub $field_name: BorrowMutField<'a, $field_type>, )*
                $( pub $cache_name: BorrowMutField<'a, $cache_type>, )*
            }

            // The Struct wraps the entire collections in Arc
            #[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
            pub struct [<$struct_name Collection>] {
                registry: std::sync::Arc<[<$struct_name KeyHashMap>]<[<$struct_name Handle>]>>,
                keys: std::sync::Arc<Vec<[<$struct_name Key>]>>,
                $( $field_name: std::sync::Arc<Vec<$field_type>>, )*
                $(
                    #[serde(skip)]
                    $cache_name: std::sync::Arc<Vec<$cache_type>>,
                )*
            }

            impl [<$struct_name Collection>] {
                /// How many elements of this type currently exist in the world
                pub fn len(&self) -> usize {
                    self.registry.len()
                }

                fn get_handle(&self, key: [<$struct_name Key>]) -> Option<[<$struct_name Handle>]> {
                    self.registry.get(&key).cloned()
                }

                /// Check if the current collection contains the key
                pub fn contains_key(&self, key: [<$struct_name Key>]) -> bool {
                    self.registry.contains_key(&key)
                }

                /// Remove an entry from the collection
                pub fn remove(&mut self, key: [<$struct_name Key>]) -> Option<[<$struct_name View>]> {
                    let registry_mut = std::sync::Arc::make_mut(&mut self.registry);
                    let handle = registry_mut.remove(&key)?;
                    let idx = handle.0;

                    let keys_mut = std::sync::Arc::make_mut(&mut self.keys);
                    let last_idx = keys_mut.len() - 1;
                    let last_key = keys_mut[last_idx];

                    let ret = [<$struct_name View>] {
                        $( $field_name: std::sync::Arc::make_mut(&mut self.$field_name).swap_remove(idx), )*
                        $( $cache_name: std::sync::Arc::make_mut(&mut self.$cache_name).swap_remove(idx), )*
                    };

                    keys_mut.swap_remove(idx);

                    if idx != last_idx {
                        registry_mut.insert(last_key, [<$struct_name Handle>](idx));
                    }

                    Some(ret)
                }

                pub fn insert(
                    &mut self,
                    key: [<$struct_name Key>],
                    view: [<$struct_name View>]
                ) -> Option<[<$struct_name View>]> {
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
                    $(
                        std::sync::Arc::make_mut(&mut self.$cache_name).push(view.$cache_name);
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
                        $( $cache_name: &self.$cache_name[idx], )*
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
                        $( $cache_name: BorrowMutField {
                            borrow: &mut self.[<$cache_name>],
                            idx
                        }, )*
                    };

                    Some(f(borrow_mut))
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
        service_class: Option<ServiceClassKey>,
    }
    cache { }
);

make_type!(
    Vehicle,
    data {
        name: EcoString,
    }
    cache { }
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
    cache { }
);

struct NodeCache(NodeKeyHashMap<Vec<i32>>);

impl NodeCache {
    fn remove(&mut self, key: NodeKey) -> Option<Vec<i32>> {
        self.0.remove(&key)
    }
}

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
enum StationRecord {
    All(StationKey),
    Some(EcoVec<NodeKey>),
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
        /// The name of the route
        name: EcoString,
        stations: EcoVec<(StationRecord, Distance)>,
    }
    cache {
        /// The routes from one station to another forms a tree structure.
        nodes: EcoVec<EcoVec<(NodeKey, IntervalProgress)>>,
    }
);

/// The direction of the interval
/// Some interval allows traversing forwards and backwards
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum IntervalDirection {
    /// Bi-directional interval
    /// For the sake of simplicity, if an interval is bi-directional,
    /// the interval with a smaller key would have its node and length information disregarded.
    TwoWay(IntervalKey),
    /// One way interval
    OneWay,
}

make_type!(
    Interval,
    data {
        /// The nodes at and between the two nodes this interval connects.
        /// This includes the starting and ending nodes.
        /// Thus it must have at least two elements and it is safe to call
        /// `.unwrap()` on `.first()` and `.last()`.
        nodes: EcoVec<LonLat>,
        /// The length of the interval. If the length is None, then it is calculated from nodes.
        length: Option<NonZeroU32>,
        /// The direction of the interval. See [`IntervalDirection`] for details.
        direction: IntervalDirection,
    }
    cache { }
);

impl<'a> IntervalBorrow<'a> {
    /// The length of the interval
    pub fn length(&self) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };
        todo!()
    }
}

/// The style of a stroke
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    color: Color32,
    width: u8,
}

/// The world graph describes the network.
pub type WorldGraph = DiGraphMap<NodeKey, IntervalKey, NodeKeyHasher>;

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
    vehicle_trip_matrix: Arc<VehicleTripMatrix>,
    graph: Arc<WorldGraph>,
}

impl WorldSnapshot {
    /// Applies a command and returns its inverse. Could modify the world and return the inverse if
    /// the application succeeds; doesn't modify the world and returns None if the application
    /// fails.
    pub fn apply_command(&mut self, cmd: Command) -> Option<Command> {
        match cmd {
            Command::AddTrip { key, info } => (!self.trips.contains_key(key)).then(|| {
                let TripInfo {
                    name,
                    schedule,
                    service_class,
                } = info;
                self.trips.insert(
                    key,
                    TripView {
                        name,
                        schedule,
                        service_class,
                    },
                );
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
                std::mem::swap(view.service_class.get_mut(), &mut new_class);
                Command::ChangeTripClass {
                    key,
                    class: new_class,
                }
            }),
            Command::RemoveTrip { key } => self.trips.remove(key).map(
                |TripView {
                     name,
                     schedule,
                     service_class,
                 }| Command::AddTrip {
                    key,
                    info: TripInfo {
                        name,
                        schedule,
                        service_class,
                    },
                },
            ),
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
            _ => {
                todo!()
            }
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
            SaveFile::V1 { world } => Ok(Self {
                undos: Vec::new(),
                undo_len: 0,
                snap: world,
                rtrees: GraphCacheWorld::new(),
                // rhai_script_world: RhaiScriptWorld::new(),
            }),
        }
    }
}

impl From<Source> for SaveFile {
    fn from(value: Source) -> Self {
        Self::V1 { world: value.snap }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    /// Add a new trip to the world
    AddTrip {
        key: TripKey,
        info: TripInfo,
    },
    /// Rename a trip
    RenameTrip {
        key: TripKey,
        name: EcoString,
    },
    ChangeTripEntries {
        key: TripKey,
        entries: EcoVec<TEntry>,
    },
    /// Change the trip's class to another class
    ChangeTripClass {
        key: TripKey,
        class: Option<ServiceClassKey>,
    },
    /// Remove a trip from the collection
    RemoveTrip {
        key: TripKey,
    },
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
    fn get_entries(
        &self,
        x_range: RangeInclusive<i32>,
        y_range: RangeInclusive<i32>,
        time: i32,
        delta_time: f64,
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

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
struct VehicleTripMatrix {
    trip_to_veh: TripKeyHashMap<EcoVec<VehicleKey>>,
    veh_to_trip: VehicleKeyHashMap<EcoVec<TripKey>>,
}
