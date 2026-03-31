use crate::tabs::diagram::calc_trip_lines::calc;
use crate::tabs::diagram::{DiagramTabNavigation, DrawnTrip};
use bevy::prelude::*;
use egui::{Color32, Vec2};
use egui::{Pos2, Rect};
use paiagram_core::export::ExportObject;
use paiagram_core::route::Route;
use paiagram_core::units::time::{Tick, TimetableTime};
use serde::Serialize;
use std::io::Write;

pub struct TypstModule;

impl ExportObject for TypstModule {
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        buffer
            .write_all(include_bytes!("./typst_diagram.typ"))
            .unwrap();
    }
    fn extension(&self) -> impl AsRef<str> {
        ".typ"
    }
}

pub struct TypstDiagram<'a> {
    pub route_entity: Entity,
    pub world: &'a mut World,
}

#[derive(Serialize)]
struct OutputRoot {
    stations: Vec<(String, f32)>,
    trips: Vec<TripsOutput>,
}

#[derive(Serialize)]
struct TripsOutput {
    name: String,
    color: Color32,
    points: Vec<Vec<[Pos2; 4]>>,
}

fn default_calc_context(route: &Route) -> DiagramTabNavigation {
    let max_ticks = Tick::from_timetable_time(TimetableTime(24 * 60 * 60));
    let width: f32 = 1200.0;
    let max_height = route.iter().last().map(|(_, h)| h).unwrap_or(0.0).max(1.0);
    let zoom_x = width / max_ticks.0 as f32;

    DiagramTabNavigation {
        x_offset: Tick::ZERO,
        y_offset: 0.0,
        zoom: Vec2::new(zoom_x, 1.0),
        visible_rect: Rect::from_two_pos(Pos2::new(0.0, 0.0), Pos2::new(width, max_height)),
        max_height,
    }
}

impl<'a> ExportObject for TypstDiagram<'a> {
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        let route = self.world.get::<Route>(self.route_entity).unwrap();
        let mut rendered_vehicle_buf = Vec::new();
        let ctx = default_calc_context(&route);
        unimplemented!();
        // self.world
        //     .run_system_cached_with(calc, (&mut rendered_vehicle_buf, &ctx, self.route_entity))
        //     .unwrap();
        let mut stations_output = Vec::new();
        let mut trips_output = Vec::new();
        self.world
            .run_system_cached_with(
                write_stations,
                (self.route_entity, &mut stations_output, 1.0),
            )
            .unwrap();
        self.world
            .run_system_cached_with(write_trips, (&rendered_vehicle_buf, &mut trips_output))
            .unwrap();
        let root = OutputRoot {
            stations: stations_output,
            trips: trips_output,
        };
        serde_json::to_writer(buffer, &root).unwrap();
    }
    fn extension(&self) -> impl AsRef<str> {
        ".json"
    }
    fn filename(&self) -> impl AsRef<str> {
        "exported_diagram_data"
    }
}

fn write_stations(
    (In(route_entity), InMut(buf), In(zoom)): (In<Entity>, InMut<Vec<(String, f32)>>, In<f32>),
    route_q: Query<&Route>,
    names: Query<&Name>,
) {
    let route = route_q.get(route_entity).unwrap();
    buf.extend(
        route
            .stops
            .iter()
            .cloned()
            .map(|it| names.get(it).unwrap().to_string())
            .zip(route.lengths.iter().cloned().map(|it| it * zoom)),
    );
}

fn write_trips(
    (InRef(trips), InMut(buf)): (InRef<[DrawnTrip]>, InMut<Vec<TripsOutput>>),
    name_q: Query<&Name>,
) {
    buf.extend(trips.iter().map(|it| {
        TripsOutput {
            name: name_q
                .get(it.entity)
                .map_or("<unknown>".to_string(), Name::to_string),
            color: it.stroke.color.get(false),
            points: it.points.clone(),
        }
    }));
}
