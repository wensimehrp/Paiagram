use crate::{
    colors::DisplayColor,
    entry::{EntryBundle, EntryMode, EntryStop, TravelMode},
    graph::Graph,
    route::Route,
    station::Station as StationComponent,
    trip::{
        TripBundle, TripClass,
        class::{Class as ClassComponent, ClassBundle, ClassResource, DisplayedStroke},
    },
    units::{distance::Distance, time::TimetableTime},
};
use bevy::{platform::collections::HashMap, prelude::*};
use egui_i18n::tr;
use itertools::Itertools;
use moonshine_core::kind::*;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "import/oudia.pest"]
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
    msg: On<super::LoadOuDiaSecond>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
    class_resource: Res<ClassResource>,
) {
    let str = &msg.content;
    graph.clear();
    info!("Loading OUD2 data...");
    let ast = parse_oud2_to_ast(str).expect("Failed to parse OUD2 file");
    let root = parse_ast(&ast).expect("Failed to convert OUD2 AST to internal representation");
    let mut station_map: HashMap<String, Instance<StationComponent>> = HashMap::new();
    for line in root.lines {
        let mut stations: Vec<Option<Instance<StationComponent>>> = vec![None; line.stations.len()];
        let mut break_flags: Vec<bool> = Vec::with_capacity(line.stations.len());
        for (i, station) in line.stations.iter().enumerate() {
            let referenced = station
                .branch_index
                .or(station.loop_index)
                .and_then(|idx| stations.get(idx).and_then(|e| *e));
            // a bit slower but standardized
            let station_entity = referenced.unwrap_or_else(|| {
                super::make_station(&station.name, &mut station_map, &mut graph, &mut commands)
            });
            stations[i] = Some(station_entity);
            break_flags.push(station.break_interval);
        }

        let station_instances: Vec<Instance<StationComponent>> =
            stations.into_iter().map(|e| e.unwrap()).collect();
        let class_instances: Vec<Entity> = line
            .classes
            .into_iter()
            .map(|it| {
                commands
                    .spawn(ClassBundle {
                        class: ClassComponent::default(),
                        name: Name::new(it.name),
                        stroke: DisplayedStroke {
                            color: DisplayColor::Custom(it.color),
                            width: 1.0,
                        },
                    })
                    .id()
            })
            .collect();

        commands.spawn((
            Name::new(line.name),
            Route {
                stops: station_instances.iter().map(|e| e.entity()).collect(),
                lengths: vec![10.0; station_instances.len()],
            },
        ));

        for i in 0..station_instances.len().saturating_sub(1) {
            if break_flags[i] {
                continue;
            }
            super::add_interval_pair(
                &mut graph,
                &mut commands,
                station_instances[i].entity(),
                station_instances[i + 1].entity(),
                Distance::from_m(1000),
            );
        }

        for diagram in line.diagrams {
            for train in diagram.trains {
                let trip_class = train
                    .class_index
                    .map_or(class_resource.default_class, |idx| class_instances[idx]);
                commands
                    .spawn(TripBundle::new(&train.name, TripClass(trip_class.entity())))
                    .with_children(|bundle| {
                        for (stop, mut times) in &train
                            .times
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, time)| {
                                let station_index = match train.direction {
                                    Direction::Down => i,
                                    Direction::Up => station_instances.len() - 1 - i,
                                };
                                let stop = station_instances[station_index];
                                Some((stop, time?))
                            })
                            .chunk_by(|(s, _t)| *s)
                        {
                            let (_, first_time) = times.next().unwrap();
                            let last_time = times.last().map(|(_, t)| t).unwrap_or(first_time);
                            let arrival = if matches!(first_time.passing_mode, PassingMode::Pass) {
                                TravelMode::Flexible
                            } else {
                                first_time
                                    .arrival
                                    .map_or(TravelMode::Flexible, |t| TravelMode::At(t))
                            };
                            let departure = if matches!(last_time.passing_mode, PassingMode::Pass) {
                                None
                            } else {
                                Some(
                                    last_time
                                        .departure
                                        .map_or(TravelMode::Flexible, |t| TravelMode::At(t)),
                                )
                            };
                            bundle.spawn(EntryBundle::new(arrival, departure, stop.entity()));
                        }
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
    classes: Vec<TrainClass>,
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
    class_index: Option<usize>,
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

#[derive(Debug, Clone)]
struct TrainClass {
    name: String,
    color: egui::Color32,
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
    let mut classes = Vec::new();
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
            Struct(k, v) if *k == "Ressyasyubetsu" => classes.push(parse_class(v)),
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
        classes,
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
        let mut class_index: Option<usize> = None;
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
                Pair(k, Single(v)) if *k == "Syubetsu" => {
                    class_index = Some(v.parse::<usize>().map_err(|e| e.to_string())?)
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
            class_index,
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

fn parse_class(fields: &[Structure]) -> TrainClass {
    let mut name: Option<String> = None;
    let mut color: Option<egui::Color32> = None;
    for field in fields {
        match field {
            Pair(k, Single(v)) if *k == "Syubetsumei" => {
                name = Some(v.to_string());
            }
            Pair(k, Single(v)) if *k == "DiagramSenColor" => {
                // AARRGGBB
                let (r, g, b) = (
                    u8::from_str_radix(&v[2..=3], 16).unwrap(),
                    u8::from_str_radix(&v[4..=5], 16).unwrap(),
                    u8::from_str_radix(&v[6..=7], 16).unwrap(),
                );
                color = Some(egui::Color32::from_rgb(r, g, b))
            }
            _ => {}
        }
    }
    TrainClass {
        name: name.unwrap(),
        color: color.unwrap(),
    }
}
