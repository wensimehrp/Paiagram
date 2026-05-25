use std::ops::RangeInclusive;

use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};
use rstar::{AABB, RTree, RTreeObject};
use smallvec::SmallVec;

use crate::entry::{self, EntryMode};
use crate::graph::Node;
use crate::settings::ProjectSettings;
use crate::station::Station;
use crate::trip::class::{Class, DisplayedStroke};
use crate::units::time::Duration;
use crate::vehicle::Vehicle;

pub mod class;
pub mod routing;

pub struct TripPlugin;
impl Plugin for TripPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(routing::RoutingPlugin)
            .init_resource::<TripSpatialIndex>()
            .add_systems(Update, update_trip_spatial_index)
            .add_observer(update_nominal_schedule)
            .add_observer(convert_derived_entry_to_explicit)
            .add_observer(update_add_trip_vehicles)
            .add_observer(update_remove_trip_vehicles)
            .add_observer(update_remove_vehicle_trips);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    entities: EntityHashMap<Vec<TripSpatialIndexItem>>,
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

    pub fn clear(&mut self) {
        self.tree = RTree::new();
        self.entities.clear();
    }
}

fn update_trip_spatial_index(
    mut index: ResMut<TripSpatialIndex>,
    trips: Query<(Entity, &TripSchedule), With<Trip>>,
    changed_trips: Query<Entity, Or<(Added<Trip>, Changed<TripSchedule>)>>,
    changed_stops: Query<Entity, Or<(Added<entry::EntryStop>, Changed<entry::EntryStop>)>>,
    changed_estimates: Query<
        Entity,
        Or<(Added<entry::EntryEstimate>, Changed<entry::EntryEstimate>)>,
    >,
    changed_nodes: Query<Entity, Or<(Added<Node>, Changed<Node>)>>,
    mut removed_trips: RemovedComponents<Trip>,
    mut removed_stop: RemovedComponents<entry::EntryStop>,
    mut removed_estimate: RemovedComponents<entry::EntryEstimate>,
    mut removed_node: RemovedComponents<Node>,
    platform_q: Query<AnyOf<(&Station, &ChildOf)>>,
    stop_q: Query<&entry::EntryStop>,
    estimate_q: Query<&entry::EntryEstimate>,
    node_q: Query<&Node>,
    settings: Res<ProjectSettings>,
) {
    let mut to_remove_trips = EntityHashSet::default();

    for entity in removed_trips.read() {
        to_remove_trips.insert(entity);
    }

    let mut changed_trip_set = EntityHashSet::default();

    for entity in &changed_trips {
        changed_trip_set.insert(entity);
    }

    let has_changed_entries = !changed_stops.is_empty()
        || !changed_estimates.is_empty()
        || !changed_nodes.is_empty()
        || removed_stop.read().next().is_some()
        || removed_estimate.read().next().is_some()
        || removed_node.read().next().is_some();

    if has_changed_entries {
        let changed_stops_set: EntityHashSet = changed_stops.iter().collect();
        let changed_est_set: EntityHashSet = changed_estimates.iter().collect();
        let changed_nodes_set: EntityHashSet = changed_nodes.iter().collect();

        let check_entry = |entry: Entity| -> bool {
            if changed_stops_set.contains(&entry) || changed_est_set.contains(&entry) {
                return true;
            }
            if let Ok(stop) = stop_q.get(entry) {
                let platform_entity = stop.entity();
                if changed_nodes_set.contains(&platform_entity) {
                    return true;
                }
                if let Ok((_, Some(parent))) = platform_q.get(platform_entity) {
                    if changed_nodes_set.contains(&parent.parent()) {
                        return true;
                    }
                }
            }
            false
        };

        for (trip_entity, schedule) in &trips {
            if schedule.iter().any(|e| check_entry(*e)) {
                changed_trip_set.insert(trip_entity);
            }
        }
    }

    for trip in to_remove_trips.iter() {
        if let Some(old_items) = index.entities.remove(trip) {
            for item in old_items {
                index.tree.remove(&item);
            }
        }
    }

    if changed_trip_set.is_empty() && to_remove_trips.is_empty() {
        return;
    }

    let get_station_xy = |entry_entity: Entity| -> Option<[f64; 2]> {
        let platform_entity = stop_q.get(entry_entity).ok()?.entity();
        let node = match platform_q.get(platform_entity).ok()? {
            (Some(_), _) => node_q.get(platform_entity).ok()?,
            (None, Some(parent)) => node_q.get(parent.parent()).ok()?,
            _ => return None,
        };
        Some(node.coor.to_xy_arr())
    };

    let repeat_time = settings.repeat_frequency.0 as f64;

    for trip_entity in changed_trip_set {
        if to_remove_trips.contains(&trip_entity) {
            continue;
        }

        if let Some(old_items) = index.entities.remove(&trip_entity) {
            for item in old_items {
                index.tree.remove(&item);
            }
        }

        let Ok((_, schedule)) = trips.get(trip_entity) else {
            continue;
        };
        if schedule.len() < 1 {
            continue;
        }

        let mut new_items = Vec::new();

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

            let t0 = estimate0.arr.0 as f64;
            let t1 = estimate0.dep.0 as f64;
            let t2 = (estimate1.arr.0 as f64).max(t1);

            if repeat_time > 0.0 {
                let dep_duration = t1 - t0;
                let arr_duration = t2 - t0;
                if arr_duration >= repeat_time {
                    new_items.push(TripSpatialIndexItem {
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
                new_items.push(TripSpatialIndexItem {
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
                    new_items.push(TripSpatialIndexItem {
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
                new_items.push(TripSpatialIndexItem {
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

        for item in &new_items {
            index.tree.insert(*item);
        }
        index.entities.insert(trip_entity, new_items);
    }
}

/// Marker component for a trip
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(TripVehicles, TripSchedule, Name)]
pub struct Trip;

/// Trip bundle.
#[derive(Bundle)]
pub struct TripBundle {
    trip: Trip,
    vehicles: TripVehicles,
    name: Name,
    class: TripClass,
    nominal_schedule: TripNominalSchedule,
}

impl TripBundle {
    pub fn new(name: &str, class: TripClass, nominal_schedule: Vec<Entity>) -> Self {
        Self {
            trip: Trip,
            vehicles: TripVehicles::default(),
            name: Name::from(name),
            class,
            nominal_schedule: TripNominalSchedule(nominal_schedule),
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

#[derive(Reflect, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
pub struct TripNominalSchedule(#[entities] pub Vec<Entity>);

#[derive(Reflect, Default, Component, MapEntities, Deref, DerefMut)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
pub struct TripSchedule(#[entities] pub Vec<Entity>);

#[derive(Debug, EntityEvent)]
pub struct ConvertDerivedEntryToExplicit {
    pub entity: Entity,
}

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
    /// The duration of the trip, from the first entry's arrival time to the
    /// last entry's departure time. This method only checks the first and
    /// last entries' times, hence any intermediate entries are not
    /// considered.
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

fn update_nominal_schedule(
    msg: On<Remove, EntryMode>,
    parent_q: Query<&ChildOf>,
    mut schedule_q: Query<&mut TripNominalSchedule>,
) {
    let Ok(parent) = parent_q.get(msg.entity) else {
        return;
    };
    let Ok(mut schedule) = schedule_q.get_mut(parent.parent()) else {
        return;
    };
    if let Some(idx) = schedule.iter().position(|e| *e == msg.entity) {
        schedule.remove(idx);
    }
}

fn convert_derived_entry_to_explicit(
    msg: On<ConvertDerivedEntryToExplicit>,
    mut commands: Commands,
    parent_q: Query<&ChildOf>,
    schedule_q: Query<&TripSchedule, With<Trip>>,
    mut nominal_q: Query<&mut TripNominalSchedule, With<Trip>>,
) {
    let entry = msg.entity;
    let parent = parent_q.get(entry).unwrap();
    let trip = parent.parent();
    let schedule = schedule_q.get(trip).unwrap();
    let mut nominal = nominal_q.get_mut(trip).unwrap();

    if !nominal.iter().any(|e| *e == entry) {
        let Some(schedule_idx) = schedule.iter().position(|e| *e == entry) else {
            nominal.push(entry);
            commands.entity(entry).remove::<entry::IsDerivedEntry>();
            return;
        };

        let prev_nominal = schedule[..schedule_idx]
            .iter()
            .rev()
            .find(|candidate| nominal.iter().any(|e| e == *candidate))
            .copied();
        let next_nominal = schedule[schedule_idx + 1..]
            .iter()
            .find(|candidate| nominal.iter().any(|e| e == *candidate))
            .copied();

        let insert_idx = if let Some(next) = next_nominal {
            nominal
                .iter()
                .position(|e| *e == next)
                .unwrap_or(nominal.len())
        } else if let Some(prev) = prev_nominal {
            nominal
                .iter()
                .position(|e| *e == prev)
                .map(|i| i + 1)
                .unwrap_or(nominal.len())
        } else {
            nominal.len()
        };

        nominal.insert(insert_idx, entry);
    }

    commands.entity(entry).remove::<entry::IsDerivedEntry>();
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
