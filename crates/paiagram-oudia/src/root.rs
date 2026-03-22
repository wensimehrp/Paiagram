use std::borrow::Cow;
use crate::time::Time;
use crate::timetable::TimetableEntry;

pub struct Root<'a> {
    pub file_type: Cow<'a, str>,
    pub routes: Vec<Route<'a>>,
}

pub struct Route<'a> {
    pub name: Cow<'a, str>,
    pub stations: Vec<Station<'a>>,
    pub classes: Vec<Class<'a>>,
    pub diagrams: Vec<Diagram<'a>>,
    pub start_time: Time,
    pub comment: Cow<'a, str>,
}

pub struct Station<'a> {
    name: Cow<'a, str>,
    // TODO: complete more fields
}

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

pub struct Class<'a> {
    name: Cow<'a, str>,
    abbreviation: Option<Cow<'a, str>>,
    diagram_line_color: Color,
}

pub struct Diagram<'a> {
    name: Option<Cow<'a, str>>,
    trips: Vec<Trip<'a>>,
}

pub enum Direction {
    Up,
    Down
}

pub struct Trip<'a> {
    name: Option<Cow<'a, str>>,
    direction: Direction,
    class_index: usize,
    times: Vec<TimetableEntry<'a>>
}

pub struct Operation<'a> {
    name: String,
    trips: Vec<&'a Trip<'a>>
}

impl<'a> Diagram<'a> {
    fn operations(&self) -> impl Iterator<Item = Operation<'a>> {
        unimplemented!();
        [].into_iter()
    }
}
