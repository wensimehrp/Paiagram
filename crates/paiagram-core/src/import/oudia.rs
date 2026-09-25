use std::collections::HashMap;
use std::num::NonZeroU32;

use ecow::EcoVec;
use ecow::string::ToEcoString;
use paiagram_oudia::petgraph::visit::EdgeRef;
use paiagram_oudia::{Station as OudStation, StationToGraph, parse_oud_to_ir, parse_oud2_to_ir};
use rustc_hash::FxBuildHasher;
use smallvec::SmallVec;

use crate::graph::IntervalDirection;
use crate::time::TimetableTime;
use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::{
    Interval, LonLat, Node, NodeKey, ServiceClass, ServiceClassKey, Station, StationKey,
    StrokeStyle, Trip, TripKey, Wfc, WorldSnapshot,
};

pub(super) enum OudFileType<'a> {
    OuDiaSecond(&'a str),
    OuDia(&'a [u8]),
}

pub(crate) fn parse_oudia(
    stream: OudFileType,
) -> Result<WorldSnapshot, Box<dyn std::error::Error>> {
    let mut world = WorldSnapshot::default();
    let root = match stream {
        OudFileType::OuDiaSecond(s) => parse_oud2_to_ir(s)?,
        OudFileType::OuDia(buf) => parse_oud_to_ir(buf)?,
    };
    let route = root.route;
    let graph = route.stations.to_graph();
    let mut stn_to_node_key = HashMap::with_capacity_and_hasher(graph.node_count(), FxBuildHasher);
    for node in graph.node_weights().copied() {
        let node_key = NodeKey::new();
        let stn_key = StationKey::new();
        stn_to_node_key.insert(node as *const OudStation, node_key);
        world.stations.insert(
            stn_key,
            Wfc::new(Station {
                name: node.name.clone().into(),
                pos: LonLat::ZERO,
            }),
        );
        world.graph.insert_node(
            node_key,
            Node {
                name: "Platform 1".into(),
                parent: stn_key,
                pos: LonLat::ZERO,
            },
        );
    }
    for edge_ref in graph.edge_references() {
        let source = (*graph.node_weight(edge_ref.source()).unwrap()) as *const OudStation;
        let target = (*graph.node_weight(edge_ref.target()).unwrap()) as *const OudStation;
        let source = *stn_to_node_key.get(&source).unwrap();
        let target = *stn_to_node_key.get(&target).unwrap();
        world.graph.insert_interval(
            (source, target),
            Interval {
                nodes: EcoVec::new(),
                length: NonZeroU32::new(1000),
                direction: IntervalDirection::Both,
            },
        );
    }
    let mut service_classes = route
        .classes
        .iter()
        .map(|cls| {
            (
                cls.name.as_str(),
                ServiceClassKey::new(),
                0u32,
                cls.diagram_line_color.clone(),
            )
        })
        .collect::<Vec<_>>();
    for (name, key, _, color) in &service_classes {
        let style = StrokeStyle {
            color: egui::Color32::from_rgb(color.r(), color.g(), color.b()),
            width: 1,
        };
        world.service_classes.insert(
            *key,
            Wfc::new(ServiceClass {
                name: name.to_eco_string(),
                style,
            }),
        );
    }
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
            let node = *stn_to_node_key.get(&(stn as *const OudStation)).unwrap();
            let id = TEntryId::new();
            let arr_or_pass: TravelMode;
            let dep: Option<TravelMode>;
            match (entry.arrival_time, entry.departure_time) {
                (Some(at), Some(dt)) => {
                    arr_or_pass = TravelMode::At(TimetableTime::from_hms(0, 0, at.seconds()));
                    dep = Some(TravelMode::At(TimetableTime::from_hms(0, 0, dt.seconds())));
                }
                (Some(at), None) => {
                    arr_or_pass = TravelMode::At(TimetableTime::from_hms(0, 0, at.seconds()));
                    dep = None;
                }
                (None, Some(dt)) => {
                    let mode = TravelMode::At(TimetableTime::from_hms(0, 0, dt.seconds()));
                    if idx == 0 {
                        arr_or_pass = TravelMode::Flexible;
                        dep = Some(mode);
                    } else {
                        arr_or_pass = mode;
                        dep = None;
                    }
                }
                (None, None) => {
                    arr_or_pass = TravelMode::Flexible;
                    dep = None;
                }
            };
            buf.push(TEntry {
                node,
                arr_or_pass,
                dep,
                id,
            });
        }
        let (cls_name, cls_key, cls_counter) =
            service_classes.get_mut(trip.class_index).map_or_else(
                || ("Unknown Class", None, &mut unknown_class_counter),
                |(s, key, count, _)| (*s, Some(*key), count),
            );
        let name = trip.name.as_ref().map_or_else(
            || {
                format!("{} ({})", cls_name, {
                    *cls_counter += 1;
                    cls_counter
                })
                .into()
            },
            |n| n.into(),
        );
        world.trips.insert(
            TripKey::new(),
            Wfc::new(Trip {
                name,
                schedule: TripSchedule::new(buf),
                service_class: cls_key,
                vehicles: SmallVec::new(),
            }),
        );
    }
    Ok(world)
}

#[cfg(test)]
mod test {
    use super::*;
    type V = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_generate() -> V {
        let test_str = include_str!("../../../paiagram-oudia/test/sample.oud2");
        let snap = parse_oudia(OudFileType::OuDiaSecond(test_str))?;
        dbg!(snap);
        Ok(())
    }
}
