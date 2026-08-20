use ecow::{EcoVec, eco_vec};
use egui::Color32;
use indexmap::IndexMap;
use paiagram_oudia::{Direction, ServiceMode, parse_oud_to_ir, parse_oud2_to_ir};
use smallvec::SmallVec;

use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::units::time::TimetableTime;
use crate::{
    Command, Interval, IntervalDirection, LonLat, NodeInfo, NodeKey, RouteInfo, RouteKey,
    ServiceClassInfo, ServiceClassKey, StationInfo, StationKey, StationRecord, TripInfo, TripKey,
};

struct OuDiaStationRecord {
    key: StationKey,
    // inbound
    kudari_arr_node: (LonLat, NodeKey),
    kudari_dep_node: (LonLat, NodeKey),
    // outbound
    nobori_arr_node: (LonLat, NodeKey),
    nobori_dep_node: (LonLat, NodeKey),
    tracks: Box<[NodeKey]>,
}

/// Emit an [`Command::AddNode`] and return the freshly created [`NodeKey`].
fn push_node(
    cmd_buf: &mut Vec<Command>,
    name: &str,
    parent: StationKey,
    is_platform: bool,
) -> NodeKey {
    let key = NodeKey::new();
    cmd_buf.push(Command::AddNode {
        key,
        info: NodeInfo {
            name: name.into(),
            parent,
            pos: LonLat::ZERO,
            is_platform,
        },
    });
    key
}

pub(super) enum OudFileType<'a> {
    OuDiaSecond(&'a str),
    OuDia(&'a [u8]),
}

pub(crate) fn parse_oudia(
    stream: OudFileType,
) -> Result<Box<[Command]>, Box<dyn std::error::Error>> {
    let root = match stream {
        OudFileType::OuDiaSecond(s) => parse_oud2_to_ir(s)?,
        OudFileType::OuDia(buf) => parse_oud_to_ir(buf)?,
    };
    let route = root.route;
    let mut cmd_buf: Vec<Command> = Vec::with_capacity(512);

    // Stations and their nodes. Nodes are created first so that the intervals
    // and trips below can reference them.
    let station_map: IndexMap<&str, OuDiaStationRecord> = {
        let mut map = IndexMap::with_capacity(route.stations.len());
        for stn in &route.stations {
            if map.contains_key(stn.name.as_str()) {
                continue;
            }
            let key = StationKey::new();
            let tracks = stn
                .tracks
                .iter()
                .map(|tr| push_node(&mut cmd_buf, tr.name.as_str(), key, true))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            cmd_buf.push(Command::AddStation {
                key,
                info: StationInfo {
                    name: (&stn.name).into(),
                    pos: LonLat::ZERO,
                },
            });
            let z = LonLat::ZERO; // to make rustfmt happy and keep stuff in one line
            let record = OuDiaStationRecord {
                key,
                kudari_arr_node: (z, push_node(&mut cmd_buf, "Kudari Arr", key, false)),
                kudari_dep_node: (z, push_node(&mut cmd_buf, "Kudari Dep", key, false)),
                nobori_arr_node: (z, push_node(&mut cmd_buf, "Nobori Arr", key, false)),
                nobori_dep_node: (z, push_node(&mut cmd_buf, "Nobori Dep", key, false)),
                tracks,
            };
            map.insert(stn.name.as_str(), record);
        }
        map
    };
    let station_list: Vec<(&str, &OuDiaStationRecord)> = route
        .stations
        .iter()
        .map(|s| {
            let name = s.name.as_str();
            let record = station_map.get(name).unwrap();
            (name, record)
        })
        .collect();

    // Connect consecutive stations with one interval per direction.
    for [(_, curr), (_, next)] in station_list.array_windows::<2>() {
        cmd_buf.push(Command::AddInterval {
            key: (curr.kudari_dep_node.1, next.kudari_arr_node.1),
            info: Interval {
                nodes: eco_vec![curr.kudari_dep_node.0, next.kudari_arr_node.0],
                length: None,
                direction: IntervalDirection::OneWay,
                trips: EcoVec::new(),
            },
        });
        cmd_buf.push(Command::AddInterval {
            key: (next.nobori_dep_node.1, curr.nobori_arr_node.1),
            info: Interval {
                nodes: eco_vec![next.nobori_dep_node.0, curr.nobori_arr_node.0],
                length: None,
                direction: IntervalDirection::OneWay,
                trips: EcoVec::new(),
            },
        });
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

    cmd_buf.push(Command::AddRoute {
        key: RouteKey::new(),
        info: RouteInfo {
            name: route.name.into(),
            stations: station_list
                .iter()
                .map(|(_, record)| (StationRecord::All(record.key), None))
                .collect(),
        },
    });

    // Trips. Only the first diagram is imported for now.
    if let Some(diagram) = route.diagrams.into_iter().next() {
        for trip in diagram.trips {
            let name = trip.name.as_deref().unwrap_or("<??>");
            let class = service_class_keys.get(trip.class_index).copied();
            let direction = trip.direction;

            // Gather the stops (skipping stations without service) and their nodes.
            let mut stops: Vec<(NodeKey, [Option<TimetableTime>; 2], ServiceMode)> = Vec::new();
            for (i, time) in trip.times.iter().enumerate() {
                if matches!(time.service_mode, ServiceMode::NoOperation) {
                    continue;
                }
                let station_index = match direction {
                    Direction::Down => i,
                    Direction::Up => station_list.len() - 1 - i,
                };
                let (_, record) = station_list[station_index];
                let node = match direction {
                    Direction::Down => record.kudari_dep_node.1,
                    Direction::Up => record.nobori_dep_node.1,
                };
                let arrival = time.arrival_time.map(|t| TimetableTime(t.seconds()));
                let departure = time.departure_time.map(|t| TimetableTime(t.seconds()));
                stops.push((node, [arrival, departure], time.service_mode));
            }

            // Wrap times that cross midnight so they remain monotonic.
            super::normalize_times(stops.iter_mut().flat_map(|(_, g, _)| g).flatten());

            let entries: EcoVec<TEntry> = stops
                .into_iter()
                .map(|(node, [arrival, departure], mode)| {
                    let id = TEntryId::new();
                    match mode {
                        ServiceMode::Stop => TEntry::Pinned {
                            node,
                            arr: arrival.map_or(TravelMode::Flexible, TravelMode::At),
                            dep: departure.map_or(TravelMode::Flexible, TravelMode::At),
                            external: false,
                            id,
                        },
                        ServiceMode::Pass => TEntry::PinnedNonStop {
                            node,
                            pass: departure.map_or(TravelMode::Flexible, TravelMode::At),
                            external: false,
                            id,
                        },
                        ServiceMode::NoOperation => unreachable!(),
                    }
                })
                .collect();

            cmd_buf.push(Command::AddTrip {
                key: TripKey::new(),
                info: TripInfo {
                    name: name.into(),
                    schedule: TripSchedule::new(entries),
                    service_class: class,
                    vehicles: SmallVec::new(),
                },
            });
        }
    }

    Ok(cmd_buf.into_boxed_slice())
}

#[cfg(test)]
mod test {
    use super::*;
    type V = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_generate() -> V {
        let test_str = include_str!("../../../paiagram-oudia/test/sample.oud2");
        let commands = parse_oudia(OudFileType::OuDiaSecond(test_str))?;
        dbg!(commands);
        Ok(())
    }
}
