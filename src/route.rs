//! # Route
//! Routes are slices of the graph that can be used as the foundation of diagrams.
//! Diagrams use routes as their station list.

use bevy::prelude::*;
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

pub struct RoutePlugin;
impl Plugin for RoutePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(auto_update_length);
    }
}

use crate::{
    graph::Graph,
    interval::{Interval, UpdateInterval},
};

/// Marker component for automatically updating route interval length.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct AutoUpdateLength;

#[derive(Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name)]
pub struct Route {
    #[entities]
    pub stops: Vec<Entity>,
    pub lengths: Vec<f32>,
}

impl Route {
    pub fn iter(&self) -> impl Iterator<Item = (Entity, f32)> {
        self.stops
            .iter()
            .copied()
            .zip(self.lengths.iter().copied())
            .scan(0.0_f32, |acc, (stop, len)| {
                let out = (stop, *acc);
                *acc += len;
                Some(out)
            })
    }
}

fn auto_update_length(
    updated: On<UpdateInterval>,
    routes: Populated<&mut Route, With<AutoUpdateLength>>,
    intervals: Query<&Interval>,
    graph: Res<Graph>,
) {
    for mut route in routes {
        let Route { stops, lengths } = &mut *route;
        for (i, w) in stops.windows(2).enumerate() {
            let [p, c] = w else { unreachable!() };
            let (p, c) = (*p, *c);
            if (p == updated.source && c == updated.target)
                || (p == updated.target && c == updated.source)
            {
            } else {
                continue;
            }
            let i1 = graph.edge_weight(p, c).cloned();
            let i2 = graph.edge_weight(c, p).cloned();
            match (i1, i2) {
                (Some(e1), Some(e2)) => {
                    let d1 = intervals.get(e1).unwrap().length;
                    let d2 = intervals.get(e2).unwrap().length;
                    let avg_len = (d1.0 as f32 + d2.0 as f32) / 2.0;
                    lengths[i] = avg_len;
                }
                (Some(e), None) | (None, Some(e)) => {
                    let d = intervals.get(e).unwrap().length;
                    lengths[i] = d.0 as f32;
                }
                (None, None) => {
                    panic!("Interval disappeared???")
                }
            }
        }
    }
}
