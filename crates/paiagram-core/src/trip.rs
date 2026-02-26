use crate::{
    entry,
    graph::Node,
    settings::ProjectSettings,
    station::Station,
    trip::class::{Class, DisplayedStroke},
    units::time::Duration,
    vehicle::Vehicle,
};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future::poll_once};
use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};
use rstar::{AABB, RTree, RTreeObject};
use smallvec::SmallVec;
use std::ops::RangeInclusive;

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
pub struct TripSpatialIndexItem {
    pub trip: Entity,
    pub entry0: Entity,
    pub entry1: Entity,
    pub t0: f64,
    pub t1: f64,
    pub t2: f64,
    pub p0: [f64; 2],
    pub p1: [f64; 2],
}

impl RTreeObject for TripSpatialIndexItem {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [
                self.p0[0].min(self.p1[0]),
                self.p0[1].min(self.p1[1]),
                self.t0,
            ],
            [
                self.p0[0].max(self.p1[0]),
                self.p0[1].max(self.p1[1]),
                self.t2,
            ],
        )
    }
}

#[derive(Resource, Default)]
pub struct TripSpatialIndex {
    tree: RTree<TripSpatialIndexItem>,
}

impl TripSpatialIndex {
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn query_xy_time(
        &self,
        x_range: RangeInclusive<f64>,
        y_range: RangeInclusive<f64>,
        time_range: RangeInclusive<f64>,
    ) -> impl Iterator<Item = TripSpatialIndexItem> + '_ {
        let x0 = (*x_range.start()).min(*x_range.end());
        let x1 = (*x_range.start()).max(*x_range.end());
        let y0 = (*y_range.start()).min(*y_range.end());
        let y1 = (*y_range.start()).max(*y_range.end());
        let t0 = (*time_range.start()).min(*time_range.end());
        let t1 = (*time_range.start()).max(*time_range.end());

        let envelope = AABB::from_corners([x0, y0, t0], [x1, y1, t1]);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .copied()
    }

    fn replace_tree(&mut self, tree: RTree<TripSpatialIndexItem>) {
        self.tree = tree;
    }
}

#[derive(Resource)]
struct TripSpatialIndexState {
    dirty: bool,
    task: Option<Task<RTree<TripSpatialIndexItem>>>,
}

impl Default for TripSpatialIndexState {
    fn default() -> Self {
        Self {
            dirty: true,
            task: None,
        }
    }
}

// TODO: replace the dirty method with specific updates
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
    settings: Res<ProjectSettings>,
) {
    if !state.dirty || state.task.is_some() {
        return;
    }
    state.dirty = false;

    let mut snapshot = Vec::<TripSpatialIndexItem>::new();

    let get_station_xy = |entry_entity: Entity| -> Option<[f64; 2]> {
        let platform_entity = stop_q.get(entry_entity).ok()?.entity();
        let node = match platform_q.get(platform_entity).ok()? {
            (Some(_), _) => node_q.get(platform_entity).ok()?,
            (None, Some(parent)) => node_q.get(parent.parent()).ok()?,
            _ => return None,
        };
        Some(node.pos.to_xy_arr())
    };

    let repeat_time = settings.repeat_frequency.0 as f64;

    for (trip_entity, schedule) in &trips {
        if schedule.len() < 1 {
            continue;
        }

        for pair in schedule.windows(2).chain(std::iter::once(
            [schedule.last().unwrap().clone(); 2].as_slice(),
        )) {
            let [entry0, entry1] = pair else {
                continue;
            };
            let entry0 = *entry0;
            let entry1 = *entry1;

            let Some(p0) = get_station_xy(entry0) else {
                continue;
            };
            let Some(p1) = get_station_xy(entry1) else {
                continue;
            };

            let Ok(estimate0) = estimate_q.get(entry0) else {
                continue;
            };
            let Ok(estimate1) = estimate_q.get(entry1) else {
                continue;
            };

            // include the previous arr time
            let t0 = estimate0.arr.0 as f64;
            let t1 = estimate0.dep.0 as f64;
            let t2 = estimate1.arr.0 as f64;
            if t1 < t0 || t2 < t1 {
                continue;
            }

            if repeat_time > 0.0 {
                let dep_duration = t1 - t0;
                let arr_duration = t2 - t0;
                if arr_duration >= repeat_time {
                    snapshot.push(TripSpatialIndexItem {
                        trip: trip_entity,
                        entry0,
                        entry1,
                        t0: 0.0,
                        t1: dep_duration.rem_euclid(repeat_time),
                        t2: repeat_time,
                        p0,
                        p1,
                    });
                    continue;
                }

                let normalized_t0 = t0.rem_euclid(repeat_time);
                let normalized_t1 = normalized_t0 + dep_duration;
                let normalized_t2 = normalized_t0 + arr_duration;
                snapshot.push(TripSpatialIndexItem {
                    trip: trip_entity,
                    entry0,
                    entry1,
                    t0: normalized_t0,
                    t1: normalized_t1,
                    t2: normalized_t2,
                    p0,
                    p1,
                });

                if normalized_t2 > repeat_time {
                    snapshot.push(TripSpatialIndexItem {
                        trip: trip_entity,
                        entry0,
                        entry1,
                        t0: normalized_t0 - repeat_time,
                        t1: normalized_t1 - repeat_time,
                        t2: normalized_t2 - repeat_time,
                        p0,
                        p1,
                    });
                }
            } else {
                snapshot.push(TripSpatialIndexItem {
                    trip: trip_entity,
                    entry0,
                    entry1,
                    t0,
                    t1,
                    t2,
                    p0,
                    p1,
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
    pub entity: Entity,
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
