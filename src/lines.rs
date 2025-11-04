use crate::basic::*;
use bevy::prelude::*;

/// Displayed line type:
/// A list of (station entity, size of the interval on canvas in mm)
/// The first entry is the starting station, where the canvas distance is simply omitted.
/// Each entry afterwards represents the interval from the previous station to this station.
pub type DisplayedLineType = Vec<(Entity, CanvasDistance)>;

pub type RulerLineType = Vec<(Entity, TimetableTime)>;

/// An imaginary (railway) line on the canvas, consisting of multiple segments.
#[derive(Component, Reflect, Debug)]
#[require(Name)]
pub struct DisplayedLine(pub DisplayedLineType);

#[derive(Component, Reflect, Debug)]
#[require(Name)]
pub struct RulerLine(pub RulerLineType);
