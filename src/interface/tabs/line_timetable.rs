use crate::intervals::Station;
use crate::lines::DisplayedLine;
use crate::vehicles::{ArrivalType, Schedule, Service, TimetableEntry, Vehicle};
use bevy::prelude::*;
use bevy_egui::egui::{self};
use egui_deferred_table::{
    AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use indexmap::IndexMap;

struct LineTimetableDataSource {
    dimensions: TableDimensions,
    cells: Vec<Option<ArrivalType>>,
}

impl LineTimetableDataSource {
    fn new(row_count: usize, column_count: usize) -> Self {
        let cell_count = row_count.saturating_mul(column_count);
        Self {
            dimensions: TableDimensions {
                row_count,
                column_count,
            },
            cells: vec![None; cell_count],
        }
    }

    fn set(&mut self, row: usize, column: usize, value: ArrivalType) {
        if self.dimensions.column_count == 0 {
            return;
        }
        let idx = row * self.dimensions.column_count + column;
        if idx < self.cells.len() {
            self.cells[idx] = Some(value);
        }
    }

    fn arrival(&self, row: usize, column: usize) -> Option<ArrivalType> {
        if self.dimensions.column_count == 0 {
            return None;
        }
        let idx = row * self.dimensions.column_count + column;
        self.cells.get(idx).and_then(|value| *value)
    }
}

impl DeferredTableDataSource for LineTimetableDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        self.dimensions
    }
}

struct LineTimetableRenderer;

impl DeferredTableRenderer<LineTimetableDataSource> for LineTimetableRenderer {
    fn render_cell(
        &self,
        ui: &mut bevy_egui::egui::Ui,
        cell_index: CellIndex,
        source: &LineTimetableDataSource,
    ) {
        let text = match source.arrival(cell_index.row, cell_index.column) {
            Some(arrival) => match arrival {
                ArrivalType::At(time) => time.to_hhmm_string_no_colon(),
                _ => "··".to_string(),
            },
            None => "··".to_string(),
        };
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| ui.monospace(text),
        );
    }
}

enum LineEntryState {
    // shows the a/d time
    Stop,
    // shows a "レ" symbol
    NonStop,
    // shows "||"
    DoesNotPass,
    // shows "··"
    Ended,
}

pub fn show_line_timetable(
    (InMut(ui), In(entity)): (InMut<egui::Ui>, In<Entity>),
    displayed_lines: Query<(&Name, &DisplayedLine)>,
    schedules: Query<&Schedule, With<Vehicle>>,
    services: Query<&Name, With<Service>>,
    entries: Query<&TimetableEntry>,
    stations: Query<&Name, With<Station>>,
) {
    let Ok((_line_name, line)) = displayed_lines.get(entity) else {
        ui.label("No displayed lines found for this entity.");
        return;
    };
    // create schedule -> timetable entry map
    let mut service_entries: IndexMap<Entity, Vec<(Entity, Entity)>> = IndexMap::new();
    for schedule in schedules {
        for entry_entity in &schedule.1 {
            let Ok(entry) = entries.get(*entry_entity) else {
                continue;
            };
            let Some(service) = entry.service else {
                continue;
            };
            // push the current entry to the corresponding service vector
            // if it does not exist yet, create it
            // Note that the vector is guaranteed to be in order since we are iterating
            service_entries
                .entry(service)
                .or_default()
                .push((entry.station, *entry_entity));
        }
    }
    let row_count = line.0.len();
    let column_count = service_entries.len();

    if row_count == 0 || column_count == 0 {
        ui.label("No timetable data available for this line.");
        return;
    }

    let mut data_source = LineTimetableDataSource::new(row_count, column_count);

    for (column_index, (_service_entity, entries_for_service)) in service_entries.iter().enumerate()
    {
        for (row_index, (station_entity, _)) in line.0.iter().enumerate() {
            if let Some((_station, entry_entity)) = entries_for_service
                .iter()
                .find(|(station, _)| station == station_entity)
            {
                if let Ok(entry) = entries.get(*entry_entity) {
                    data_source.set(row_index, column_index, entry.arrival);
                }
            }
        }
    }

    let mut column_parameters: Vec<AxisParameters> = Vec::with_capacity(column_count);
    for i in 0..column_count {
        column_parameters.push(
            AxisParameters::default()
                .name({
                    services
                        .get(*service_entries.get_index(i).unwrap().0)
                        .map(|name| name.as_str().to_owned())
                        .unwrap_or_else(|_| "<unknown>".to_string())
                })
                .default_dimension(28.0)
                .resizable(false),
        );
    }

    let station_names: Vec<String> = line
        .0
        .iter()
        .map(|(station_entity, _)| {
            stations
                .get(*station_entity)
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|_| "<unknown>".to_string())
        })
        .collect();

    let row_parameters: Vec<AxisParameters> = station_names
        .iter()
        .map(|name| {
            AxisParameters::default()
                .name(name.clone())
                .default_dimension(14.0)
                .resizable(false)
        })
        .collect();

    let mut table = DeferredTable::new(ui.id().with("line_timetable"))
        .default_cell_size(egui::vec2(80.0, 14.0))
        .selectable_rows_disabled()
        .highlight_hovered_cell();

    if !column_parameters.is_empty() {
        table = table.column_parameters(&column_parameters);
    }

    if !row_parameters.is_empty() {
        table = table.row_parameters(&row_parameters);
    }

    let mut renderer = LineTimetableRenderer;
    let (_response, _actions) = table.show(ui, &mut data_source, &mut renderer);
}
