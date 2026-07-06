use std::collections::HashMap;

use ecow::{EcoString, EcoVec};
use serde::{Deserialize, Serialize};

use crate::units::time::TimetableTime;
use crate::trip::{TEntry, TravelMode, TripSchedule};
use crate::{
    ClassKey, ClassView, Command, IntervalKey, IntervalView, LonLat, RouteKey, RouteView,
    StationKey, StationView, StrokeStyle, TripKey, TripView,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableData {
    pub stations: Vec<String>,
    pub lines: Vec<Line>,
    pub trains: Vec<Train>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub code: i64,
    pub name: String,
    pub stations: Vec<LineStation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineStation {
    pub station: String,
    pub telecode: String,
    #[serde(rename = "routeFlag")]
    pub route_flag: i64,
    pub distance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Train {
    pub number: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub stops: Vec<TrainStop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainStop {
    pub station: String,
    #[serde(rename = "type")]
    pub r#type: StopType,
    pub line_code: i64,
    pub arrival_time: String,
    pub departure_time: String,
    pub mileage: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopType {
    Stop,
    Crossline,
}

fn ensure_station(
    name: &str,
    station_map: &mut HashMap<String, StationKey>,
    commands: &mut Vec<Command>,
    pos: LonLat,
) -> StationKey {
    if let Some(&sk) = station_map.get(name) {
        return sk;
    }
    let sk = StationKey::new();
    commands.push(Command::AddStation {
        key: sk,
        name: EcoString::from(name),
        pos,
    });
    station_map.insert(name.to_string(), sk);
    sk
}

fn ensure_class(
    r#type: &str,
    class_map: &mut HashMap<String, ClassKey>,
    commands: &mut Vec<Command>,
) -> ClassKey {
    if let Some(&ck) = class_map.get(r#type) {
        return ck;
    }
    let ck = ClassKey::new();
    commands.push(Command::AddClass {
        key: ck,
        view: ClassView {
            name: EcoString::from(r#type),
            style: StrokeStyle {
                color: egui::Color32::from_rgb(100, 100, 100),
                width: 1,
            },
        },
    });
    class_map.insert(r#type.to_string(), ck);
    ck
}

/// Parse LLT (Chinese railway timetable JSON) and return a list of Commands.
pub fn load_llt(content: &str) -> Result<Vec<Command>, String> {
    let root: TimetableData =
        serde_json::from_str(content).map_err(|e| format!("LLT JSON error: {e}"))?;
    let mut commands = Vec::new();
    let mut station_map: HashMap<String, StationKey> = HashMap::new();
    let mut class_map: HashMap<String, ClassKey> = HashMap::new();

    let zero_pos = LonLat { lon: 0, lat: 0 };

    // Pre‑create all stations from the stations list
    for station_name in &root.stations {
        ensure_station(station_name, &mut station_map, &mut commands, zero_pos);
    }

    // Lines → routes + intervals
    for line in &root.lines {
        if line.stations.is_empty() {
            continue;
        }

        let mut station_keys = Vec::with_capacity(line.stations.len());

        for ls in &line.stations {
            let sk = ensure_station(&ls.station, &mut station_map, &mut commands, zero_pos);
            station_keys.push(sk);
        }

        // Create intervals between consecutive stations using distance diff
        for w in station_keys.windows(2) {
            let dist_m = 1000.max(1); // placeholder fallback
            let ik = IntervalKey::new();
            commands.push(Command::AddInterval {
                key: ik,
                view: IntervalView {
                    nodes: EcoVec::new(),
                    length: std::num::NonZeroU32::new(dist_m),
                },
                from: Some(w[0]),
                to: Some(w[1]),
            });
        }

        // Create route
        let rk = RouteKey::new();
        commands.push(Command::AddRoute {
            key: rk,
            view: RouteView {
                name: EcoString::from(line.name.as_str()),
                stations: station_keys.into_iter().collect(),
            },
        });
    }

    // Trains → trips
    for train in &root.trains {
        let mut entries: Vec<(Option<TimetableTime>, Option<TimetableTime>, StationKey)> =
            Vec::new();

        for stop in &train.stops {
            let arr = TimetableTime::from_str(&stop.arrival_time);
            let dep = TimetableTime::from_str(&stop.departure_time);
            let sk = ensure_station(&stop.station, &mut station_map, &mut commands, zero_pos);
            entries.push((arr, dep, sk));
        }

        // Normalize times
        {
            let all_times: Vec<&mut TimetableTime> = entries
                .iter_mut()
                .flat_map(|(a, d, _)| a.iter_mut().chain(d.iter_mut()))
                .collect();
            crate::import::normalize_times(all_times.into_iter());
        }

        let class_key = ensure_class(&train.r#type, &mut class_map, &mut commands);

        let trip_entries: EcoVec<TEntry> = entries
            .into_iter()
            .map(|(arr, dep, stn)| {
                let dep_mode = match dep {
                    Some(t) => TravelMode::At(t),
                    None => TravelMode::Flexible,
                };
                match arr {
                    Some(at) => {
                        let dt = dep.unwrap_or(at);
                        if at == dt {
                            TEntry::PinnedNonStop {
                                stn,
                                trk: 0,
                                pass: dep_mode,
                                id: 0,
                            }
                        } else {
                            TEntry::Pinned {
                                stn,
                                trk: 0,
                                arr: TravelMode::At(at),
                                dep: dep_mode,
                                id: 0,
                            }
                        }
                    }
                    None => TEntry::Derived(stn),
                }
            })
            .collect();

        let tk = TripKey::new();
        commands.push(Command::AddTrip {
            key: tk,
            view: TripView {
                name: EcoString::from(train.number.as_str()),
                schedule: TripSchedule::new(trip_entries),
                class: Some(class_key),
            },
        });
    }

    Ok(commands)
}
