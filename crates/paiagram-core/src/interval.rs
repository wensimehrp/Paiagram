use bevy::{ecs::query::QueryData, prelude::*};

use crate::units::distance::Distance;

/// Intervals
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct Interval {
    pub length: Distance,
}

#[derive(QueryData)]
pub struct IntervalQuery {
    distance: &'static Interval,
}

impl<'w, 's> IntervalQueryItem<'w, 's> {
    pub fn distance(&self) -> Distance {
        self.distance.length
    }
}

#[derive(EntityEvent)]
pub struct UpdateInterval {
    pub entity: Entity,
    pub source: Entity,
    pub target: Entity,
}
