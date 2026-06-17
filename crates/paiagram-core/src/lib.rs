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
pub mod plugin;
pub mod problems;
pub mod route;
pub mod script;
pub mod settings;
pub mod station;
pub mod trip;
pub mod units;
pub mod vehicle;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};

use ecow::{EcoString, EcoVec};
use egui::Color32;
use petgraph::graphmap::DiGraphMap;
use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};
pub use trip::class;

macro_rules! make_type {
    (
        $struct_vis:vis struct $struct_name:ident {
            $($field_vis:vis $field_name:ident: $field_type:ty,)*
        }
    ) => {
        paste::paste! {
            #[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
            $struct_vis struct [<$struct_name Key>](std::num::NonZeroU64);

            impl nohash_hasher::IsEnabled for [<$struct_name Key>] {}

            impl [<$struct_name Key>] {
                pub fn new() -> Self {
                    let raw_id = fastrand::u64(1..=u64::MAX);
                    Self(std::num::NonZeroU64::new(raw_id).unwrap())
                }
            }

            #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
            $struct_vis struct [<$struct_name Handle>](pub usize);

            // View stays raw data, as it's just used for passing data in/out
            #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
            $struct_vis struct [<$struct_name View>] {
                $(
                    pub $field_name: $field_type,
                )*
            }

            // The Struct wraps the entire collections in Arc
            #[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
            $struct_vis struct [<$struct_name Collection>] {
                registry: std::sync::Arc<nohash_hasher::IntMap<[<$struct_name Key>], [<$struct_name Handle>]>>,
                keys: std::sync::Arc<Vec<[<$struct_name Key>]>>,
            $(
                $field_vis $field_name: std::sync::Arc<Vec<$field_type>>,
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
    pub struct Trip {
        name: EcoString,
        entries: EcoVec<TEntry>,
        class: Option<ClassKey>,
    }
);

make_type!(
    pub struct Vehicle {
        name: EcoString,
    }
);

make_type!(
    pub struct Station {
        name: EcoString,
        pos: LonLat,
    }
);

make_type!(
    pub struct Class {
        name: EcoString,
        style: StrokeStyle,
    }
);

make_type!(
    pub struct Route {
        name: EcoString,
        stations: EcoVec<StationKey>,
    }
);

make_type!(
    pub struct Interval {
        nodes: EcoVec<LonLat>,
        length: u32,
        stations: (StationKey, StationKey),
    }
);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TEntry {
    fd1: u32,
    fd2: u32,
    stn: Option<StationKey>,
}

const _: [u8; 16] = [0; size_of::<TEntry>()];

/// The style of a stroke
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    color: Color32,
    width: u8,
}

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct LonLat {
    lon: i32,
    lat: i32,
}

// future idea: scripting via rhai
/// The world stores much of the content using SoA.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub struct WorldSnapshot {
    pub trips: TripCollection,
    pub vehicles: VehicleCollection,
    pub stations: StationCollection,
    pub intervals: IntervalCollection,
    pub classes: ClassCollection,
    pub routes: RouteCollection,
    vehicle_trip_matrix: Arc<VehicleTripMatrix>,
    // graph: Arc<DiGraphMap<StationKey, IntervalKey, FxBuildHasher>>,
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

#[cfg(test)]
mod world_snapshot_test {
    type E = Result<(), Box<dyn std::error::Error>>;
    use ecow::string::ToEcoString;

    use super::*;
    #[test]
    fn apply_command_1() -> E {
        let mut final_world = WorldSnapshot::default();
        let commands = (0..=10000)
            .map(|it| it.to_eco_string())
            .map(|name| Command::AddTrip {
                key: TripKey::new(),
                view: TripView {
                    name,
                    entries: EcoVec::new(),
                    class: None,
                },
            });
        let cmd_result = commands
            .into_iter()
            .map(|cmd| final_world.apply_command(cmd))
            .collect::<Vec<_>>();
        for cmd in cmd_result {
            let Some(cmd) = cmd else {
                continue;
            };
            final_world.apply_command(cmd);
        }
        assert_eq!(WorldSnapshot::default(), final_world);
        Ok(())
    }
}

/// The truth of the application. This structure holds a write-only log and a set of undos and
/// redos, as well as the world's current snapshot.
///
/// The source is not clonable, and should not be cloned.
pub struct Source {
    history_log: Vec<Command>,
    undos: Vec<Command>,
    // I don't really like inexing stuff
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
        self.history_log.push(cmd);
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
        self.history_log.push(cmd);
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
        self.history_log.push(cmd);
        self.undos[self.undo_len] = undo_cmd;
        self.undo_len += 1;

        true
    }

    /// Internal helper to construct a snapshot from a slice of history.
    /// Returns Ok(snap) if all commands works.
    /// Returns Err(snap) if at least one command fails, and returns the so-far-so-good progress.
    fn build_snapshot(commands: &[Command]) -> Result<WorldSnapshot, WorldSnapshot> {
        let mut new_snap = WorldSnapshot::default();
        for cmd in commands.iter().cloned() {
            if new_snap.apply_command(cmd).is_none() {
                return Err(new_snap);
            }
        }
        Ok(new_snap)
    }

    /// Crushes the history and rebuilds the world snapshot.
    pub fn crush_history(&mut self) -> bool {
        let Ok(new_snap) = Self::build_snapshot(&self.history_log) else {
            return false;
        };

        self.history_log.clear();
        self.undos.clear();
        self.undo_len = 0;
        self.snap = WorldSnapshot::default();

        self.apply_command(Command::LoadWorld {
            snapshot: Box::new(new_snap),
        })
    }

    /// Rebuild the world snapshot if the user believes the world is contaminated.
    pub fn rebuild_snapshot(&mut self) -> bool {
        let Ok(new_snap) = Self::build_snapshot(&self.history_log) else {
            return false;
        };

        self.snap = new_snap;
        true
    }

    /// Checkout the snapshot at a specific timepoint
    pub fn checkout_snapshot(&mut self, idx: usize) -> bool {
        let Some(commands) = self.history_log.get(..=idx) else {
            return false;
        };

        let Ok(new_snap) = Self::build_snapshot(commands) else {
            return false;
        };

        self.apply_command(Command::LoadWorld {
            snapshot: Box::new(new_snap),
        })
    }
}

/// The save file format.
#[derive(Serialize, Deserialize, Clone)]
pub enum SaveFile {
    V1 { history_log: Vec<Command> },
}

impl TryFrom<SaveFile> for Source {
    type Error = &'static str;
    fn try_from(value: SaveFile) -> Result<Self, Self::Error> {
        match value {
            SaveFile::V1 { history_log } => {
                let Ok(snap) = Source::build_snapshot(history_log.as_slice()) else {
                    return Err("Cannot load world: commands corrupted");
                };
                Ok(Self {
                    history_log,
                    undos: Vec::new(),
                    undo_len: 0,
                    snap,
                    rtrees: GraphCacheWorld::new(),
                    rhai_script_world: RhaiScriptWorld::new(),
                })
            }
        }
    }
}

impl From<Source> for SaveFile {
    fn from(value: Source) -> Self {
        Self::V1 {
            history_log: value.history_log,
        }
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

type IntervalReq = (Arc<Vec<IntervalKey>>, Arc<Vec<EcoVec<LonLat>>>);
type IntervalRes = RTree<IntervalSpatialEntry>;

/// The graph cache world
pub struct GraphCacheWorld {
    entry_rtree: RTree<TEntrySpatialEntry>,
    station_rtree: RTree<StationSpatialEntry>,
    interval_rtree: RTree<IntervalSpatialEntry>,
    entry_req_tx: (),   // unimplemented
    entry_res_rx: (),   // unimplemented
    station_req_tx: (), // unimplemented
    station_res_rx: (), // unimplemented
    interval_req_tx: Sender<IntervalReq>,
    interval_res_rx: Receiver<IntervalRes>,
}

// TODO: find a way to let it work on wasm
// On wasm this should use something like gloo-worker
// TODO: add generation counter to avoid desync
impl GraphCacheWorld {
    fn new() -> Self {
        // TODO
        // boilerplate hehe
        let (entry_req_tx, _entry_req_rx) = ((), ());
        let (_entry_res_tx, entry_res_rx) = ((), ());
        let (station_req_tx, _station_req_rx) = ((), ());
        let (_station_res_tx, station_res_rx) = ((), ());
        let (interval_req_tx, interval_req_rx) = channel::<IntervalReq>();
        let (interval_res_tx, interval_res_rx) = channel::<IntervalRes>();
        // Find a way to abort the task in this case
        // Also find a way to do some sort of damage control
        // TODO: limit the scope of rebuilds
        // This does a total rebuild for now. Computing could get real slow by some point
        std::thread::spawn(move || {
            while let Ok((keys, points)) = interval_req_rx.recv() {
                let mut data = Vec::with_capacity(keys.len());
                for (key, points) in std::iter::zip(keys.iter().cloned(), points.iter().cloned()) {
                    data.push(IntervalSpatialEntry { key, points });
                }
                let built = RTree::bulk_load(data);
                // discard the error in this case
                let _ = interval_res_tx.send(built);
            }
        });
        Self {
            entry_rtree: RTree::default(),
            station_rtree: RTree::default(),
            interval_rtree: RTree::default(),
            entry_req_tx,
            entry_res_rx,
            station_req_tx,
            station_res_rx,
            interval_req_tx,
            interval_res_rx,
        }
    }
    /// Called each frame to check if there are any finished works
    pub fn poll_result(&mut self) {
        if let Ok(res) = self.interval_res_rx.try_recv() {
            self.interval_rtree = res;
        }
    }
    /// Update the intervals
    fn update_intervals(&mut self, intervals: &IntervalCollection) {
        // rebuild the intervals
        let keys = intervals.keys.clone();
        let points = intervals.nodes.clone();
        self.interval_req_tx
            .send((keys, points))
            .expect("Cannot send data to calculate interval rtree!");
    }
}

// the serde here is required for gloo-worker
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
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

#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
#[derive(Clone, Copy)]
pub struct StationSpatialEntry {
    pub key: StationKey,
    pub point: LonLat,
}

/// Should be trivial to clone this
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
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
