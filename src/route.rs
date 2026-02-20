//! # Route
//! Routes are slices of the graph that can be used as the foundation of diagrams.
//! Diagrams use routes as their station list.

use bevy::{ecs::entity::EntityHashSet, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

pub struct RoutePlugin;
impl Plugin for RoutePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(auto_update_length)
            .add_systems(Update, (update_route_trips, auto_generate_display_modes));
    }
}

use crate::{
    entry::EntryMode,
    graph::Graph,
    interval::{Interval, UpdateInterval},
    station::{Platform, PlatformEntries, Station, StationQuery},
};

/// Marker component for automatically updating route interval length.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct AutoUpdateLength;

#[derive(Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name, RouteTrips)]
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

#[derive(Default, Reflect, Component, Deref, DerefMut)]
#[reflect(Component)]
#[require(Name)]
pub struct RouteTrips(Vec<Entity>);

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
    mut routes: Query<(Entity, &Route, &mut RouteTrips)>,
    changed_routes: Query<Entity, (With<Route>, Changed<Route>)>,
    changed_station_entries: Query<Entity, (With<Station>, Changed<PlatformEntries>)>,
    changed_platform_entries: Query<&ChildOf, (With<Platform>, Changed<PlatformEntries>)>,
    stations: Query<StationQuery>,
    platform_entries: Query<&PlatformEntries>,
    entries: Query<&ChildOf, With<EntryMode>>,
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
        for (route_entity, route, _) in &routes {
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

    for (route_entity, route, mut route_trips) in &mut routes {
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

        let mut next = trips.into_iter().collect::<Vec<_>>();
        next.sort_unstable();
        if route_trips.0 != next {
            route_trips.0 = next;
        }
    }
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
