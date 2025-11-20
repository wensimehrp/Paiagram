mod about;
mod camera;
mod tabs;
mod widgets;

use bevy::{ecs::system::SystemState, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use egui::{CornerRadius, Margin};
use egui_dock::{DockArea, DockState};
use std::{collections::VecDeque, sync::Arc};

/// Plugin that sets up the user interface
pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_message::<UiCommand>()
            .add_systems(Startup, camera::setup_camera)
            .add_systems(EguiPrimaryContextPass, show_ui)
            .add_systems(Update, modify_dock_state.run_if(on_message::<UiCommand>))
            .insert_resource(UiState::new());
    }
}

/// The state of the user interface
#[derive(Resource)]
struct UiState {
    dock_state: DockState<AppTab>,
    status_bar_text: String,
}

/// Modify the dock state based on UI commands
fn modify_dock_state(mut dock_state: ResMut<UiState>, mut msg_reader: MessageReader<UiCommand>) {
    for msg in msg_reader.read() {
        match msg {
            UiCommand::OpenOrFocusTab(tab) => {
                dock_state.open_or_focus_tab(tab.clone());
            }
            UiCommand::OpenOrFocusStationTab(tab, _) => {
                dock_state.open_or_focus_tab(tab.clone());
            }
            UiCommand::SetStatusBarText(text) => {
                dock_state.status_bar_text = text.clone();
            }
        }
    }
}

impl UiState {
    fn new() -> Self {
        Self {
            dock_state: DockState::new(vec![]),
            status_bar_text: "Ready".into(),
        }
    }
    /// Open a tab if it is not already open, or focus it if it is
    fn open_or_focus_tab(&mut self, tab: AppTab) {
        if let Some((surface_index, node_index, tab_index)) = self.dock_state.find_tab(&tab) {
            self.dock_state
                .set_active_tab((surface_index, node_index, tab_index));
            self.dock_state
                .set_focused_node_and_surface((surface_index, node_index));
        } else {
            self.dock_state.push_to_focused_leaf(tab);
        }
    }
}

/// An application tab
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum AppTab {
    Vehicle(Entity),
    StationTimetable(Entity),
    Diagram(Entity),
}

/// User interface commands sent between systems
#[derive(Message)]
pub enum UiCommand {
    OpenOrFocusTab(AppTab),
    OpenOrFocusStationTab(AppTab, Entity),
    SetStatusBarText(String),
}

/// A viewer for application tabs. This struct holds a single mutable reference to the world,
/// and is constructed each frame.
struct AppTabViewer<'w> {
    world: &'w mut World,
}

impl<'w> egui_dock::TabViewer for AppTabViewer<'w> {
    type Tab = AppTab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            AppTab::Vehicle(entity) => {
                self.world
                    .run_system_cached_with(tabs::vehicle::show_vehicle, (ui, *entity))
                    .unwrap();
            }
            AppTab::StationTimetable(station_entity) => {
                self.world
                    .run_system_cached_with(
                        tabs::station_timetable::show_station_timetable,
                        (ui, *station_entity),
                    )
                    .unwrap();
            }
            AppTab::Diagram(displayed_line_entity) => {
                self.world
                    .run_system_cached_with(
                        tabs::diagram::show_diagram,
                        (ui, *displayed_line_entity),
                    )
                    .unwrap();
            }
        };
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            AppTab::Vehicle(entity) => {
                // query the vehicle name from the world
                let name = self
                    .world
                    .get::<Name>(*entity)
                    .map_or_else(|| "Unknown Vehicle".into(), |n| format!("{}", n));
                format!("{}", name).into()
            }
            AppTab::StationTimetable(station_entity) => {
                // query the station name from the world
                let name = self
                    .world
                    .get::<Name>(*station_entity)
                    .map_or_else(|| "Unknown Station".into(), |n| format!("{}", n));
                format!("Station Timetable - {}", name).into()
            }
            AppTab::Diagram(displayed_line_entity) => "Diagram".into(),
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            AppTab::Vehicle(entity) => egui::Id::new(format!("VehicleTab_{:?}", entity)),
            AppTab::StationTimetable(station_entity) => {
                egui::Id::new(format!("StationTimetableTab_{:?}", station_entity))
            }
            AppTab::Diagram(entity) => egui::Id::new(format!("DiagramTab_{:?}", entity)),
            _ => egui::Id::new(self.title(tab).text()),
        }
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        match tab {
            AppTab::Vehicle(_) => [false; 2],
            AppTab::StationTimetable(_) => [true; 2],
            AppTab::Diagram(_) => [false; 2],
        }
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        let title = self.title(tab).text().to_string();
        if response.hovered() {
            self.world
                .write_message(UiCommand::SetStatusBarText(format!("🖳 {}", title)));
        }
    }
}

/// Main function to show the user interface
fn show_ui(
    world: &mut World,
    ctx: &mut SystemState<EguiContexts>,
    mut initialized: Local<bool>,
    mut frame_history: Local<VecDeque<f64>>,
    mut counter: Local<u8>,
    mut modal_open: Local<bool>,
) -> Result<()> {
    let now = instant::Instant::now();
    let mut ctx = ctx.get_mut(world);
    let ctx = &ctx.ctx_mut().unwrap().clone();
    if !*initialized {
        ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        ctx.set_visuals(egui::Visuals::light());
        apply_custom_fonts(ctx);
        *initialized = true;
    }
    world.resource_scope(|world, mut ui_state: Mut<UiState>| {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            world
                .run_system_cached_with(about::show_about, (ui, &mut *modal_open))
                .unwrap();
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.label(&ui_state.status_bar_text);
        });

        egui::SidePanel::left("TreeView").show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                world
                    .run_system_cached_with(tabs::tree_view::show_tree_view, ui)
                    .unwrap();
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
                let painter = ui.painter();
                let rect = ui.max_rect();

                let spacing = 24.0; // grid cell size
                let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 78));

                // vertical lines
                let start_x = (rect.left() / spacing).floor() * spacing;
                let end_x = rect.right();
                let mut x = start_x;
                while x <= end_x {
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        stroke,
                    );
                    x += spacing;
                }

                // horizontal lines
                let start_y = (rect.top() / spacing).floor() * spacing;
                let end_y = rect.bottom();
                let mut y = start_y;
                while y <= end_y {
                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        stroke,
                    );
                    y += spacing;
                }
                let mut tab_viewer = AppTabViewer { world: world };
                let mut style = egui_dock::Style::from_egui(ui.style());
                style.tab.tab_body.inner_margin = Margin::same(0);
                style.tab.tab_body.corner_radius = CornerRadius::ZERO;
                style.tab.tab_body.stroke.width = 0.0;
                style.tab.hline_below_active_tab_name = true;
                style.tab_bar.corner_radius = CornerRadius::ZERO;
                DockArea::new(&mut ui_state.dock_state)
                    .style(style)
                    .show_inside(ui, &mut tab_viewer);
            });
    });
    *counter = counter.wrapping_add(1);
    // keep a frame history of 256 frames
    frame_history.push_back(now.elapsed().as_secs_f64());
    if frame_history.len() > 256 {
        frame_history.pop_front();
    }
    if *counter == 0 {
        debug!(
            "UI frame took {:?} on average. Min {:?}, Max {:?}",
            {
                let total: f64 = frame_history.iter().sum();
                instant::Duration::from_secs_f64(total / (frame_history.len() as f64))
            },
            {
                let min = frame_history.iter().cloned().fold(f64::INFINITY, f64::min);
                instant::Duration::from_secs_f64(min)
            },
            {
                let max = frame_history
                    .iter()
                    .cloned()
                    .fold(f64::NEG_INFINITY, f64::max);
                instant::Duration::from_secs_f64(max)
            }
        );
    }
    Ok(())
}

/// Apply custom fonts to the egui context
fn apply_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "app_default".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/SarasaUiSC-Regular.ttf"
        ))),
    );

    fonts.font_data.insert(
        "app_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/SarasaTermSC-Regular.ttf"
        ))),
    );

    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "app_default".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "app_mono".to_owned());
    }

    ctx.set_fonts(fonts);
}
