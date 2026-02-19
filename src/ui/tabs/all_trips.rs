use bevy::prelude::*;
use egui::{FontId, RichText, vec2};
use egui_table::{Column, Table, TableDelegate};
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
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        // use a table
        let route = world.get::<Route>(self.route_entity).unwrap();
        let trips = world.get::<RouteTrips>(self.route_entity).unwrap();
        let table = egui_table::Table::new()
            .id_salt(self.route_entity)
            .num_rows(route.stops.len() as u64)
            .columns(
                std::iter::once(Column::new(80.0).resizable(true))
                    .chain((1..trips.len()).map(|_| Column::new(40.0).resizable(false)))
                    .collect::<Vec<_>>(),
            )
            .num_sticky_cols(1);
        world
            .run_system_cached_with(display_table, (table, ui, self))
            .unwrap();
    }
}

struct AllTripsDisplayer<'w> {
    route: &'w Route,
    tab: &'w mut AllTripsTab,
    names: &'w [&'w str],
    trips: &'w [(TripQueryItem<'w, 'w>, Vec<EntryDisplayMode<'w>>)],
}

impl<'w> TableDelegate for AllTripsDisplayer<'w> {
    fn prepare(&mut self, _info: &egui_table::PrefetchInfo) {
        // TODO: move all data generation here
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
        let (t, _) = &self.trips[cell.group_index - 1];
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
        let entry = &self.trips[cell.col_nr - 1].1[cell.row_nr as usize];
        let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
        let res = ui
            .vertical_centered(|ui| match entry {
                EntryDisplayMode::Skipped => ui.label(RichText::new("║").font(font)),
                EntryDisplayMode::NoOperation => ui.label(RichText::new("‥").font(font)),
                EntryDisplayMode::Terminated => ui.label(RichText::new("⁼").font(font)),
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
                    TravelMode::Flexible => ui.label(RichText::new("⇂").font(font)),
                    _ => ui.label(RichText::new("⇂").font(font)),
                },
            })
            .response;
    }
}

enum EntryDisplayMode<'w> {
    Skipped,
    NoOperation,
    Terminated,
    Some(EntryQueryItem<'w, 'w>),
}

fn display_table(
    (In(table), InMut(ui), InMut(tab)): (In<Table>, InMut<egui::Ui>, InMut<AllTripsTab>),
    route_q: Query<(&Route, &RouteTrips)>,
    trip_q: Query<TripQuery>,
    entry_q: Query<EntryQuery>,
    names: Query<&Name, With<Station>>,
    parent_station_or_station: Query<ParentStationOrStation>,
) {
    let (route, route_trips) = route_q.get(tab.route_entity).unwrap();
    let names: Vec<_> = names
        .iter_many(route.stops.iter())
        .map(|it| it.as_str())
        .collect();
    let trips: Vec<_> = trip_q
        .iter_many(route_trips.iter())
        .map(|it| {
            let mut v = Vec::with_capacity(route.stops.len());
            v.resize_with(route.stops.len(), || EntryDisplayMode::Skipped);
            let schedule_it = entry_q.iter_many(it.schedule.iter());
            let mut previous_pos = 0;
            for it in schedule_it {
                let station_entity = parent_station_or_station.get(it.stop()).unwrap().parent();
                if let Some(found_pos) = route.stops.iter().position(|it| *it == station_entity)
                    && found_pos > previous_pos
                {
                    previous_pos = found_pos;
                    v[found_pos] = EntryDisplayMode::Some(it)
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
        })
        .collect();
    let mut displayer = AllTripsDisplayer {
        route,
        tab,
        names: &names,
        trips: trips.as_slice(),
    };
    let dia_pro_style = egui::TextStyle::Name("dia_pro".into());
    ui.style_mut().text_styles.insert(
        dia_pro_style.clone(),
        egui::FontId::new(15.0, egui::FontFamily::Name("dia_pro".into())),
    );
    ui.style_mut().drag_value_text_style = dia_pro_style;
    ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;
    ui.style_mut().visuals.button_frame = false;
    table.show(ui, &mut displayer);
}
