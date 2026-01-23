use std::{collections::HashMap, io::Cursor};

use bevy::prelude::*;
use egui::Pos2;
use moonshine_core::kind::{InsertInstance, Instance};

use crate::{
    graph::{Graph, Station},
    units::time::TimetableTime,
    vehicles::{
        Vehicle,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};

#[derive(Event, Deref)]
pub struct GtfsLoaded(pub Vec<u8>);

pub fn load_gtfs_static(data: On<GtfsLoaded>, mut commands: Commands, mut graph: ResMut<Graph>) {
    let reader = Cursor::new(data.as_slice());
    let Ok(gtfs) = gtfs_structures::Gtfs::from_reader(reader) else {
        return;
    };
    let mut station_map: HashMap<String, Instance<Station>> =
        HashMap::with_capacity(gtfs.stops.len());
    for (_, stop) in gtfs.stops.iter() {
        let longitude = stop.longitude.unwrap_or_default();
        let latitude = stop.latitude.unwrap_or_default();
        let name = stop.name.as_ref().unwrap_or(&stop.id);
        let (northing, easting, _) = utm::to_utm_wgs84_no_zone(latitude, longitude);
        let station_instance = commands
            .spawn((Name::new(name.clone()),))
            .insert_instance(Station(Pos2 {
                x: easting as f32,
                y: -northing as f32,
            }))
            .instance();
        graph.add_node(station_instance);
        station_map.insert(stop.id.clone(), station_instance);
    }
    let vehicle_set_entity = commands
        .spawn((Name::new("New GTFS Import"), VehicleSet))
        .id();
    for (_, trip) in gtfs.trips.into_iter() {
        let vehicle_entity = commands
            .spawn(Name::new(trip.route_id))
            .insert_instance(Vehicle)
            .id();
        commands
            .entity(vehicle_set_entity)
            .add_child(vehicle_entity);
        let mut schedule_entities = Vec::new();
        for stop_time in trip.stop_times {
            let arrival = stop_time.arrival_time.map_or(TravelMode::Flexible, |t| {
                TravelMode::At(TimetableTime(t as i32))
            });
            let departure = Some(stop_time.departure_time.map_or(TravelMode::Flexible, |t| {
                TravelMode::At(TimetableTime(t as i32))
            }));
            let Some(station_instance) = station_map.get(&stop_time.stop.id) else {
                continue;
            };
            let entry_entity = commands
                .spawn(TimetableEntry {
                    station: station_instance.entity(),
                    arrival,
                    departure,
                    service: None,
                    track: None,
                })
                .id();
            commands.entity(vehicle_entity).add_child(entry_entity);
            schedule_entities.push(entry_entity);
        }
        commands.entity(vehicle_entity).insert(VehicleSchedule {
            entities: schedule_entities,
            ..Default::default()
        });
    }
}
