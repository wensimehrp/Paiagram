// SPDX-License-Identifier: MPL-2.0
//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
pub mod entry;
pub mod export;
pub mod graph;
pub mod i18n;
pub mod import;
pub mod interval;
pub mod problems;
pub mod route;
pub mod script;
pub mod settings;
pub mod station;
pub mod trip;
pub mod units;
pub mod vehicle;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::mpsc::{Receiver, Sender, channel};

use ecow::{EcoString, EcoVec};
use egui::Color32;
use nohash_hasher::BuildNoHashHasher;
use petgraph::graphmap::DiGraphMap;
use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};
pub use trip::class;
pub use units::*;

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
            pub struct [<$struct_name Handle>](pub usize);

            // View stays raw data, as it's just used for passing data in/out
            #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
            pub struct [<$struct_name View>] {
                $(
                    pub $field_name: $field_type,
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

                $(
                    pub fn [<get_ $field_name>](
                        &self, handle: [<$struct_name Handle>]
                    ) -> &$field_type {
                        &self.$field_name[handle.0]
                    }

                    pub fn [<get_ $field_name _mut>](
                        &mut self, handle: [<$struct_name Handle>]
                    ) -> &mut $field_type {
                        let vec_mut = std::sync::Arc::make_mut(&mut self.$field_name);
                        &mut vec_mut[handle.0]
                    }
                )*
            }
        }
    };
}

make_type!(
    Trip,
    data {
        name: EcoString,
        entries: EcoVec<TEntry>,
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TEntry {
    fd1: u32,
    fd2: u32,
    stn: Option<StationKey>,
}

// Assert the size
const _: [u8; 16] = [0; size_of::<TEntry>()];

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
    pub classes: ClassCollection,
    pub routes: RouteCollection,
    vehicle_trip_matrix: Arc<VehicleTripMatrix>,
    graph: Arc<DiGraphMap<StationKey, IntervalKey, BuildNoHashHasher<StationKey>>>,
}

impl WorldSnapshot {
    /// Applies a command and returns its inverse. Could modify the world and return the inverse if
    /// the application succeeds; doesn't modify the world and returns None if the application
    /// fails.
    pub fn apply_command(&mut self, cmd: Command) -> Option<Command> {
        match cmd {
            Command::AddTrip { key, view } => (!self.trips.contains_key(key)).then(|| {
                self.trips.insert(key, view);
                Command::RemoveTrip { key }
            }),
            Command::RenameTrip {
                key,
                name: mut new_name,
            } => self.trips.get_handle(key).map(|handle| {
                let old_name = self.trips.get_name_mut(handle);
                std::mem::swap(old_name, &mut new_name);
                Command::RenameTrip {
                    key,
                    name: new_name,
                }
            }),
            Command::ChangeTripClass {
                key,
                class: mut new_class,
            } => self.trips.get_handle(key).map(|handle| {
                let old_class = self.trips.get_class_mut(handle);
                std::mem::swap(old_class, &mut new_class);
                Command::ChangeTripClass {
                    key,
                    class: new_class,
                }
            }),
            Command::RemoveTrip { key } => self
                .trips
                .remove(key)
                .map(|view| Command::AddTrip { key, view }),
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
    rhai_script_world: RhaiScriptWorld,
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
    trip_to_veh: nohash_hasher::IntMap<TripKey, EcoVec<VehicleKey>>,
    veh_to_trip: nohash_hasher::IntMap<VehicleKey, EcoVec<TripKey>>,
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
