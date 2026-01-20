use crate::graph::Interval;
use crate::units::distance::Distance;
use crate::{graph::Station, units::time::TimetableTime};
use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use moonshine_core::kind::{Instance, SpawnInstance};
use moonshine_core::save::prelude::*;

/// Displayed line type:
/// A list of (station entity, size of the interval on canvas in mm)
/// The first entry is the starting station, where the canvas distance is simply omitted.
/// Each entry afterwards represents the interval from the previous station to this station.
pub type DisplayedLineType = Vec<(Instance<Station>, f32)>;

pub type RulerLineType = Vec<(Instance<Station>, TimetableTime)>;

#[derive(Reflect, Debug, Default)]
pub enum ScaleMode {
    Linear,
    #[default]
    Logarithmic,
    Uniform,
}

/// An imaginary (railway) line on the canvas, consisting of multiple segments.
#[derive(Reflect, Component, Debug)]
#[component(map_entities)]
#[reflect(Component, MapEntities)]
#[require(Name, Save)]
pub struct DisplayedLine {
    pub stations: Vec<(Instance<Station>, f32)>,
    pub scale_mode: ScaleMode,
}

impl MapEntities for DisplayedLine {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for (station, _) in &mut self.stations {
            station.map_entities(entity_mapper);
        }
    }
}

pub enum DisplayedLineError {
    InvalidIndex,
    SameStationAsNeighbor,
    AdjacentIntervals((Entity, Entity)),
}

impl std::fmt::Debug for DisplayedLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayedLineError::InvalidIndex => write!(f, "Invalid index for inserting station"),
            DisplayedLineError::SameStationAsNeighbor => {
                write!(f, "Cannot insert the same station as a neighbor")
            }
            DisplayedLineError::AdjacentIntervals((e1, e2)) => {
                write!(
                    f,
                    "Cannot insert station that would create adjacent identical intervals: ({:?}, {:?})",
                    e1, e2
                )
            }
        }
    }
}

impl DisplayedLine {
    pub fn new(stations: DisplayedLineType) -> Self {
        Self {
            stations,
            scale_mode: ScaleMode::default(),
        }
    }
    pub fn _new(stations: impl Iterator<Item = Entity>) -> Self {
        todo!("implement this stuff")
    }
    pub fn stations(&self) -> &DisplayedLineType {
        &self.stations
    }
    pub unsafe fn stations_mut(&mut self) -> &mut DisplayedLineType {
        &mut self.stations
    }
    pub fn insert(
        &mut self,
        index: usize,
        (station, height): (Instance<Station>, f32),
    ) -> Result<(), DisplayedLineError> {
        // Two same intervals cannot be neighbours
        // an interval is defined by (prev_entity, this_entity)
        if index > self.stations.len() {
            return Err(DisplayedLineError::InvalidIndex);
        };
        let prev_prev = if index >= 2 {
            Some(self.stations[index - 2].0)
        } else {
            None
        };
        let prev = if index >= 1 {
            Some(self.stations[index - 1].0)
        } else {
            None
        };
        let next = self.stations.get(index).map(|(e, _)| *e);
        let next_next = self.stations.get(index + 1).map(|(e, _)| *e);
        if let Some(prev_prev) = prev_prev
            && prev_prev == station
        {
            return Err(DisplayedLineError::AdjacentIntervals((
                prev_prev.entity(),
                prev.unwrap().entity(),
            )));
        };
        if let Some(next_next) = next_next
            && next_next == station
        {
            return Err(DisplayedLineError::AdjacentIntervals((
                next.unwrap().entity(),
                next_next.entity(),
            )));
        };
        if let Some(prev) = prev
            && prev == station
        {
            return Err(DisplayedLineError::SameStationAsNeighbor);
        };
        if let Some(next) = next
            && next == station
        {
            return Err(DisplayedLineError::SameStationAsNeighbor);
        };
        self.stations.insert(index, (station, height));
        Ok(())
    }
    pub fn push(&mut self, station: (Instance<Station>, f32)) -> Result<(), DisplayedLineError> {
        self.insert(self.stations.len(), station)
    }
}

pub fn adjust_intervals_length(
    In(entity): In<Entity>,
    graph: Res<crate::graph::Graph>,
    intervals: Query<&Interval>,
    mut displayed_lines: Query<&mut DisplayedLine>,
    names: Query<&Name>,
) {
    let mut displayed_line = match displayed_lines.get_mut(entity) {
        Ok(l) => l,
        Err(e) => {
            error!("Could not get displayed line: {:?}", e);
            return;
        }
    };
    let mut stations_iter = displayed_line.stations.iter_mut();
    let Some((prev, _)) = stations_iter.next() else {
        warn!(
            "Displayed line {} is empty, skipping...",
            names.get(entity).map_or("<unknown>", Name::as_str)
        );
        return;
    };
    let mut prev = *prev;
    for (curr, height) in stations_iter {
        let (count, acc) = graph
            .edges_connecting(prev, *curr)
            .filter_map(|r| intervals.get(r.weight.entity()).ok())
            .fold((0i32, Distance(0)), |(count, len), i| {
                (count + 1, len + i.length)
            });
        if count == 0 {
            warn!(
                "There are no intervals connecting between {} and {}, skipping",
                names.get(prev.entity()).map_or("<unknown>", Name::as_str),
                names.get(curr.entity()).map_or("<unknown>", Name::as_str)
            );
            continue;
        }
        let average_length = acc / count;
        *height = average_length.0 as f32;
        prev = *curr;
    }
}

pub fn create_intervals_from_displayed_line(
    In(line_entity): In<Entity>,
    displayed_lines: Query<&DisplayedLine>,
    mut graph: ResMut<crate::graph::Graph>,
    mut commands: Commands,
    stations: Query<&Station>,
) {
    let line = match displayed_lines.get(line_entity) {
        Ok(l) => l,
        Err(e) => {
            error!("Could not find displayed line: {:?}", e);
            return;
        }
    };
    // TODO: switch to array_windows in the future
    for w in line.stations.windows(2) {
        let [(prev, _), (curr, _)] = w else {
            unreachable!()
        };
        let length = {
            let Ok(p) = stations.get(prev.entity()) else {
                return;
            };
            let Ok(c) = stations.get(curr.entity()) else {
                return;
            };
            Distance(p.0.distance(c.0) as i32)
        };
        if !graph.contains_edge(*prev, *curr) {
            let i1 = commands
                .spawn_instance(Interval {
                    length,
                    speed_limit: None,
                })
                .instance();
            graph.add_edge(*prev, *curr, i1);
        }
        if !graph.contains_edge(*curr, *prev) {
            let i2 = commands
                .spawn_instance(Interval {
                    length,
                    speed_limit: None,
                })
                .instance();
            graph.add_edge(*curr, *prev, i2);
        }
    }
}

#[derive(Component, Debug)]
#[require(Name)]
pub struct RulerLine(pub RulerLineType);

pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {}
}
