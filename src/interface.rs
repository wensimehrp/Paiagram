mod about;
mod side_panel;
mod tabs;
mod widgets;

use crate::{
    colors,
    interface::tabs::{diagram::SelectedEntityType, tree_view},
};
use bevy::{
    color::palettes::tailwind::{EMERALD_700, EMERALD_800, GRAY_900},
    prelude::*,
};
use egui::{self, Color32, CornerRadius, Frame, Margin, ScrollArea, Stroke, Ui};
use egui_dock::{DockArea, DockState};
use std::sync::Arc;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::interface::tabs::{displayed_lines, start};

/// Plugin that sets up the user interface
pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiCommand>()
            .init_resource::<MiscUiState>()
            .insert_resource(UiState::new())
            .insert_resource(StatusBarState::default())
            .add_systems(Update, modify_dock_state.run_if(on_message::<UiCommand>));
    }
}

/// The state of the user interface
#[derive(Resource)]
struct UiState {
    dock_state: DockState<AppTab>,
}

#[derive(Resource)]
pub struct MiscUiState {
    is_dark_mode: bool,
    initialized: bool,
    frame_times: egui::util::History<f32>,
    workspace: CurrentWorkspace,
    side_panel_tab: side_panel::CurrentTab,
    selected_entity_type: Option<tabs::diagram::SelectedEntityType>,
    modal_open: bool,
    side_panel_on_left: bool,
    fullscreened: bool,
}

impl Default for MiscUiState {
    fn default() -> Self {
        let max_age: f32 = 1.0;
        let max_len = (max_age * 300.0).round() as usize;
        Self {
            is_dark_mode: true,
            frame_times: egui::util::History::new(0..max_len, max_age),
            initialized: false,
            side_panel_tab: side_panel::CurrentTab::default(),
            selected_entity_type: None,
            workspace: CurrentWorkspace::default(),
            modal_open: false,
            side_panel_on_left: true,
            fullscreened: false,
        }
    }
}

impl MiscUiState {
    pub fn on_new_frame(&mut self, now: f64, previous_frame_time: Option<f32>) {
        let previous_frame_time = previous_frame_time.unwrap_or_default();
        if let Some(latest) = self.frame_times.latest_mut() {
            *latest = previous_frame_time; // rewrite history now that we know
        }
        self.frame_times.add(now, previous_frame_time); // projected
    }
    pub fn mean_frame_time(&self) -> f32 {
        self.frame_times.average().unwrap_or_default()
    }
    pub fn fps(&self) -> f32 {
        1.0 / self.frame_times.mean_time_interval().unwrap_or_default()
    }
}

#[derive(Default, Resource)]
struct StatusBarState {
    tooltip: String,
}

/// Modify the dock state based on UI commands
fn modify_dock_state(mut dock_state: ResMut<UiState>, mut msg_reader: MessageReader<UiCommand>) {
    for msg in msg_reader.read() {
        match msg {
            UiCommand::OpenOrFocusTab(tab) => {
                dock_state.open_or_focus_tab(tab.clone());
            }
        }
    }
}

impl UiState {
    fn new() -> Self {
        Self {
            dock_state: DockState::new(vec![]),
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
    DisplayedLines,
}

/// User interface commands sent between systems
#[derive(Message)]
pub enum UiCommand {
    OpenOrFocusTab(AppTab),
}

/// A viewer for application tabs. This struct holds a single mutable reference to the world,
/// and is constructed each frame.
struct AppTabViewer<'w> {
    world: &'w mut World,
    selected_entity_type: &'w mut Option<tabs::diagram::SelectedEntityType>,
}

impl<'w> egui_dock::TabViewer for AppTabViewer<'w> {
    type Tab = AppTab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            AppTab::Vehicle(entity) => {
                if let Err(e) = self
                    .world
                    .run_system_cached_with(tabs::vehicle::show_vehicle, (ui, *entity))
                {
                    error!("UI Error: {}", e)
                }
            }
            AppTab::StationTimetable(station_entity) => {
                if let Err(e) = self.world.run_system_cached_with(
                    tabs::station_timetable::show_station_timetable,
                    (ui, *station_entity),
                ) {
                    error!("UI Error: {}", e)
                }
            }
            AppTab::Diagram(displayed_line_entity) => {
                if let Err(e) = self.world.run_system_cached_with(
                    tabs::diagram::show_diagram,
                    (ui, *displayed_line_entity, self.selected_entity_type),
                ) {
                    error!("UI Error: {}", e)
                }
            }
            AppTab::DisplayedLines => {
                if let Err(e) = self
                    .world
                    .run_system_cached_with(displayed_lines::show_displayed_lines, ui)
                {
                    error!("UI Error: {}", e)
                }
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
            AppTab::Diagram(_) => "Diagram".into(),
            AppTab::DisplayedLines => "Available Lines".into(),
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
            AppTab::DisplayedLines => [false; 2],
        }
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        if response.hovered() {
            let widget_text = self.title(tab);
            let s = &mut self.world.resource_mut::<StatusBarState>().tooltip;
            s.clear();
            s.push_str("🖳 ");
            s.push_str(widget_text.text());
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum CurrentWorkspace {
    Settings,
    #[default]
    Start,
    Edit,
    Publish,
}

impl CurrentWorkspace {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Edit => "Edit",
            Self::Publish => "Publish",
            Self::Settings => "⚙",
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function go_fullscreen(id) {
    const el = document.getElementById(id);
    if (el?.requestFullscreen) el.requestFullscreen();
}
export function exit_fullscreen() {
    if (document.fullscreenElement) {
        document.exitFullscreen();
    }
}
"#)]
unsafe extern "C" {
    fn go_fullscreen(id: &str);
    fn exit_fullscreen();
}

/// Main function to show the user interface
pub fn show_ui(app: &mut super::PaiagramApp, ctx: &egui::Context) -> Result<()> {
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    let mut mus = app
        .bevy_app
        .world_mut()
        .remove_resource::<MiscUiState>()
        .unwrap();
    if !mus.initialized {
        ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        apply_custom_fonts(&ctx);
        mus.initialized = true;
    }
    let frame_time = mus.mean_frame_time();
    app.bevy_app
        .world_mut()
        .resource_scope(|world, mut ui_state: Mut<UiState>| {
            egui::TopBottomPanel::top("menu_bar")
                .frame(Frame::side_top_panel(&ctx.style()))
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        for v in CurrentWorkspace::iter() {
                            ui.selectable_value(&mut mus.workspace, v, v.name());
                        }
                        ui.checkbox(&mut mus.is_dark_mode, "D").changed().then(|| {
                            if mus.is_dark_mode {
                                ctx.set_theme(egui::Theme::Dark);
                            } else {
                                ctx.set_theme(egui::Theme::Light);
                            }
                        });
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.button("F").clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                    !mus.fullscreened,
                                ));
                            mus.fullscreened = !mus.fullscreened;
                        }
                        #[cfg(target_arch = "wasm32")]
                        if ui.button("F").clicked() {
                            unsafe {
                                if mus.fullscreened {
                                    exit_fullscreen();
                                } else {
                                    go_fullscreen("paiagram_canvas");
                                }
                            }
                            mus.fullscreened = !mus.fullscreened;
                        }
                        if ui.button("S").clicked() {
                            mus.side_panel_on_left = !mus.side_panel_on_left;
                        }
                        world
                            .run_system_cached_with(about::show_about, (ui, &mut mus.modal_open))
                            .unwrap();
                    })
                });

            let old_bg_stroke_color = ctx.style().visuals.widgets.noninteractive.bg_stroke.color;

            ctx.style_mut(|s| {
                s.visuals.widgets.noninteractive.bg_stroke.color =
                    colors::translate_srgba_to_color32(EMERALD_700)
            });
            // TODO: make the bottom status bar a separate system
            egui::TopBottomPanel::bottom("status_bar")
                .frame(
                    Frame::side_top_panel(&ctx.style())
                        .fill(colors::translate_srgba_to_color32(EMERALD_800)),
                )
                .show(&ctx, |ui| {
                    ui.visuals_mut().override_text_color = Some(Color32::from_gray(200));
                    ui.horizontal(|ui| {
                        ui.label(&world.resource::<StatusBarState>().tooltip);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let current_time = chrono::Local::now();
                            ui.monospace(current_time.format("%H:%M:%S").to_string());
                            ui.monospace(format!("eFPS: {:>6.1}", 1.0 / frame_time));
                            ui.monospace(format!("{:>5.2} ms/f", 1e3 * frame_time));
                        });
                    });
                });

            ctx.style_mut(|s| {
                s.visuals.widgets.noninteractive.bg_stroke.color = old_bg_stroke_color
            });

            match mus.workspace {
                CurrentWorkspace::Start => {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::new()
                                .inner_margin(egui::Margin::same(0))
                                .fill(ctx.theme().default_visuals().panel_fill),
                        )
                        .show(&ctx, |ui| {
                            world.run_system_cached_with(start::show_start, ui)
                        });
                }
                CurrentWorkspace::Edit => {
                    let supplementary_panel_content = |ui: &mut Ui| {
                        side_panel::show_side_panel(ui, &mut mus.side_panel_tab);
                        match (mus.side_panel_tab, mus.selected_entity_type) {
                            (side_panel::CurrentTab::Edit, _) => {
                                ScrollArea::both().show(ui, |ui| {
                                    world.run_system_cached_with(tree_view::show_tree_view, ui)
                                });
                            }
                            (
                                side_panel::CurrentTab::Details,
                                Some(SelectedEntityType::Station(station_entity)),
                            ) => {
                                ScrollArea::vertical().show(ui, |ui| {
                                    world.run_system_cached_with(
                                        side_panel::station_stats::show_station_stats,
                                        (ui, station_entity),
                                    )
                                });
                            }
                            (
                                side_panel::CurrentTab::Details,
                                Some(SelectedEntityType::Vehicle(vehicle_entity)),
                            ) => {
                                ScrollArea::vertical().show(ui, |ui| {
                                    world.run_system_cached_with(
                                        side_panel::vehicle_stats::show_vehicle_stats,
                                        (ui, vehicle_entity),
                                    )
                                });
                            }
                            (
                                side_panel::CurrentTab::Details,
                                Some(SelectedEntityType::Interval(interval)),
                            ) => {
                                ScrollArea::vertical().show(ui, |ui| {
                                    world.run_system_cached_with(
                                        side_panel::interval_stats::show_interval_stats,
                                        (ui, interval),
                                    )
                                });
                            }
                            (side_panel::CurrentTab::Details, _) => {}
                        }
                    };

                    if mus.side_panel_on_left {
                        egui::SidePanel::left("TreeView")
                            .default_width(ctx.used_size().x / 4.0)
                            .show(&ctx, supplementary_panel_content);
                    } else {
                        egui::TopBottomPanel::bottom("TreeView")
                            .resizable(false)
                            .exact_height(ctx.used_size().y / 2.5)
                            .show(&ctx, supplementary_panel_content);
                    }

                    egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(Margin::ZERO))
                        .show(&ctx, |ui| {
                            let painter = ui.painter();
                            let max_rect = ui.max_rect();
                            painter.rect_filled(
                                max_rect,
                                CornerRadius::ZERO,
                                colors::translate_srgba_to_color32(GRAY_900),
                            );
                            const LINE_SPACING: f32 = 24.0;
                            const LINE_STROKE: Stroke = Stroke {
                                color: Color32::from_additive_luminance(30),
                                width: 1.0,
                            };
                            let x_max = (ui.available_size().x / LINE_SPACING) as usize;
                            let y_max = (ui.available_size().y / LINE_SPACING) as usize;
                            for xi in 0..=x_max {
                                let mut x = xi as f32 * LINE_SPACING + max_rect.min.x;
                                LINE_STROKE.round_center_to_pixel(ui.pixels_per_point(), &mut x);
                                painter.vline(x, max_rect.min.y..=max_rect.max.y, LINE_STROKE);
                            }
                            for yi in 0..=y_max {
                                let mut y = yi as f32 * LINE_SPACING + max_rect.min.y;
                                LINE_STROKE.round_center_to_pixel(ui.pixels_per_point(), &mut y);
                                painter.hline(max_rect.min.x..=max_rect.max.x, y, LINE_STROKE);
                            }
                            let mut tab_viewer = AppTabViewer {
                                world: world,
                                selected_entity_type: &mut mus.selected_entity_type,
                            };
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
                }
                CurrentWorkspace::Publish => {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(&ctx.style()))
                        .show(&ctx, |ui| {
                            ui.centered_and_justified(|ui| ui.heading("Under Construction..."))
                        });
                }
                CurrentWorkspace::Settings => {
                    egui::CentralPanel::default()
                        .frame(Frame::central_panel(&ctx.style()))
                        .show(&ctx, |ui| {
                            if let Err(e) =
                                world.run_system_cached_with(tabs::settings::show_settings, ui)
                            {
                                error!("UI error: {}", e);
                            };
                        });
                }
            }
        });
    app.bevy_app.world_mut().insert_resource(mus);
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
