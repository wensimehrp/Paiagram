use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
use crate::time::Time;
use crate::timetable::{TimetableEntry, parse_to_timetable_entry};
use smallvec::SmallVec;
use std::borrow::Cow;
use thiserror::Error;

/// The root of the structure
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    /// File type. Usually the software name + version.
    pub file_type: String,
    /// The route in the file.
    pub route: Route,
}

/// A route (路線).
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// The name of the route
    pub name: String,
    /// What stations are included in the route
    pub stations: Vec<Station>,
    /// The available train classes. E.g., local, express.
    pub classes: Vec<Class>,
    /// The diagrams included in this route. Each diagram is a timetable set.
    pub diagrams: Vec<Diagram>,
    /// When to start displaying times on the diagram page.
    pub display_start_time: Time,
    pub comment: String,
}

/// A station on the route.
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub name: String,
    /// The abbreviation used in timetables.
    pub timetable_abbreviation: Option<String>,
    /// The abbreviation used in diagrams.
    pub diagram_abbreviation: Option<String>,
    pub branch_index: Option<usize>,
    pub loop_index: Option<usize>,
    pub tracks: SmallVec<[Track; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    /// An optional abbreviation.
    pub abbreviation: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    pub diagram_line_color: Color,
}

/// A timetable set.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub name: Option<String>,
    pub trips: Vec<Trip>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    /// Nobori
    Up,
    /// Kudari
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

#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    pub name: Option<String>,
    pub direction: Direction,
    pub class_index: usize,
    pub times: Vec<TimetableEntry>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation<'a> {
    pub name: String,
    pub trips: Vec<&'a Trip>,
}

impl Diagram {
    fn trips_at_station(&self, index: usize) -> impl Iterator<Item = Trip> {
        unimplemented!();
        [].into_iter()
    }
    fn operations(&self) -> impl Iterator<Item = Operation> {
        unimplemented!();
        [].into_iter()
    }
}

use crate::ast::GetItemWithKey;
use crate::ast::Structure;

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
}

fn infer_name(v: &[Cow<'_, str>]) -> Result<String, IrConversionError> {
    Ok(v[0].to_string())
}

fn infer_parse<T>(v: &[Cow<'_, str>]) -> Result<T, IrConversionError>
where
    T: std::str::FromStr,
    IrConversionError: From<T::Err>,
{
    v[0].parse::<T>().map_err(IrConversionError::from)
}

fn pass<'r, 'a>(v: &'r [Structure<'a>]) -> Result<&'r [Structure<'a>], IrConversionError> {
    Ok(v)
}

macro_rules! parse_fields {
    ($iter:expr; $($once_or_many:ident($variant:ident($key:expr, $variable_name:ident)) => $action:expr,)*) => {
        $(
            parse_fields!(@make_variable $once_or_many($variable_name));
        )*
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
        Ok(Self {
            file_type,
            route,
        })
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
        })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Diagram {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        parse_fields!(value;
            OptionalOnce(Pair("DiaName", name)) => infer_name,
            Many(Struct("Kudari", down_trips)) => pass,
            Many(Struct("Kudari", up_trips)) => pass,
        );
        let mut trips = Vec::new();
        let down_trips_iter = down_trips.into_iter().flatten();
        let up_trips_iter = up_trips.into_iter().flatten();
        for (_, trip) in down_trips_iter.chain(up_trips_iter).every_struct("Ressya") {
            trips.push(Trip::try_from(trip)?)
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::parse_to_ast;
    type E = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_ast_to_ir() -> E {
        let s = include_str!("../test/sample2.oud2");
        let ast = parse_to_ast(s)?;
        let ir = Root::try_from(ast.as_slice())?;
        println!("{ir:#?}");
        Ok(())
    }
}
