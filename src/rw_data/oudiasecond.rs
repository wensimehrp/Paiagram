use bevy::{
    ecs::{message::MessageReader, name},
    log::error,
};
use pest::Parser;
use pest_derive::Parser;

use crate::{rw_data::ModifyData, units::time::TimetableTime};

#[derive(Parser)]
#[grammar = "rw_data/oudiasecond.pest"]
pub struct OUD2Parser;

#[derive(Debug)]
enum OUD2AstStruct<'a> {
    Struct(&'a str, Vec<OUD2AstStruct<'a>>),
    Pair(&'a str, OUD2AstValue<'a>),
}

#[derive(Debug)]
enum OUD2AstValue<'a> {
    Single(&'a str),
    List(Vec<&'a str>),
}

fn parse_oud2_to_ast(file: &str) -> Result<OUD2AstStruct<'_>, pest::error::Error<Rule>> {
    let oud2 = OUD2Parser::parse(Rule::file, file)?
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();
    use pest::iterators::Pair;
    fn parse_struct(pair: Pair<Rule>) -> OUD2AstStruct {
        match pair.as_rule() {
            Rule::r#struct => {
                let mut inner = pair.into_inner();
                let name = inner.next().unwrap().as_str();
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OUD2AstStruct::Struct(name, fields)
            }
            Rule::wrapper => {
                let inner = pair.into_inner();
                let name = "file";
                let mut fields = Vec::new();
                for field_pair in inner {
                    let field_struct = parse_struct(field_pair);
                    fields.push(field_struct);
                }
                OUD2AstStruct::Struct(name, fields)
            }
            Rule::kvpair => {
                let mut inner = pair.into_inner();
                let key = inner.next().unwrap().as_str();
                let val = inner.next().unwrap();
                let val = match val.as_rule() {
                    Rule::value => OUD2AstValue::Single(val.as_str()),
                    Rule::list => {
                        let list_vals = val.into_inner().map(|v| v.as_str()).collect();
                        OUD2AstValue::List(list_vals)
                    }
                    _ => unreachable!(),
                };
                OUD2AstStruct::Pair(key, val)
            }
            _ => unreachable!(),
        }
    }
    Ok(parse_struct(oud2))
}

struct OUD2File {
    file_type: String,
    line: OUD2Line,
}

struct OUD2Line {
    name: String,
    stations: Vec<OUD2Station>,
    vehicle_sets: Vec<OUD2VehicleSet>,
}

struct OUD2Station {
    name: String,
    tracks: Vec<OUD2Track>,
}

struct OUD2Track {
    name: String,
    alias: String,
}

struct OUD2VehicleSet {
    name: String,
    // down and up. kudari for down and nobori for up.
    kudari: Vec<OUD2Vehicle>,
    nobori: Vec<OUD2Vehicle>,
}

struct OUD2Vehicle {
    // class: String,
    /// Ressyabangou
    name: String,
    times: Vec<Option<OUD2Entry>>,
}

struct OUD2Entry {
    pass_type: OUD2PassType,
    arrival_time: Option<TimetableTime>,
    departure_time: Option<TimetableTime>,
    track: usize,
    operation_tree: OUD2OperationTree,
}

enum OUD2PassType {
    Stop,
    NonStop,
    NoOp,
}

struct OUD2OperationTree {
    value: Option<[String; 6]>,
    before: Option<Box<OUD2OperationTree>>,
    after: Option<Box<OUD2OperationTree>>,
}

fn parse_to_file(ast: OUD2AstStruct<'_>) -> Result<OUD2File, String> {
    let mut line: Option<OUD2Line> = None;
    let mut file_type: Option<String> = None;
    match ast {
        OUD2AstStruct::Pair("FileType", OUD2AstValue::Single(s)) => {
            file_type = Some(s.to_string());
        }
        OUD2AstStruct::Struct("Rosen", inner) => {
            line = Some(parse_to_line(inner)?);
        }
        _ => {}
    };
    if let (Some(file_type), Some(line)) = (file_type, line) {
        Ok(OUD2File { file_type, line })
    } else {
        Err("Missing required fields in OUD2 file.".to_string())
    }
}

fn parse_to_line(inner: Vec<OUD2AstStruct<'_>>) -> Result<OUD2Line, String> {
    let mut name: Option<String> = None;
    let mut stations = Vec::new();
    let mut vehicle_sets = Vec::new();
    for field in inner {
        match field {
            OUD2AstStruct::Pair("DiaName", OUD2AstValue::Single(s)) => {
                name = Some(s.to_string());
            }
            OUD2AstStruct::Struct("Eki", inner) => {
                let station = parse_to_station(inner)?;
                stations.push(station);
            }
            OUD2AstStruct::Struct("Dia", inner) => {
                let vehicle_set = parse_to_vehicle_set(inner)?;
                vehicle_sets.push(vehicle_set);
            }
            _ => {}
        }
    }
    if let Some(name) = name {
        Ok(OUD2Line {
            name,
            stations,
            vehicle_sets,
        })
    } else {
        Err("Missing line name in OUD2 file.".to_string())
    }
}

fn parse_to_station(inner: Vec<OUD2AstStruct<'_>>) -> Result<OUD2Station, String> {
    let mut name: Option<String> = None;
    let mut tracks = Vec::new();
    for field in inner {
        match field {
            OUD2AstStruct::Pair("Ekimei", OUD2AstValue::Single(s)) => {
                name = Some(s.to_string());
            }
            OUD2AstStruct::Struct("EkiTrack2Cont", inner) => {
                for field in inner {
                    let OUD2AstStruct::Struct(_, inner) = field else {
                        continue;
                    };
                    let mut name: Option<String> = None;
                    let mut alias: Option<String> = None;
                    for field in inner {
                        match field {
                            OUD2AstStruct::Pair("TrackName", OUD2AstValue::Single(s)) => {
                                name = Some(s.to_string());
                            }
                            OUD2AstStruct::Pair("TrackRyakusyou", OUD2AstValue::Single(s)) => {
                                alias = Some(s.to_string());
                            }
                            _ => {}
                        }
                    }
                    if let (Some(name), Some(alias)) = (name, alias) {
                        tracks.push(OUD2Track { name, alias });
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(name) = name {
        Ok(OUD2Station { name, tracks })
    } else {
        Err("Missing station name in OUD2 file.".to_string())
    }
}

fn parse_to_vehicle_set(inner: Vec<OUD2AstStruct<'_>>) -> Result<OUD2VehicleSet, String> {
    let mut name: Option<String> = None;
    let mut kudari = Vec::new();
    let mut nobori = Vec::new();
    for field in inner {
        match field {
            OUD2AstStruct::Pair("DiaName", OUD2AstValue::Single(s)) => {
                name = Some(s.to_string());
            }
            OUD2AstStruct::Struct("Kudari", inner) => {
                for field in inner {
                    let OUD2AstStruct::Struct(name, inner) = field else {
                        continue;
                    };
                    if name != "Ressya" {
                        continue;
                    }
                    let vehicle = parse_to_vehicle(inner)?;
                    kudari.push(vehicle);
                }
            }
            OUD2AstStruct::Struct("Nobori", inner) => {
                for field in inner {
                    let OUD2AstStruct::Struct(name, inner) = field else {
                        continue;
                    };
                    if name != "Ressya" {
                        continue;
                    }
                    let vehicle = parse_to_vehicle(inner)?;
                    nobori.push(vehicle);
                }
            }
            _ => {}
        }
    }
    if let Some(name) = name {
        Ok(OUD2VehicleSet {
            name,
            kudari,
            nobori,
        })
    } else {
        Err("Missing vehicle set name in OUD2 file.".to_string())
    }
}

fn parse_to_vehicle(inner: Vec<OUD2AstStruct<'_>>) -> Result<OUD2Vehicle, String> {
    let mut name: Option<String> = None;
    let mut class: Option<usize> = None;
    let mut times: Vec<Option<OUD2Entry>> = Vec::new();
    let mut operations: Option<String> = None;
    for field in inner {
        match field {
            OUD2AstStruct::Pair("Syubetsu", OUD2AstValue::Single(s)) => {
                class = s.parse().ok();
            }
            OUD2AstStruct::Pair("Ressyabangou", OUD2AstValue::Single(s)) => {
                name = Some(s.to_string());
            }
            OUD2AstStruct::Pair("EkiJikoku", inner) => {
                let inner = match inner {
                    OUD2AstValue::Single(s) => vec![s],
                    OUD2AstValue::List(v) => v,
                };
                for entry in inner {
                    let time = parse_to_time(entry).ok();
                    times.push(time);
                }
            }
            OUD2AstStruct::Pair(s, v) if s.starts_with("Operation") => {
                // TODO
            }
            _ => {}
        };
    }
    if let Some(name) = name {
        Ok(OUD2Vehicle { name, times })
    } else {
        Err("Missing vehicle name in OUD2 file.".to_string())
    }
}

fn parse_to_time(inner: &str) -> Result<OUD2Entry, String> {
    let parts = OUD2Parser::parse(Rule::timetable_entry, inner).map_err(|e| e.to_string())?;
    let mut pass_type = OUD2PassType::NoOp;
    let mut arrival_time: Option<TimetableTime> = None;
    let mut departure_time: Option<TimetableTime> = None;
    let mut track: usize = 0;
    // for part in parts {
    //     match part.as_rule() {
    //         Rule::service_mode => {
    //             if part.as_str() == "0" {
    //                 pass_type = OUD2PassType::Stop;
    //             } else if part.as_str() == "1" {
    //                 pass_type = OUD2PassType::NonStop;
    //             }
    //         }
    //         Rule::arrival => {
    //             let time_str = part.into_inner().next().unwrap().as_str();
    //             let time = TimetableTime::from_str(time_str).map_err(|e| e.to_string())?;
    //             arrival_time = Some(time);
    //         }
    //         // TODO finish for tomorrow
    //     }
    // }
    Ok(OUD2Entry {
        pass_type,
        arrival_time,
        departure_time,
        track,
        operation_tree: OUD2OperationTree {
            value: None,
            before: None,
            after: None,
        },
    })
}

use bevy::prelude::*;
pub fn load_oud2(
    mut commands: Commands,
    mut msg_read_rw: MessageReader<ModifyData>, // TODO: error display
                                                // mut msg_display_error: MessageWriter<DisplayError>,
) {
    let mut data: Option<&str> = None;
    for msg in msg_read_rw.read() {
        if let ModifyData::LoadOuDiaSecond(content) = msg {
            data = Some(content);
        }
    }
    let Some(data) = data else {
        return;
    };
    // TODO: error display
    let Ok(ast) = parse_oud2_to_ast(data) else {
        error!("Failed to parse OuDiaSecond data.");
        return;
    };
}
