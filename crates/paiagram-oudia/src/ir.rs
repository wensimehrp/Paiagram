// SPDX-License-Identifier: MPL-2.0
/*!
Intermediate representation of the .oud/oud2 formats.
Take a look at [`Root`] to get started.
*/
use std::borrow::Cow;
use std::cmp::{max, min};

use smallvec::SmallVec;
use thiserror::Error;

use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
use crate::time::Time;
use crate::timetable::{TimetableEntry, parse_to_timetable_entry};
use crate::{pair, structure};

/// The root of the structure
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Root {
    /// File type. Usually the software name + version.
    /// Also known as `FileType`.
    #[doc(alias = "FileType")]
    pub file_type: String,
    /// The route in the file.
    /// Also known as `Rosen`.
    /// Also known as `路線`.
    #[doc(alias = "Rosen")]
    #[doc(alias = "路線")]
    pub route: Route,
}

/// Also known as `Rosen`.
/// Also known as `路線`.
#[doc(alias = "Rosen")]
#[doc(alias = "路線")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// The name of the route
    /// Also known as `Rosenmei`.
    /// Also known as `路線名`.
    #[doc(alias = "Rosenmei")]
    #[doc(alias = "路線名")]
    pub name: String,
    /// What stations are included in the route
    /// Also known as `Eki`.
    /// Also known as `駅`.
    #[doc(alias = "Eki")]
    #[doc(alias = "駅")]
    pub stations: Vec<Station>,
    /// The available train classes. E.g., local, express.
    /// Also known as `Ressyasyubetsu`.
    /// Also known as `列車種別`.
    #[doc(alias = "Ressyasyubetsu")]
    #[doc(alias = "列車種別")]
    pub classes: Vec<Class>,
    /// The diagrams included in this route. Each diagram is a timetable set.
    /// Also known as `Dia`.
    /// Also known as `ダイヤ`.
    #[doc(alias = "Dia")]
    #[doc(alias = "ダイヤ")]
    pub diagrams: Vec<Diagram>,
    /// When to start displaying times on the diagram page.
    /// Also known as `KitenJikoku`.
    /// Also known as `起点時刻`.
    #[doc(alias = "KitenJikoku")]
    #[doc(alias = "起点時刻")]
    pub display_start_time: Time,
    /// Also known as `Comment`.
    #[doc(alias = "Comment")]
    pub comment: String,
}

/// A station on the route.
/// Also known as `Eki`.
/// Also known as `駅`.
#[doc(alias = "Eki")]
#[doc(alias = "駅")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    /// Also known as `Ekimei`.
    /// Also known as `駅名`.
    #[doc(alias = "Ekimei")]
    #[doc(alias = "駅名")]
    pub name: String,
    /// The abbreviation used in timetables.
    /// Also known as `EkimeiJikokuRyaku`.
    /// Also known as `駅名時刻略`.
    #[doc(alias = "EkimeiJikokuRyaku")]
    #[doc(alias = "駅名時刻略")]
    pub timetable_abbreviation: Option<String>,
    /// The abbreviation used in diagrams.
    /// Also known as `EkimeiDiaRyaku`.
    /// Also known as `駅名ダイヤ略`.
    #[doc(alias = "EkimeiDiaRyaku")]
    #[doc(alias = "駅名ダイヤ略")]
    pub diagram_abbreviation: Option<String>,
    /// Stations that branch off at certain points may repeat themselves on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station. Please also note that
    /// the name `BrunchCoreEkiIndex` contains a spelling mistake. It should be
    /// `branch` instead of `brunch`.
    ///  Also known as `BrunchCoreEkiIndex`.
    #[doc(alias = "BrunchCoreEkiIndex")]
    pub branch_index: Option<usize>,
    /// Diagrams representing loop lines may repeat certain stations on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station.
    /// Also known as `LoopOriginEkiIndex`.
    #[doc(alias = "LoopOriginEkiIndex")]
    pub loop_index: Option<usize>,
    /// The tracks of the station
    /// Also known as `EkiTrack2Cont`.
    #[doc(alias = "EkiTrack2Cont")]
    pub tracks: SmallVec<[Track; 2]>,
    /// Also known as `Ekikibo`
    /// Also known as `駅規模`
    #[doc(alias = "Ekikibo")]
    #[doc(alias = "駅規模")]
    pub station_type: StationType,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Also known as `TrackName`.
    #[doc(alias = "TrackName")]
    pub name: String,
    /// Also known as `TrackRyakusyou`.
    /// Also known as `Track略称`.
    #[doc(alias = "TrackRyakusyou")]
    #[doc(alias = "Track略称")]
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
/// Also known as `Ressyasyubetsu`.
/// Also known as `列車種別`.
#[doc(alias = "Ressyasyubetsu")]
#[doc(alias = "列車種別")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    /// Also known as `Syubetsumei`.
    /// Also known as `種別名`.
    #[doc(alias = "Syubetsumei")]
    #[doc(alias = "種別名")]
    pub name: String,
    /// An optional abbreviation.
    /// Also known as `Ryakusyou`.
    /// Also known as `略称`.
    #[doc(alias = "Ryakusyou")]
    #[doc(alias = "略称")]
    pub abbreviation: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    /// Also known as `DiagramSenColor`.
    /// Also known as `ダイア線Color`.
    #[doc(alias = "DiagramSenColor")]
    #[doc(alias = "ダイア線Color")]
    pub diagram_line_color: Color,
}

/// A timetable set.
/// Also known as `Dia`.
/// Also known as `ダイヤ`.
#[doc(alias = "Dia")]
#[doc(alias = "ダイヤ")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Diagram {
    /// Also known as `DiaName`.
    #[doc(alias = "DiaName")]
    pub name: Option<String>,
    pub trips: Vec<Trip>,
}

/// Also known as `Houkou`.
/// Also known as `方向`.
#[doc(alias = "Houkou")]
#[doc(alias = "方向")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Direction {
    /// Also known as `Nobori`.
    /// Also known as `上り`.
    #[doc(alias = "Nobori")]
    #[doc(alias = "上り")]
    Up,
    /// Also known as `Kudari`.
    /// Also known as `下り`.
    #[doc(alias = "Kudari")]
    #[doc(alias = "下り")]
    Down,
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

/// Also known as `Ekikibo`.
/// Also known as `駅規模`.
#[doc(alias = "Ekikibo")]
#[doc(alias = "駅規模")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum StationType {
    /// Also known as `Ekikibo_Syuyou`.
    /// Also known as `駅規模_主要`.
    #[doc(alias = "Ekikibo_Syuyou")]
    #[doc(alias = "駅規模_主要")]
    Major,
    /// Also known as `Kudari`.
    /// Also known as `下り`.
    #[doc(alias = "Ekikibo_Ippan")]
    #[doc(alias = "駅規模_一般")]
    Minor,
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

/// Also known as `Ressya`.
/// Also known as `列車`.
#[doc(alias = "Ressya")]
#[doc(alias = "列車")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Trip {
    /// Also known as `Ressyabangou`.
    /// Also known as `列車番号`.
    #[doc(alias = "Ressyabangou")]
    #[doc(alias = "列車番号")]
    pub name: Option<String>,
    /// Also known as `Bikou`.
    /// Also known as `備考`.
    #[doc(alias = "Bikou")]
    #[doc(alias = "備考")]
    pub comment: Option<String>,
    /// Also known as `Houkou`.
    /// Also known as `方向`.
    #[doc(alias = "Houkou")]
    #[doc(alias = "方向")]
    pub direction: Direction,
    /// Also known as `Syubetsu`.
    /// Also known as `種別`.
    #[doc(alias = "Syubetsu")]
    #[doc(alias = "種別")]
    pub class_index: usize,
    /// Also known as `EkiJikoku`.
    /// Also known as `駅時刻`.
    #[doc(alias = "EkiJikoku")]
    #[doc(alias = "駅時刻")]
    pub times: Vec<TimetableEntry>,
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

    fn rotations<'a>(&self, _stations: &[Station]) -> Vec<Rotation<'a>> {
        // struct Train<'a> {
        //     head: &'a str,
        //     rest: Vec<&'a str>,
        //     time: Time,
        // }
        // impl<'a> Train<'a> {
        //     fn rotations(&self) -> impl Iterator<Item = &'a str> {
        //         std::iter::once(self.head).chain(self.rest.iter().copied())
        //     }
        // }
        // let mut rotations = Vec::new();
        // let mut active_trains: Vec<Train> = Vec::new();
        // // Maybe it's better to use a hashmap instead?
        // let mut train_on_station_tracks: FxHashMap<(usize, Option<usize>), Vec<Train>> =
        //     HashMap::with_hasher(FxBuildHasher);
        // for root_tree in self
        //     .trips
        //     .iter()
        //     .filter_map(|it| {
        //         it.times
        //             .iter()
        //             .find(|it| it.service_mode != ServiceMode::NoOperation)
        //     })
        //     .filter_map(|it| it.operations())
        // {
        //     let before_tree = &root_tree.befores;
        // }
        // for val in train_on_station_tracks.values_mut() {
        //     val.sort_unstable_by_key(|it| it.time);
        // }
        // rotations
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

macro_rules! parse_fields {
    ($iter:expr; $($once_or_many:ident($variant:ident($key:expr, $variable_name:ident)) => $action:expr,)*) => {
        $(
            parse_fields!(@make_variable $once_or_many($variable_name));
        )*
        if $iter.is_empty() {
            return Err(IrConversionError::EmptyError(std::any::type_name::<Self>()));
        }
        for field in $iter {
            match field {
                $(
                    $crate::Structure::$variant(k, v) if k == $key => {
                        parse_fields!(@populate_inner $once_or_many($variable_name), v.as_slice(), $action);
                    },
                )*
                _ => {}
            }
        }
        $(
            parse_fields!(@post_population $once_or_many($key, $variable_name));
        )*
    };

    (@make_variable RequiredOnce($variable_name:ident)) => {
        let mut $variable_name = None;
    };

    (@make_variable OptionalOnce($variable_name:ident)) => {
        let mut $variable_name = None;
    };

    (@make_variable Many($variable_name:ident)) => {
        let mut $variable_name = Vec::new();
    };

    (@populate_inner RequiredOnce($variable_name:ident), $value:expr, $action:expr) => {
        $variable_name = Some($action($value)?);
    };

    (@populate_inner OptionalOnce($variable_name:ident), $value:expr, $action:expr) => {
        $variable_name = Some($action($value)?);
    };

    (@populate_inner Many($variable_name:ident), $value:expr, $action:expr) => {
        $variable_name.push($action($value)?);
    };

    (@post_population RequiredOnce($key:expr, $variable_name:ident)) => {
        let Some($variable_name) = $variable_name else {
            return Err(IrConversionError::MissingField {
                processing: std::any::type_name::<Self>(),
                missing: $key,
            })
        };
    };

    (@post_population $($tokens:tt)*) => {}
}

impl<'a> TryFrom<&[Structure<'a>]> for Root {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value;
            RequiredOnce(Pair("FileType", file_type)) => infer_name,
            RequiredOnce(Struct("Rosen", route)) => Route::try_from,
        );
        Ok(Self { file_type, route })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Route {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value;
            Many(Struct("Eki", stations)) => Station::try_from,
            Many(Struct("Dia", diagrams)) => Diagram::try_from,
            Many(Struct("Ressyasyubetsu", classes)) => Class::try_from,
            RequiredOnce(Pair("Rosenmei", name)) => infer_name,
            RequiredOnce(Pair("KitenJikoku", display_start_time)) => infer_parse::<Time>,
            RequiredOnce(Pair("Comment", comment)) => infer_name,
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
        parse_fields!(value;
            RequiredOnce(Pair("Ekimei", name)) => infer_name,
            RequiredOnce(Pair("Ekikibo", station_type)) => infer_parse::<StationType>,
            OptionalOnce(Pair("EkimeiJikokuRyaku", timetable_abbreviation)) => infer_name,
            OptionalOnce(Pair("EkimeiDiaRyaku", diagram_abbreviation)) => infer_name,
            // There is a spelling mistake in the original software. Instead of "Brunch" it should be "Branch"
            OptionalOnce(Pair("BrunchCoreEkiIndex", branch_index)) => infer_parse::<usize>,
            OptionalOnce(Pair("LoopOriginEkiIndex", loop_index)) => infer_parse::<usize>,
            OptionalOnce(Struct("EkiTrack2Cont", all_tracks)) => pass,
        );
        let mut tracks = SmallVec::new();
        for (_, ast) in all_tracks.into_iter().flatten().every_struct("EkiTrack2") {
            parse_fields!(ast;
                RequiredOnce(Pair("TrackName", name)) => infer_name,
                RequiredOnce(Pair("TrackRyakusyou", abbreviation)) => infer_name,
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
        parse_fields!(value;
            OptionalOnce(Pair("DiaName", name)) => infer_name,
            Many(Struct("Nobori", up_trips)) => pass,
            Many(Struct("Kudari", down_trips)) => pass,
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
        parse_fields!(value;
            OptionalOnce(Pair("Ressyabangou", name)) => infer_name,
            OptionalOnce(Pair("Bikou", comment)) => infer_name,
            RequiredOnce(Pair("Houkou", direction)) => infer_parse::<Direction>,
            RequiredOnce(Pair("Syubetsu", class_index)) => infer_parse::<usize>,
            RequiredOnce(Pair("EkiJikoku", times)) =>
                |v: &[Cow<'a, str>]| -> Result<_, IrConversionError> {
                let mut times = Vec::with_capacity(v.len());
                for entry in v {
                    let v = parse_to_timetable_entry(entry).unwrap();
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
            let operations = vals
                .iter()
                .map(|it| parse_to_raw_operation(it))
                .collect::<Result<Vec<_>, _>>()?;
            times.insert_operations(hierarchy, operations);
        }
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
        parse_fields!(value;
            RequiredOnce(Pair("Syubetsumei", name)) => infer_name,
            OptionalOnce(Pair("Ryakusyou", abbreviation)) => infer_name,
            RequiredOnce(Pair("DiagramSenColor", diagram_line_color)) => infer_parse::<Color>,
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

    fn get_ir() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample.oud2");
        let ast = parse_to_ast(s)?;
        Root::try_from(ast.as_slice())
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
