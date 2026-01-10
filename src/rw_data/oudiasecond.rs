use crate::{
    graph::{Graph, Interval},
    lines::DisplayedLine,
    rw_data::ModifyData,
    units::{distance::Distance, time::TimetableTime},
    vehicles::{
        Vehicle,
        entries::{TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};
use bevy::prelude::*;
use egui_i18n::tr;
use moonshine_core::kind::*;
use pest::Parser;
use pest_derive::Parser;

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
    graph.clear();
    info!("Loading OUD2 data...");
    let ast = parse_oud2_to_ast(str).expect("Failed to parse OUD2 file");
    let root = parse_ast(&ast).expect("Failed to convert OUD2 AST to internal representation");
    for line in root.lines {
        let mut stations: Vec<(Instance<crate::graph::Station>, bool)> = Vec::new();
        for station in line.stations.iter() {
            let station_entity = commands
                .spawn(Name::new(station.name.clone()))
                .insert_instance(crate::graph::Station::default())
                .into();
            stations.push((station_entity, station.break_interval));
        }
        for (i, station) in line.stations.iter().enumerate() {
            if let Some(branch_index) = station.branch_index {
                let (e, _) = stations[i];
                commands.entity(e.entity()).despawn();
                stations[i].0 = stations[branch_index].0;
            }
            if let Some(loop_index) = station.loop_index {
                let (e, _) = stations[i];
                commands.entity(e.entity()).despawn();
                stations[i].0 = stations[loop_index].0;
            }
        }
        commands.spawn((
            Name::new(tr!("oud2-default-line")),
            DisplayedLine::new(stations.iter().map(|(e, _)| (*e, 0.0)).collect()),
        ));
        for w in stations.windows(2) {
            let [(prev, break_interval), (next, _)] = w else {
                unreachable!()
            };
            // create new intervals
            if *break_interval {
                continue;
            }
            let e1 = commands
                .spawn_instance(Interval {
                    length: Distance::from_m(1000),
                    speed_limit: None,
                })
                .into();
            let e2 = commands
                .spawn_instance(Interval {
                    length: Distance::from_m(1000),
                    speed_limit: None,
                })
                .into();
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
                            }]
                            .0,
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
    is_timing_foundation: bool,
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
    let Struct(_, v) = ast else {
        return Err("Expected root structure".to_string());
    };
    let mut version = Option::None;
    let mut lines = Vec::new();
    let mut unnamed_line_counter = 0;
    for field in v {
        match field {
            Struct(k, v) if *k == "Rosen" => {
                lines.push(parse_line_meta(v, &mut unnamed_line_counter)?);
            }
            Pair(k, Single(v)) if *k == "FileType" => {
                version = Some(v.to_string());
            }
            _ => {}
        }
    }
    Ok(Root {
        version: version.ok_or("File does not have a version")?,
        lines,
    })
}
fn parse_line_meta(
    fields: &[Structure],
    unnamed_line_counter: &mut usize,
) -> Result<LineMeta, String> {
    let mut name: Option<String> = None;
    let mut stations = Vec::new();
    let mut diagrams = Vec::new();
    let mut unnamed_station_counter = 0;
    let mut unnamed_diagram_counter = 0;
    let mut unnamed_train_counter = 0;
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "Rosenmei" => {
                name = Some(v.to_string());
            }
            Struct(k, v) if *k == "Eki" => {
                stations.push(parse_station(v, &mut unnamed_station_counter)?);
            }
            Struct(k, v) if *k == "Dia" => {
                diagrams.push(parse_diagram(
                    v,
                    &mut unnamed_diagram_counter,
                    &mut unnamed_train_counter,
                )?);
            }
            _ => {}
        }
    }
    Ok(LineMeta {
        name: name.unwrap_or_else(|| {
            *unnamed_line_counter += 1;
            let name = tr!("oud2-unnamed-line", {
                number: unnamed_line_counter.to_string()
            });
            name
        }),
        stations,
        diagrams,
    })
}

fn parse_station(
    fields: &[Structure],
    unnamed_station_counter: &mut usize,
) -> Result<Station, String> {
    let mut name: Option<String> = None;
    let mut branch_index: Option<usize> = None;
    let mut loop_index: Option<usize> = None;
    let mut kudari_display = false;
    let mut nobori_display = false;
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "Ekimei" => {
                name = Some(v.to_string());
            }
            // The "brunch" here is intended - it is spelling mistake in the original software
            Pair(k, Single(v)) if *k == "BrunchCoreEkiIndex" => {
                branch_index = Some(
                    v.parse::<usize>()
                        .map_err(|e| format!("Failed to parse branch index: {}", e))?,
                );
            }
            Pair(k, Single(v)) if *k == "LoopOriginEkiIndex" => {
                loop_index = Some(
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
    let break_interval = kudari_display && nobori_display;
    Ok(Station {
        name: name.unwrap_or_else(|| {
            *unnamed_station_counter += 1;
            let name = tr!("oud2-unnamed-station", {
                number: unnamed_station_counter.to_string()
            });
            name
        }),
        branch_index,
        loop_index,
        break_interval,
    })
}

fn parse_diagram(
    fields: &[Structure],
    unnamed_diagram_counter: &mut usize,
    unnamed_train_counter: &mut usize,
) -> Result<Diagram, String> {
    let mut name: Option<String> = None;
    let mut trains: Vec<Train> = Vec::new();
    let mut is_timing_foundation = false;
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "DiaName" => {
                // hard coded eh
                is_timing_foundation = *v == "基準運転時分";
                name = Some(v.to_string());
            }
            Struct(k, v) if *k == "Kudari" => {
                trains.extend(parse_trains(Direction::Down, v, unnamed_train_counter)?);
            }
            Struct(k, v) if *k == "Nobori" => {
                trains.extend(parse_trains(Direction::Up, v, unnamed_train_counter)?);
            }
            _ => {}
        }
    }
    Ok(Diagram {
        name: name.unwrap_or_else(|| {
            *unnamed_diagram_counter += 1;
            let name = tr!("oud2-unnamed-diagram", {
                number: unnamed_diagram_counter.to_string()
            });
            name
        }),
        trains,
        is_timing_foundation,
    })
}

fn parse_trains(
    direction: Direction,
    fields: &[Structure],
    unnamed_train_counter: &mut usize,
) -> Result<Vec<Train>, String> {
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
    let mut parse_trains = |fields: &[Structure]| -> Result<Train, String> {
        let mut name: Option<String> = None;
        let mut entries: Vec<Option<TimetableEntry>> = Vec::new();
        for field in fields {
            match field {
                Pair(k, Single(v)) if *k == "Ressyabangou" && !v.trim().is_empty() => {
                    name = Some(v.to_string());
                }
                Pair(k, v) if *k == "EkiJikoku" => {
                    let times = match v {
                        Single(s) => &vec![*s],
                        List(l) => l,
                    };
                    for time in times {
                        entries.push(parse_time(time)?);
                    }
                }
                _ => {}
            }
        }
        let time_iter = entries.iter_mut().flat_map(|t| {
            std::iter::once(t).flatten().flat_map(|t| {
                std::iter::once(&mut t.arrival)
                    .flatten()
                    .chain(std::iter::once(&mut t.departure).flatten())
            })
        });
        super::normalize_times(time_iter);
        Ok(Train {
            direction,
            name: name.unwrap_or_else(|| {
                *unnamed_train_counter += 1;
                let name = tr!("oud2-unnamed-train", {
                    number: unnamed_train_counter.to_string()
                });
                name
            }),
            times: entries,
        })
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
