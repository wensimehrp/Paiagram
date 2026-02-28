use core::f32;

use bevy::{ecs::entity::EntityHashMap, prelude::*};
use egui::{
    Align2, Margin, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Visuals,
    epaint::TextShape, pos2,
};
use either::Either;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

use crate::tabs::Navigatable;
use paiagram_core::{
    colors::DisplayColor,
    entry::EntryQuery,
    route::{Route, RouteByDirectionTrips},
    station::{ParentStationOrStation, Station},
    trip::TripQuery,
    units::time::TimetableTime,
};

#[derive(Serialize, Deserialize, MapEntities, Clone)]
pub struct PriorityGraphTab {
    #[entities]
    route_entity: Entity,
    navi: PriorityTabNavigation,
    #[serde(skip, default)]
    downward_priorities: Option<Vec<Vec<(Entity, TimetableTime)>>>,
    #[serde(skip, default)]
    upward_priorities: Option<Vec<(Entity, TimetableTime)>>,
}

impl PriorityGraphTab {
    pub fn new(route_entity: Entity) -> Self {
        Self {
            route_entity,
            navi: PriorityTabNavigation::default(),
            downward_priorities: None,
            upward_priorities: None,
        }
    }
}

impl PartialEq for PriorityGraphTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_entity == other.route_entity
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct PriorityTabNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: Vec2,
    visible_rect: Rect,
}

impl Default for PriorityTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: Vec2::splat(1.0),
            visible_rect: Rect::ZERO,
        }
    }
}

impl super::Navigatable for PriorityTabNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = offset_x;
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom.x = zoom_x;
        self.zoom.y = zoom_y;
    }
}

impl super::Tab for PriorityGraphTab {
    const NAME: &'static str = "Priority Graph";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        world
            .run_system_cached_with(
                calculate_priority,
                (self.route_entity, &mut self.downward_priorities, true),
            )
            .unwrap();
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| main_display(self, world, ui));
    }
}

fn calculate_priority(
    (In(route_entity), InMut(maps), In(downwards)): (
        In<Entity>,
        InMut<Option<Vec<Vec<(Entity, TimetableTime)>>>>,
        In<bool>,
    ),
    route_q: Query<(&Route, Ref<RouteByDirectionTrips>)>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
) {
    let (route, trips) = route_q.get(route_entity).unwrap();
    if maps.is_some() && !trips.is_changed() {
        return;
    }
    let maps = maps.get_or_insert_default();
    let stops = if downwards {
        Either::Left(route.stops.iter().copied())
    } else {
        Either::Right(route.stops.iter().rev().copied())
    };
    let trips = if downwards {
        trips.downward.as_slice()
    } else {
        trips.upward.as_slice()
    };
    let get_times = |stop: Entity| -> Vec<(Entity, TimetableTime)> {
        let mut v = Vec::new();
        for trip in trip_q.iter_many(trips) {
            for e in entry_q.iter_many(trip.schedule) {
                if parent_station_or_station.get(e.stop()).unwrap().parent() == stop
                    && let Some(es) = e.estimate
                {
                    v.push((trip.entity, es.dep));
                }
            }
        }
        v.sort_unstable_by_key(|(_, t)| *t);
        v
    };
    for stop in stops {
        maps.push(get_times(stop));
    }
}

fn main_display(tab: &mut PriorityGraphTab, world: &mut World, ui: &mut egui::Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible_rect = response.rect;
    tab.navi.handle_navigation(ui, &response);
    // we take samples at each station, then connect them;
    // try to straighten the ones that overpasses other trains
    world
        .run_system_cached_with(
            draw_station_lines,
            (&mut painter, tab.route_entity, &tab.navi, ui.visuals()),
        )
        .unwrap();
    world
        .run_system_cached_with(
            draw_priority_lines,
            (
                &mut painter,
                tab.route_entity,
                &tab.navi,
                tab.downward_priorities.as_deref().unwrap(),
            ),
        )
        .unwrap();
}

const STATION_SPACING: f64 = 10.0;

fn draw_station_lines(
    (InMut(painter), In(route_entity), InRef(navi), InRef(visuals)): (
        InMut<Painter>,
        In<Entity>,
        InRef<PriorityTabNavigation>,
        InRef<Visuals>,
    ),
    route_q: Query<&Route>,
    station_name_q: Query<&Name, With<Station>>,
) {
    let route = route_q.get(route_entity).unwrap();
    let stroke = Stroke {
        width: 0.6,
        color: visuals.window_stroke().color,
    };
    let text_color = visuals.text_color();
    for (idx, name) in station_name_q
        .iter_many(route.stops.iter())
        .map(Name::to_string)
        .enumerate()
    {
        let pos = idx as f64 * STATION_SPACING;
        let pos = navi.xy_to_screen_pos(pos, 0.0).x;
        painter.vline(pos, navi.visible_rect.y_range(), stroke);
        let galley = painter.layout_no_wrap(name, egui::FontId::proportional(13.0), text_color);
        // rotate 45 degrees
        let text_shape = TextShape::new(pos2(pos, navi.visible_rect.top()), galley, text_color)
            .with_angle_and_anchor(f32::consts::FRAC_PI_4, Align2::LEFT_BOTTOM);
        painter.add(text_shape);
    }
}

fn draw_priority_lines(
    (InMut(painter), In(route_entity), InRef(navi), InRef(maps)): (
        InMut<Painter>,
        In<Entity>,
        InRef<PriorityTabNavigation>,
        InRef<[Vec<(Entity, TimetableTime)>]>,
    ),
    route_q: Query<&Route>,
) {
    // let route = route_q.get(route_entity).unwrap();
    let stroke = Stroke {
        width: 2.0,
        color: DisplayColor::Predefined(paiagram_core::colors::PredefinedColor::Amber).get(false),
    };
    let mut line_map: EntityHashMap<Vec<Pos2>> = EntityHashMap::new();
    for (station_idx, map) in maps.iter().enumerate() {
        let x = station_idx as f64 * STATION_SPACING;
        for (priority, (e, _)) in map.iter().enumerate() {
            let pos = navi.xy_to_screen_pos(x, priority as f64 * STATION_SPACING);
            let p = line_map.entry(*e).or_insert(Vec::new());
            p.push(pos);
        }
    }
    for (_, points) in line_map {
        painter.line(points, stroke);
    }
}
