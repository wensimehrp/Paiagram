use crate::interface::{AppTab, UiCommand};
use crate::vehicles::*;
use bevy::prelude::*;
use bevy_egui::egui;
use egui_deferred_table::{
    Action, AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource,
    DeferredTableRenderer, TableDimensions,
};

struct VehicleOverviewRow {
    vehicle: Entity,
    name: String,
    entry_count: usize,
    service_count: usize,
}

struct VehicleOverviewDataSource {
    rows: Vec<VehicleOverviewRow>,
}

impl VehicleOverviewDataSource {
    fn new(rows: Vec<VehicleOverviewRow>) -> Self {
        Self { rows }
    }

    fn row(&self, index: usize) -> Option<&VehicleOverviewRow> {
        self.rows.get(index)
    }
}

impl DeferredTableDataSource for VehicleOverviewDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: 2,
        }
    }
}

struct VehicleOverviewRenderer;

impl DeferredTableRenderer<VehicleOverviewDataSource> for VehicleOverviewRenderer {
    fn render_cell(
        &self,
        ui: &mut bevy_egui::egui::Ui,
        cell_index: CellIndex,
        source: &VehicleOverviewDataSource,
    ) {
        let Some(row) = source.row(cell_index.row) else {
            return;
        };

        match cell_index.column {
            0 => {
                ui.label(format!("{}", row.entry_count));
            }
            1 => {
                ui.label(format!("{}", row.service_count));
            }
            _ => {}
        }
    }
}

pub fn show_vehicle_overview(
    InMut(ui): InMut<egui::Ui>,
    vehicles: Query<(Entity, &Name, &crate::vehicles::Schedule, &Children), With<Vehicle>>,
    services: Query<&Service>,
    mut msg_ui: MessageWriter<UiCommand>,
    displayed_lines: Query<(Entity, &Name), With<crate::lines::DisplayedLine>>,
) {
    for line in displayed_lines.iter() {
        let (entity, name) = line;
        if ui.button(format!("{}", name)).clicked() {
            msg_ui.write(UiCommand::OpenOrFocusVehicleTab(AppTab::LineTimetable(
                entity,
            )));
        }
    }

    let rows: Vec<VehicleOverviewRow> = vehicles
        .iter()
        .map(|(entity, name, schedule, children)| VehicleOverviewRow {
            vehicle: entity,
            name: name.as_str().to_owned(),
            entry_count: schedule.1.len(),
            service_count: {
                let mut count = 0;
                for child in children.iter() {
                    if let Ok(_service) = services.get(child) {
                        count += 1;
                    }
                }
                count
            },
        })
        .collect();

    if rows.is_empty() {
        ui.label("No vehicles found.");
        return;
    }

    let row_parameters: Vec<AxisParameters> = rows
        .iter()
        .map(|row| {
            AxisParameters::default()
                .name(row.name.clone())
                .default_dimension(14.0)
                .monospace(true)
        })
        .collect();

    let mut data_source = VehicleOverviewDataSource::new(rows);

    let column_parameters = vec![
        AxisParameters::default()
            .name("Entries")
            .default_dimension(80.0)
            .resizable(false)
            .monospace(true),
        AxisParameters::default()
            .name("Services")
            .default_dimension(80.0)
            .resizable(false)
            .monospace(true),
    ];

    let mut table = DeferredTable::new(ui.id().with("vehicle_overview"))
        .default_cell_size(egui::vec2(160.0, 14.0))
        .highlight_hovered_cell();

    table = table.column_parameters(&column_parameters);

    if !row_parameters.is_empty() {
        table = table.row_parameters(&row_parameters);
    }

    let mut renderer = VehicleOverviewRenderer;
    let (_response, actions) = table.show(ui, &mut data_source, &mut renderer);

    for action in actions {
        if let Action::CellClicked(cell_index) = action {
            if let Some(row) = data_source.row(cell_index.row) {
                msg_ui.write(UiCommand::OpenOrFocusVehicleTab(AppTab::Vehicle(
                    row.vehicle,
                )));
            }
        }
    }
}
