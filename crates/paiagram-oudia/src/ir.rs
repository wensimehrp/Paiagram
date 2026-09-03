// SPDX-License-Identifier: MPL-2.0
//! Intermediate representation of the .oud/oud2 formats.
//! Take a look at [`Root`] to get started.

use std::cmp::{max, min};

use paiagram_oudia_macros::oudia;
use thiserror::Error;

use crate::Structure;
use crate::ir_macros::make_ir_enum;
use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
use crate::time::Time;
use crate::timetable::{TimetableEntry, normalize_times};

mod diagram_trips;

/// The root of the structure
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "")]
pub struct Root {
    /// File type. Usually the software name + version.
    #[oudia(type(single_pair_single_entry = "FileType"))]
    pub file_type: String,
    /// The route in the file.
    #[oudia(type(single_struct = "Rosen"), alias = "路線")]
    pub route: Route,
}

/// The name of the route
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Rosen", alias = "路線")]
pub struct Route {
    /// The name of the route
    #[oudia(type(single_pair_single_entry = "Rosenmei"), alias = "路線名")]
    pub name: String,
    /// What stations are included in the route
    #[oudia(type(many_structs = "Eki"), alias = "駅")]
    pub stations: Vec<Station>,
    /// The available train classes. E.g., local, express.
    #[oudia(type(many_structs = "Ressyasyubetsu"), alias = "列車種別")]
    pub classes: Vec<Class>,
    /// The diagrams included in this route. Each diagram is a timetable set.
    #[oudia(type(many_structs = "Dia"), alias = "ダイヤ")]
    pub diagrams: Vec<Diagram>,
    /// When to start displaying times on the diagram page.
    #[oudia(type(single_pair_single_entry = "KitenJikoku"), alias = "起点時刻")]
    pub display_start_time: Time,
    #[oudia(type(single_pair_single_entry = "Comment"))]
    pub comment: String,
}

/// A station on the route.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Eki", alias = "駅")]
pub struct Station {
    #[oudia(type(single_pair_single_entry = "Ekimei"), alias = "駅名")]
    pub name: String,
    /// The abbreviation used in timetables.
    #[oudia(
        type(single_pair_single_entry = "EkimeiJikokuRyaku"),
        alias = "駅名時刻略"
    )]
    pub timetable_abbreviation: Option<String>,
    /// The abbreviation used in diagrams.
    #[oudia(
        type(single_pair_single_entry = "EkimeiDiaRyaku"),
        alias = "駅名ダイヤ略"
    )]
    pub diagram_abbreviation: Option<String>,
    /// Stations that branch off at certain points may repeat themselves on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station. Please also note that
    /// the name `BrunchCoreEkiIndex` contains a spelling mistake. It should be
    /// `branch` instead of `brunch`.
    #[oudia(type(single_pair_single_entry = "BrunchCoreEkiIndex"))]
    pub branch_index: Option<usize>,
    /// Diagrams representing loop lines may repeat certain stations on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station.
    #[oudia(type(single_pair_single_entry = "LoopOriginEkiIndex"))]
    pub loop_index: Option<usize>,
    /// The tracks of the station
    #[oudia(type(single_struct_many_entries = "EkiTrack2Cont"))]
    pub tracks: Vec<Track>,
    #[oudia(type(single_pair_single_entry = "Ekikibo"), alias = "駅規模")]
    pub station_type: StationType,
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

/// A station track.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "EkiTrack2")]
pub struct Track {
    #[oudia(type(single_pair_single_entry = "TrackName"))]
    pub name: String,
    #[oudia(type(single_pair_single_entry = "TrackRyakusyou"), alias = "Track略称")]
    pub abbreviation: String,
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

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The OuDia color string is `00BBGGRR`, matching [`Color::from_str`].
        write!(f, "00{:02X}{:02X}{:02X}", self.b(), self.g(), self.r())
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

/// A train class. E.g., local, express.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Ressyasyubetsu", alias = "列車種別")]
pub struct Class {
    #[oudia(type(single_pair_single_entry = "Syubetsumei"), alias = "種別名")]
    pub name: String,
    /// An optional abbreviation.
    #[oudia(type(single_pair_single_entry = "Ryakusyou"), alias = "略称")]
    pub abbreviation: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    #[oudia(
        type(single_pair_single_entry = "DiagramSenColor"),
        alias = "ダイヤ線Color"
    )]
    pub diagram_line_color: Color,
}

/// A timetable set.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Dia", alias = "ダイヤ")]
pub struct Diagram {
    #[oudia(type(single_pair_single_entry = "DiaName"))]
    pub name: Option<String>,
    #[oudia(type(twin_struct_multiple_entries(first = "Kudari", second = "Nobori")))]
    pub trips: Vec<Trip>,
}

make_ir_enum! {
    enum Direction as ["Houkou", "方向"];
    Up as ["Nobori", "上り"],
    Down as ["Kudari", "下り"],
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.oud_name())
    }
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

impl std::fmt::Display for StationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.oud_name())
    }
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

/// A train trip.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Ressya", alias = "列車")]
pub struct Trip {
    #[oudia(type(single_pair_single_entry = "Ressyabangou"), alias = "列車番号")]
    pub name: Option<String>,
    #[oudia(type(single_pair_single_entry = "Bikou"), alias = "備考")]
    pub comment: Option<String>,
    #[oudia(type(single_pair_single_entry = "Houkou"), alias = "方向")]
    pub direction: Direction,
    #[oudia(type(single_pair_single_entry = "Syubetsu"), alias = "種別")]
    pub class_index: usize,
    #[oudia(
        type(single_pair_many_entries = "EkiJikoku"),
        alias       = "駅時刻",
        parse_fn    = parse_timetable_entries,
        silence_fn  = |s: &str| s == "Ekijikoku" || s.starts_with("Operation")
    )]
    pub times: Vec<TimetableEntry>,
}

fn parse_timetable_entries(
    ast: &[Structure<'_>],
) -> Result<Vec<TimetableEntry>, IrConversionError> {
    let times = ast
        .iter()
        .find_map(|node| match node {
            Structure::Pair(k, v) if k == "EkiJikoku" => Some(v),
            _ => None,
        })
        .ok_or(IrConversionError::MissingField {
            processing: std::any::type_name::<TimetableEntry>(),
            missing: "EkiJikoku",
        })?;
    let mut times: Vec<_> = times
        .into_iter()
        .map(|ent| ent.parse::<TimetableEntry>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(IrConversionError::from)?;
    for (key, operations) in ast.iter().filter_map(|node| match node {
        Structure::Pair(k, v) if k.starts_with("Operation") => Some((k, v)),
        _ => None,
    }) {
        let hierarchy = parse_to_operation_hierarchy(key)?;
        let operations =
            operations.iter().map(|s| parse_to_raw_operation(s)).collect::<Result<Vec<_>, _>>()?;
        times.insert_operations(hierarchy, operations);
    }
    normalize_times(times.iter_mut().flat_map(|ent| {
        [ent.arrival_time.as_mut(), ent.departure_time.as_mut()].into_iter().flatten()
    }));
    Ok(times)
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

impl From<std::convert::Infallible> for IrConversionError {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
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
        <Root as crate::OuDiaIo>::from_structure(ast.as_slice())
    }

    pub(crate) fn get_ir_small() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample2.oud2");
        let ast = parse_to_ast(s)?;
        <Root as crate::OuDiaIo>::from_structure(ast.as_slice())
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
