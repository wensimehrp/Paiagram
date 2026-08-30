use std::collections::HashMap;
use std::num::NonZeroU32;

use ecow::{EcoVec, eco_vec};
use paiagram_oudia::{Station, StationToGraph, parse_oud_to_ir, parse_oud2_to_ir};
use petgraph::visit::EdgeRef;
use rustc_hash::FxBuildHasher;
use smallvec::SmallVec;

use crate::time::TimetableTime;
use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::{
    Command, Distance, Interval, LonLat, NodeInfo, NodeKey, ServiceClassKey, StationInfo,
    StationKey, TripInfo, TripKey,
};

pub(super) enum OudFileType<'a> {
    OuDiaSecond(&'a str),
    OuDia(&'a [u8]),
}

pub(crate) fn parse_oudia(stream: OudFileType) -> Result<Command, Box<dyn std::error::Error>> {
    let root = match stream {
        OudFileType::OuDiaSecond(s) => parse_oud2_to_ir(s)?,
        OudFileType::OuDia(buf) => parse_oud_to_ir(buf)?,
    };
    let route = root.route;
    let mut cmd_buf: Vec<Command> = Vec::with_capacity(512);
    let graph = route.stations.to_graph();
    let mut stn_to_node_key = HashMap::with_capacity_and_hasher(graph.node_count(), FxBuildHasher);
    for node in graph.node_weights().copied() {
        let node_key = NodeKey::new();
        let stn_key = StationKey::new();
        stn_to_node_key.insert(node as *const Station, node_key);
        cmd_buf.extend_from_slice(&[
            Command::AddStation {
                key: stn_key,
                info: StationInfo {
                    name: node.name.clone().into(),
                    pos: LonLat::ZERO,
                },
            },
            Command::AddNode {
                key: node_key,
                info: NodeInfo {
                    name: "".into(),
                    parent: stn_key,
                    pos: LonLat::ZERO,
                    is_platform: true,
                },
            },
        ]);
    }
    for (source, target) in graph.edge_references().map(|e| {
        (
            *stn_to_node_key
                .get(&(*graph.node_weight(e.source()).unwrap() as *const Station))
                .unwrap(),
            *stn_to_node_key
                .get(&(*graph.node_weight(e.target()).unwrap() as *const Station))
                .unwrap(),
        )
    }) {
        cmd_buf.extend_from_slice(&[
            Command::AddInterval {
                key: (source, target),
                info: Interval {
                    nodes: eco_vec![],
                    length: NonZeroU32::new(1000),
                    trips: eco_vec![],
                },
            },
            Command::AddInterval {
                key: (target, source),
                info: Interval {
                    nodes: eco_vec![],
                    length: NonZeroU32::new(1000),
                    trips: eco_vec![],
                },
            },
        ]);
    }
    let mut service_classes = route
        .classes
        .iter()
        .map(|cls| (cls.name.as_str(), ServiceClassKey::new(), 0u32))
        .collect::<Vec<_>>();
    let mut unknown_class_counter = 0u32;
    let Some(diagram) = route.diagrams.get(0) else {
        return Err(Box::new(std::io::Error::other(
            "Route doesn't have a diagram!",
        )));
    };
    let deduplicated = route.stations.merge_duplicate();
    for (trip, schedule) in diagram.trip_station_times(&deduplicated) {
        let mut buf = EcoVec::new();
        for (idx, (stn, entry)) in schedule.enumerate() {
            let node = *stn_to_node_key.get(&(stn as *const Station)).unwrap();
            let id = TEntryId::new();
            let external = false;
            buf.push(match (entry.arrival_time, entry.departure_time) {
                (Some(at), Some(dt)) => TEntry::Pinned {
                    node,
                    arr: TravelMode::At(TimetableTime::from_hms(0, 0, at.seconds())),
                    dep: TravelMode::At(TimetableTime::from_hms(0, 0, dt.seconds())),
                    external,
                    id,
                },
                (Some(at), None) => TEntry::Pinned {
                    node,
                    arr: TravelMode::At(TimetableTime::from_hms(0, 0, at.seconds())),
                    dep: TravelMode::Flexible,
                    external,
                    id,
                },
                (None, Some(dt)) => {
                    let mode = TravelMode::At(TimetableTime::from_hms(0, 0, dt.seconds()));
                    if idx == 0 {
                        TEntry::Pinned {
                            node,
                            arr: TravelMode::Flexible,
                            dep: mode,
                            external,
                            id,
                        }
                    } else {
                        TEntry::PinnedNonStop {
                            node,
                            pass: mode,
                            external,
                            id,
                        }
                    }
                }
                (None, None) => TEntry::PinnedNonStop {
                    node,
                    pass: TravelMode::Flexible,
                    external,
                    id,
                },
            })
        }
        let (cls_name, cls_key, cls_counter) =
            service_classes.get_mut(trip.class_index).map_or_else(
                || ("Unknown Class", None, &mut unknown_class_counter),
                |(s, key, count)| (*s, Some(*key), count),
            );
        cmd_buf.push(Command::AddTrip {
            key: TripKey::new(),
            info: TripInfo {
                name: trip.name.as_ref().map_or_else(
                    || {
                        format!("{} ({})", cls_name, {
                            *cls_counter += 1;
                            cls_counter
                        })
                        .into()
                    },
                    |n| n.into(),
                ),
                schedule: TripSchedule::new(buf),
                service_class: cls_key,
                vehicles: SmallVec::new(),
            },
        });
    }
    Ok(Command::Macro(cmd_buf.into_boxed_slice()))
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
