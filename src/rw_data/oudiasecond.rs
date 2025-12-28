use crate::{
    intervals::{Graph, Interval},
    rw_data::ModifyData,
    units::{
        distance::Distance,
        time::{Duration, TimetableTime},
    },
    vehicles::{
        Vehicle,
        entries::{TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};
use bevy::{ecs::system::command, prelude::*};
use pest::Parser;
use pest_derive::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[grammar = "rw_data/oudiasecond.pest"]
pub struct OUD2Parser;

#[derive(Debug)]
enum Structure<'a> {
    Struct(&'a str, Vec<Structure<'a>>),
    Pair(&'a str, Value<'a>),
}

#[derive(Debug)]
enum Value<'a> {
    Single(&'a str),
    List(Vec<&'a str>),
}

fn parse_oud2_to_ast(file: &str) -> Result<Structure<'_>, pest::error::Error<Rule>> {
    let oud2 = OUD2Parser::parse(Rule::file, file)?
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();
    use pest::iterators::Pair;
    fn parse_struct(pair: Pair<Rule>) -> Structure {
        match pair.as_rule() {
            Rule::r#struct => {
                let mut inner = pair.into_inner();
                let name = inner.next().unwrap().as_str();
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                Structure::Struct(name, fields)
            }
            Rule::wrapper => {
                let inner = pair.into_inner();
                let name = "file";
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                Structure::Struct(name, fields)
            }
            Rule::kvpair => {
                let mut inner = pair.into_inner();
                let key = inner.next().unwrap().as_str();
                let val = inner.next().unwrap();
                let val = match val.as_rule() {
                    Rule::value => Value::Single(val.as_str()),
                    Rule::list => {
                        let list_vals = val.into_inner().map(|v| v.as_str()).collect();
                        Value::List(list_vals)
                    }
                    _ => unreachable!(),
                };
                Structure::Pair(key, val)
            }
            _ => unreachable!(),
        }
    }
    Ok(parse_struct(oud2))
}

pub fn load_oud2(
    mut commands: Commands,
    mut reader: MessageReader<ModifyData>,
    mut graph: ResMut<Graph>,
) {
    graph.clear();
    let mut str: Option<&str> = None;
    for modification in reader.read() {
        match modification {
            ModifyData::LoadOuDiaSecond(s) => str = Some(s.as_str()),
            _ => {}
        }
    }
    let Some(str) = str else {
        return;
    };
    info!("Loading OUD2 data...");
    let ast = parse_oud2_to_ast(str).expect("Failed to parse OUD2 file");
    let root = parse_ast(&ast).expect("Failed to convert OUD2 AST to internal representation");
    for line in root.lines {
        let mut stations: Vec<(Entity, bool)> = Vec::new();
        for station in line.stations.iter() {
            let station_entity = commands
                .spawn((
                    Name::new(station.name.clone()),
                    crate::intervals::Station::default(),
                ))
                .id();
            stations.push((station_entity, station.break_interval));
        }
        for (i, station) in line.stations.iter().enumerate() {
            if let Some(branch_index) = station.branch_index {
                let (e, _) = stations[i];
                commands.entity(e).despawn();
                stations[i].0 = stations[branch_index].0;
            }
            if let Some(loop_index) = station.loop_index {
                let (e, _) = stations[i];
                commands.entity(e).despawn();
                stations[i].0 = stations[loop_index].0;
            }
        }
        for w in stations.windows(2) {
            let [(prev, break_interval), (next, _)] = w else { continue };
            // create new intervals
            if *break_interval {
                continue;
            }
            let e1 = commands
                .spawn(Interval {
                    length: Distance::from_m(1000),
                    speed_limit: None,
                })
                .id();
            let e2 = commands
                .spawn(Interval {
                    length: Distance::from_m(1000),
                    speed_limit: None,
                })
                .id();
            graph.add_edge(*prev, *next, e1);
            graph.add_edge(*next, *prev, e2);
        }
        for diagram in line.diagrams {
            let vehicle_set_entity = commands.spawn((Name::new(diagram.name), VehicleSet)).id();
            for train in diagram.trains {
                let vehicle_entity = commands.spawn((Vehicle, Name::new(train.name))).id();
                let mut schedule = Vec::new();
                commands
                    .entity(vehicle_set_entity)
                    .add_child(vehicle_entity);
                for (i, time) in train.times.into_iter().enumerate() {
                    let Some(time) = time else {
                        continue;
                    };
                    if matches!(time.passing_mode, PassingMode::NoOperation) {
                        continue;
                    }
                    let arrival = if matches!(time.passing_mode, PassingMode::Pass) {
                        TravelMode::Flexible
                    } else {
                        time.arrival
                            .map_or(TravelMode::Flexible, |t| TravelMode::At(t))
                    };
                    let departure = if matches!(time.passing_mode, PassingMode::Pass) {
                        None
                    } else {
                        Some(
                            time.departure
                                .map_or(TravelMode::Flexible, |t| TravelMode::At(t)),
                        )
                    };
                    let entry_entity = commands
                        .spawn(crate::vehicles::entries::TimetableEntry {
                            arrival,
                            departure,
                            station: stations[match train.direction {
                                Direction::Down => i,
                                Direction::Up => stations.len() - 1 - i,
                            }].0,
                            service: None,
                            track: None,
                        })
                        .id();
                    schedule.push(entry_entity);
                    commands.entity(vehicle_entity).add_child(entry_entity);
                }
                commands.entity(vehicle_entity).insert(VehicleSchedule {
                    entities: schedule,
                    ..Default::default()
                });
            }
        }
    }
}

#[derive(Debug)]
struct Root {
    version: String,
    lines: Vec<LineMeta>,
}

#[derive(Debug)]
struct LineMeta {
    name: String,
    stations: Vec<Station>,
    diagrams: Vec<Diagram>,
}

#[derive(Debug)]
struct Station {
    name: String,
    branch_index: Option<usize>,
    loop_index: Option<usize>,
    break_interval: bool,
}

#[derive(Debug)]
struct Diagram {
    name: String,
    trains: Vec<Train>,
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    // kudari
    Down,
    // nobori
    Up,
}

#[derive(Debug)]
struct Train {
    direction: Direction,
    name: String,
    times: Vec<Option<TimetableEntry>>,
}

#[derive(Debug, Clone, Copy)]
enum PassingMode {
    Stop,
    Pass,
    NoOperation,
}

#[derive(Debug, Clone, Copy)]
struct TimetableEntry {
    passing_mode: PassingMode,
    arrival: Option<TimetableTime>,
    departure: Option<TimetableTime>,
    track: Option<usize>,
}

use Structure::*;
use Value::*;
fn parse_ast(ast: &Structure) -> Result<Root, String> {
    let Struct(k, v) = ast else {
        return Err("Expected root structure".to_string());
    };
    let mut root = Root {
        version: String::new(),
        lines: Vec::new(),
    };
    for field in v {
        match field {
            Struct(k, v) if *k == "Rosen" => {
                root.lines.push(parse_line_meta(v)?);
            }
            Pair(k, Single(v)) if *k == "FileType" => {
                root.version = v.to_string();
            }
            _ => {}
        }
    }
    Ok(root)
}
fn parse_line_meta(fields: &[Structure]) -> Result<LineMeta, String> {
    let mut line_meta = LineMeta {
        name: String::new(),
        stations: Vec::new(),
        diagrams: Vec::new(),
    };
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "Rosenmei" => {
                line_meta.name = v.to_string();
            }
            Struct(k, v) if *k == "Eki" => {
                line_meta.stations.push(parse_station(v)?);
            }
            Struct(k, v) if *k == "Dia" => {
                line_meta.diagrams.push(parse_diagram(v)?);
            }
            _ => {}
        }
    }
    Ok(line_meta)
}

fn parse_station(fields: &[Structure]) -> Result<Station, String> {
    let mut station = Station {
        name: String::new(),
        branch_index: None,
        loop_index: None,
        break_interval: false,
    };
    let mut kudari_display = false;
    let mut nobori_display = false;
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "Ekimei" => {
                station.name = v.to_string();
            }
            Pair(k, Single(v)) if *k == "BrunchCoreEkiIndex" => {
                station.branch_index = Some(
                    v.parse::<usize>()
                        .map_err(|e| format!("Failed to parse branch index: {}", e))?,
                );
            }
            Pair(k, Single(v)) if *k == "LoopOriginEkiIndex" => {
                station.loop_index = Some(
                    v.parse::<usize>()
                        .map_err(|e| format!("Failed to parse loop index: {}", e))?,
                );
            }
            Pair(k, List(v)) if *k == "JikokuhyouJikokuDisplayKudari" => {
                kudari_display = v.len() == 2 && [v[0], v[1]] == ["1", "0"];
            }
            Pair(k, List(v)) if *k == "JikokuhyouJikokuDisplayNobori" => {
                nobori_display = v.len() == 2 && [v[0], v[1]] == ["0", "1"];
            }
            _ => {}
        }
    }
    station.break_interval = kudari_display && nobori_display;
    Ok(station)
}

fn parse_diagram(fields: &[Structure]) -> Result<Diagram, String> {
    let mut diagram = Diagram {
        name: String::new(),
        trains: Vec::new(),
    };
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "DiaName" => {
                diagram.name = v.to_string();
            }
            Struct(k, v) if *k == "Kudari" => {
                diagram.trains.extend(parse_trains(Direction::Down, v)?);
            }
            Struct(k, v) if *k == "Nobori" => {
                diagram.trains.extend(parse_trains(Direction::Up, v)?);
            }
            _ => {}
        }
    }
    Ok(diagram)
}

fn parse_trains(direction: Direction, fields: &[Structure]) -> Result<Vec<Train>, String> {
    fn parse_time(str: &str) -> Result<Option<TimetableEntry>, String> {
        let mut entry = TimetableEntry {
            passing_mode: PassingMode::NoOperation,
            arrival: None,
            departure: None,
            track: None,
        };
        if str.is_empty() {
            return Ok(None);
        }
        let parts = OUD2Parser::parse(Rule::timetable_entry, str)
            .map_err(|e| e.to_string())?
            .next()
            .ok_or("Unexpected error while unwrapping")?;
        for field in parts.into_inner() {
            match field.as_rule() {
                Rule::service_mode => match field.as_str() {
                    "1" => entry.passing_mode = PassingMode::Stop,
                    "2" => entry.passing_mode = PassingMode::Pass,
                    _ => entry.passing_mode = PassingMode::NoOperation,
                },
                Rule::arrival => entry.arrival = TimetableTime::from_oud2_str(field.as_str()),
                Rule::departure => entry.departure = TimetableTime::from_oud2_str(field.as_str()),
                Rule::track => {
                    // TODO
                }
                _ => {}
            }
        }
        Ok(Some(entry))
    }
    let parse_trains = |fields: &[Structure]| -> Result<Train, String> {
        let mut train = Train {
            direction,
            name: String::new(),
            times: Vec::new(),
        };
        for field in fields {
            match field {
                Pair(k, Single(v)) if *k == "Ressyabangou" => {
                    train.name = v.to_string();
                }
                Pair(k, v) if *k == "EkiJikoku" => {
                    let times = match v {
                        Single(s) => &vec![*s],
                        List(l) => l,
                    };
                    for time in times {
                        train.times.push(parse_time(time)?);
                    }
                }
                _ => {}
            }
        }
        let mut previous_valid_time = train
            .times
            .iter()
            .find_map(|e| {
                if let Some(e) = e {
                    e.departure.or(e.arrival)
                } else {
                    None
                }
            })
            .unwrap_or(TimetableTime::from_hms(0, 0, 0));
        for time in train
            .times
            .iter_mut()
            .flat_map(|t| {
                std::iter::once(t).flatten().flat_map(|t| {
                    std::iter::once(&mut t.arrival)
                        .flatten()
                        .chain(std::iter::once(&mut t.departure).flatten())
                })
            })
            .skip(1)
        {
            if *time >= previous_valid_time {
                previous_valid_time = *time;
                continue;
            }
            *time += Duration(86400);
            previous_valid_time = *time;
        }
        Ok(train)
    };
    let mut trains = Vec::new();
    for field in fields {
        match field {
            Struct(k, v) if *k == "Ressya" => {
                trains.push(parse_trains(v)?);
            }
            _ => {}
        }
    }
    Ok(trains)
}
