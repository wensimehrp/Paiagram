use crate::{
    route::Route,
    trip::class::DisplayedStroke,
};

use super::{Navigatable, Tab};
use bevy::prelude::*;
use egui::{Margin, Painter, Pos2, Sense, Ui, Vec2};
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

mod calc_trip_lines;
mod draw_lines;

#[derive(Serialize, Deserialize, Clone, Copy)]
enum SelectedItem {
    TimetableEntry { entry: Entity, parent: Entity },
    Interval(Entity, Entity),
    Station(Entity),
}

// TODO: dt & td graphs
#[derive(Serialize, Deserialize, Clone)]
pub struct DiagramTab {
    /// X offset as ticks
    x_offset: i64,
    y_offset: f32,
    zoom: Vec2,
    selected: Option<SelectedItem>,
    route_entity: Entity,
    // cache zone
    max_height: f32,
    trips: Vec<Entity>,
}

impl DiagramTab {
    pub fn new(route_entity: Entity) -> Self {
        Self {
            x_offset: 0,
            y_offset: 0.0,
            zoom: Vec2::splat(1.0),
            selected: None,
            route_entity,
            max_height: 0.0,
            trips: Vec::new(),
        }
    }
    pub fn time_view(&self, rect: egui::Rect) -> (std::ops::Range<i64>, f64) {
        let width = rect.width().max(1.0);
        let zoom_x = self.zoom.x.max(f32::EPSILON);

        let visible_ticks = self.x_offset..self.x_offset + (width as f64 / zoom_x as f64) as i64;
        let ticks_per_screen_unit = (visible_ticks.end - visible_ticks.start) as f64 / width as f64;

        (visible_ticks, ticks_per_screen_unit)
    }
}

impl MapEntities for DiagramTab {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.route_entity.map_entities(entity_mapper);
    }
}

impl Navigatable for DiagramTab {
    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom = Vec2::new(zoom_x, zoom_y);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset as f64
    }
    fn offset_y(&self) -> f32 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f32) {
        self.x_offset = offset_x.round() as i64;
        self.y_offset = offset_y;
    }
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x.clamp(0.00001, 0.4), zoom_y.clamp(0.025, 2048.0))
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        self.x_offset = self.x_offset.clamp(
            -366 * 86400 * TICKS_PER_SECOND,
            366 * 86400 * TICKS_PER_SECOND
                - (response.rect.width() as f64 / self.zoom.x as f64) as i64,
        );
        const TOP_BOTTOM_PADDING: f32 = 30.0;
        self.y_offset = if response.rect.height() / self.zoom.y
            > (self.max_height + TOP_BOTTOM_PADDING * 2.0 / self.zoom.y)
        {
            (-response.rect.height() / self.zoom.y + self.max_height) / 2.0
        } else {
            self.y_offset.clamp(
                -TOP_BOTTOM_PADDING / self.zoom.y,
                self.max_height - response.rect.height() / self.zoom.y
                    + TOP_BOTTOM_PADDING / self.zoom.y,
            )
        }
    }
}

/// Time and time-canvas related constant. How many ticks is a second
const TICKS_PER_SECOND: i64 = 100;

#[derive(Debug)]
pub struct DrawnTrip {
    entity: Entity,
    stroke: DisplayedStroke,
    points: Vec<Vec<[Pos2; 4]>>,
    entries: Vec<Vec<Entity>>,
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .show(ui, |ui| {
                let route = world
                    .get::<Route>(self.route_entity)
                    .expect("Entity should have a route");
                let station_heights: Vec<_> = route
                    .iter()
                    .map(|(e, h)| (e, h, world.get::<Name>(e).unwrap().as_str()))
                    .collect();
                self.max_height = station_heights.last().map_or(0.0, |(_, h, _)| *h);
                let _route_name = world.get::<Name>(self.route_entity).unwrap().as_str();
                let (response, mut painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
                // note that order matters here: navigation must be handled before anything else is calculated
                self.handle_navigation(ui, &response);
                let (visible_ticks, ticks_per_screen_unit) = self.time_view(response.rect);
                draw_lines::draw_station_lines(
                    self.y_offset,
                    &mut painter,
                    response.rect,
                    self.zoom.y,
                    station_heights.iter().copied(),
                    ui.pixels_per_point(),
                );
                draw_lines::draw_time_lines(
                    self.x_offset,
                    &mut painter,
                    response.rect,
                    ticks_per_screen_unit,
                    &visible_ticks,
                    ui.pixels_per_point(),
                );
                world
                    .run_system_cached_with(
                        calc_trip_lines::calculate_trips,
                        (&mut self.trips, self.route_entity),
                    )
                    .unwrap();
                let mut trip_line_buf = Vec::new();
                world
                    .run_system_cached_with(
                        calc_trip_lines::calc,
                        (
                            &mut trip_line_buf,
                            &self,
                            response.rect,
                            ticks_per_screen_unit,
                            visible_ticks.clone(),
                        ),
                    )
                    .unwrap();
                world
                    .run_system_cached_with(draw_lines, (&trip_line_buf, ui, &mut painter))
                    .unwrap();
            });
    }
}

/// Takes a buffer the calculate trains

fn draw_lines(
    (InRef(trips), InMut(ui), InMut(painter)): (InRef<[DrawnTrip]>, InMut<Ui>, InMut<Painter>),
) {
    let is_dark = ui.visuals().dark_mode;
    for trip in trips {
        let stroke = egui::Stroke {
            width: trip.stroke.width,
            color: trip.stroke.color.get(is_dark),
        };
        for group in &trip.points {
            let mut points = Vec::with_capacity(group.len() * 4);
            for segment in group {
                points.extend(segment.iter().copied());
            }
            if points.len() >= 2 {
                painter.line(points, stroke);
            }
        }
    }
}
