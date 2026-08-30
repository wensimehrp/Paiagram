// SPDX-License-Identifier: MPL-2.0
//! Intermediate representation of the .oud/oud2 formats.
//! Take a look at [`Root`] to get started.

use std::borrow::Cow;
use std::cmp::{max, min};

use smallvec::SmallVec;
use thiserror::Error;

use crate::ir_macros::{make_ir_enum, make_ir_type, parse_fields};
use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
use crate::time::Time;
use crate::timetable::{TimetableEntry, normalize_times, parse_to_timetable_entry};
use crate::{pair, structure};
mod diagram_trips;

make_ir_type! {
    /// The root of the structure
    struct Root;
    /// File type. Usually the software name + version.
    pub file_type as ["FileType"]: String,
    /// The route in the file.
    pub route as ["Rosen", "路線"]: Route,
}

make_ir_type! {
    struct Route as ["Rosen", "路線"];
    /// The name of the route
    pub name as ["Rosenmei", "路線名"]: String,
    /// What stations are included in the route
    pub stations as ["Eki", "駅"]: Vec<Station>,
    /// The available train classes. E.g., local, express.
    pub classes as ["Ressyasyubetsu", "列車種別"]: Vec<Class>,
    /// The diagrams included in this route. Each diagram is a timetable set.
    pub diagrams as ["Dia", "ダイヤ"]: Vec<Diagram>,
    /// When to start displaying times on the diagram page.
    pub display_start_time as ["KitenJikoku", "起点時刻"]: Time,
    pub comment as ["Comment"]: String,
}

make_ir_type! {
    /// A station on the route.
    struct Station as ["Eki", "駅"];
    pub name as ["Ekimei", "駅名"]: String,
    /// The abbreviation used in timetables.
    pub timetable_abbreviation as ["EkimeiJikokuRyaku", "駅名時刻略"]: Option<String>,
    /// The abbreviation used in diagrams.
    pub diagram_abbreviation as ["EkimeiDiaRyaku", "駅名ダイヤ略"]: Option<String>,
    /// Stations that branch off at certain points may repeat themselves on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station. Please also note that
    /// the name `BrunchCoreEkiIndex` contains a spelling mistake. It should be
    /// `branch` instead of `brunch`.
    pub branch_index as ["BrunchCoreEkiIndex"]: Option<usize>,
    /// Diagrams representing loop lines may repeat certain stations on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station.
    pub loop_index as ["LoopOriginEkiIndex"]: Option<usize>,
    /// The tracks of the station
    pub tracks as ["EkiTrack2Cont"]: SmallVec<[Track; 2]>,
    pub station_type as ["Ekikibo", "駅規模"]: StationType,
}

pub trait StationToGraph {
    fn merge_duplicate(&self) -> Vec<&Station>;
    fn to_graph<'a>(&'a self) -> petgraph::graph::UnGraph<&'a Station, ()>;
}

impl StationToGraph for [Station] {
    fn merge_duplicate(&self) -> Vec<&Station> {
        let mut ret: Vec<&Station> = self.iter().collect();
        for curr in 0..ret.len() {
            let Some(ext) = ret[curr].branch_index.or(ret[curr].loop_index) else {
                continue;
            };
            if let Some(stn) = ret.get(ext).copied() {
                ret[curr] = stn;
            }
        }
        ret
    }
    fn to_graph<'a>(&'a self) -> petgraph::graph::UnGraph<&'a Station, ()> {
        // only merge stations based on branch index and loop index
        let mut graph = petgraph::graph::UnGraph::new_undirected();
        let mut idxs: Vec<_> = self.iter().map(|stn| graph.add_node(stn)).collect();
        for curr in 0..idxs.len() {
            let Some(ext) = self[curr].branch_index.or(self[curr].loop_index) else {
                continue;
            };
            if let Some(node_idx) = idxs.get(ext).copied() {
                let old_idx = idxs[curr];
                idxs[curr] = node_idx;
                graph.remove_node(old_idx);
            }
        }
        for [prev, next] in idxs.array_windows::<2>().copied() {
            if graph.node_weight(prev).is_some() && graph.node_weight(next).is_some() {
                graph.update_edge(prev, next, ());
            }
        }
        graph
    }
}

make_ir_type! {
    struct Track;
    pub name as ["TrackName"]: String,
    pub abbreviation as ["TrackRyakusyou", "Track略称"]: String,
}

/// Color. This color is stored in ARGB format.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub fn a(&self) -> u8 {
        self.0[0]
    }
    pub fn r(&self) -> u8 {
        self.0[1]
    }
    pub fn g(&self) -> u8 {
        self.0[2]
    }
    pub fn b(&self) -> u8 {
        self.0[3]
    }
}

impl std::str::FromStr for Color {
    type Err = IrConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 8 {
            return Err(IrConversionError::ColorConversionError(s.to_string()));
        }
        let (b, g, r) = (
            u8::from_str_radix(&s[2..=3], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
            u8::from_str_radix(&s[4..=5], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
            u8::from_str_radix(&s[6..=7], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
        );
        Ok(Self([0, r, g, b]))
    }
}

make_ir_type! {
    /// A train class. E.g., local, express.
    struct Class as ["Ressyasyubetsu", "列車種別"];
    pub name as ["Syubetsumei", "種別名"]: String,
    /// An optional abbreviation.
    pub abbreviation as ["Ryakusyou", "略称"]: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    pub diagram_line_color as ["DiagramSenColor", "ダイヤ線Color"]: Color,
}

make_ir_type! {
    /// A timetable set.
    struct Diagram as ["Dia", "ダイヤ"];
    pub name as ["DiaName"]: Option<String>,
    pub trips: Vec<Trip>,
}

make_ir_enum! {
    enum Direction as ["Houkou", "方向"];
    Up as ["Nobori", "上り"],
    Down as ["Kudari", "下り"],
}

impl std::str::FromStr for Direction {
    type Err = IrConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "Kudari" {
            Ok(Self::Down)
        } else if s == "Nobori" {
            Ok(Self::Up)
        } else {
            Err(IrConversionError::UnknownToken(s.to_string()))
        }
    }
}

make_ir_enum! {
    enum StationType as ["Ekikibo", "駅規模"];
    Major as ["Ekikibo_Syuyou", "駅規模_主要"],
    Minor as ["Ekikibo_Ippan", "駅規模_一般"],
}

impl std::str::FromStr for StationType {
    type Err = IrConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "Ekikibo_Syuyou" {
            Ok(Self::Major)
        } else if s == "Ekikibo_Ippan" {
            Ok(Self::Minor)
        } else {
            Err(IrConversionError::UnknownToken(s.to_string()))
        }
    }
}

make_ir_type! {
    struct Trip as ["Ressya", "列車"];
    pub name as ["Ressyabangou", "列車番号"]: Option<String>,
    pub comment as ["Bikou", "備考"]: Option<String>,
    pub direction as ["Houkou", "方向"]: Direction,
    pub class_index as ["Syubetsu", "種別"]: usize,
    pub times as ["EkiJikoku", "駅時刻"]: Vec<TimetableEntry>,
}

/// Also known as `運用`.
#[doc(alias = "運用")]
pub struct Rotation<'a> {
    /// Also known as `運用番号`.
    #[doc(alias = "運用番号")]
    pub name: String,
    /// Also known as `列車番号`.
    #[doc(alias = "列車番号")]
    pub trips: Vec<&'a Trip>,
}

fn travelling_duration(curr: &TimetableEntry, next: &TimetableEntry) -> Option<Time> {
    let curr_time = curr.departure_time.or(curr.arrival_time)?;
    let mut next_time = next.arrival_time.or(next.departure_time)?;
    if curr_time > next_time {
        next_time += Time::from_seconds(86400);
    }
    Some(next_time - curr_time)
}

impl Diagram {
    /// 5 minutes
    pub const DEFAULT_INTERVAL_SECONDS: Time = Time::from_seconds(60 * 5);

    /// Return the average travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn average_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        (0..stations.len().saturating_sub(1)).map(move |idx| {
            let mut avg_seconds: i32 = 0;
            let mut count: i32 = 0;
            for trip in self.trips.iter() {
                let (curr, next) = match trip.direction {
                    Direction::Down => {
                        let Some(next_entry) = trip.times.get(idx + 1) else {
                            continue;
                        };
                        (&trip.times[idx], next_entry)
                    }
                    Direction::Up => {
                        let base = stations.len() - 2 - idx;
                        let Some(next_entry) = trip.times.get(base + 1) else {
                            continue;
                        };
                        (&trip.times[base], next_entry)
                    }
                };
                let Some(diff) = travelling_duration(curr, next) else {
                    continue;
                };
                avg_seconds += diff.seconds();
                count += 1;
            }
            (count != 0).then(|| Time::from_seconds(avg_seconds / count))
        })
    }

    /// Return the extreme travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    fn extrema_interval_durations<const MINIMUM: bool>(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        (0..stations.len().saturating_sub(1)).map(move |idx| {
            let mut extreme = if MINIMUM {
                Time::from_seconds(i32::MAX)
            } else {
                Time::from_seconds(i32::MIN)
            };
            let mut exist: bool = false;
            for trip in self.trips.iter() {
                let (curr, next) = match trip.direction {
                    Direction::Down => {
                        let Some(next_entry) = trip.times.get(idx + 1) else {
                            continue;
                        };
                        (&trip.times[idx], next_entry)
                    }
                    Direction::Up => {
                        let base = stations.len() - 2 - idx;
                        let Some(next_entry) = trip.times.get(base + 1) else {
                            continue;
                        };
                        (&trip.times[base], next_entry)
                    }
                };
                let Some(diff) = travelling_duration(curr, next) else {
                    continue;
                };
                extreme = if MINIMUM {
                    min(extreme, diff)
                } else {
                    max(extreme, diff)
                };
                exist = true;
            }
            exist.then_some(extreme)
        })
    }

    /// Return the minimum travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn minimum_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        self.extrema_interval_durations::<true>(stations)
    }

    /// Return the maximum travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn maximum_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        self.extrema_interval_durations::<false>(stations)
    }

    pub fn rotations<'a>(&self, _stations: &[Station]) -> Vec<Rotation<'a>> {
        unimplemented!()
    }
}

use crate::ast::{GetItemWithKey, Structure};

#[derive(Debug, Clone, Error)]
pub enum IrConversionError {
    #[error("Missing field '{missing}' when converting AST to '{processing}'")]
    MissingField {
        processing: &'static str,
        missing: &'static str,
    },
    #[error(
        "Index out of bounds when trying to generate '{field}' for '{processing}' (checked index '{index}', but the length is only '{len}')"
    )]
    IndexOutOfBounds {
        field: &'static str,
        processing: &'static str,
        index: usize,
        len: usize,
    },
    #[error("Failed to parse integer: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Failed to parse timetable entry: {0}")]
    EntryParseError(#[from] pest::error::Error<crate::timetable::time::Rule>),
    #[error("Failed to parse operation: {0}")]
    OperationParseError(#[from] pest::error::Error<crate::operation::operation::Rule>),
    #[error("Failed to parse input to AST: {0}")]
    AstParseError(#[from] pest::error::Error<crate::ast::oudia::Rule>),
    #[error("Unknown token: {0}")]
    UnknownToken(String),
    #[error("Could not convert string {0} to valid color")]
    ColorConversionError(String),
    #[error("Structure is empty while parsing {0}")]
    EmptyError(&'static str),
}

fn infer_name(v: &[Cow<'_, str>]) -> Result<String, IrConversionError> {
    let Some(s) = v.get(0) else {
        return Err(IrConversionError::IndexOutOfBounds {
            field: "UNIMPLEMENTED",
            processing: "UNIMPLEMENTED",
            index: 0,
            len: v.len(),
        });
    };
    Ok(s.to_string())
}

fn infer_parse<T>(v: &[Cow<'_, str>]) -> Result<T, IrConversionError>
where
    T: std::str::FromStr,
    IrConversionError: From<T::Err>,
{
    let Some(s) = v.get(0) else {
        return Err(IrConversionError::IndexOutOfBounds {
            field: "UNIMPLEMENTED",
            processing: "UNIMPLEMENTED",
            index: 0,
            len: v.len(),
        });
    };
    s.parse::<T>().map_err(IrConversionError::from)
}

fn pass<'r, 'a>(v: &'r [Structure<'a>]) -> Result<&'r [Structure<'a>], IrConversionError> {
    Ok(v)
}

impl<'a> TryFrom<&[Structure<'a>]> for Root {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Root;
            RequiredOnce(file_type: Pair) => infer_name,
            RequiredOnce(route: Struct) => Route::try_from,
        );
        Ok(Self { file_type, route })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Route {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Route;
            Many(stations: Struct) => Station::try_from,
            Many(diagrams: Struct) => Diagram::try_from,
            Many(classes: Struct) => Class::try_from,
            RequiredOnce(name: Pair) => infer_name,
            RequiredOnce(display_start_time: Pair) => infer_parse::<Time>,
            RequiredOnce(comment: Pair) => infer_name,
        );
        Ok(Self {
            name,
            stations,
            classes,
            diagrams,
            display_start_time,
            comment,
        })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Station {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Station;
            RequiredOnce(name: Pair) => infer_name,
            RequiredOnce(station_type: Pair) => infer_parse::<StationType>,
            OptionalOnce(timetable_abbreviation: Pair) => infer_name,
            OptionalOnce(diagram_abbreviation: Pair) => infer_name,
            OptionalOnce(branch_index: Pair) => infer_parse::<usize>,
            OptionalOnce(loop_index: Pair) => infer_parse::<usize>,
            OptionalOnce(all_tracks: Struct(Self::TRACKS_OUD_NAME)) => pass,
        );
        let mut tracks = SmallVec::new();
        for (_, ast) in all_tracks.into_iter().flatten().every_struct("EkiTrack2") {
            parse_fields!(ast; Track;
                RequiredOnce(name: Pair) => infer_name,
                RequiredOnce(abbreviation: Pair) => infer_name,
            );
            tracks.push(Track { name, abbreviation })
        }
        Ok(Self {
            name,
            timetable_abbreviation,
            diagram_abbreviation,
            branch_index,
            loop_index,
            tracks,
            station_type,
        })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Diagram {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Diagram;
            OptionalOnce(name: Pair) => infer_name,
            Many(up_trips: Struct(Direction::Up.oud_name())) => pass,
            Many(down_trips: Struct(Direction::Down.oud_name())) => pass,
        );
        let mut trips = Vec::new();
        let down_trips_iter = down_trips.into_iter().flatten();
        let up_trips_iter = up_trips.into_iter().flatten();
        for trip_result in down_trips_iter
            .chain(up_trips_iter)
            .every_struct("Ressya")
            .map(|(_, trip)| Trip::try_from(trip))
        {
            match trip_result {
                Ok(r) => trips.push(r),
                Err(Self::Error::EmptyError(_)) => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Self { name, trips })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Trip {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Trip;
            OptionalOnce(name: Pair) => infer_name,
            OptionalOnce(comment: Pair) => infer_name,
            RequiredOnce(direction: Pair) => infer_parse::<Direction>,
            RequiredOnce(class_index: Pair) => infer_parse::<usize>,
            RequiredOnce(times: Pair) =>
                |v: &[Cow<'a, str>]| -> Result<_, IrConversionError> {
                let mut times = Vec::with_capacity(v.len());
                for entry in v {
                    let v = parse_to_timetable_entry(entry)?;
                    times.push(v);
                }
                Ok(times)
            },
        );
        let mut times = times;
        for it in value.iter() {
            let Structure::Pair(k, vals) = it else {
                continue;
            };
            if !k.starts_with("Operation") {
                continue;
            }
            let hierarchy = parse_to_operation_hierarchy(k)?;
            let operations =
                vals.iter().map(|it| parse_to_raw_operation(it)).collect::<Result<Vec<_>, _>>()?;
            times.insert_operations(hierarchy, operations);
        }
        normalize_times(times.iter_mut().flat_map(|ent| {
            [ent.arrival_time.as_mut(), ent.departure_time.as_mut()].into_iter().flatten()
        }));
        Ok(Self {
            name,
            direction,
            class_index,
            times,
            comment,
        })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Class {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value; Class;
            RequiredOnce(name: Pair) => infer_name,
            OptionalOnce(abbreviation: Pair) => infer_name,
            RequiredOnce(diagram_line_color: Pair) => infer_parse::<Color>,
        );
        Ok(Self {
            name,
            abbreviation,
            diagram_line_color,
        })
    }
}

impl<'a> Into<Vec<Structure<'a>>> for Root {
    fn into(self) -> Vec<Structure<'a>> {
        vec![
            pair!("FileType" => self.file_type),
            structure!("Rosen" => ..<Route as Into<Vec<Structure>>>::into(self.route)),
        ]
    }
}

impl<'a> Into<Vec<Structure<'a>>> for Route {
    fn into(self) -> Vec<Structure<'a>> {
        vec![pair!("Rosenmei" => self.name)]
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::parse_to_ast;
    type E = Result<(), Box<dyn std::error::Error>>;

    pub(crate) fn get_ir() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample.oud2");
        let ast = parse_to_ast(s)?;
        Root::try_from(ast.as_slice())
    }

    pub(crate) fn get_ir_small() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample2.oud2");
        let ast = parse_to_ast(s)?;
        Root::try_from(ast.as_slice())
    }

    #[test]
    fn gen_graph() -> E {
        let root = get_ir()?;
        let graph = root.route.stations.to_graph();
        use petgraph::dot::{Config, Dot};
        println!(
            "{:?}",
            Dot::with_attr_getters(
                &graph,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|_, _| String::new(),
                &|_, (_, stn)| format!("label = \"{}\"", stn.name)
            )
        );
        Ok(())
    }

    #[test]
    fn test_parse_ast_to_ir() -> E {
        let ir = get_ir()?;
        println!("{ir:#?}");
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_rotations() -> E {
        let ir = get_ir()?;
        if let Some(diagram) = ir.route.diagrams.first() {
            let mut rotations = diagram.rotations(&ir.route.stations);
            rotations.sort_by_key(|it| it.name.clone());
            for Rotation { name, trips } in rotations.into_iter() {
                println!("========== Rotation '{name}' ==========");
                for trip in trips {
                    println!("{}", trip.name.as_deref().unwrap_or("<unnamed>"))
                }
            }
        }
        Ok(())
    }

    #[test]
    fn average_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.average_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }

    #[test]
    fn minimum_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.minimum_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }

    #[test]
    fn maximum_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.maximum_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }
}
