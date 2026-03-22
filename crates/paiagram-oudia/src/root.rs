use crate::timetable::TimetableEntry;
use crate::{SerializeToOud, time::Time};
use smallvec::SmallVec;
use thiserror::Error;

/// The root of the structure
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    /// File type. Usually the software name + version.
    pub file_type: String,
    /// All routes in the file. In most cases there only be a single route in the file.
    pub routes: SmallVec<[Route; 1]>,
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
    name: String,
    tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    name: String,
    abbreviation: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Color([u8; 4]);

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

/// A train class. E.g., local, express.
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    name: String,
    /// An optional abbreviation.
    abbreviation: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    diagram_line_color: Color,
}

/// A timetable set.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    name: Option<String>,
    trips: Vec<Trip>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    /// Nobori
    Up,
    /// Kudari
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    name: Option<String>,
    direction: Direction,
    class_index: usize,
    times: Vec<TimetableEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation<'a> {
    name: String,
    trips: Vec<&'a Trip>,
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
use crate::ast::Structure::{self, *};

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
    #[error("Failed to parse integer: {0} ({0})")]
    ParseIntError(std::num::ParseIntError, String),
}

fn get_name<'a: 'r, 'r>(
    it: impl GetItemWithKey<'a, 'r>,
    processing: &'static str,
    missing: &'static str,
) -> Result<String, IrConversionError> {
    let Some(Pair(_, v)) = it.once(missing) else {
        return Err(IrConversionError::MissingField {
            processing,
            missing,
        });
    };
    let name = v
        .get(0)
        .ok_or_else(|| IrConversionError::IndexOutOfBounds {
            field: missing,
            processing,
            index: 0,
            len: v.len(),
        })?
        .to_string();
    Ok(name)
}

impl<'a> TryFrom<&[Structure<'a>]> for Root {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        let file_type = get_name(value.iter(), "Root", "FileType")?;
        let mut routes = SmallVec::new();
        for (_, fields) in value.iter().every_struct("Rosen") {
            routes.push(fields.try_into()?);
        }
        Ok(Self { file_type, routes })
    }
}

impl<'a> TryFrom<&[Structure<'a>]> for Route {
    type Error = IrConversionError;
    fn try_from(value: &[Structure<'a>]) -> Result<Self, Self::Error> {
        let name = get_name(value.iter(), "Route", "Rosenmei")?;
        let mut stations = Vec::new();
        let mut classes = Vec::new();
        let mut diagrams = Vec::new();
        let display_start_time_string = get_name(value.iter(), "Route", "KitenJikoku")?;
        let display_start_time = display_start_time_string
            .parse::<Time>()
            .map_err(|e| IrConversionError::ParseIntError(e, display_start_time_string))?;
        let comment = get_name(value.iter(), "Route", "Comment")?;
        for (_, vals) in value.iter().every_struct("Eki") {
            stations.push(vals.try_into()?);
        }
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
        let name = get_name(value.iter(), "Station", "Ekimei")?;
        let mut tracks = Vec::new();
        if let Some(Struct(_, vals)) = value.iter().once("EkiTrack2Cont") {
            for (_, track_vals) in vals.iter().every_struct("EkiTrack2") {
                let name = get_name(track_vals.iter(), "Track", "TrackName")?;
                let abbreviation = get_name(track_vals.iter(), "Track", "TrackRyakusyou")?;
                tracks.push(Track { name, abbreviation });
            }
        }
        Ok(Self { name, tracks })
    }
}

use crate::{pair, structure};

impl SerializeToOud for Root {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        unimplemented!();
        let mut v = vec![pair!("FileType" => self.file_type)];
        for route in &self.routes {}
        v.serialize_oud_to(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::parse_to_ast;
    type E = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_ast_to_ir() -> E {
        let s = include_str!("../test/sample.oud2");
        let ast = parse_to_ast(s)?;
        let ir = Root::try_from(ast.as_slice())?;
        println!("{ir:#?}");
        Ok(())
    }
}
