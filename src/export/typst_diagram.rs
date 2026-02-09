use crate::route::Route;
use crate::ui::tabs::diagram::DrawnTrip;
use crate::ui::tabs::diagram::TICKS_PER_SECOND;
use crate::ui::tabs::diagram::calc_trip_lines::{CalcContext, calc};
use bevy::prelude::*;
use egui::Color32;
use egui::{Pos2, Rect};
use serde::Serialize;
use std::io::Write;

pub struct TypstModule;

impl super::ExportObject<()> for TypstModule {
    fn export_to_buffer(&mut self, world: &mut World, buffer: &mut Vec<u8>, input: ()) {
        buffer.write(include_bytes!("./typst_diagram.typ")).unwrap();
    }
}

pub struct TypstDiagram;

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

fn default_calc_context(route: &Route, route_entity: Entity) -> CalcContext {
    let max_ticks: i64 = 24 * 60 * 60 * TICKS_PER_SECOND;
    let width: f32 = 1200.0;
    let max_height = route.iter().last().map(|(_, h)| h).unwrap_or(0.0).max(1.0);
    let height = max_height + 100.0;
    let ticks_per_screen_unit = max_ticks as f64 / width as f64;
    CalcContext {
        route_entity,
        y_offset: 0.0,
        zoom_y: 1.0,
        x_offset: 0,
        screen_rect: Rect {
            min: Pos2::new(0.0, 0.0),
            max: Pos2 {
                x: width,
                y: height,
            },
        },
        ticks_per_screen_unit,
        visible_ticks: 0i64..max_ticks,
    }
}

impl super::ExportObject<(Entity, &[Entity])> for TypstDiagram {
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        (route_entity, trips): (Entity, &[Entity]),
    ) {
        let mut rendered_vehicle_buf = Vec::with_capacity(trips.len());
        let route = world.get::<Route>(route_entity).unwrap();
        let ctx = default_calc_context(&route, route_entity);
        world
            .run_system_cached_with(calc, (&mut rendered_vehicle_buf, ctx, trips))
            .unwrap();
        let mut stations_output = Vec::new();
        let mut trips_output = Vec::new();
        world
            .run_system_cached_with(write_stations, (route_entity, &mut stations_output, 1.0))
            .unwrap();
        world
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
