use std::collections::HashMap;

use ecow::{EcoString, EcoVec};
use itertools::Itertools;
use paiagram_oudia::{
    Direction, ServiceMode, Time as OuDiaTime, TimetableEntry as OuDiaTimetableEntry,
    parse_oud_to_ir, parse_oud2_to_ir,
};
use std::num::NonZeroU32;

use crate::units::time::TimetableTime;
use crate::{
    ClassKey, ClassView, Command, IntervalKey, IntervalView, LonLat, RouteKey, RouteView,
    StationKey, StrokeStyle, TripKey, TripView,
};
use crate::trip::{TEntry, TravelMode, TripSchedule};

/// Result of parsing an OUD/OUD2 file: a list of commands that reconstruct the
/// timetable in a WorldSnapshot.
pub fn parse_oud(
    content: &[u8],
) -> Result<Vec<Command>, String> {
    let root = parse_oud_to_ir(content).map_err(|e| format!("OUD parse error: {e:?}"))?;
    Ok(build_commands_from_oud(root))
}

pub fn parse_oud2(
    content: &str,
) -> Result<Vec<Command>, String> {
    let root = parse_oud2_to_ir(content).map_err(|e| format!("OUD2 parse error: {e:?}"))?;
    Ok(build_commands_from_oud(root))
}

fn build_commands_from_oud(root: paiagram_oudia::Root) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut station_map: HashMap<String, StationKey> = HashMap::new();
    let mut class_map: HashMap<String, ClassKey> = HashMap::new();
    let route = root.route;

    // Create stations
    for station in &route.stations {
        let sk = StationKey::new();
        station_map.insert(station.name.clone(), sk);
        let pos = LonLat { lon: 0, lat: 0 }; // OUD doesn't provide coordinates
        commands.push(Command::AddStation {
            key: sk,
            name: EcoString::from(station.name.as_str()),
            pos,
        });
    }

    // Create classes
    for class_info in &route.classes {
        let ck = ClassKey::new();
        let [_, r, g, b] = class_info.diagram_line_color.0;
        class_map.insert(class_info.name.clone(), ck);
        commands.push(Command::AddClass {
            key: ck,
            view: ClassView {
                name: EcoString::from(class_info.name.as_str()),
                style: StrokeStyle {
                    color: egui::Color32::from_rgb(r, g, b),
                    width: 1,
                },
            },
        });
    }

    // Create route
    let station_keys: Vec<StationKey> = route
        .stations
        .iter()
        .map(|s| station_map[&s.name])
        .collect();
    let rk = RouteKey::new();
    commands.push(Command::AddRoute {
        key: rk,
        view: RouteView {
            name: EcoString::from(route.name.as_str()),
            stations: station_keys.clone().into_iter().collect::<EcoVec<_>>(),
        },
    });

    // Create intervals between consecutive stations
    let travel_durations: Vec<Option<OuDiaTime>> = route.diagrams[0]
        .minimum_interval_durations(&route.stations)
        .collect();
    for i in 0..station_keys.len().saturating_sub(1) {
        let dist = travel_durations[i].map_or(1000, |t| (t.seconds() / 60 * 1000).max(1));
        let ik = IntervalKey::new();
        let from = station_keys[i];
        let to = station_keys[i + 1];
        commands.push(Command::AddInterval {
            key: ik,
            view: IntervalView {
                nodes: EcoVec::new(),
                length: NonZeroU32::new(dist as u32),
            },
            from: Some(from),
            to: Some(to),
        });
    }

    // Create trips for each diagram
    for diagram in route.diagrams.into_iter().take(1) {
        for trip in diagram.trips {
            let times: Vec<TimetableEntry> = trip
                .times
                .into_iter()
                .map(convert_timetable_entry)
                .collect();

            let class_key = class_map.get(
                &route.classes[trip.class_index].name,
            ).copied();

            let mut times_chunked: Vec<_> = times
                .into_iter()
                .enumerate()
                .filter_map(|(i, time)| {
                    if matches!(time.service_mode, ServiceMode::NoOperation) {
                        return None;
                    }
                    let station_index = match trip.direction {
                        Direction::Down => i,
                        Direction::Up => station_keys.len() - 1 - i,
                    };
                    let stn = station_keys[station_index];
                    Some((stn, time))
                })
                .chunk_by(|(s, _)| *s)
                .into_iter()
                .map(|(s, mut g)| {
                    let (_, first_time) = g.next().unwrap();
                    let mut group = [None; 2];
                    group[0] = first_time.arrival_time;
                    group[1] = first_time.departure_time;
                    if let Some((_, last_time)) = g.last() {
                        group[1] = last_time.departure_time;
                    }
                    (s, group, first_time.service_mode)
                })
                .collect();

            // Normalize times
            let mut all_times: Vec<&mut TimetableTime> = times_chunked
                .iter_mut()
                .flat_map(|(_, g, _)| g.iter_mut().filter_map(|t| t.as_mut()))
                .collect();
            normalize_times(&mut all_times);

            let entries: Vec<TEntry> = times_chunked
                .into_iter()
                .map(|(stn, [arrival_time, departure_time], passing_mode)| {
                    let arr = arrival_time.map(|t| TravelMode::At(t));
                    let dep = departure_time.map_or(TravelMode::Flexible, |t| TravelMode::At(t));
                    let is_pass = matches!(passing_mode, ServiceMode::Pass);
                    let same_time = arr.map_or(false, |a| a == dep);
                    if is_pass || same_time {
                        // Non-stop entry
                        TEntry::PinnedNonStop {
                            stn,
                            trk: 0,
                            pass: dep,
                            id: 0,
                        }
                    } else if let (Some(arr_mode), dep_mode) = (arr, dep) {
                        TEntry::Pinned {
                            stn,
                            trk: 0,
                            arr: arr_mode,
                            dep: dep_mode,
                            id: 0,
                        }
                    } else {
                        TEntry::Derived(stn)
                    }
                })
                .collect();

            let tk = TripKey::new();
            commands.push(Command::AddTrip {
                key: tk,
                view: TripView {
                    name: EcoString::from(
                        trip.name.as_deref().unwrap_or("<??>"),
                    ),
                    schedule: TripSchedule::new(entries.into_iter().collect::<EcoVec<_>>()),
                    class: class_key,
                },
            });
        }
    }

    commands
}

fn normalize_times(times: &mut [&mut TimetableTime]) {
    let mut prev: Option<TimetableTime> = None;
    for t in times.iter_mut() {
        if let Some(prev_t) = prev {
            while **t < prev_t {
                **t = TimetableTime((*t).0 + 86400);
            }
        }
        prev = Some(**t);
    }
}

struct TimetableEntry {
    service_mode: ServiceMode,
    arrival_time: Option<TimetableTime>,
    departure_time: Option<TimetableTime>,
}

fn convert_timetable_entry(entry: OuDiaTimetableEntry) -> TimetableEntry {
    TimetableEntry {
        service_mode: entry.service_mode,
        arrival_time: entry.arrival_time.map(|t| TimetableTime(t.seconds())),
        departure_time: entry.departure_time.map(|t| TimetableTime(t.seconds())),
    }
}
