//! # Route
//! Routes are slices of the graph that can be used as the foundation of diagrams.
//! Diagrams use routes as their station list.

use bevy::{ecs::entity::EntityHashSet, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

pub struct RoutePlugin;
impl Plugin for RoutePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(auto_update_length)
            .add_observer(sort_route_by_direction_trips)
            .add_systems(Update, (update_route_trips, auto_generate_display_modes));
    }
}

use crate::{
    entry::{EntryMode, EntryQuery},
    graph::Graph,
    interval::{Interval, UpdateInterval},
    station::{ParentStationOrStation, Platform, PlatformEntries, Station, StationQuery},
    trip::TripQuery,
};

/// Marker component for automatically updating route interval length.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct AutoUpdateLength;

#[derive(Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name, RouteTrips, RouteByDirectionTrips)]
pub struct Route {
    #[entities]
    pub stops: Vec<Entity>,
    pub lengths: Vec<f32>,
}

#[derive(Reflect)]
pub struct AllTripsDisplayMode {
    pub departure: bool,
    pub arrival: bool,
}

impl AllTripsDisplayMode {
    pub fn count(&self) -> usize {
        self.departure as usize + self.arrival as usize
    }
}

#[derive(Reflect, Component, Deref, DerefMut)]
#[reflect(Component)]
pub struct RouteDisplayModes(Vec<AllTripsDisplayMode>);

fn auto_generate_display_modes(
    routes: Populated<(Entity, &Route), Without<RouteDisplayModes>>,
    graph: Res<Graph>,
    mut commands: Commands,
) {
    for (route_entity, route) in routes.iter().filter(|(_, it)| it.stops.len() > 0) {
        let mut modes: Vec<AllTripsDisplayMode> = Vec::new();
        modes.resize_with(route.stops.len(), || AllTripsDisplayMode {
            departure: true,
            arrival: false,
        });
        modes.last_mut().unwrap().arrival = true;
        modes.last_mut().unwrap().departure = false;
        for (idx, s) in route.stops.windows(2).enumerate() {
            let [prev, curr] = s else { unreachable!() };
            if graph.contains_edge(*prev, *curr) || graph.contains_edge(*curr, *prev) {
                continue;
            }
            modes[idx].departure = false;
            modes[idx].arrival = true;
        }
        commands
            .entity(route_entity)
            .insert(RouteDisplayModes(modes));
    }
}

// TODO: handle update of route
// TODO: improve sorting logic
#[derive(Default, Reflect, Component, MapEntities, Deref, DerefMut)]
#[reflect(Component, MapEntities)]
#[require(Name)]
pub struct RouteTrips(#[entities] Vec<Entity>);

#[derive(Default, Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
pub struct RouteByDirectionTrips {
    #[entities]
    pub downward: Vec<Entity>,
    #[entities]
    pub upward: Vec<Entity>,
}

#[derive(EntityEvent)]
pub struct SortRouteByDirectionTrips {
    pub entity: Entity,
}

fn compute_sorted_by_first_entry_estimate(
    trip_entities: &[Entity],
    trip_q: &Query<TripQuery>,
    entry_q: &Query<EntryQuery>,
) -> Vec<Entity> {
    let mut out = trip_entities.to_vec();
    out.sort_unstable_by_key(|trip_entity| {
        let trip = trip_q.get(*trip_entity).ok();
        let first_time = trip
            .and_then(|trip| {
                entry_q
                    .iter_many(trip.schedule.iter())
                    .find_map(|entry| entry.estimate.map(|it| it.arr.min(it.dep)))
            })
            .unwrap_or(crate::units::time::TimetableTime(i32::MAX));
        (first_time, trip_entity.to_bits())
    });
    out
}

fn compute_directional_members(
    route: &Route,
    trip_entities: &[Entity],
    downwards: bool,
    trip_q: &Query<TripQuery>,
    entry_q: &Query<EntryQuery>,
    parent_station_or_station: &Query<ParentStationOrStation>,
) -> Vec<Entity> {
    trip_entities
        .iter()
        .copied()
        .filter_map(|trip_entity| {
            let trip = trip_q.get(trip_entity).ok()?;
            let mut stations = if downwards {
                either::Either::Left(route.stops.iter())
            } else {
                either::Either::Right(route.stops.iter().rev())
            };
            let mut found_counter = 0;
            for it in entry_q.iter_many(trip.schedule.iter()) {
                let station_entity = parent_station_or_station.get(it.stop()).ok()?.parent();
                if stations.any(|it| *it == station_entity) {
                    found_counter += 1;
                    if found_counter >= 2 {
                        return Some(trip_entity);
                    }
                }
            }
            None
        })
        .collect()
}

fn sync_direction_order(existing: &mut Vec<Entity>, members: &[Entity], fallback_order: &[Entity]) {
    let member_set: EntityHashSet = members.iter().copied().collect();
    let mut next = Vec::with_capacity(members.len());

    for entity in existing.iter().copied() {
        if member_set.contains(&entity) {
            next.push(entity);
        }
    }

    for entity in fallback_order.iter().copied() {
        if member_set.contains(&entity) && !next.contains(&entity) {
            next.push(entity);
        }
    }

    *existing = next;
}

impl Route {
    pub fn iter(&self) -> impl Iterator<Item = (Entity, f32)> {
        self.stops
            .iter()
            .copied()
            .zip(self.lengths.iter().copied())
            .scan(0.0_f32, |acc, (stop, len)| {
                *acc += len;
                let out = (stop, *acc);
                Some(out)
            })
    }
}

fn update_route_trips(
    mut routes: Query<(Entity, &Route, &mut RouteTrips, &mut RouteByDirectionTrips)>,
    changed_routes: Query<Entity, (With<Route>, Changed<Route>)>,
    changed_station_entries: Query<Entity, (With<Station>, Changed<PlatformEntries>)>,
    changed_platform_entries: Query<&ChildOf, (With<Platform>, Changed<PlatformEntries>)>,
    stations: Query<StationQuery>,
    platform_entries: Query<&PlatformEntries>,
    entries: Query<&ChildOf, With<EntryMode>>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
) {
    let mut affected_routes = EntityHashSet::default();

    for route_entity in &changed_routes {
        affected_routes.insert(route_entity);
    }

    let mut changed_stations = EntityHashSet::default();
    for station in &changed_station_entries {
        changed_stations.insert(station);
    }
    for parent in &changed_platform_entries {
        changed_stations.insert(parent.parent());
    }

    if !changed_stations.is_empty() {
        for (route_entity, route, _, _) in &routes {
            if route
                .stops
                .iter()
                .any(|station| changed_stations.contains(station))
            {
                affected_routes.insert(route_entity);
            }
        }
    }

    if affected_routes.is_empty() {
        return;
    }

    for (route_entity, route, mut route_trips, mut by_direction) in &mut routes {
        if !affected_routes.contains(&route_entity) {
            continue;
        }

        let mut trips = EntityHashSet::default();
        for station_entity in route.stops.iter().copied() {
            let Ok(station) = stations.get(station_entity) else {
                continue;
            };
            for entry in station.passing_entries(&platform_entries) {
                let Ok(parent) = entries.get(entry) else {
                    continue;
                };
                trips.insert(parent.parent());
            }
        }

        let mut next = route_trips
            .0
            .iter()
            .copied()
            .filter(|entity| trips.contains(entity))
            .collect::<Vec<_>>();
        for entity in trips.iter().copied() {
            if !next.contains(&entity) {
                next.push(entity);
            }
        }
        if route_trips.0 != next {
            route_trips.0 = next.clone();
        }

        let new_downward = compute_directional_members(
            route,
            &next,
            true,
            &trip_q,
            &entry_q,
            &parent_station_or_station,
        );
        let new_upward = compute_directional_members(
            route,
            &next,
            false,
            &trip_q,
            &entry_q,
            &parent_station_or_station,
        );
        sync_direction_order(&mut by_direction.downward, &new_downward, &next);
        sync_direction_order(&mut by_direction.upward, &new_upward, &next);
    }
}

fn sort_route_by_direction_trips(
    trigger: On<SortRouteByDirectionTrips>,
    mut routes: Query<(&RouteTrips, &mut RouteByDirectionTrips)>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
) {
    let Ok((route_trips, mut by_direction)) = routes.get_mut(trigger.entity) else {
        return;
    };

    let sorted_downward =
        compute_sorted_by_first_entry_estimate(&by_direction.downward, &trip_q, &entry_q);
    let sorted_upward =
        compute_sorted_by_first_entry_estimate(&by_direction.upward, &trip_q, &entry_q);

    by_direction.downward = sorted_downward;
    by_direction.upward = sorted_upward;
}

fn auto_update_length(
    updated: On<UpdateInterval>,
    routes: Populated<&mut Route, With<AutoUpdateLength>>,
    intervals: Query<&Interval>,
    graph: Res<Graph>,
) {
    for mut route in routes {
        let Route { stops, lengths } = &mut *route;
        for (i, w) in stops.windows(2).enumerate() {
            let [p, c] = w else { unreachable!() };
            let (p, c) = (*p, *c);
            if (p == updated.source && c == updated.target)
                || (p == updated.target && c == updated.source)
            {
            } else {
                continue;
            }
            let i1 = graph.edge_weight(p, c).cloned();
            let i2 = graph.edge_weight(c, p).cloned();
            match (i1, i2) {
                (Some(e1), Some(e2)) => {
                    let d1 = intervals.get(e1).unwrap().length;
                    let d2 = intervals.get(e2).unwrap().length;
                    let avg_len = (d1.0 as f32 + d2.0 as f32) / 2.0;
                    lengths[i] = avg_len;
                }
                (Some(e), None) | (None, Some(e)) => {
                    let d = intervals.get(e).unwrap().length;
                    lengths[i] = d.0 as f32;
                }
                (None, None) => {
                    panic!("Interval disappeared???")
                }
            }
        }
    }
}
