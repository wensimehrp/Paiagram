use crate::{
    entry,
    graph::{Node, NodePos},
    station::Station,
    trip::class::{Class, DisplayedStroke},
    units::time::{Duration, TimetableTime},
    vehicle::Vehicle,
};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future::poll_once};
use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};
use rstar::{AABB, RTree, RTreeObject};
use smallvec::{SmallVec, smallvec};
use std::collections::HashMap;

pub mod class;
pub mod routing;

pub struct TripPlugin;
impl Plugin for TripPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(routing::RoutingPlugin)
            .init_resource::<TripSpatialIndex>()
            .init_resource::<TripSpatialIndexState>()
            .add_systems(
                Update,
                (
                    mark_trip_spatial_index_dirty,
                    start_trip_spatial_index_rebuild,
                    apply_trip_spatial_index_task,
                )
                    .chain(),
            )
            .add_observer(update_add_trip_vehicles)
            .add_observer(update_remove_trip_vehicles)
            .add_observer(update_remove_vehicle_trips);
    }
}

#[derive(Clone, Copy, Debug)]
struct TripSegmentIndexItem {
    trip: Entity,
    t0: f64,
    t1: f64,
    p0: [f64; 2],
    p1: [f64; 2],
}

impl TripSegmentIndexItem {
    fn sample_at(self, time: f64) -> Option<[f64; 2]> {
        if time < self.t0 || time > self.t1 {
            return None;
        }
        let duration = (self.t1 - self.t0).max(1e-9);
        let alpha = ((time - self.t0) / duration).clamp(0.0, 1.0);
        let x = self.p0[0] + (self.p1[0] - self.p0[0]) * alpha;
        let y = self.p0[1] + (self.p1[1] - self.p0[1]) * alpha;
        Some([x, y])
    }
}

impl RTreeObject for TripSegmentIndexItem {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [
                self.p0[0].min(self.p1[0]),
                self.p0[1].min(self.p1[1]),
                self.t0.min(self.t1),
            ],
            [
                self.p0[0].max(self.p1[0]),
                self.p0[1].max(self.p1[1]),
                self.t0.max(self.t1),
            ],
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TripSpatialSample {
    pub trip: Entity,
    pub x: f64,
    pub y: f64,
}

#[derive(Resource, Default)]
pub struct TripSpatialIndex {
    tree: RTree<TripSegmentIndexItem>,
}

impl TripSpatialIndex {
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn query_xy_time(
        &self,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        time: f64,
    ) -> Vec<TripSpatialSample> {
        if self.is_empty() {
            return Vec::new();
        }
        let query_env = AABB::from_corners(
            [min_x.min(max_x), min_y.min(max_y), time],
            [min_x.max(max_x), min_y.max(max_y), time],
        );

        let mut by_trip: HashMap<Entity, TripSpatialSample> = HashMap::new();
        for item in self.tree.locate_in_envelope_intersecting(&query_env) {
            let Some([x, y]) = item.sample_at(time) else {
                continue;
            };
            if x < min_x.min(max_x)
                || x > min_x.max(max_x)
                || y < min_y.min(max_y)
                || y > min_y.max(max_y)
            {
                continue;
            }
            by_trip.entry(item.trip).or_insert(TripSpatialSample {
                trip: item.trip,
                x,
                y,
            });
        }

        by_trip.into_values().collect()
    }

    fn replace_tree(&mut self, tree: RTree<TripSegmentIndexItem>) {
        self.tree = tree;
    }
}

#[derive(Resource)]
struct TripSpatialIndexState {
    dirty: bool,
    task: Option<Task<RTree<TripSegmentIndexItem>>>,
}

impl Default for TripSpatialIndexState {
    fn default() -> Self {
        Self {
            dirty: true,
            task: None,
        }
    }
}

fn mark_trip_spatial_index_dirty(
    mut state: ResMut<TripSpatialIndexState>,
    changed_trips: Query<(), Or<(Added<Trip>, Changed<Children>)>>,
    changed_stops: Query<(), Or<(Added<entry::EntryStop>, Changed<entry::EntryStop>)>>,
    changed_estimates: Query<
        (),
        Or<(
            Added<entry::EntryEstimate>,
            Changed<entry::EntryEstimate>,
            Added<Node>,
            Changed<Node>,
        )>,
    >,
    mut removed_trips: RemovedComponents<Trip>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_stop: RemovedComponents<entry::EntryStop>,
    mut removed_estimate: RemovedComponents<entry::EntryEstimate>,
    mut removed_node: RemovedComponents<Node>,
) {
    if !changed_trips.is_empty()
        || !changed_stops.is_empty()
        || !changed_estimates.is_empty()
        || removed_trips.read().next().is_some()
        || removed_children.read().next().is_some()
        || removed_stop.read().next().is_some()
        || removed_estimate.read().next().is_some()
        || removed_node.read().next().is_some()
    {
        state.dirty = true;
    }
}

fn start_trip_spatial_index_rebuild(
    mut state: ResMut<TripSpatialIndexState>,
    trips: Query<(Entity, &TripSchedule), With<Trip>>,
    stop_q: Query<&entry::EntryStop>,
    estimate_q: Query<&entry::EntryEstimate>,
    platform_q: Query<AnyOf<(&Station, &ChildOf)>>,
    node_q: Query<&Node>,
) {
    if !state.dirty || state.task.is_some() {
        return;
    }
    state.dirty = false;

    let mut snapshot = Vec::<TripSegmentIndexItem>::new();

    let get_station_xy = |entry_entity: Entity| -> Option<[f64; 2]> {
        let platform_entity = stop_q.get(entry_entity).ok()?.entity();
        let node = match platform_q.get(platform_entity).ok()? {
            (Some(_), _) => node_q.get(platform_entity).ok()?,
            (None, Some(parent)) => node_q.get(parent.parent()).ok()?,
            _ => return None,
        };
        Some([node.pos.x(), node.pos.y()])
    };

    for (trip_entity, schedule) in &trips {
        if schedule.len() < 2 {
            continue;
        }

        for idx in 1..schedule.len() {
            let prev_entry = schedule[idx - 1];
            let curr_entry = schedule[idx];

            let Ok(prev_estimate) = estimate_q.get(prev_entry) else {
                continue;
            };
            let Ok(curr_estimate) = estimate_q.get(curr_entry) else {
                continue;
            };

            let Some(prev_xy) = get_station_xy(prev_entry) else {
                continue;
            };
            let Some(curr_xy) = get_station_xy(curr_entry) else {
                continue;
            };

            let prev_arr = prev_estimate.arr.0 as f64;
            let prev_dep = prev_estimate.dep.0 as f64;
            let curr_arr = curr_estimate.arr.0 as f64;

            if prev_dep > prev_arr {
                snapshot.push(TripSegmentIndexItem {
                    trip: trip_entity,
                    t0: prev_arr,
                    t1: prev_dep,
                    p0: prev_xy,
                    p1: prev_xy,
                });
            }

            if curr_arr > prev_dep {
                snapshot.push(TripSegmentIndexItem {
                    trip: trip_entity,
                    t0: prev_dep,
                    t1: curr_arr,
                    p0: prev_xy,
                    p1: curr_xy,
                });
            }
        }
    }

    state.task = Some(AsyncComputeTaskPool::get().spawn(async move { RTree::bulk_load(snapshot) }));
}

fn apply_trip_spatial_index_task(
    mut state: ResMut<TripSpatialIndexState>,
    mut index: ResMut<TripSpatialIndex>,
) {
    let Some(task) = state.task.as_mut() else {
        return;
    };
    let Some(tree) = block_on(poll_once(task)) else {
        return;
    };
    index.replace_tree(tree);
    state.task = None;
}

/// Marker component for a trip
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(TripVehicles, TripSchedule, Name)]
pub struct Trip;

/// Trip bundle.
/// Spawn with [`EntityCommands::with_children`]
#[derive(Bundle)]
pub struct TripBundle {
    trip: Trip,
    vehicles: TripVehicles,
    name: Name,
    class: TripClass,
}

impl TripBundle {
    pub fn new(name: &str, class: TripClass) -> Self {
        Self {
            trip: Trip,
            vehicles: TripVehicles::default(),
            name: Name::from(name),
            class,
        }
    }
}

/// Marker component for timing reference trips.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct IsTimingReference;

/// A trip in the world
#[derive(Default, Reflect, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
pub struct TripVehicles(#[entities] pub SmallVec<[Entity; 1]>);

/// The class of the trip
#[derive(Reflect, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
#[relationship(relationship_target = class::Class)]
#[require(Name)]
pub struct TripClass(#[entities] pub Entity);

/// A type alias for [`Children`]
pub type TripSchedule = Children;

/// Common query data for trips
#[derive(QueryData)]
pub struct TripQuery {
    trip: &'static Trip,
    pub vehicles: &'static TripVehicles,
    pub name: &'static Name,
    pub class: &'static TripClass,
    pub schedule: &'static TripSchedule,
}

impl<'w, 's> TripQueryItem<'w, 's> {
    /// The duration of the trip, from the first entry's arrival time to the last entry's
    /// departure time. This method only checks the first and last entries' times, hence
    /// any intermediate entries are not considered.
    pub fn duration<'a>(&self, q: &Query<'a, 'a, &entry::EntryEstimate>) -> Option<Duration> {
        let beg = self.schedule.first().cloned()?;
        let end = self.schedule.last().cloned()?;
        let end_t = q.get(end).ok()?;
        let beg_t = q.get(beg).ok()?;
        Some(end_t.dep - beg_t.arr)
    }
    pub fn stroke<'a>(&self, q: &Query<'a, 'a, &DisplayedStroke, With<Class>>) -> DisplayedStroke {
        q.get(self.class.entity()).unwrap().clone()
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This removes vehicles from trip data.
fn update_remove_trip_vehicles(
    removed_vehicle: On<Remove, Vehicle>,
    mut trips: Populated<&mut TripVehicles>,
    vehicles: Query<&Vehicle>,
) {
    let veh = removed_vehicle.entity;
    let Ok(Vehicle {
        trips: remove_pending,
    }) = vehicles.get(veh)
    else {
        return;
    };
    for &pending in remove_pending {
        let Ok(mut trip_vehicles) = trips.get_mut(pending) else {
            return;
        };
        trip_vehicles.retain(|v| *v != veh);
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This adds vehicles into trip data.
fn update_add_trip_vehicles(
    removed_vehicle: On<Add, Vehicle>,
    mut trips: Populated<&mut TripVehicles>,
    vehicles: Query<&Vehicle>,
) {
    let veh = removed_vehicle.entity;
    let Ok(Vehicle { trips: add_pending }) = vehicles.get(veh) else {
        return;
    };
    for pending in add_pending.iter().copied() {
        let Ok(mut trip_vehicles) = trips.get_mut(pending) else {
            return;
        };
        trip_vehicles.push(veh);
    }
}

/// Helper function that manually synchronizes [`TripVehicles`] and [`Vehicle`].
/// This removes trips from vehicle data.
fn update_remove_vehicle_trips(
    removed_trip: On<Remove, TripVehicles>,
    mut vehicles: Populated<&mut Vehicle>,
    trips: Query<&TripVehicles>,
) {
    let trip = removed_trip.entity;
    let Ok(remove_pending) = trips.get(trip) else {
        return;
    };
    for &pending in &remove_pending.0 {
        let Ok(mut trip_vehicles) = vehicles.get_mut(pending) else {
            return;
        };
        trip_vehicles.trips.retain(|v| *v != trip);
    }
}
