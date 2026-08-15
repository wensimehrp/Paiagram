use std::collections::HashMap;

use egui::Color32;
use paiagram_oudia::{parse_oud_to_ir, parse_oud2_to_ir};

use crate::{
    Command, LonLat, NodeInfo, NodeKey, RouteInfo, RouteKey, ServiceClassInfo, ServiceClassKey,
    StationInfo, StationKey, StationRecord,
};

// #[derive(Debug, Clone, Copy)]
// struct TimetableEntry {
//     service_mode: ServiceMode,
//     arrival_time: Option<TimetableTime>,
//     departure_time: Option<TimetableTime>,
// }
//
// impl From<OuDiaTime> for TimetableTime {
//     fn from(value: OuDiaTime) -> Self {
//         Self(value.seconds())
//     }
// }

pub(crate) fn load_oud(
    mut stream: impl std::io::Read,
    is_oudia: bool,
) -> Result<Box<[Command]>, Box<dyn std::error::Error>> {
    let root = if is_oudia {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        parse_oud_to_ir(&buf)?
    } else {
        let mut buf = String::new();
        stream.read_to_string(&mut buf)?;
        parse_oud2_to_ir(&buf)?
    };
    let route = root.route;
    let mut cmd_buf: Vec<Command> = Vec::with_capacity(512);
    let mut station_map: HashMap<&str, (StationKey, Vec<NodeKey>)> = HashMap::new();
    let mut station_list = Vec::new();
    for (i, station) in route.stations.iter().enumerate() {
        let station_key = StationKey::new();
        let cmd = Command::AddStation {
            key: station_key,
            info: StationInfo {
                name: (&station.name).into(),
                pos: LonLat { lon: 0, lat: 0 },
            },
        };
        station_list.push(station_key);
        cmd_buf.push(cmd);
        let mut track_keys: Vec<NodeKey> = Vec::with_capacity(station.tracks.len());
        for track in &station.tracks {
            let track_key = NodeKey::new();
            cmd_buf.push(Command::AddNode {
                key: track_key,
                info: NodeInfo {
                    name: (&track.name).into(),
                    parent: station_key,
                    pos: LonLat { lon: 0, lat: 0 },
                    is_platform: true,
                },
            });
            track_keys.push(track_key);
        }
        station_map.insert(&station.name, (station_key, track_keys));
    }

    let service_class_keys: Vec<ServiceClassKey> = route
        .classes
        .iter()
        .map(|it| {
            let key = ServiceClassKey::new();
            let [_, r, g, b] = it.diagram_line_color.0;
            cmd_buf.push(Command::AddServiceClass {
                key,
                info: ServiceClassInfo {
                    name: (&it.name).into(),
                    style: crate::StrokeStyle {
                        color: Color32::from_rgb(r, g, b),
                        width: 1,
                    },
                },
            });
            key
        })
        .collect();
    // let travel_durations: Vec<Option<OuDiaTime>> = route.diagrams[0]
    //     .minimum_interval_durations(&route.stations)
    //     .collect();

    cmd_buf.push(Command::AddRoute {
        key: RouteKey::new(),
        info: RouteInfo {
            name: route.name.into(),
            stations: station_list
                .into_iter()
                .map(|k| (StationRecord::All(k), None))
                .collect(),
        },
    });

    Ok(cmd_buf.into_boxed_slice())

    // for i in 0..station_instances.len().saturating_sub(1) {
    //     // if break_flags[i] {
    //     //     continue;
    //     // }
    //     super::add_interval_pair(
    //         &mut graph,
    //         &mut commands,
    //         station_instances[i].entity(),
    //         station_instances[i + 1].entity(),
    //         travel_durations[i].map_or(Distance::from_m(1000), |it| {
    //             Distance::from_m(it.seconds() / 60 * 1000)
    //         }),
    //     );
    // }

    // // TODO: find a method to support multiple diagrams
    // for diagram in route.diagrams.into_iter().take(1) {
    //     for trip in diagram.trips {
    //         let times: Vec<TimetableEntry> = trip
    //             .times
    //             .into_iter()
    //             .map(convert_timetable_entry)
    //             .collect();

    //         let trip_class = class_instances[trip.class_index];

    //         let mut times_chunked: Vec<_> = times
    //             .into_iter()
    //             .enumerate()
    //             .filter_map(|(i, time)| {
    //                 if matches!(time.service_mode, ServiceMode::NoOperation) {
    //                     return None;
    //                 }
    //                 let station_index = match trip.direction {
    //                     Direction::Down => i,
    //                     Direction::Up => station_instances.len() - 1 - i,
    //                 };
    //                 let stop = station_instances[station_index];
    //                 Some((stop, time))
    //             })
    //             .chunk_by(|(s, _t)| *s)
    //             .into_iter()
    //             .map(|(s, mut g)| {
    //                 let (_, first_time) = g.next().unwrap();
    //                 let mut group = [None; 2];
    //                 group[0] = first_time.arrival_time;
    //                 group[1] = first_time.departure_time;
    //                 if let Some((_, last_time)) = g.last() {
    //                     group[1] = last_time.departure_time;
    //                 }
    //                 (s, group, first_time.service_mode)
    //             })
    //             .collect();

    //         super::normalize_times(times_chunked.iter_mut().flat_map(|(_, g, _)| g).flatten());

    //         let nominal_entries: Vec<_> = times_chunked
    //             .into_iter()
    //             .map(|(stop, [arrival_time, departure_time], passing_mode)| {
    //                 // in this case, this would consume the iterator.
    //                 let arrival_mode = if matches!(passing_mode, ServiceMode::Pass) {
    //                     None
    //                 } else {
    //                     Some(arrival_time.map_or(TravelMode::Flexible, |t| TravelMode::At(t)))
    //                 };
    //                 let departure_mode =
    //                     departure_time.map_or(TravelMode::Flexible, |t| TravelMode::At(t));
    //                 commands
    //                     .spawn(EntryBundle::new(
    //                         arrival_mode,
    //                         departure_mode,
    //                         stop.entity(),
    //                     ))
    //                     .id()
    //             })
    //             .collect();

    //         commands
    //             .spawn_empty()
    //             .add_children(&nominal_entries)
    //             .insert(TripBundle::new(
    //                 &trip.name.unwrap_or("<??>".to_string()),
    //                 TripClass(trip_class.entity()),
    //                 nominal_entries,
    //             ));
    //     }
    // }
}

// fn convert_timetable_entry(entry: OuDiaTimetableEntry) -> TimetableEntry {
//     TimetableEntry {
//         service_mode: entry.service_mode,
//         arrival_time: entry.arrival_time.map(TimetableTime::from),
//         departure_time: entry.departure_time.map(TimetableTime::from),
//     }
// }
