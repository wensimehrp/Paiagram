use crate::{route::Route, trip::class::DisplayedStroke};

use super::{Navigatable, Tab};
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use egui::{Color32, Margin, Painter, Pos2, Rect, Sense, Ui, Vec2};
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

mod calc_trip_lines;
mod draw_lines;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
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
                if response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    if self.selected.is_some() {
                        self.selected = None
                    } else {
                        self.selected = handle_selection(&trip_line_buf, pos);
                    }
                }
                match self.selected {
                    Some(SelectedItem::TimetableEntry { entry, parent }) => {}
                    _ => {}
                }
                let selection_strength = ui
                    .ctx()
                    .animate_bool(ui.id().with("selection"), self.selected.is_some());
                let selected_idx_rect = world
                    .run_system_once_with(
                        draw_lines,
                        (&trip_line_buf, ui, &mut painter, self.selected),
                    )
                    .unwrap();
                let s = (selection_strength * 0.5 * u8::MAX as f32) as u8;
                painter.rect_filled(
                    response.rect,
                    0,
                    if ui.visuals().dark_mode {
                        Color32::from_black_alpha(s)
                    } else {
                        Color32::from_white_alpha(s)
                    },
                );
                if let Some((idx, rects)) = selected_idx_rect {
                    let trip = &trip_line_buf[idx];
                    let stroke = egui::Stroke {
                        width: trip.stroke.width + 3.0 * selection_strength * trip.stroke.width,
                        color: trip.stroke.color.get(ui.visuals().dark_mode),
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
                    for rect in rects {
                        painter.rect(
                            rect,
                            8,
                            Color32::BLUE.gamma_multiply(0.5),
                            egui::Stroke {
                                width: 1.0,
                                color: Color32::BLUE,
                            },
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            });
    }
}

/// Takes a buffer the calculate trains

fn draw_lines<'a>(
    (InRef(trips), InMut(ui), InMut(painter), In(selected)): (
        InRef<[DrawnTrip]>,
        InMut<Ui>,
        InMut<Painter>,
        In<Option<SelectedItem>>,
    ),
) -> Option<(usize, Vec<Rect>)> {
    let is_dark = ui.visuals().dark_mode;
    let mut ret = None;
    for (idx, trip) in trips.iter().enumerate() {
        if let Some(SelectedItem::TimetableEntry { entry, parent }) = selected
            && trip.entity == parent
        {
            let rects = trip
                .points
                .iter()
                .flatten()
                .zip(trip.entries.iter().flatten())
                .filter_map(|(p, e)| {
                    if *e == entry {
                        Some(Rect::from_two_pos(p[1], p[2]).expand(8.0))
                    } else {
                        None
                    }
                })
                .collect();
            ret = Some((idx, rects));
            continue;
        }
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
    ret
}

fn handle_selection(drawn_trips: &[DrawnTrip], pos: Pos2) -> Option<SelectedItem> {
    const VEHICLE_SELECTION_RADIUS: f32 = 7.0;
    const STATION_SELECTION_RADIUS: f32 = VEHICLE_SELECTION_RADIUS;
    for trip in drawn_trips {
        for (points, entries) in trip.points.iter().zip(trip.entries.iter()) {
            let entries_iter = entries
                .iter()
                .flat_map(|it| std::iter::repeat(it).take(4))
                .copied();
            for (w, e) in points.as_flattened().windows(2).zip(entries_iter) {
                let [curr, next] = w else { unreachable!() };
                let a = pos.x - curr.x;
                let b = pos.y - curr.y;
                let c = next.x - curr.x;
                let d = next.y - curr.y;
                let dot = a * c + b * d;
                let len_sq = c * c + d * d;
                if len_sq == 0.0 {
                    continue;
                }
                let t = (dot / len_sq).clamp(0.0, 1.0);
                let px = curr.x + t * c;
                let py = curr.y + t * d;
                let dx = pos.x - px;
                let dy = pos.y - py;

                if dx * dx + dy * dy < VEHICLE_SELECTION_RADIUS.powi(2) {
                    return Some(SelectedItem::TimetableEntry {
                        entry: e,
                        parent: trip.entity,
                    });
                }
            }
        }
    }
    return None;
}
