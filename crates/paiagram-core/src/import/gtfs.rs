use std::collections::HashMap;
use std::io::Cursor;
use std::num::NonZeroU32;

use ecow::{EcoString, EcoVec};
use egui::Color32;

use crate::colors::{DisplayedColor, PredefinedColor};
use crate::units::distance::Distance;
use crate::units::time::TimetableTime;
use crate::{
    ClassKey, ClassView, Command, IntervalKey, IntervalView, LonLat, RouteKey, RouteView,
    StationKey, StationView, StrokeStyle, TripKey, TripView, VehicleKey, VehicleView,
};
use crate::trip::{TEntry, TravelMode, TripSchedule};

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

fn class_color(route: Option<&gtfs_structures::Route>) -> DisplayedColor {
    if let Some(rgb) = route.and_then(|r| r.color) {
        return DisplayedColor::Custom(Color32::from_rgb(rgb.r, rgb.g, rgb.b));
    }
    DisplayedColor::Predefined(PredefinedColor::Neutral)
}

fn stop_display_name(stop: &gtfs_structures::Stop) -> String {
    stop.name.clone().unwrap_or_else(|| stop.id.clone())
}

/// Load GTFS static data and return a list of Commands that reconstruct
/// the timetable in a WorldSnapshot.
pub fn load_gtfs_static(data: &[u8]) -> Result<Vec<Command>, String> {
    log::info!("Loading GTFS static data...");
    let reader = Cursor::new(data);
    let gtfs = gtfs_structures::Gtfs::from_reader(reader)
        .map_err(|e| format!("Failed to parse GTFS zip: {e}"))?;

    let mut commands: Vec<Command> = Vec::new();
    let mut station_map: HashMap<String, StationKey> = HashMap::new();
    let mut class_map: HashMap<String, ClassKey> = HashMap::new();
    let mut route_built: HashMap<String, RouteKey> = HashMap::new();
    let mut block_to_trips: HashMap<String, Vec<TripKey>> = HashMap::new();

    macro_rules! ensure_station {
        ($id:expr, $name:expr) => {{
            let id = $id;
            let name = $name;
            if let Some(&sk) = station_map.get(id) {
                sk
            } else {
                let sk = StationKey::new();
                commands.push(Command::AddStation {
                    key: sk,
                    name: EcoString::from(name),
                    pos: LonLat { lon: 0, lat: 0 },
                });
                station_map.insert(id.to_string(), sk);
                sk
            }
        }};
    }

    // First pass: ensure all stations exist
    for trip in gtfs.trips.values() {
        if trip.stop_times.is_empty() {
            continue;
        }
        for stop_time in &trip.stop_times {
            let stop = &stop_time.stop;
            let stop_name = stop_display_name(stop);
            if let Some(parent_id) = &stop.parent_station {
                let parent_stop = gtfs.stops.get(parent_id);
                let parent_name = parent_stop
                    .map(|s| stop_display_name(s.as_ref()))
                    .unwrap_or_else(|| parent_id.clone());
                ensure_station!(parent_id, &parent_name);
            }
            ensure_station!(&stop.id, &stop_name);
        }
    }

    // Process each trip and collect route/class/vehicle data
    for trip in gtfs.trips.values() {
        if trip.stop_times.is_empty() {
            continue;
        }

        let route = gtfs.routes.get(&trip.route_id);
        let c_name = class_name(route, &trip.route_id);

        // Ensure class exists
        let class_key = if let Some(&ck) = class_map.get(&c_name) {
            ck
        } else {
            let ck = ClassKey::new();
            let color = class_color(route);
            let style = StrokeStyle {
                color: color.into_color32(false),
                width: 1,
            };
            commands.push(Command::AddClass {
                key: ck,
                view: ClassView {
                    name: EcoString::from(c_name.clone()),
                    style,
                },
            });
            class_map.insert(c_name.clone(), ck);
            ck
        };

        // Collect stations for this trip and build route if needed
        let mut stops_for_trip: Vec<(StationKey, Option<f64>, Option<f64>, Option<f32>)> =
            Vec::with_capacity(trip.stop_times.len());

        for stop_time in &trip.stop_times {
            let stop = &stop_time.stop;
            let station_key = if let Some(parent_id) = &stop.parent_station {
                let parent_stop = gtfs.stops.get(parent_id);
                let parent_name = parent_stop
                    .map(|s| stop_display_name(s.as_ref()))
                    .unwrap_or_else(|| parent_id.clone());
                ensure_station!(parent_id, &parent_name)
            } else {
                ensure_station!(&stop.id, &stop_display_name(stop))
            };

            stops_for_trip.push((
                station_key,
                stop.latitude,
                stop.longitude,
                stop_time.shape_dist_traveled,
            ));
        }

        // Build route if not yet built for this route_id
        if !route_built.contains_key(&trip.route_id) {
            let mut route_stations: Vec<StationKey> = Vec::new();
            let mut prev_station: Option<StationKey> = None;
            let mut prev_shape_dist: Option<f32> = None;
            let mut prev_lat_lon: Option<(f64, f64)> = None;

            for (sk, lat, lon, shape_dist) in &stops_for_trip {
                let curr = *sk;
                if prev_station == Some(curr) {
                    continue;
                }

                route_stations.push(curr);
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
                    let dist_m = (km * 1000.0).round() as u32;
                    let ik = IntervalKey::new();
                    commands.push(Command::AddInterval {
                        key: ik,
                        view: IntervalView {
                            nodes: EcoVec::new(),
                            length: NonZeroU32::new(dist_m.max(1)),
                        },
                        from: Some(prev),
                        to: Some(curr),
                    });
                }

                prev_station = Some(curr);
                prev_shape_dist = *shape_dist;
                prev_lat_lon = lat.zip(*lon);
            }

            if route_stations.len() >= 2 {
                let rk = RouteKey::new();
                commands.push(Command::AddRoute {
                    key: rk,
                    view: RouteView {
                        name: EcoString::from(route_name(route)),
                        stations: route_stations.into_iter().collect::<EcoVec<_>>(),
                    },
                });
                route_built.insert(trip.route_id.clone(), rk);
            }
        }

        // Build trip entries
        let trip_name = trip
            .trip_short_name
            .as_ref()
            .or(trip.trip_headsign.as_ref())
            .map_or_else(|| trip.id.clone(), std::clone::Clone::clone);

        let mut entries: Vec<TEntry> = Vec::with_capacity(trip.stop_times.len());
        let mut previous_arrival: Option<TimetableTime> = None;
        for stop_time in &trip.stop_times {
            let stop = &stop_time.stop;
            let station_key = if let Some(parent_id) = &stop.parent_station {
                let parent_name = if let Some(parent_stop) = gtfs.stops.get(parent_id) {
                    stop_display_name(parent_stop.as_ref())
                } else {
                    parent_id.clone()
                };
                ensure_station!(parent_id, &parent_name)
            } else {
                ensure_station!(&stop.id, &stop_display_name(stop))
            };

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
                log::warn!("GTFS trip has non-monotonic time: trip_id={}", trip.id);
            }
            previous_arrival = Some(arrival);

            let arr_mode = if departure != arrival {
                Some(TravelMode::At(arrival))
            } else {
                None
            };
            let dep_mode = TravelMode::At(departure);

            let entry = match arr_mode {
                Some(arr) if arr == dep_mode => TEntry::PinnedNonStop {
                    stn: station_key,
                    trk: 0,
                    pass: dep_mode,
                    id: 0,
                },
                Some(arr) => TEntry::Pinned {
                    stn: station_key,
                    trk: 0,
                    arr,
                    dep: dep_mode,
                    id: 0,
                },
                None => TEntry::Derived(station_key),
            };
            entries.push(entry);
        }

        let tk = TripKey::new();
        commands.push(Command::AddTrip {
            key: tk,
            view: TripView {
                name: EcoString::from(trip_name),
                schedule: TripSchedule::new(entries.into_iter().collect::<EcoVec<_>>()),
                class: Some(class_key),
            },
        });

        if let Some(block_id) = &trip.block_id {
            block_to_trips
                .entry(block_id.clone())
                .or_default()
                .push(tk);
        }
    }

    // Create vehicles for blocks
    for (block_id, trip_keys) in block_to_trips {
        let vk = VehicleKey::new();
        commands.push(Command::AddVehicle {
            key: vk,
            name: EcoString::from(format!("GTFS block {block_id}")),
        });
        commands.push(Command::ChangeVehicleTrips {
            key: vk,
            trips: trip_keys.into_iter().collect::<EcoVec<_>>(),
        });
    }

    log::info!(
        "GTFS import completed: stations={}, classes={}, routes={}, vehicles={}",
        station_map.len(),
        class_map.len(),
        route_built.len(),
        gtfs.trips
            .values()
            .filter(|t| t.block_id.is_some())
            .map(|t| t.block_id.as_ref().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len()
    );

    Ok(commands)
}
