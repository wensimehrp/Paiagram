use bevy::prelude::*;
use egui::{FontId, RichText, Vec2, vec2};
use egui_table::{Column, Table, TableDelegate};
use either::Either;
use emath::Numeric;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

use crate::{
    entry::{EntryQuery, EntryQueryItem, TravelMode},
    route::{Route, RouteTrips},
    station::{ParentStationOrStation, Station},
    trip::{TripQuery, TripQueryItem},
    units::time::TimetableTime,
};

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct AllTripsTab {
    #[entities]
    route_entity: Entity,
    #[entities]
    downward_entities: Option<Vec<Entity>>,
    #[entities]
    upward_entities: Option<Vec<Entity>>,
}

impl AllTripsTab {
    pub fn new(e: Entity) -> Self {
        Self {
            route_entity: e,
            upward_entities: None,
            downward_entities: None,
        }
    }
}

impl PartialEq for AllTripsTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_entity == other.route_entity
    }
}

impl super::Tab for AllTripsTab {
    const NAME: &'static str = "All Trips";
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        // prepare the downward and upward data
        let route = world.get::<Route>(self.route_entity).unwrap();
        // use a table
        let table = egui_table::Table::new()
            .id_salt(self.route_entity)
            .num_rows(route.stops.len() as u64)
            .num_sticky_cols(1);
        world
            .run_system_cached_with(
                prepare_trips,
                (self.route_entity, &mut self.downward_entities, true),
            )
            .unwrap();
        // downward entities must be initialized from this point
        let downward_entities = self.downward_entities.as_deref().unwrap();
        world
            .run_system_cached_with(
                display_table,
                (table, ui, self.route_entity, downward_entities),
            )
            .unwrap();
    }
}

fn prepare_trips(
    (In(route_entity), InMut(buf), In(downwards)): (
        In<Entity>,
        InMut<Option<Vec<Entity>>>,
        In<bool>,
    ),
    route_q: Query<(&Route, Ref<RouteTrips>)>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
) {
    let (route, trips) = route_q.get(route_entity).unwrap();
    // if the buffer is not initialized, or the trips is refreshed, update the entities
    if buf.is_some() && !trips.is_changed() {
        return;
    }

    let out = buf.get_or_insert_default();
    out.clear();

    let extend_iter = trips.iter().copied().filter_map(|trip_entity| {
        let trip = trip_q.get(trip_entity).unwrap();
        let mut stations = if downwards {
            Either::Left(route.stops.iter())
        } else {
            Either::Right(route.stops.iter().rev())
        };
        let mut found_counter = 0;
        for it in entry_q.iter_many(trip.schedule.iter()) {
            let station_entity = parent_station_or_station.get(it.stop()).unwrap().parent();
            // The first match is consumed here
            // reuse the same iterator afterwards
            if stations.any(|it| *it == station_entity) {
                found_counter += 1;
                if found_counter >= 2 {
                    return Some(trip_entity);
                }
            }
        }
        return None;
    });
    out.extend(extend_iter);
}

struct AllTripsDisplayer<'w> {
    route: &'w Route,
    names: &'w [&'w str],
    available_trips: &'w [Entity],
    column_offset: usize,
    trips: Vec<(TripQueryItem<'w, 'w>, Vec<EntryDisplayMode<'w>>)>,
    trip_q: &'w Query<'w, 'w, TripQuery>,
    entry_q: &'w Query<'w, 'w, EntryQuery>,
    parent_station_or_station: &'w Query<'w, 'w, ParentStationOrStation>,
}

impl<'w> TableDelegate for AllTripsDisplayer<'w> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        self.trips.clear();

        let visible_trip_cols_start = info.visible_columns.start.max(1);
        let visible_trip_cols_end = info.visible_columns.end;

        if visible_trip_cols_start >= visible_trip_cols_end {
            return;
        }

        let trip_start = visible_trip_cols_start - 1;
        let trip_end = visible_trip_cols_end - 1;
        self.column_offset = trip_start;

        let trips_iter = self
            .trip_q
            .iter_many(self.available_trips[trip_start..trip_end].iter())
            .map(|it| {
                let mut v = Vec::with_capacity(self.route.stops.len());
                v.resize_with(self.route.stops.len(), || EntryDisplayMode::Skipped);
                let schedule_it = self.entry_q.iter_many(it.schedule.iter());
                let mut next_abs_idx = 0;
                let mut stations = self.route.stops.iter();
                for it in schedule_it {
                    let station_entity = self
                        .parent_station_or_station
                        .get(it.stop())
                        .unwrap()
                        .parent();
                    // we reuse the same iterator here
                    // the pointer would advance every time we use the .position() method
                    if let Some(found_pos) = stations.position(|it| *it == station_entity) {
                        let abs_idx = next_abs_idx + found_pos;
                        v[abs_idx] = EntryDisplayMode::Some(it);
                        next_abs_idx = abs_idx + 1;
                    }
                }
                for i in v
                    .iter_mut()
                    .take_while(|it| matches!(it, EntryDisplayMode::Skipped))
                {
                    *i = EntryDisplayMode::NoOperation
                }
                let mut last_processed: Option<&mut _> = None;
                for i in v
                    .iter_mut()
                    .rev()
                    .take_while(|it| matches!(it, EntryDisplayMode::Skipped))
                {
                    *i = EntryDisplayMode::NoOperation;
                    last_processed = Some(i);
                }
                if let Some(m) = last_processed {
                    *m = EntryDisplayMode::Terminated;
                }
                (it, v)
            });
        self.trips.extend(trips_iter);
    }
    fn default_row_height(&self) -> f32 {
        16.0
    }
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        let dy = vec2(0.0, -ui.visuals().window_stroke.width);
        ui.painter().line_segment(
            [
                ui.max_rect().left_bottom() + dy,
                ui.max_rect().right_bottom() + dy,
            ],
            ui.visuals().window_stroke(),
        );
        if cell.group_index == 0 {
            ui.label("Stations");
            return;
        }
        let dx = vec2(-ui.visuals().window_stroke.width, 0.0);
        ui.painter().line_segment(
            [
                ui.max_rect().right_top() + dx,
                ui.max_rect().right_bottom() + dx,
            ],
            ui.visuals().window_stroke(),
        );
        let trip_index = cell.group_index - 1;
        let trip_end = self.column_offset + self.trips.len();
        if trip_index < self.column_offset || trip_index >= trip_end {
            return;
        }
        let local_trip_index = trip_index - self.column_offset;
        let (t, _) = &self.trips[local_trip_index];
        ui.label(t.name.as_str());
    }
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        if cell.col_nr == 0 {
            ui.label(self.names[cell.row_nr as usize]);
            return;
        }
        let dx = vec2(-ui.visuals().window_stroke.width, 0.0);
        ui.painter().line_segment(
            [
                ui.max_rect().right_top() + dx,
                ui.max_rect().right_bottom() + dx,
            ],
            ui.visuals().window_stroke(),
        );
        let trip_index = cell.col_nr - 1;
        let trip_end = self.column_offset + self.trips.len();
        if trip_index < self.column_offset || trip_index >= trip_end {
            return;
        }
        let local_trip_index = trip_index - self.column_offset;
        let (_, entries) = &self.trips[local_trip_index];
        let entry = &entries[cell.row_nr as usize];
        let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
        let res = ui.add_sized(ui.available_size(), |ui: &mut egui::Ui| match entry {
            EntryDisplayMode::Skipped => ui.button(RichText::new("║").font(font)),
            EntryDisplayMode::NoOperation => ui.button(
                RichText::new(if (cell.row_nr + 1) % 10 == 0 {
                    "┄"
                } else {
                    "‥"
                })
                .font(font),
            ),
            EntryDisplayMode::Terminated => ui.button(RichText::new("▔").font(font)),
            EntryDisplayMode::Some(e) => match e.mode.arr {
                TravelMode::At(t) => {
                    let mut new_t = t;
                    ui.add(
                        egui::DragValue::new(&mut new_t)
                            .custom_formatter(|it, _| {
                                TimetableTime::from_f64(it).to_oud2_str(false)
                            })
                            .custom_parser(|s| {
                                TimetableTime::from_oud2_str(s).map(|it| it.to_f64())
                            }),
                    )
                }
                TravelMode::Flexible => ui.button(RichText::new("⇂").font(font)),
                _ => ui.label(RichText::new("⇂").font(font)),
            },
        });
        egui::Popup::menu(&res).show(|ui| {
            ui.label("Hi!");
        });
    }
}

enum EntryDisplayMode<'w> {
    Skipped,
    NoOperation,
    Terminated,
    Some(EntryQueryItem<'w, 'w>),
}

fn display_table(
    (In(table), InMut(ui), In(route_entity), InRef(trips_to_display)): (
        In<Table>,
        InMut<egui::Ui>,
        In<Entity>,
        InRef<[Entity]>,
    ),
    route_q: Query<&Route>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
    names: Query<&Name, With<Station>>,
) {
    let route = route_q.get(route_entity).unwrap();
    let names: Vec<_> = names
        .iter_many(route.stops.iter())
        .map(|it| it.as_str())
        .collect();
    let mut displayer = AllTripsDisplayer {
        route,
        names: &names,
        trips: Vec::new(),
        available_trips: trips_to_display,
        column_offset: 0,
        trip_q: &trip_q,
        entry_q: &entry_q,
        parent_station_or_station: &parent_station_or_station,
    };
    let dia_pro_style = egui::TextStyle::Name("dia_pro".into());
    ui.style_mut().text_styles.insert(
        dia_pro_style.clone(),
        egui::FontId::new(15.0, egui::FontFamily::Name("dia_pro".into())),
    );
    ui.style_mut().drag_value_text_style = dia_pro_style;
    ui.spacing_mut().interact_size = Vec2::ZERO;
    ui.spacing_mut().button_padding = Vec2::ZERO;
    ui.style_mut().visuals.button_frame = false;
    table
        .columns(
            std::iter::once(Column::new(80.0).resizable(true))
                .chain((0..trips_to_display.len()).map(|_| Column::new(36.0).resizable(false)))
                .collect::<Vec<_>>(),
        )
        .show(ui, &mut displayer);
}
