//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use crate::{
    graph::Graph,
    interval::Interval,
    station::Station,
    trip::class::{Class, ClassBundle},
    units::{distance::Distance, time::{Duration, TimetableTime}},
};
use bevy::{platform::collections::HashMap, prelude::*};
use moonshine_core::kind::*;

mod qetrc;
mod oudia;

pub struct ImportPlugin;
impl Plugin for ImportPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(qetrc::load_qetrc).add_observer(oudia::load_oud2);
    }
}

#[derive(Event)]
pub struct LoadQETRC {
    pub content: String,
}

#[derive(Event)]
pub struct LoadOuDiaSecond {
    pub content: String,
}

fn normalize_times<'a>(mut time_iter: impl Iterator<Item = &'a mut TimetableTime> + 'a) {
    let Some(mut previous_time) = time_iter.next().copied() else {
        return;
    };
    for time in time_iter {
        while *time < previous_time {
            *time += Duration(86400);
        }
        previous_time = *time;
    }
}

pub(crate) fn make_station(
    name: &str,
    station_map: &mut HashMap<String, Instance<Station>>,
    graph: &mut Graph,
    commands: &mut Commands,
) -> Instance<Station> {
    if let Some(&entity) = station_map.get(name) {
        return entity;
    }
    let station_entity = commands
        .spawn(Name::new(name.to_string()))
        .insert_instance(Station::default())
        .into();
    station_map.insert(name.to_string(), station_entity);
    graph.add_node(station_entity.entity());
    station_entity
}

pub(crate) fn make_class(
    name: &str,
    class_map: &mut HashMap<String, Instance<Class>>,
    commands: &mut Commands,
    mut make_class: impl FnMut() -> ClassBundle,
) -> Instance<Class> {
    if let Some(&entity) = class_map.get(name) {
        return entity;
    };
    let class_bundle = make_class();
    let class_entity = commands
        .spawn((class_bundle.name, class_bundle.stroke))
        .insert_instance(class_bundle.class)
        .into();
    class_map.insert(name.to_string(), class_entity);
    class_entity
}

pub(crate) fn add_interval_pair(
    graph: &mut Graph,
    commands: &mut Commands,
    from: Entity,
    to: Entity,
    length: Distance,
) {
    if graph.contains_edge(from, to) || graph.contains_edge(to, from) {
        return;
    }
    let e1: Instance<Interval> = commands.spawn_instance(Interval { length }).into();
    let e2: Instance<Interval> = commands.spawn_instance(Interval { length }).into();
    graph.add_edge(from, to, e1.entity());
    graph.add_edge(to, from, e2.entity());
}
