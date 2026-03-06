use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use moonshine_core::kind::Instance;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    class::{Class, ClassBundle, DisplayedStroke},
    entry::{EntryBundle, TravelMode},
    graph::Graph,
    route::Route,
    station::Station,
    trip::{TripBundle, TripClass},
    units::{distance::Distance, time::TimetableTime},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableData {
    pub stations: Vec<String>,
    pub lines: Vec<Line>,
    pub trains: Vec<Train>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub code: i64,
    pub name: String,
    pub stations: Vec<LineStation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineStation {
    pub station: String,
    pub telecode: String,
    #[serde(rename = "routeFlag")]
    pub route_flag: i64,
    pub distance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Train {
    pub number: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub stops: Vec<TrainStop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainStop {
    pub station: String,
    #[serde(rename = "type")]
    pub r#type: StopType,
    pub line_code: i64,
    pub arrival_time: String,
    pub departure_time: String,
    pub mileage: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopType {
    Stop,
    Crossline,
}

pub fn load_llt(msg: On<super::LoadLlt>, mut graph: ResMut<Graph>, mut commands: Commands) {
    let root: TimetableData = serde_json::from_str(&msg.content).unwrap();
    let mut station_map: HashMap<String, Instance<Station>> = HashMap::new();
    let mut class_map: HashMap<String, Instance<Class>> = HashMap::new();
    for station_name in root.stations {
        super::make_station(&station_name, &mut station_map, &mut graph, &mut commands);
    }
    for Line {
        code: _,
        name: line_name,
        stations,
    } in root.lines
    {
        let mut station_entities = Vec::with_capacity(stations.len());
        let mut distances: Vec<f32> = Vec::with_capacity(stations.len());
        let mut station_iter = stations.into_iter();
        let Some(station) = station_iter.next() else {
            continue;
        };
        let mut previous_station = super::make_station(
            &station.station,
            &mut station_map,
            &mut graph,
            &mut commands,
        );
        let mut previous_distance = station.distance;
        station_entities.push(previous_station.entity());
        distances.push(0.0f32);
        for station in station_iter {
            let current_station = super::make_station(
                &station.station,
                &mut station_map,
                &mut graph,
                &mut commands,
            );
            let interval_length = station.distance - previous_distance;
            super::add_interval_pair(
                &mut graph,
                &mut commands,
                previous_station.entity(),
                current_station.entity(),
                Distance::from_m(interval_length as i32 * 1000),
            );
            previous_distance = station.distance;
            previous_station = current_station;
            station_entities.push(previous_station.entity());
            distances.push(interval_length as f32);
        }
        commands.spawn((
            Route {
                stops: station_entities,
                lengths: distances,
            },
            Name::new(line_name),
        ));
    }
    for Train {
        number: trip_name,
        r#type,
        stops,
    } in root.trains
    {
        let mut entries: Vec<_> = stops
            .into_iter()
            .map(|e| {
                (
                    TimetableTime::from_str(&e.arrival_time),
                    TimetableTime::from_str(&e.departure_time),
                    super::make_station(&e.station, &mut station_map, &mut graph, &mut commands),
                )
            })
            .collect();
        super::normalize_times(
            entries
                .iter_mut()
                .flat_map(|(a, d, _)| a.iter_mut().chain(d.iter_mut())),
        );
        let trip_class =
            super::make_class(&r#type, &mut class_map, &mut commands, || ClassBundle {
                class: Class::default(),
                name: Name::new(r#type.clone()),
                stroke: DisplayedStroke::from_seed(r#type.as_bytes()),
            });
        commands
            .spawn(TripBundle::new(&trip_name, TripClass(trip_class.entity())))
            .with_children(|bundle| {
                for (arr, dep, stop) in entries {
                    let (arr, dep) = match (arr, dep) {
                        (None, None) => (None, TravelMode::Flexible),
                        (Some(t), None) | (None, Some(t)) => (None, TravelMode::At(t)),
                        (Some(at), Some(dt)) => (Some(TravelMode::At(at)), TravelMode::At(dt)),
                    };
                    bundle.spawn(EntryBundle::new(arr, dep, stop.entity()));
                }
            });
    }
}
