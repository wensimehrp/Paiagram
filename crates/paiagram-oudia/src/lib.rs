use anyhow::{Result, anyhow};
use pest::Parser;
use pest_derive::Parser;
use std::borrow::Cow;
pub use ast::Structure;
pub use ast::SerializeToOud;

pub mod operation;
mod ast;
pub mod time;
pub mod timetable;
pub mod root;

#[macro_export]
macro_rules! structure {
    ($k:expr => $($x:expr),+ $(,)?) => {
        $crate::Structure::Struct($k.into(), vec![$($x.into(),)+])
    };
}

#[macro_export]
macro_rules! pair {
    ($k:expr => $($x:expr),+ $(,)?) => {
        $crate::Structure::Pair($k.into(), smallvec::smallvec![$($x.into(),)+])
    };
}


#[derive(Parser)]
#[grammar = "oudia.pest"]
pub struct OUD2Parser;

#[derive(Debug)]
pub struct Root {
    pub version: String,
    pub lines: Vec<LineMeta>,
}

#[derive(Debug)]
pub struct LineMeta {
    pub name: String,
    pub stations: Vec<Station>,
    pub diagrams: Vec<Diagram>,
    pub classes: Vec<TrainClass>,
}

#[derive(Debug)]
pub struct Station {
    pub name: String,
    pub branch_index: Option<usize>,
    pub loop_index: Option<usize>,
    pub break_interval: bool,
}

#[derive(Debug)]
pub struct Diagram {
    pub name: String,
    pub trains: Vec<Train>,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Down,
    Up,
}

#[derive(Debug)]
pub struct Train {
    pub direction: Direction,
    pub name: String,
    pub times: Vec<Option<TimetableEntry>>,
    pub class_index: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum PassingMode {
    Stop,
    Pass,
    NoOperation,
}

#[derive(Debug, Clone, Copy)]
pub struct TimetableEntry {
    pub passing_mode: PassingMode,
    pub arrival: Option<i32>,
    pub departure: Option<i32>,
    pub track: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TrainClass {
    pub name: String,
    pub color: [u8; 3],
}


pub fn parse_oud2(file: &str) -> Result<Root> {
    let ast = parse_oud2_to_ast(file)?;
    parse_ast(ast)
}

pub fn parse_oud2_to_ast(file: &str) -> Result<Structure<'_>, pest::error::Error<Rule>> {
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
                Structure::Struct(Cow::Borrowed(name), fields)
            }
            Rule::wrapper => {
                let inner = pair.into_inner();
                let name = "file";
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                Structure::Struct(Cow::Borrowed(name), fields)
            }
            Rule::kvpair => {
                let mut inner = pair.into_inner();
                let key = inner.next().unwrap().as_str();
                let val = inner.next().unwrap();
                let val = match val.as_rule() {
                    Rule::value => {
                        let mut values = SmallVec::new();
                        values.push(Cow::Borrowed(val.as_str()));
                        values
                    }
                    Rule::list => {
                        let list_vals = val
                            .into_inner()
                            .map(|v| Cow::Borrowed(v.as_str()))
                            .collect();
                        list_vals
                    }
                    _ => unreachable!(),
                };
                Structure::Pair(Cow::Borrowed(key), val)
            }
            _ => unreachable!(),
        }
    }
    Ok(parse_struct(oud2))
}

use ast::Structure::*;
use smallvec::SmallVec;

fn only_value<'a, 'b>(values: &'b SmallVec<[Cow<'a, str>; 1]>) -> Option<&'b str> {
    if values.len() == 1 {
        values.first().map(|v| v.as_ref())
    } else {
        None
    }
}

fn parse_time(input: &str) -> Result<Option<TimetableEntry>> {
    let mut entry = TimetableEntry {
        passing_mode: PassingMode::NoOperation,
        arrival: None,
        departure: None,
        track: None,
    };
    if input.is_empty() {
        return Ok(None);
    }
    let parts = OUD2Parser::parse(Rule::timetable_entry, input)
        .map_err(|e| anyhow!(e))?
        .next()
        .ok_or(anyhow!("Unexpected error while unwrapping"))?;
    for field in parts.into_inner() {
        match field.as_rule() {
            Rule::service_mode => match field.as_str() {
                "1" => entry.passing_mode = PassingMode::Stop,
                "2" => entry.passing_mode = PassingMode::Pass,
                _ => entry.passing_mode = PassingMode::NoOperation,
            },
            Rule::arrival => entry.arrival = parse_oud2_time(field.as_str()),
            Rule::departure => entry.departure = parse_oud2_time(field.as_str()),
            Rule::track => {
                // TODO
            }
            _ => {}
        }
    }
    Ok(Some(entry))
}

fn parse_ast<'a>(ast: Structure<'a>) -> Result<Root> {
    let mut version = Option::None;
    let mut lines = Vec::new();
    let mut unnamed_line_counter = 0;
    match ast {
        Struct(_, fields) => {
            for field in fields {
                match field {
                    Struct(k, v) if k == "Rosen" => {
                        lines.push(parse_line_meta(v, &mut unnamed_line_counter)?);
                    }
                    Pair(k, v) if k == "FileType" => {
                        if let Some(v) = only_value(&v) {
                            version = Some(v.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => return Err(anyhow!("Expected root structure")),
    }
    Ok(Root {
        version: version.ok_or(anyhow!("File does not have a version"))?,
        lines,
    })
}

fn parse_line_meta<'a>(
    fields: Vec<Structure<'a>>,
    unnamed_line_counter: &mut usize,
) -> Result<LineMeta> {
    let mut name: Option<String> = None;
    let mut stations = Vec::new();
    let mut diagrams = Vec::new();
    let mut classes = Vec::new();
    let mut unnamed_station_counter = 0;
    let mut unnamed_diagram_counter = 0;
    let mut unnamed_train_counter = 0;
    for field in fields {
        match field {
            Pair(k, v) if k == "Rosenmei" => {
                if let Some(v) = only_value(&v) {
                    name = Some(v.to_string());
                }
            }
            Struct(k, v) if k == "Eki" => {
                stations.push(parse_station(v, &mut unnamed_station_counter)?);
            }
            Struct(k, v) if k == "Dia" => {
                diagrams.push(parse_diagram(
                    v,
                    &mut unnamed_diagram_counter,
                    &mut unnamed_train_counter,
                )?);
            }
            Struct(k, v) if k == "Ressyasyubetsu" => classes.push(parse_class(v)?),
            _ => {}
        }
    }
    Ok(LineMeta {
        name: name.unwrap_or_else(|| {
            *unnamed_line_counter += 1;
            format!("Unnamed line {}", unnamed_line_counter)
        }),
        stations,
        diagrams,
        classes,
    })
}

fn parse_station<'a>(
    fields: Vec<Structure<'a>>,
    unnamed_station_counter: &mut usize,
) -> Result<Station> {
    let mut name: Option<String> = None;
    let mut branch_index: Option<usize> = None;
    let mut loop_index: Option<usize> = None;
    let mut kudari_display = false;
    let mut nobori_display = false;
    for field in fields {
        match field {
            Pair(k, v) if k == "Ekimei" => {
                if let Some(v) = only_value(&v) {
                    name = Some(v.to_string());
                }
            }
            Pair(k, v) if k == "BrunchCoreEkiIndex" => {
                if let Some(v) = only_value(&v) {
                    branch_index = Some(
                        v.parse::<usize>()
                            .map_err(|e| anyhow!("Failed to parse branch index: {}", e))?,
                    );
                }
            }
            Pair(k, v) if k == "LoopOriginEkiIndex" => {
                if let Some(v) = only_value(&v) {
                    loop_index = Some(
                        v.parse::<usize>()
                            .map_err(|e| anyhow!("Failed to parse loop index: {}", e))?,
                    );
                }
            }
            Pair(k, v) if k == "JikokuhyouJikokuDisplayKudari" => {
                kudari_display = v.len() == 2 && v[0].as_ref() == "1" && v[1].as_ref() == "0";
            }
            Pair(k, v) if k == "JikokuhyouJikokuDisplayNobori" => {
                nobori_display = v.len() == 2 && v[0].as_ref() == "0" && v[1].as_ref() == "1";
            }
            _ => {}
        }
    }
    let break_interval = kudari_display && nobori_display;
    Ok(Station {
        name: name.unwrap_or_else(|| {
            *unnamed_station_counter += 1;
            format!("Unnamed station {}", unnamed_station_counter)
        }),
        branch_index,
        loop_index,
        break_interval,
    })
}

fn parse_diagram<'a>(
    fields: Vec<Structure<'a>>,
    unnamed_diagram_counter: &mut usize,
    unnamed_train_counter: &mut usize,
) -> Result<Diagram> {
    let mut name: Option<String> = None;
    let mut trains: Vec<Train> = Vec::new();
    for field in fields {
        match field {
            Pair(k, v) if k == "DiaName" => {
                if let Some(v) = only_value(&v) {
                    name = Some(v.to_string());
                }
            }
            Struct(k, v) if k == "Kudari" => {
                trains.extend(parse_trains(Direction::Down, v, unnamed_train_counter)?);
            }
            Struct(k, v) if k == "Nobori" => {
                trains.extend(parse_trains(Direction::Up, v, unnamed_train_counter)?);
            }
            _ => {}
        }
    }
    Ok(Diagram {
        name: name.unwrap_or_else(|| {
            *unnamed_diagram_counter += 1;
            format!("Unnamed diagram {}", unnamed_diagram_counter)
        }),
        trains,
    })
}

fn parse_trains<'a>(
    direction: Direction,
    fields: Vec<Structure<'a>>,
    unnamed_train_counter: &mut usize,
) -> Result<Vec<Train>> {
    let mut trains = Vec::new();
    for field in fields {
        match field {
            Struct(k, v) if k == "Ressya" => {
                trains.push(parse_train(direction, v, unnamed_train_counter)?);
            }
            _ => {}
        }
    }
    Ok(trains)
}

fn parse_train<'a>(
    direction: Direction,
    fields: Vec<Structure<'a>>,
    unnamed_train_counter: &mut usize,
) -> Result<Train> {
    let mut name: Option<String> = None;
    let mut entries: Vec<Option<TimetableEntry>> = Vec::new();
    let mut class_index: Option<usize> = None;
    for field in fields {
        match field {
            Pair(k, v) if k == "Ressyabangou" => {
                if let Some(v) = only_value(&v) {
                    if !v.trim().is_empty() {
                        name = Some(v.to_string());
                    }
                }
            }
            Pair(k, v) if k == "EkiJikoku" => {
                for time in v {
                    entries.push(parse_time(&time)?);
                }
            }
            Pair(k, v) if k == "Syubetsu" => {
                if let Some(v) = only_value(&v) {
                    class_index = Some(v.parse::<usize>().map_err(|e| anyhow!("{:?}", e))?)
                }
            }
            _ => {}
        }
    }
    Ok(Train {
        direction,
        name: name.unwrap_or_else(|| {
            *unnamed_train_counter += 1;
            format!("Unnamed train {}", unnamed_train_counter)
        }),
        times: entries,
        class_index,
    })
}

fn parse_class<'a>(fields: Vec<Structure<'a>>) -> Result<TrainClass> {
    let mut name: Option<String> = None;
    let mut color: Option<[u8; 3]> = None;
    for field in fields {
        match field {
            Pair(k, v) if k == "Syubetsumei" => {
                if let Some(v) = only_value(&v) {
                    name = Some(v.to_string());
                }
            }
            Pair(k, v) if k == "DiagramSenColor" => {
                if let Some(v) = only_value(&v) {
                    let (b, g, r) = (
                        u8::from_str_radix(&v[2..=3], 16)
                            .map_err(|e| anyhow!("Invalid class color (B): {e}"))?,
                        u8::from_str_radix(&v[4..=5], 16)
                            .map_err(|e| anyhow!("Invalid class color (G): {e}"))?,
                        u8::from_str_radix(&v[6..=7], 16)
                            .map_err(|e| anyhow!("Invalid class color (R): {e}"))?,
                    );
                    color = Some([r, g, b]);
                }
            }
            _ => {}
        }
    }
    Ok(TrainClass {
        name: name.ok_or(anyhow!("Class missing Syubetsumei"))?,
        color: color.ok_or(anyhow!("Class missing DiagramSenColor"))?,
    })
}

pub fn parse_oud2_time(s: &str) -> Option<i32> {
    let (time_part, day_offset_seconds) = if let Some(idx) = s.rfind(['+', '-']) {
        let (time, offset_str) = s.split_at(idx);
        let days = offset_str.parse::<i32>().ok()?;
        (time, days * 86400)
    } else {
        (s, 0)
    };

    match time_part.len() {
        3 => {
            let h = time_part[0..1].parse::<i32>().ok()?;
            let m = time_part[1..3].parse::<i32>().ok()?;
            Some(h * 3600 + m * 60 + day_offset_seconds)
        }
        4 => {
            let h = time_part[0..2].parse::<i32>().ok()?;
            let m = time_part[2..4].parse::<i32>().ok()?;
            Some(h * 3600 + m * 60 + day_offset_seconds)
        }
        5 => {
            let h = time_part[0..1].parse::<i32>().ok()?;
            let m = time_part[1..3].parse::<i32>().ok()?;
            let sec = time_part[3..5].parse::<i32>().ok()?;
            Some(h * 3600 + m * 60 + sec + day_offset_seconds)
        }
        6 => {
            let h = time_part[0..2].parse::<i32>().ok()?;
            let m = time_part[2..4].parse::<i32>().ok()?;
            let sec = time_part[4..6].parse::<i32>().ok()?;
            Some(h * 3600 + m * 60 + sec + day_offset_seconds)
        }
        _ => None,
    }
}
