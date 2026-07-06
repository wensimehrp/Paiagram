use std::collections::HashMap;

use ecow::{EcoString, EcoVec};
use serde::Deserialize;

use crate::trip::{TEntry, TravelMode, TripSchedule};
use crate::units::time::TimetableTime;
use crate::{
    ClassKey, ClassView, Command, IntervalKey, IntervalView, LonLat, RouteKey, RouteView,
    StationKey, StationView, StrokeStyle, TripKey, TripView, VehicleKey,
};

/// The root structure of the qETRC JSON data
#[derive(Deserialize)]
struct Root {
    #[serde(rename = "trains")]
    services: Vec<Service>,
    line: Line,
    lines: Option<Vec<Line>>,
    #[serde(rename = "circuits")]
    vehicles: Vec<Vehicle>,
    config: Option<Config>,
}

#[derive(Deserialize)]
struct Line {
    name: String,
    stations: Vec<QStation>,
}

#[derive(Deserialize)]
struct QStation {
    #[serde(rename = "zhanming")]
    name: String,
    #[serde(rename = "licheng")]
    distance_km: f32,
}

#[derive(Deserialize)]
struct Service {
    #[serde(rename = "checi")]
    service_number: Vec<String>,
    #[serde(rename = "type")]
    service_type: String,
    timetable: Vec<TimetableEntry>,
}

#[derive(Deserialize)]
struct TimetableEntry {
    #[serde(rename = "business")]
    would_stop: Option<bool>,
    #[serde(rename = "ddsj")]
    arrival: String,
    #[serde(rename = "cfsj")]
    departure: String,
    #[serde(rename = "zhanming")]
    station_name: String,
}

#[derive(Deserialize)]
struct Vehicle {
    #[serde(rename = "model")]
    make: String,
    name: String,
    #[serde(rename = "order")]
    services: Vec<VehicleServiceEntry>,
}

#[derive(Deserialize)]
struct VehicleServiceEntry {
    #[serde(rename = "checi")]
    service_number: String,
}

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    default_colors: HashMap<String, String>,
}

/// Parse qETRC/pyETRC JSON timetable data into a list of commands.
pub fn load_qetrc(content: &str) -> Result<Vec<Command>, String> {
    let root: Root = serde_json::from_str(content).map_err(|e| format!("Failed to parse QETRC data: {e:?}"))?;
    let mut commands = Vec::new();
    let mut station_map: HashMap<String, StationKey> = HashMap::new();
    let mut class_map: HashMap<String, ClassKey> = HashMap::new();
    let mut trip_pool: HashMap<String, TripKey> = HashMap::new();

    // Create classes from config colors
    if let Some(config) = root.config {
        for (class_name, color_hex) in config.default_colors {
            let (r, g, b) = (
                u8::from_str_radix(&color_hex[1..=2], 16).map_err(|e| format!("Invalid color hex: {e}"))?,
                u8::from_str_radix(&color_hex[3..=4], 16).map_err(|e| format!("Invalid color hex: {e}"))?,
                u8::from_str_radix(&color_hex[5..=6], 16).map_err(|e| format!("Invalid color hex: {e}"))?,
            );
            let ck = ClassKey::new();
            class_map.insert(class_name.clone(), ck);
            commands.push(Command::AddClass {
                key: ck,
                view: ClassView {
                    name: EcoString::from(class_name),
                    style: StrokeStyle {
                        color: egui::Color32::from_rgb(r, g, b),
                        width: 1,
                    },
                },
            });
        }
    }

    // Helpers as macros to avoid closure borrow issues
    macro_rules! ensure_station {
        ($name:expr) => {{
            let name = $name;
            if let Some(&sk) = station_map.get(name) {
                sk
            } else {
                let sk = StationKey::new();
                commands.push(Command::AddStation {
                    key: sk,
                    name: EcoString::from(name),
                    pos: LonLat { lon: 0, lat: 0 },
                });
                station_map.insert(name.to_string(), sk);
                sk
            }
        }};
    }

    macro_rules! ensure_class {
        ($name:expr) => {{
            let name = $name;
            if let Some(&ck) = class_map.get(name) {
                ck
            } else {
                let ck = ClassKey::new();
                commands.push(Command::AddClass {
                    key: ck,
                    view: ClassView {
                        name: EcoString::from(name),
                        style: StrokeStyle {
                            color: egui::Color32::GRAY,
                            width: 1,
                        },
                    },
                });
                class_map.insert(name.to_string(), ck);
                ck
            }
        }};
    }

    // Process lines -> routes + intervals
    let lines_iter = std::iter::once(root.line).chain(root.lines.into_iter().flatten());
    for line in lines_iter {
        let mut station_keys: Vec<(StationKey, f32)> = Vec::with_capacity(line.stations.len());
        for s in &line.stations {
            let sk = ensure_station!(&s.name);
            station_keys.push((sk, s.distance_km));
        }

        // Create edges (intervals) between consecutive stations
        for w in station_keys.windows(2) {
            let [(prev, prev_d), (curr, curr_d)] = w else { unreachable!() };
            let dist_m = ((curr_d - prev_d).abs() * 1000.0) as u32;
            let ik = IntervalKey::new();
            commands.push(Command::AddInterval {
                key: ik,
                view: IntervalView {
                    nodes: EcoVec::new(),
                    length: std::num::NonZeroU32::new(dist_m.max(1)),
                },
                from: Some(*prev),
                to: Some(*curr),
            });
        }

        // Create route
        let rk = RouteKey::new();
        let mut previous_km = station_keys.first().map_or(0.0, |(_, d)| *d);
        let mut relative_lengths: Vec<f32> = Vec::with_capacity(station_keys.len());
        relative_lengths.push(0.0);
        for (_, d) in station_keys.iter().skip(1) {
            let rel = *d - previous_km;
            relative_lengths.push(rel);
            previous_km = *d;
        }
        commands.push(Command::AddRoute {
            key: rk,
            view: RouteView {
                name: EcoString::from(line.name),
                stations: station_keys.iter().map(|(sk, _)| *sk).collect::<EcoVec<_>>(),
            },
        });
    }

    // Process services -> trips
    for service in root.services {
        let class_key = ensure_class!(&service.service_type);

        let mut entries: Vec<_> = service
            .timetable
            .iter()
            .map(|e| {
                let arr = TimetableTime::from_str(&e.arrival).unwrap_or(TimetableTime(0));
                let dep = TimetableTime::from_str(&e.departure).unwrap_or(TimetableTime(0));
                let stn = ensure_station!(&e.station_name);
                (arr, dep, stn)
            })
            .collect();

        // Normalize times
        normalize_times_flat(&mut entries);

        let trip_entries: EcoVec<TEntry> = entries
            .into_iter()
            .map(|(arr, dep, stn)| {
                if dep < arr {
                    log::info!("Trip {:?} has dep={:?} < arr={:?}", service.service_number.first(), dep, arr);
                }
                let arr_mode = if dep != arr {
                    TravelMode::At(arr)
                } else {
                    TravelMode::Flexible
                };
                TEntry::Pinned {
                    stn,
                    trk: 0,
                    arr: arr_mode,
                    dep: TravelMode::At(dep),
                    id: 0,
                }
            })
            .collect();

        let tk = TripKey::new();
        let trip_name = service.service_number.first().cloned().unwrap_or_else(|| "<unnamed>".to_string());
        trip_pool.insert(trip_name.clone(), tk);

        commands.push(Command::AddTrip {
            key: tk,
            view: TripView {
                name: EcoString::from(trip_name),
                schedule: TripSchedule::new(trip_entries),
                class: Some(class_key),
            },
        });
    }

    // Process vehicles
    for vehicle in root.vehicles {
        let vk = VehicleKey::new();
        let vehicle_name = format!("{} [{}]", vehicle.name, vehicle.make);
        commands.push(Command::AddVehicle {
            key: vk,
            name: EcoString::from(&vehicle_name),
        });
        let trip_keys: EcoVec<TripKey> = vehicle
            .services
            .iter()
            .filter_map(|s| trip_pool.get(&s.service_number).copied())
            .collect();
        if trip_keys.is_empty() {
            log::warn!("Vehicle {vehicle_name} has no matching trips in pool");
        }
        commands.push(Command::ChangeVehicleTrips {
            key: vk,
            trips: trip_keys,
        });
    }

    Ok(commands)
}

fn normalize_times_flat(entries: &mut [(TimetableTime, TimetableTime, StationKey)]) {
    let mut prev: Option<TimetableTime> = None;
    for (arr, dep, _) in entries.iter_mut() {
        if let Some(p) = prev {
            while *arr < p {
                *arr = TimetableTime(arr.0 + 86400);
            }
            while *dep < *arr {
                *dep = TimetableTime(dep.0 + 86400);
            }
        }
        prev = Some(*dep);
    }
}
