use std::{any::Any, collections::BTreeMap, iter::zip};

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "rw_data/oudiasecond.pest"]
pub struct OuDiaSecondParser;

#[derive(Debug)]
enum OuDiaSecondStruct<'a> {
    Struct(&'a str, Vec<OuDiaSecondStruct<'a>>),
    Pair(&'a str, OuDiaSecondValue<'a>),
}

#[derive(Debug)]
enum OuDiaSecondValue<'a> {
    Single(&'a str),
    List(Vec<&'a str>),
}

use pest::error::Error;

use crate::{
    basic::TimetableTime,
    vehicle_set::VehicleSet,
    vehicles::{ArrivalType, DepartureType, TimetableEntry},
};

fn parse_oud2_to_ast(file: &str) -> Result<OuDiaSecondStruct<'_>, Error<Rule>> {
    let oud2 = OuDiaSecondParser::parse(Rule::file, file)?
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();
    use pest::iterators::Pair;
    fn parse_struct(pair: Pair<Rule>) -> OuDiaSecondStruct {
        match pair.as_rule() {
            Rule::r#struct => {
                let mut inner = pair.into_inner();
                let name = inner.next().unwrap().as_str();
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OuDiaSecondStruct::Struct(name, fields)
            }
            Rule::wrapper => {
                let inner = pair.into_inner();
                let name = "file";
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OuDiaSecondStruct::Struct(name, fields)
            }
            Rule::kvpair => {
                let mut inner = pair.into_inner();
                let key = inner.next().unwrap().as_str();
                let val = inner.next().unwrap();
                let val = match val.as_rule() {
                    Rule::value => OuDiaSecondValue::Single(val.as_str()),
                    Rule::list => {
                        let list_vals = val.into_inner().map(|v| v.as_str()).collect();
                        OuDiaSecondValue::List(list_vals)
                    }
                    _ => unreachable!(),
                };
                OuDiaSecondStruct::Pair(key, val)
            }
            _ => unreachable!(),
        }
    }
    Ok(parse_struct(oud2))
}

struct OUD2Root {
    file_type: String,
    line: OUD2Line,
}

struct OUD2Line {
    name: String,
    // those are accessed by the index later, by the trains
    stations: Vec<OUD2Station>,
    // classes: Vec<OUD2Class>,
    diagrams: Vec<OUD2Diagram>,
}

struct OUD2Station {
    name: String,
    branch_index: Option<usize>, // tracks
}

struct OUD2Diagram {
    name: String,
    services: Vec<OUD2Service>,
    // vehicles are not yet implemented
}

struct OUD2Service {
    reverse: bool,
    name: String,
    timetable: Vec<OUD2TimetableEntry>,
}

#[derive(Debug, Clone, Copy)]
enum OUD2OperationType {
    After,
    Before,
}

/// How a train would service a station
enum OUD2ServiceMode {
    /// This train does not pass the station at all
    NoPassing,
    /// This train makes a stop at the station
    Stop,
    /// This train passes the station, but does not stop
    NonStop,
}

struct OUD2TimetableEntry {
    service_mode: OUD2ServiceMode,
    arrival: ArrivalType,
    departure: DepartureType,
    track: Option<nonmax::NonMaxUsize>,
    operations: Option<
        // A station entry could optionally have a set of operations attached
        Vec<(
            Vec<(Option<nonmax::NonMaxUsize>, OUD2OperationType)>,
            Vec<Vec<String>>,
        )>,
    >,
}

fn parse_oud2(file: &str) -> Result<OUD2Root, String> {
    let ast = parse_oud2_to_ast(file).map_err(|e| e.to_string())?;
    parse_root(ast)
}

fn parse_root(input: OuDiaSecondStruct) -> Result<OUD2Root, String> {
    let input = match input {
        OuDiaSecondStruct::Struct(_, vals) => vals,
        _ => {
            return Err("Invalid OuDiaSecond root structure".to_string());
        }
    };
    let mut file_type: Option<String> = None;
    let mut line: Option<Vec<OuDiaSecondStruct>> = None;
    use OuDiaSecondStruct::*;
    for entry in input {
        match entry {
            Pair("FileType", OuDiaSecondValue::Single(ft)) => {
                if file_type.is_none() {
                    file_type = Some(ft.to_string());
                }
            }
            Struct("Rosen", val) => {
                if line.is_none() {
                    line = Some(val);
                }
            }
            _ => {}
        }
        if file_type.is_some() && line.is_some() {
            return Ok(OUD2Root {
                file_type: file_type.unwrap(),
                line: parse_line(line.unwrap())?,
            });
        }
    }
    Err("Failed to parse OuDiaSecond file: missing `FileType` or `Rosen` field(s)".to_string())
}

fn parse_line(input: Vec<OuDiaSecondStruct>) -> Result<OUD2Line, String> {
    let mut name: Option<String> = None;
    let mut stations: Vec<OUD2Station> = Vec::new();
    let mut diagram: Vec<Vec<OuDiaSecondStruct>> = Vec::new();
    use OuDiaSecondStruct::*;
    for entry in input {
        match entry {
            Pair("Rosenmei", OuDiaSecondValue::Single(n)) => {
                if name.is_none() {
                    name = Some(n.to_string());
                }
            }
            Struct("Eki", vals) => {
                let mut station_name = None;
                let mut branch_index = None;
                for station_entry in vals {
                    match station_entry {
                        Pair("Ekimei", OuDiaSecondValue::Single(n)) => {
                            station_name = Some(n.to_string());
                        }
                        Pair("BrunchCoreEkiIndex", OuDiaSecondValue::Single(idx)) => {
                            branch_index = idx.parse::<usize>().ok();
                        }
                        _ => {}
                    }
                    if station_name.is_some() && branch_index.is_some() {
                        break;
                    }
                }
                stations.push(OUD2Station {
                    name: station_name.unwrap_or_default(),
                    branch_index,
                });
            }
            Struct("Dia", vals) => {
                info!("Found diagram with {} entries", vals.len());
                diagram.push(vals);
            }
            _ => {}
        }
    }
    if name.is_some() {
        return Ok(OUD2Line {
            name: name.unwrap(),
            stations,
            diagrams: diagram
                .into_iter()
                .map(parse_diagram)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Err("Failed to parse OuDiaSecond line: incomplete information".to_string())
}

fn parse_diagram(input: Vec<OuDiaSecondStruct>) -> Result<OUD2Diagram, String> {
    let mut name: Option<String> = None;
    let mut services: Vec<OUD2Service> = Vec::new();
    use OuDiaSecondStruct::*;
    for entry in input {
        match entry {
            Pair("DiaName", OuDiaSecondValue::Single(n)) => {
                if name.is_none() {
                    name = Some(n.to_string());
                }
            }
            Struct("Kudari", vals) => {
                services.extend(parse_services(vals, false)?);
            }
            Struct("Nobori", vals) => {
                services.extend(parse_services(vals, true)?);
            }
            _ => {}
        }
    }
    if name.is_some() {
        return Ok(OUD2Diagram {
            name: name.unwrap(),
            services,
        });
    }
    Err("Failed to parse OuDiaSecond diagram: missing name".to_string())
}

fn parse_services(
    input: Vec<OuDiaSecondStruct>,
    reverse: bool,
) -> Result<Vec<OUD2Service>, String> {
    use OuDiaSecondStruct::*;
    let mut services = Vec::new();
    for entry in input {
        let Struct("Ressya", vals) = entry else {
            continue;
        };
        let service = parse_service(vals, reverse)?;
        let Some(service) = service else {
            continue;
        };
        services.push(service);
    }
    Ok(services)
}

fn parse_service(
    input: Vec<OuDiaSecondStruct>,
    reverse: bool,
) -> Result<Option<OUD2Service>, String> {
    let mut name: Option<String> = None;
    let mut timetable: Option<Vec<OUD2TimetableEntry>> = None;
    let mut operations: Vec<(
        Vec<(nonmax::NonMaxUsize, OUD2OperationType)>,
        Vec<Vec<String>>,
    )> = Vec::new();
    use OuDiaSecondStruct::*;
    for entry in input {
        match entry {
            Pair("Ressyabangou", OuDiaSecondValue::Single(n)) => {
                if name.is_none() {
                    name = Some(n.to_string());
                }
            }
            Pair("EkiJikoku", v) => {
                let times = match v {
                    OuDiaSecondValue::Single(s) => vec![s],
                    OuDiaSecondValue::List(l) => l,
                };
                if timetable.is_none() {
                    timetable = Some(
                        times
                            .iter()
                            .map(|t| parse_timetable_entry(t))
                            .collect::<Result<_, _>>()?,
                    );
                }
            }
            Pair(n, v) if n.starts_with("Operation") => {
                // remove the word Operation from the name
                let tree = n
                    .strip_prefix("Operation")
                    .unwrap()
                    .split(".")
                    .map(|s| {
                        // always in the form of <number><A|B>
                        let (num, operation_type) = s.split_at(s.len() - 1);
                        let operation_type = match operation_type {
                            "A" => OUD2OperationType::After,
                            "B" => OUD2OperationType::Before,
                            _ => unreachable!(),
                        };
                        (num.parse::<nonmax::NonMaxUsize>().unwrap(), operation_type)
                    })
                    .collect::<Vec<_>>();
                let ops = match v {
                    OuDiaSecondValue::Single(s) => vec![s],
                    OuDiaSecondValue::List(l) => l,
                };
                let ops = ops
                    .iter()
                    .map(|s| {
                        OuDiaSecondParser::parse(Rule::event, s)
                            .unwrap()
                            .next()
                            .unwrap()
                            .into_inner()
                            .map(|p| p.as_str().to_string())
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<_>>();
                operations.push((tree, ops));
            }
            _ => {}
        }
    }
    if timetable.is_some() {
        return Ok(Some(OUD2Service {
            reverse,
            name: name.unwrap_or("<unnamed>".to_string()),
            timetable: {
                // modify the timetable with operations
                let mut timetable = timetable.unwrap();
                for (tree, ops) in operations {
                    // the index is always the first element of the tree
                    let mut tree = tree.iter();
                    let (index, op_type) = tree.next().unwrap();
                    let idx = nonmax::NonMaxUsize::get(index);
                    match tree.next() {
                        None => {
                            // apply to single entry
                            if let Some(entry) = timetable.get_mut(idx) {
                                entry
                                    .operations
                                    .get_or_insert_default()
                                    .push((vec![(None, *op_type)], ops));
                            }
                        }
                        Some(entry) => {
                            let mut remainder = vec![entry];
                            remainder.extend(tree);
                            if let Some(entry) = timetable.get_mut(idx) {
                                entry.operations.get_or_insert_default().push((
                                    {
                                        let mut returned_remainder = vec![(None, *op_type)];
                                        returned_remainder.extend(
                                            remainder.iter().map(|(idx, op)| (Some(*idx), *op)),
                                        );
                                        returned_remainder
                                    },
                                    ops,
                                ));
                            }
                        }
                    }
                }
                timetable
            },
        }));
    }
    Ok(None)
}

#[inline(always)]
fn parse_timetable_entry(input: &str) -> Result<OUD2TimetableEntry, String> {
    let mut service_mode = OUD2ServiceMode::NoPassing;
    let mut arrival = ArrivalType::Flexible;
    let mut departure = DepartureType::NonStop;
    let mut track = None;
    if input.is_empty() {
        return Ok(OUD2TimetableEntry {
            service_mode,
            arrival,
            departure,
            operations: None,
            track,
        });
    }
    let parsed = OuDiaSecondParser::parse(Rule::timetable_entry, input)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    for pair in parsed {
        match pair.as_rule() {
            Rule::service_mode => match pair.as_str() {
                "0" => service_mode = OUD2ServiceMode::NoPassing,
                "1" => service_mode = OUD2ServiceMode::Stop,
                "2" => service_mode = OUD2ServiceMode::NonStop,
                _ => return Err(format!("Unknown service mode: {}", pair.as_str())),
            },
            Rule::arrival => {
                arrival = match TimetableTime::from_oud2_str(pair.as_str()) {
                    Some(t) => ArrivalType::At(t),
                    None => return Err(format!("Failed to parse arrival time: {}", pair.as_str())),
                };
            }
            Rule::departure => {
                departure = match TimetableTime::from_oud2_str(pair.as_str()) {
                    Some(t) => DepartureType::At(t),
                    None => {
                        return Err(format!("Failed to parse departure time: {}", pair.as_str()));
                    }
                };
            }
            Rule::track => {
                track = pair.as_str().parse::<nonmax::NonMaxUsize>().ok();
            }
            _ => unreachable!(),
        }
    }
    // swap arrival and departure iff arrival is of flexible type and departure is of at type
    match (&arrival, &departure) {
        (ArrivalType::Flexible, DepartureType::At(time)) => {
            arrival = ArrivalType::At(*time);
            departure = DepartureType::Flexible;
        }
        _ => {}
    }
    Ok(OUD2TimetableEntry {
        service_mode,
        arrival,
        departure,
        operations: None,
        track,
    })
}

use super::ModifyData;
use crate::basic::*;
use crate::intervals::*;
use crate::vehicles::*;
use bevy::prelude::{Commands, Entity, MessageReader, Name, Res, info};
use petgraph::graphmap::GraphMap;

/// A pool of vehicles available at each station and track.
/// This is because OuDiaSecond uses a service based model, while Paiagram uses a vehicle based model.
/// OuDiaSecond does not keep track of which vehicle is assigned to which service directly, rather, each service would
/// have events at different stations and tracks. This structure helps to map services to vehicles.
#[rustfmt::skip]
type StationVehicleSchedulePool =
    Vec< // stations
    Vec< // tracks
    BTreeMap< // train components
    TimetableTime, Vec< // The schedule. The TimetableTime is the last entry's arrival/departure time, whichever is later
    Entity>>>> // each available vehicle entity
;

pub fn load_oud2(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    intervals_resource: Res<IntervalsResource>,
) {
    let mut data: Option<&str> = None;
    for modification in reader.read() {
        let ModifyData::LoadOuDiaSecond(d) = modification else {
            continue;
        };
        data = Some(d);
    }
    let Some(data) = data else {
        return;
    };
    let now = instant::Instant::now();
    let oud2_data = parse_oud2(data).unwrap();
    info!("Parsed OuDiaSecond data in {:?}", now.elapsed());
    // save the kdl info to "parsed.kdl"
    #[cfg(not(target_arch = "wasm32"))]
    {
        // TODO: remove this later
        let kdl_string = make_kdl(&parse_oud2_to_ast(data).unwrap());
        std::fs::write("parsed.kdl", kdl_string).unwrap();
    }
    let now = instant::Instant::now();
    let (graph_map, stations) = make_graph_map(&mut commands, &oud2_data.line.stations);
    commands.insert_resource(Graph(graph_map));
    for diagram in oud2_data.line.diagrams {
        make_vehicle_set(
            &mut commands,
            diagram,
            &stations,
            intervals_resource.default_depot,
        );
    }
    info!("Loaded OUD2 data in {:?}", now.elapsed());
}

fn make_vehicle_set(
    commands: &mut Commands,
    diagram: OUD2Diagram,
    stations: &Vec<Entity>,
    depot: Entity,
) -> Entity {
    let vehicle_set_entity = commands.spawn((VehicleSet, Name::new(diagram.name))).id();
    // collect, sort, then organize
    let mut station_vehicle_schedule_pool: StationVehicleSchedulePool =
        vec![Vec::new(); stations.len()];
    let mut service_entities: Vec<Entity> = Vec::new();
    for service in diagram.services.iter() {
        let service_entity = commands
            .spawn((Name::new(service.name.clone()), Service { class: None }))
            .id();
        service_entities.push(service_entity);
        let mut service_schedule: Vec<Entity> = Vec::new();
        let mut first_stop_info: Option<(usize, usize, TimetableTime)> = None;
        for (entry_index, timetable_entry) in service.timetable.iter().enumerate() {
            if matches!(timetable_entry.service_mode, OUD2ServiceMode::NoPassing) {
                continue;
            }
            let station_index = if service.reverse {
                stations.len() - 1 - entry_index
            } else {
                entry_index
            };
            let track_index = timetable_entry
                .track
                .and_then(|v| Some(v.get()))
                .unwrap_or(0);
            if first_stop_info.is_none() {
                first_stop_info = Some((
                    station_index,
                    track_index,
                    timetable_entry.arrival.time().unwrap(),
                ))
            }
            let entry_entity = commands
                .spawn(TimetableEntry {
                    arrival: timetable_entry.arrival,
                    departure: timetable_entry.departure,
                    service: Some(service_entity),
                    track: None,
                    station: stations[station_index],
                })
                .id();
            service_schedule.push(entry_entity)
        }
        let Some((station_index, track_index, first_stop_arrival_time)) = first_stop_info else {
            continue;
        };
        let station_pool = station_vehicle_schedule_pool
            .get_mut(station_index)
            .unwrap();
        while station_pool.len() <= track_index {
            station_pool.push(BTreeMap::new())
        }
        station_pool[track_index].insert(first_stop_arrival_time, service_schedule);
    }
    info!(?station_vehicle_schedule_pool);
    for (service, service_entity) in zip(diagram.services, service_entities) {
        // TODO
    }
    vehicle_set_entity
}

/// Creates the graph map from the list of stations in the OuDiaSecond data.
fn make_graph_map(
    commands: &mut Commands,
    oud2_stations: &Vec<OUD2Station>,
) -> (IntervalGraphType, Vec<Entity>) {
    let mut stations: Vec<Entity> = Vec::with_capacity(oud2_stations.len());
    let mut prev_entity = None;
    let mut graph_map: IntervalGraphType = GraphMap::new();
    for (_ci, curr_station) in oud2_stations.iter().enumerate() {
        let branch_index = curr_station.branch_index;
        let station_entity;
        station_entity = commands
            .spawn((Name::new(curr_station.name.clone()), Station))
            .id();
        if let Some(prev) = prev_entity
            && branch_index.is_none()
        {
            let interval_entity = commands
                .spawn(Interval {
                    // OuDiaSecond does not provide length or speed limit info
                    length: TrackDistance::from_km(1),
                    speed_limit: None,
                })
                .id();
            graph_map.add_edge(prev, station_entity, interval_entity);
        }
        stations.push(station_entity);
        prev_entity = Some(station_entity);
    }
    (graph_map, stations)
}

/// Converts the OuDiaSecond AST back into KDL format for debugging purposes.
fn make_kdl(oud2_root: &OuDiaSecondStruct) -> String {
    fn to_kdl_value(raw: &str) -> KdlValue {
        KdlValue::String(raw.trim().to_string())
    }

    fn to_kdl_node(node: &OuDiaSecondStruct) -> KdlNode {
        match node {
            OuDiaSecondStruct::Struct(name, fields) => {
                let mut kdl_node = KdlNode::new(*name);
                if !fields.is_empty() {
                    let mut children = KdlDocument::new();
                    for field in fields {
                        children.nodes_mut().push(to_kdl_node(field));
                    }
                    kdl_node.set_children(children);
                }
                kdl_node
            }
            OuDiaSecondStruct::Pair(key, value) => {
                let mut kdl_node = KdlNode::new(*key);
                match value {
                    OuDiaSecondValue::Single(val) => {
                        kdl_node.push(KdlEntry::new(to_kdl_value(val)));
                    }
                    OuDiaSecondValue::List(vals) => {
                        for val in vals {
                            kdl_node.push(KdlEntry::new(to_kdl_value(val)));
                        }
                    }
                }
                kdl_node
            }
        }
    }

    let mut document = KdlDocument::new();
    document.nodes_mut().push(to_kdl_node(oud2_root));
    document.autoformat();
    document.to_string()
}
