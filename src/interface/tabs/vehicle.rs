use crate::interface::UiCommand;
use crate::status_bar_text::SetStatusBarText;
use crate::{
    search::{SearchCommand, SearchResponse},
    vehicles::{Schedule, TimetableEntry},
};
use bevy::ecs::message::MessageId;
use bevy::prelude::*;
use bevy_egui::egui;
use egui_deferred_table::{
    AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
struct VehicleScheduleRow {
    station_name: String,
    arrival: String,
    departure: String,
    service: String,
    track: String,
}

impl VehicleScheduleRow {
    fn all_fields(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.station_name, self.arrival, self.departure, self.service, self.track
        )
    }
}

#[derive(Default)]
struct VehicleScheduleDataSource {
    rows: Vec<VehicleScheduleRow>,
}

impl VehicleScheduleDataSource {
    fn clear(&mut self) {
        self.rows.clear();
    }

    fn push(&mut self, row: VehicleScheduleRow) {
        self.rows.push(row);
    }

    fn row(&self, index: usize) -> Option<&VehicleScheduleRow> {
        self.rows.get(index)
    }

    fn iter(&self) -> std::slice::Iter<'_, VehicleScheduleRow> {
        self.rows.iter()
    }
}

impl DeferredTableDataSource for VehicleScheduleDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: 4,
        }
    }
}

#[derive(Default)]
pub struct VehicleScheduleCache {
    data_source: VehicleScheduleDataSource,
}

impl VehicleScheduleCache {
    fn refresh(
        &mut self,
        schedule: &Schedule,
        entries: &Query<(&TimetableEntry, &ChildOf)>,
        names: &Query<&Name>,
        entity: Entity,
    ) {
        self.data_source.clear();

        let expected_rows = schedule.1.len();
        if expected_rows > 0 {
            self.data_source.rows.reserve(expected_rows);
        }

        for entry_entity in &schedule.1 {
            let Ok((entry, parent)) = entries.get(*entry_entity) else {
                continue;
            };

            let station_name = names
                .get(entry.station)
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|_| "???".to_string());

            let service = entry
                .service
                .and_then(|service_entity| {
                    names
                        .get(service_entity)
                        .ok()
                        .map(|n| n.as_str().to_owned())
                })
                .unwrap_or_else(|| "—".to_string());

            let track = entry
                .track
                .map(|track_entity| format!("{track_entity:?}"))
                .unwrap_or_else(|| "—".to_string());

            self.data_source.push(VehicleScheduleRow {
                station_name,
                arrival: entry.arrival.to_string(),
                departure: entry.departure.to_string(),
                service,
                track,
            });
        }
    }
}

struct VehicleScheduleRenderer<'a> {
    filtered_rows: &'a [usize],
}

impl<'a> DeferredTableRenderer<VehicleScheduleDataSource> for VehicleScheduleRenderer<'a> {
    fn render_cell(
        &self,
        ui: &mut bevy_egui::egui::Ui,
        cell_index: CellIndex,
        source: &VehicleScheduleDataSource,
    ) {
        let Some(row) = source.row(cell_index.row) else {
            return;
        };

        let label = match cell_index.column {
            0 => &row.arrival,
            1 => &row.departure,
            2 => &row.service,
            3 => &row.track,
            _ => return,
        };
        ui.monospace(label);
    }

    fn rows_to_filter(&self) -> Option<&[usize]> {
        Some(self.filtered_rows)
    }
}

const FILTER_THROTTLE: f32 = 0.01;

#[derive(Default)]
pub(crate) struct VehicleDisplayCache {
    query: String,
    filtered_rows: Vec<usize>,
    last_schedule_len: usize,
    dirty: bool,
    last_request_time: Option<f32>,
    in_flight: Option<MessageId<SearchCommand>>,
}

impl VehicleDisplayCache {
    fn query_mut(&mut self) -> &mut String {
        &mut self.query
    }

    fn on_query_changed(&mut self) {
        self.dirty = true;
        if self.query.is_empty() {
            self.filtered_rows.clear();
        }
    }

    fn update_schedule_len(&mut self, len: usize) {
        if self.last_schedule_len != len {
            self.last_schedule_len = len;
            self.dirty = true;
        }
    }

    fn filtered_rows<'a>(
        &'a mut self,
        entity: Entity,
        now_seconds: f32,
        data_source: &VehicleScheduleDataSource,
        search_writer: &mut MessageWriter<SearchCommand>,
        pending_requests: &mut HashMap<MessageId<SearchCommand>, Entity>,
    ) -> &'a [usize] {
        if self.query.trim().is_empty() {
            if !self.filtered_rows.is_empty() {
                self.filtered_rows.clear();
            }
            if let Some(previous) = self.in_flight.take() {
                pending_requests.remove(&previous);
            }
            self.dirty = false;
            return &self.filtered_rows;
        }

        let throttle_ok = self
            .last_request_time
            .map_or(true, |last| now_seconds - last >= FILTER_THROTTLE);
        let must_dispatch =
            self.dirty || (self.in_flight.is_none() && self.filtered_rows.is_empty());

        if must_dispatch && throttle_ok {
            if let Some(previous) = self.in_flight.take() {
                pending_requests.remove(&previous);
            }

            let payload: Vec<String> = data_source.iter().map(|row| row.all_fields()).collect();

            let command_id = search_writer.write(SearchCommand::Table {
                data: Arc::new(payload),
                query: self.query.clone(),
            });

            pending_requests.insert(command_id, entity);
            self.in_flight = Some(command_id);
            self.last_request_time = Some(now_seconds);
            self.dirty = false;
        }

        &self.filtered_rows
    }
}

pub fn show_vehicle(
    (InMut(ui), In(entity)): (InMut<egui::Ui>, In<Entity>),
    mut cache: Local<VehicleScheduleCache>,
    mut display_cache: Local<HashMap<Entity, VehicleDisplayCache>>,
    mut pending_requests: Local<HashMap<MessageId<SearchCommand>, Entity>>,
    mut search_responses: MessageReader<SearchResponse>,
    mut search_writer: MessageWriter<SearchCommand>,
    mut ui_command_writer: MessageWriter<UiCommand>,
    time: Res<Time>,
    schedules: Query<&Schedule>,
    entries: Query<(&TimetableEntry, &ChildOf)>,
    names: Query<&Name>,
) {
    let Some(schedule) = schedules.get(entity).ok() else {
        ui.label("No schedule found");
        return;
    };
    cache.refresh(schedule, &entries, &names, entity);

    let now_seconds = time.elapsed_secs();

    for response in search_responses.read() {
        match response {
            SearchResponse::Table(command_id, rows) => {
                if let Some(target_entity) = pending_requests.remove(command_id) {
                    if let Some(entry_cache) = display_cache.get_mut(&target_entity) {
                        if entry_cache.in_flight == Some(*command_id) {
                            entry_cache.filtered_rows = rows.clone();
                            entry_cache.in_flight = None;
                            entry_cache.last_request_time = Some(now_seconds);
                        }
                    }
                }
            }
        }
    }

    let cache_entry = display_cache.entry(entity).or_default();
    cache_entry.update_schedule_len(cache.data_source.rows.len());

    // search box
    ui.horizontal(|ui| {
        ui.label("Search:");
        let search_input = ui
            .text_edit_singleline(cache_entry.query_mut())
            .set_status_bar_text("🔍 Search entries", &mut ui_command_writer);
        if search_input.changed() {
            cache_entry.on_query_changed();
        }
    });

    if cache.data_source.rows.is_empty() {
        ui.label("No timetable entries for this vehicle.");
        return;
    }

    let row_parameters: Vec<AxisParameters> = cache
        .data_source
        .iter()
        .map(|row| {
            AxisParameters::default()
                .name(&row.station_name)
                .default_dimension(14.0)
                .resizable(false)
        })
        .collect();

    let column_parameters = vec![
        AxisParameters::default()
            .name("Arrival")
            .default_dimension(70.0)
            .resizable(false),
        AxisParameters::default()
            .name("Departure")
            .default_dimension(70.0)
            .resizable(false),
        AxisParameters::default()
            .name("Service")
            .default_dimension(70.0)
            .resizable(false),
        AxisParameters::default()
            .name("Track")
            .default_dimension(70.0)
            .resizable(false),
    ];

    let mut table = DeferredTable::new(ui.id().with("vehicle_schedule"))
        .default_cell_size(egui::vec2(90.0, 14.0))
        .selectable_rows_disabled()
        .highlight_hovered_cell();

    table = table.column_parameters(&column_parameters);

    if !row_parameters.is_empty() {
        table = table.row_parameters(&row_parameters);
    }

    let filtered_rows = cache_entry.filtered_rows(
        entity,
        now_seconds,
        &cache.data_source,
        &mut search_writer,
        &mut pending_requests,
    );

    let data_source = &mut cache.data_source;
    let mut renderer = VehicleScheduleRenderer { filtered_rows };
    let (_response, _actions) = table.show(ui, data_source, &mut renderer);
}
