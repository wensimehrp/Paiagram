use std::io::Cursor;

use bevy::{platform::collections::HashMap, prelude::*};
use moonshine_core::kind::Instance;

use crate::{
    colors::{DisplayColor, PredefinedColor},
    entry::{EntryBundle, TravelMode},
    graph::{Graph, Node, NodePos},
    route::Route,
    station::{Platform, Station},
    trip::{
        TripBundle, TripClass,
        class::{Class, ClassBundle, DisplayedStroke},
    },
    units::{distance::Distance, time::TimetableTime},
    vehicle::Vehicle,
};

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r_km = 6371.0_f64;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r_km * c
}

fn route_name(route: Option<&gtfs_structures::Route>) -> String {
    route
        .and_then(|r| {
            r.long_name
                .clone()
                .or_else(|| r.short_name.clone())
                .or_else(|| Some(r.id.clone()))
        })
        .unwrap_or_else(|| "GTFS Route".to_string())
}

fn class_name(route: Option<&gtfs_structures::Route>, route_id: &str) -> String {
    route
        .and_then(|r| {
            r.short_name
                .clone()
                .or_else(|| r.long_name.clone())
                .or_else(|| Some(r.id.clone()))
        })
        .unwrap_or_else(|| route_id.to_string())
}

fn class_color(route: Option<&gtfs_structures::Route>) -> DisplayColor {
    if let Some(rgb) = route.and_then(|r| r.color) {
        return DisplayColor::Custom(egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b));
    }
    DisplayColor::Predefined(PredefinedColor::Neutral)
}

fn stop_display_name(stop: &gtfs_structures::Stop) -> String {
    stop.name.clone().unwrap_or_else(|| stop.id.clone())
}

pub fn load_gtfs_static(
    data: On<super::LoadGTFS>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
) {
    info!("Loading GTFS static data...");
    let reader = Cursor::new(data.content.as_slice());
    let Ok(gtfs) = gtfs_structures::Gtfs::from_reader(reader) else {
        warn!("Failed to parse GTFS zip");
        return;
    };

    let mut station_entities: HashMap<String, Entity> = HashMap::new();
    let mut platform_entities: HashMap<String, Entity> = HashMap::new();
    let mut class_map: HashMap<String, Instance<Class>> = HashMap::new();
    let mut route_built: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut block_to_trips: HashMap<String, Vec<Entity>> = HashMap::new();

    let mut ensure_station =
        |station_id: &str, station_name: &str, graph: &mut Graph, commands: &mut Commands| {
            if let Some(&entity) = station_entities.get(station_id) {
                return entity;
            }
            let entity = commands
                .spawn((Station::default(), Name::new(station_name.to_string())))
                .id();
            graph.add_node(entity);
            station_entities.insert(station_id.to_string(), entity);
            entity
        };

    let mut ensure_platform = |platform_id: &str,
                               platform_name: &str,
                               parent_station: Entity,
                               commands: &mut Commands| {
        if let Some(&entity) = platform_entities.get(platform_id) {
            return entity;
        }
        let entity = commands
            .spawn((
                Platform::default(),
                Name::new(platform_name.to_string()),
                ChildOf(parent_station),
            ))
            .id();
        platform_entities.insert(platform_id.to_string(), entity);
        entity
    };

    for trip in gtfs.trips.values() {
        if trip.stop_times.is_empty() {
            continue;
        }

        let route = gtfs.routes.get(&trip.route_id);
        let class_name = class_name(route, &trip.route_id);
        let trip_class =
            super::make_class(&class_name, &mut class_map, &mut commands, || ClassBundle {
                class: Class::default(),
                name: Name::new(class_name.clone()),
                stroke: DisplayedStroke {
                    color: class_color(route),
                    width: 1.0,
                },
            });

        let mut stops_for_trip: Vec<(Entity, Option<f64>, Option<f64>, Option<f32>)> =
            Vec::with_capacity(trip.stop_times.len());
        for stop_time in &trip.stop_times {
            let stop = &stop_time.stop;
            let stop_name = stop_display_name(stop);

            let (station_entity, platform_entity) =
                if let Some(parent_station_id) = &stop.parent_station {
                    let parent_stop = gtfs.stops.get(parent_station_id);
                    let parent_name = parent_stop
                        .map(|stop| stop_display_name(stop.as_ref()))
                        .unwrap_or_else(|| parent_station_id.clone());
                    let station_entity =
                        ensure_station(parent_station_id, &parent_name, &mut graph, &mut commands);
                    let platform_entity =
                        ensure_platform(&stop.id, &stop_name, station_entity, &mut commands);
                    (station_entity, platform_entity)
                } else {
                    let station_entity =
                        ensure_station(&stop.id, &stop_name, &mut graph, &mut commands);
                    (station_entity, station_entity)
                };

            if let (Some(lat), Some(lon)) = (stop.latitude, stop.longitude) {
                commands.entity(platform_entity).insert(Node {
                    pos: NodePos::new_lon_lat(lon, lat),
                });
                if platform_entity != station_entity {
                    commands.entity(station_entity).insert(Node {
                        pos: NodePos::new_lon_lat(lon, lat),
                    });
                }
            }

            stops_for_trip.push((
                station_entity,
                stop.latitude,
                stop.longitude,
                stop_time.shape_dist_traveled,
            ));
        }

        if !route_built.contains(&trip.route_id) {
            let mut route_stops: Vec<Entity> = Vec::new();
            let mut lengths: Vec<f32> = Vec::new();
            let mut prev_station: Option<Entity> = None;
            let mut prev_shape_dist: Option<f32> = None;
            let mut prev_lat_lon: Option<(f64, f64)> = None;

            for (stop, lat, lon, shape_dist) in &stops_for_trip {
                let curr_station = *stop;
                if prev_station == Some(curr_station) {
                    continue;
                }

                route_stops.push(curr_station);
                if let Some(prev) = prev_station {
                    let mut km = match (shape_dist, prev_shape_dist) {
                        (Some(curr), Some(prev)) => (*curr - prev).abs(),
                        _ => 0.0,
                    };
                    if km <= f32::EPSILON
                        && let (Some((p_lat, p_lon)), Some(c_lat), Some(c_lon)) =
                            (prev_lat_lon, *lat, *lon)
                    {
                        km = haversine_km(p_lat, p_lon, c_lat, c_lon) as f32;
                    }
                    if km <= f32::EPSILON {
                        km = 1.0;
                    }
                    super::add_interval_pair(
                        &mut graph,
                        &mut commands,
                        prev,
                        curr_station,
                        Distance::from_km(km),
                    );
                    lengths.push(km);
                } else {
                    lengths.push(0.0);
                }

                prev_station = Some(curr_station);
                prev_shape_dist = *shape_dist;
                prev_lat_lon = lat.zip(*lon);
            }

            if route_stops.len() >= 2 {
                commands.spawn((
                    Name::new(route_name(route)),
                    Route {
                        stops: route_stops,
                        lengths,
                    },
                ));
            }
            route_built.insert(trip.route_id.clone());
        }

        let trip_name = trip
            .trip_short_name
            .as_ref()
            .or(trip.trip_headsign.as_ref())
            .map_or_else(|| trip.id.clone(), std::clone::Clone::clone);

        let mut entry_payloads: Vec<(Entity, TimetableTime, Option<TimetableTime>)> =
            Vec::with_capacity(trip.stop_times.len());
        let mut previous_arrival: Option<TimetableTime> = None;
        for stop_time in &trip.stop_times {
            let stop = &stop_time.stop;
            let stop_name = stop_display_name(stop);

            let stop_entity = if let Some(parent_station_id) = &stop.parent_station {
                let parent_stop = gtfs.stops.get(parent_station_id);
                let parent_name = parent_stop
                    .map(|stop| stop_display_name(stop.as_ref()))
                    .unwrap_or_else(|| parent_station_id.clone());
                let parent_station =
                    ensure_station(parent_station_id, &parent_name, &mut graph, &mut commands);
                ensure_platform(&stop.id, &stop_name, parent_station, &mut commands)
            } else {
                ensure_station(&stop.id, &stop_name, &mut graph, &mut commands)
            };

            if let (Some(lat), Some(lon)) = (stop.latitude, stop.longitude) {
                commands.entity(stop_entity).insert(Node {
                    pos: NodePos::new_lon_lat(lon, lat),
                });
            }

            let arr = stop_time
                .arrival_time
                .or(stop_time.departure_time)
                .map(|t| TimetableTime(t as i32));
            let dep = stop_time
                .departure_time
                .or(stop_time.arrival_time)
                .map(|t| TimetableTime(t as i32));

            let Some(arrival) = arr else {
                continue;
            };
            let departure = dep.unwrap_or(arrival);
            if let Some(prev) = previous_arrival
                && arrival < prev
            {
                warn!("GTFS trip has non-monotonic time: trip_id={}", trip.id);
            }
            previous_arrival = Some(arrival);

            let dep_mode = (departure != arrival).then_some(departure);
            entry_payloads.push((stop_entity, arrival, dep_mode));
        }

        let trip_entity = commands
            .spawn(TripBundle::new(&trip_name, TripClass(trip_class.entity())))
            .with_children(|bundle| {
                for (stop_entity, arrival, dep_mode) in entry_payloads {
                    let dep_mode = dep_mode.map(TravelMode::At);
                    bundle.spawn(EntryBundle::new(
                        TravelMode::At(arrival),
                        dep_mode,
                        stop_entity,
                    ));
                }
            })
            .id();

        if let Some(block_id) = &trip.block_id {
            block_to_trips
                .entry(block_id.clone())
                .or_default()
                .push(trip_entity);
        }
    }

    for (block_id, trips) in block_to_trips {
        commands.spawn((
            Name::new(format!("GTFS block {block_id}")),
            Vehicle { trips },
        ));
    }

    info!(
        "GTFS import completed: stations={}, classes={}, routes={}, vehicles={}",
        station_entities.len(),
        class_map.len(),
        route_built.len(),
        gtfs.trips
            .values()
            .filter(|t| t.block_id.is_some())
            .map(|t| t.block_id.as_ref().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len()
    );
}
