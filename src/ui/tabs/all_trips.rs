use bevy::prelude::*;
use egui::{FontId, Layout, Rect, RichText, Ui, Vec2, vec2};
use egui_table::{Column, Table, TableDelegate};
use emath::Numeric;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};

use crate::{
    entry::{EntryQuery, EntryQueryItem, TravelMode},
    route::{
        AllTripsDisplayMode, Route, RouteByDirectionTrips, RouteDisplayModes,
        SortRouteByDirectionTrips,
    },
    station::{ParentStationOrStation, Station},
    trip::{TripQuery, TripQueryItem},
    units::time::TimetableTime,
};

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct AllTripsTab {
    #[entities]
    route_entity: Entity,
}

impl AllTripsTab {
    pub fn new(e: Entity) -> Self {
        Self { route_entity: e }
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
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        if ui.button("Sort entries").clicked() {
            world.trigger(SortRouteByDirectionTrips {
                entity: self.route_entity,
            });
        }
    }
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        let route = world.get::<Route>(self.route_entity).unwrap();
        let by_direction = world
            .get::<RouteByDirectionTrips>(self.route_entity)
            .expect("Route should have RouteByDirectionTrips");
        let downward_entities = by_direction.downward.clone();
        // use a table
        let table = egui_table::Table::new()
            .id_salt(self.route_entity)
            .num_rows(route.stops.len() as u64)
            .num_sticky_cols(2);
        world
            .run_system_cached_with(
                display_table,
                (table, ui, self.route_entity, downward_entities.as_slice()),
            )
            .unwrap();
    }
}

struct AllTripsDisplayer<'w> {
    route: &'w Route,
    route_display_modes: &'w mut RouteDisplayModes,
    names: &'w [&'w str],
    available_trips: &'w [Entity],
    column_offset: usize,
    trips: Vec<(TripQueryItem<'w, 'w>, Vec<EntryDisplayMode<'w>>)>,
    trip_q: &'w Query<'w, 'w, TripQuery>,
    entry_q: &'w Query<'w, 'w, EntryQuery>,
    parent_station_or_station: &'w Query<'w, 'w, ParentStationOrStation>,
}

impl<'w> AllTripsDisplayer<'w> {
    fn table_cell_width() -> f32 {
        36.0
    }
    fn cell_size() -> Vec2 {
        vec2(36.0, 16.0)
    }
}

impl<'w> TableDelegate for AllTripsDisplayer<'w> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        self.trips.clear();

        let visible_trip_cols_start = info.visible_columns.start.max(2);
        let visible_trip_cols_end = info.visible_columns.end;

        if visible_trip_cols_start >= visible_trip_cols_end {
            return;
        }

        let trip_start = visible_trip_cols_start - 2;
        let trip_end = visible_trip_cols_end - 2;
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
    fn row_top_offset(&self, _ctx: &egui::Context, _table_id: egui::Id, row_nr: u64) -> f32 {
        let offset_count: usize = self.route_display_modes[0..(row_nr as usize)]
            .iter()
            .map(|mode| mode.count())
            .sum();

        (offset_count as f32) * self.default_row_height()
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
        if cell.group_index == 1 {
            return;
        }
        let trip_index = cell.group_index - 2;
        let trip_end = self.column_offset + self.trips.len();
        if trip_index < self.column_offset || trip_index >= trip_end {
            return;
        }
        let local_trip_index = trip_index - self.column_offset;
        let (t, _) = &self.trips[local_trip_index];
        ui.label(t.name.as_str());
    }
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let row_nr = cell.row_nr as usize;
        let display_mode = &self.route_display_modes[row_nr];
        if cell.col_nr == 0 {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(self.names[row_nr]);
                },
            );
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
        if cell.col_nr == 1 {
            let prev_arr = row_nr
                .checked_sub(1)
                .map_or(false, |idx| self.route_display_modes[idx].arrival);
            let prev_dep = !display_mode.arrival
                && row_nr
                    .checked_sub(1)
                    .map_or(false, |idx| self.route_display_modes[idx].departure);
            let m = &mut self.route_display_modes[row_nr];
            let show_edit_button = |m: &mut AllTripsDisplayMode, ui: &mut egui::Ui, s: &str| {
                let res = ui.button(s);
                egui::Popup::menu(&res).show(|ui| {
                    ui.add_enabled(
                        !m.arrival || m.departure,
                        egui::Checkbox::new(&mut m.arrival, "Arrival"),
                    );
                    ui.add_enabled(
                        !m.departure || m.arrival,
                        egui::Checkbox::new(&mut m.departure, "Departure"),
                    );
                });
            };
            ui.vertical(|ui| {
                if m.arrival {
                    let s = if prev_arr { "〃" } else { "Ａ" };
                    show_edit_button(m, ui, s);
                }
                if m.departure {
                    let s = if prev_dep { "〃" } else { "Ｄ" };
                    show_edit_button(m, ui, s);
                }
            });
            return;
        }
        if display_mode.arrival {
            if display_mode.departure {
                ui.painter().line_segment(
                    [ui.max_rect().left_center(), ui.max_rect().right_center()],
                    ui.visuals().window_stroke(),
                );
            } else {
                let dy = vec2(0.0, -ui.visuals().window_stroke.width);
                ui.painter().line_segment(
                    [
                        ui.max_rect().left_bottom() + dy,
                        ui.max_rect().right_bottom() + dy,
                    ],
                    ui.visuals().window_stroke(),
                );
            }
        }
        let trip_index = cell.col_nr - 2;
        let trip_end = self.column_offset + self.trips.len();
        if trip_index < self.column_offset || trip_index >= trip_end {
            return;
        }
        let local_trip_index = trip_index - self.column_offset;
        let (_, entries) = &self.trips[local_trip_index];
        let entry = &entries[row_nr];
        ui.vertical(|ui| {
            if display_mode.arrival {
                let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
                let res = ui.put(
                    Rect::from_min_size(ui.max_rect().left_top(), AllTripsDisplayer::cell_size()),
                    |ui: &mut egui::Ui| match entry {
                        EntryDisplayMode::Skipped => ui.button(RichText::new("║").font(font)),
                        EntryDisplayMode::NoOperation => ui.button(
                            RichText::new(
                                if (cell.row_nr + 1) % 10 == 0 && display_mode.count() < 2 {
                                    "┄"
                                } else {
                                    "‥"
                                },
                            )
                            .font(font),
                        ),
                        EntryDisplayMode::Terminated => ui.button(RichText::new("▔").font(font)),
                        EntryDisplayMode::Some(e) => match e.mode.arr {
                            Some(TravelMode::At(t)) => {
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
                            None | Some(TravelMode::Flexible) => {
                                ui.button(RichText::new("⇂").font(font))
                            }
                            _ => ui.label(RichText::new("⇂").font(font)),
                        },
                    },
                );
                egui::Popup::menu(&res).show(|ui| {
                    ui.label("Hi!");
                });
            }
            if display_mode.departure {
                let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
                let res = ui.put(
                    Rect::from_min_size(
                        if display_mode.arrival {
                            ui.max_rect().left_center()
                        } else {
                            ui.max_rect().left_top()
                        },
                        AllTripsDisplayer::cell_size(),
                    ),
                    |ui: &mut egui::Ui| match entry {
                        EntryDisplayMode::Skipped => ui.button(RichText::new("║").font(font)),
                        EntryDisplayMode::NoOperation => ui.button(
                            RichText::new(
                                if (cell.row_nr + 1) % 10 == 0 && display_mode.count() < 2 {
                                    "┄"
                                } else {
                                    "‥"
                                },
                            )
                            .font(font),
                        ),
                        EntryDisplayMode::Terminated => ui.button(RichText::new("▔").font(font)),
                        EntryDisplayMode::Some(e) => match e.mode.dep {
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
                    },
                );
                egui::Popup::menu(&res).show(|ui| {
                    ui.label("Hi!");
                });
            }
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
    mut route_q: Query<(&Route, &mut RouteDisplayModes)>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
    names: Query<&Name, With<Station>>,
) {
    let (route, mut route_display_modes) = route_q.get_mut(route_entity).unwrap();
    let names: Vec<_> = names
        .iter_many(route.stops.iter())
        .map(|it| it.as_str())
        .collect();
    let mut displayer = AllTripsDisplayer {
        route,
        route_display_modes: &mut *route_display_modes,
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
                .chain(std::iter::once(Column::new(20.0).resizable(false)))
                .chain(
                    (0..trips_to_display.len()).map(|_| {
                        Column::new(AllTripsDisplayer::table_cell_width()).resizable(false)
                    }),
                )
                .collect::<Vec<_>>(),
        )
        .show(ui, &mut displayer);
}
