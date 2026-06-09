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
pub mod settings;
pub mod station;
pub mod trip;
pub mod units;
pub mod vehicle;

use std::collections::VecDeque;

use ecow::{EcoString, EcoVec};
use egui::Color32;
use petgraph::graphmap::DiGraphMap;
use rstar::{AABB, RTree, RTreeObject};
use rustc_hash::FxBuildHasher;
use serde::{Deserialize, Serialize};
use slotmap::*;
pub use trip::class;

new_key_type! {
    /// Trip's key
    pub struct TripKey;

    /// Vehicle's key
    pub struct VehicleKey;

    /// Station's key
    pub struct StationKey;

    /// Interval's key
    pub struct IntervalKey;

    /// Class's key
    pub struct ClassKey;

    /// Route's key
    pub struct RouteKey;
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct TEntry {
    fd1: u32,
    fd2: u32,
    stn: Option<StationKey>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct StrokeStyle {
    color: Color32,
    width: u8,
}

// future idea: scripting via rhai
/// The world stores much of the content using SoA
/// This is not ECS (however I do thing we would need archetypal ECS by some point)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DataWorld {
    // trips
    pub trips: DenseSlotMap<TripKey, ()>,
    trip_names: SecondaryMap<TripKey, EcoString>,
    trip_entries: SecondaryMap<TripKey, EcoVec<TEntry>>,
    trip_classes: SecondaryMap<TripKey, ClassKey>,
    // vehicles
    pub vehicles: DenseSlotMap<VehicleKey, ()>,
    vehicle_names: SecondaryMap<VehicleKey, EcoString>,
    // stations
    pub stations: DenseSlotMap<StationKey, ()>,
    station_names: SecondaryMap<StationKey, EcoString>,
    // intervals
    pub intervals: DenseSlotMap<IntervalKey, ()>,
    interval_nodes: SecondaryMap<IntervalKey, EcoVec<(i32, i32)>>,
    interval_stations: SecondaryMap<IntervalKey, (StationKey, StationKey)>,
    // Classes
    pub classes: DenseSlotMap<ClassKey, ()>,
    class_names: SecondaryMap<ClassKey, EcoString>,
    class_styles: SecondaryMap<ClassKey, StrokeStyle>,
    // Routes
    pub routes: DenseSlotMap<RouteKey, ()>,
    pub route_names: SecondaryMap<RouteKey, EcoString>,
    pub route_stations: SecondaryMap<RouteKey, EcoVec<StationKey>>,
    // hybrid
    vehicle_trip_matrix: VehicleTripMatrix,
    graph: DiGraphMap<StationKey, IntervalKey, FxBuildHasher>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct History {
    stack: VecDeque<Command>,
    ptr: usize,
}

impl History {
    pub fn undo(&mut self, world: &mut DataWorld) {
        todo!()
    }
    pub fn redo(&mut self, world: &mut DataWorld) {
        todo!()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Command {
    // ...
    /// A user-defined macro.
    Macro(Box<[Command]>),
}

impl Command {
    pub fn execute(&self, world: &mut DataWorld) {
        todo!()
    }
    pub fn undo(&self, world: &mut DataWorld) {
        todo!()
    }
}

#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct GraphCacheWorld {
    pub entry_rtree: RTree<TEntrySpatialEntry>,
    pub station_rtree: RTree<StationSpatialEntry>,
    pub interval_rtree: RTree<StationSpatialEntry>,
}

// the serde here is required for gloo-worker
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
#[derive(Clone, Copy)]
pub struct TEntrySpatialEntry {
    pub k: TripKey,
    pub t1: i32,
    pub t2: i16, // delta value
    pub t3: i16, // delta value
    pub p1: (i32, i32),
    pub p2: (i32, i32),
}

#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
#[derive(Clone, Copy)]
pub struct StationSpatialEntry {
    pub k: StationKey,
    pub p: (i32, i32),
}

/// Should be trivial to clone this
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
#[derive(Clone)]
pub struct IntervalSpatialEntry {
    pub k: IntervalKey,
    pub points: EcoVec<(i32, i32)>,
}

impl RTreeObject for TEntrySpatialEntry {
    type Envelope = AABB<[i64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.p1.0 as i64, self.p1.1 as i64, self.t1 as i64],
            [
                self.p1.0 as i64,
                self.p1.1 as i64,
                self.t1 as i64 + self.t2 as i64 + self.t3 as i64,
            ],
        )
    }
}

impl RTreeObject for StationSpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.p.0 as i64, self.p.1 as i64])
    }
}

impl RTreeObject for IntervalSpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        let lon_minimum = self.points.iter().map(|(lon, _)| *lon).min().unwrap();
        let lon_maximum = self.points.iter().map(|(lon, _)| *lon).max().unwrap();
        let lat_minimum = self.points.iter().map(|(_, lat)| *lat).min().unwrap();
        let lat_maximum = self.points.iter().map(|(_, lat)| *lat).max().unwrap();
        AABB::from_corners(
            [lon_minimum as i64, lat_minimum as i64],
            [lon_maximum as i64, lat_maximum as i64],
        )
    }
}

// relatively slow to clone because SparseSecondaryMap is backed by a string
#[derive(Serialize, Deserialize, Default, Clone)]
struct VehicleTripMatrix {
    trip_to_veh: SparseSecondaryMap<TripKey, EcoVec<VehicleKey>, FxBuildHasher>,
    veh_to_trip: SparseSecondaryMap<VehicleKey, EcoVec<TripKey>, FxBuildHasher>,
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
