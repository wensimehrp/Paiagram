mod about;
mod side_panel;
mod tabs;
mod widgets;
use crate::colors;
use crate::settings::ApplicationSettings;
use bevy::color::palettes::tailwind::{EMERALD_700, EMERALD_800, GRAY_900};
use bevy::prelude::*;
use egui::{
    self, Color32, CornerRadius, Frame, Id, Margin, Rect, Sense, Shape, Stroke, Ui, UiBuilder,
};
use egui_dock::{DockArea, DockState, TabInteractionStyle};
use egui_i18n::tr;
use moonshine_core::save::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use strum::{EnumCount, IntoEnumIterator};
use tabs::diagram::SelectedEntityType;
use tabs::minesweeper::MinesweeperData;
use tabs::{Tab, all_tabs::*};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

/// Plugin that sets up the user interface
pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiCommand>()
            .init_resource::<MiscUiState>()
            .init_resource::<SelectedElement>()
            .init_resource::<MinesweeperData>()
            .init_resource::<SidePanelState>()
            .insert_resource(UiState::new())
            .insert_resource(StatusBarState::default())
            .add_systems(Update, (modify_dock_state.run_if(on_message::<UiCommand>),));
    }
}

/// The state of the user interface
#[derive(Resource, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Resource, MapEntities, Serialize, Deserialize, opaque)]
pub struct UiState {
    dock_state: DockState<AppTab>,
}

impl MapEntities for UiState {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for (_, tab) in self.dock_state.iter_all_tabs_mut() {
            tab.map_entities(entity_mapper);
        }
    }
}

// TODO: move this elsewhere
#[derive(Default, Resource, Deref, DerefMut)]
pub struct SelectedElement(pub Option<SelectedEntityType>);

// TODO: move this UI state elsewhere
#[derive(Resource)]
pub struct MiscUiState {
    is_dark_mode: bool,
    initialized: bool,
    frame_times: egui::util::History<f32>,
    modal_open: bool,
    fullscreened: bool,
    supplementary_panel_state: SupplementaryPanelState,
}

#[derive(Default)]
pub struct SupplementaryPanelState {
    expanded: bool,
    is_on_bottom: bool,
}

impl Default for MiscUiState {
    fn default() -> Self {
        let max_age: f32 = 1.0;
        let max_len = (max_age * 300.0).round() as usize;
        Self {
            is_dark_mode: true,
            frame_times: egui::util::History::new(0..max_len, max_age),
            initialized: false,
            modal_open: false,
            fullscreened: false,
            supplementary_panel_state: SupplementaryPanelState::default(),
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
            dock_state: DockState::new(vec![AppTab::Start(StartTab::default())]),
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

macro_rules! for_all_tabs {
    ($tab:expr, $t:ident, $body:expr) => {
        match $tab {
            AppTab::Start($t) => $body,
            AppTab::Vehicle($t) => $body,
            AppTab::StationTimetable($t) => $body,
            AppTab::Diagram($t) => $body,
            AppTab::DisplayedLines($t) => $body,
            AppTab::Settings($t) => $body,
            AppTab::Classes($t) => $body,
            AppTab::Services($t) => $body,
            AppTab::Minesweeper($t) => $body,
            AppTab::Graph($t) => $body,
            AppTab::Inspector($t) => $body,
        }
    };
}

/// An application tab
#[derive(Clone, Serialize, Deserialize)]
pub enum AppTab {
    Start(StartTab),
    Vehicle(VehicleTab),
    StationTimetable(StationTimetableTab),
    Diagram(DiagramTab),
    DisplayedLines(DisplayedLinesTab),
    Settings(SettingsTab),
    Classes(ClassesTab),
    Services(ServicesTab),
    Minesweeper(MinesweeperTab),
    Graph(GraphTab),
    Inspector(InspectorTab),
}

impl MapEntities for AppTab {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        match self {
            AppTab::Vehicle(tab) => tab.map_entities(entity_mapper),
            AppTab::StationTimetable(tab) => tab.map_entities(entity_mapper),
            AppTab::Diagram(tab) => tab.map_entities(entity_mapper),
            AppTab::Graph(tab) => tab.map_entities(entity_mapper),
            AppTab::Start(_)
            | AppTab::DisplayedLines(_)
            | AppTab::Settings(_)
            | AppTab::Classes(_)
            | AppTab::Services(_)
            | AppTab::Inspector(_)
            | AppTab::Minesweeper(_) => {}
        }
    }
}

impl AppTab {
    pub fn id(&self) -> egui::Id {
        for_all_tabs!(self, t, t.id())
    }
    pub fn color(&self) -> Color32 {
        let num = self.id().value();
        // given the num, generate a color32 from it
        let color = colors::PredefinedColor::iter()
            .nth(num as usize % colors::PredefinedColor::COUNT)
            .unwrap();
        color.get(true)
    }
}

impl PartialEq for AppTab {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
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
    ctx: &'w egui::Context,
    focused_id: Option<egui::Id>,
}

impl<'w> egui_dock::TabViewer for AppTabViewer<'w> {
    type Tab = AppTab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        for_all_tabs!(
            tab,
            t,
            t.frame().show(ui, |ui| t.main_display(self.world, ui))
        );

        // focus ring
        let is_focused = self.focused_id == Some(tab.id());
        let strength = ui
            .ctx()
            .animate_bool(ui.id().with("focus_highlight"), is_focused);
        if strength > 0.0 {
            ui.painter().rect_stroke(
                ui.clip_rect(),
                0,
                Stroke {
                    width: 1.8,
                    color: tab.color().linear_multiply(strength),
                },
                egui::StrokeKind::Inside,
            );
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        for_all_tabs!(tab, t, t.title())
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        tab.id()
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        for_all_tabs!(tab, t, t.scroll_bars())
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        for_all_tabs!(tab, t, t.on_tab_button(self.world, response))
    }

    fn tab_style_override(
        &self,
        tab: &Self::Tab,
        global_style: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        Some(egui_dock::TabStyle {
            focused: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(180),
                text_color: if self.ctx.theme().default_visuals().dark_mode {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
                ..global_style.focused
            },
            active: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(120),
                ..global_style.active
            },
            active_with_kb_focus: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(120),
                ..global_style.active_with_kb_focus
            },
            hovered: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(120),
                ..global_style.hovered
            },
            inactive: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(90),
                ..global_style.inactive
            },
            inactive_with_kb_focus: TabInteractionStyle {
                bg_fill: tab.color().gamma_multiply_u8(90),
                ..global_style.inactive_with_kb_focus
            },
            ..global_style.clone()
        })
    }
}

#[derive(Resource)]
struct SidePanelState {
    dock_state: DockState<SidePanelTab>,
}

impl Default for SidePanelState {
    fn default() -> Self {
        let dock_state = DockState::new(vec![
            SidePanelTab::Edit,
            SidePanelTab::Details,
            SidePanelTab::Export,
        ]);
        Self { dock_state }
    }
}

enum SidePanelTab {
    Edit,
    Details,
    Export,
}

struct SidePanelViewer<'w> {
    world: &'w mut World,
    focused_tab: Option<&'w mut AppTab>,
}

impl<'w> egui_dock::TabViewer for SidePanelViewer<'w> {
    type Tab = SidePanelTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let Some(focused_tab) = self.focused_tab.as_deref_mut() else {
            ui.label("No tabs focused. Open a tab to see its properties.");
            return;
        };
        use egui::NumExt;
        let dt = ui.ctx().input(|input| input.stable_dt).at_most(0.1);
        let t = egui::emath::exponential_smooth_factor(0.9, 0.2, dt);
        let mut repaint = false;
        let opacity = ui.ctx().data_mut(|map| {
            let opacity: &mut f32 = map.get_temp_mut_or(Id::new("side panel opacity"), 0.0);
            if *opacity > 0.99 {
                *opacity = 1.0;
            } else {
                *opacity = emath::lerp(*opacity..=1.0, t);
                repaint = true;
            }
            *opacity
        });
        if repaint {
            ui.ctx().request_repaint();
        }
        ui.multiply_opacity(opacity);
        match tab {
            SidePanelTab::Edit => {
                for_all_tabs!(focused_tab, t, {
                    egui::Frame::new()
                        .inner_margin(4)
                        .show(ui, |ui| t.edit_display(self.world, ui));
                })
            }
            SidePanelTab::Details => {
                for_all_tabs!(focused_tab, t, {
                    egui::Frame::new()
                        .inner_margin(4)
                        .show(ui, |ui| t.display_display(self.world, ui));
                })
            }
            SidePanelTab::Export => {
                for_all_tabs!(focused_tab, t, {
                    egui::Frame::new()
                        .inner_margin(4)
                        .show(ui, |ui| t.export_display(self.world, ui));
                })
            }
        }
    }
    fn on_tab_button(&mut self, _tab: &mut Self::Tab, response: &egui::Response) {
        if !response.clicked() {
            return;
        }
        // we split the check into two parts. The first part is the new tab check
        let reload = response.ctx.data_mut(|map| {
            // store the previous tab id
            let previous_tab =
                map.get_temp_mut_or(Id::new("side panel previous tab id"), response.id);
            if *previous_tab != response.id {
                *previous_tab = response.id;
                true
            } else {
                false
            }
        });
        if !reload {
            return;
        }
        // reset the animation
        response.ctx.data_mut(|map| {
            let opacity: &mut f32 = map.get_temp_mut_or(Id::new("side panel opacity"), 0.0);
            *opacity = 0.0;
        });
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            SidePanelTab::Edit => tr!("side-panel-edit"),
            SidePanelTab::Details => tr!("side-panel-details"),
            SidePanelTab::Export => tr!("side-panel-export"),
        }
        .into()
    }
    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        match tab {
            SidePanelTab::Edit => Id::new("edit"),
            SidePanelTab::Details => Id::new("details"),
            SidePanelTab::Export => Id::new("export"),
        }
    }
    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }
}

/// WASM fullscreen functions
/// SAFETY: These functions are unsafe because they interact with the DOM directly.
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
    let mut side_panel_state = app
        .bevy_app
        .world_mut()
        .remove_resource::<SidePanelState>()
        .unwrap();
    let frame_time = mus.mean_frame_time();
    app.bevy_app
        .world_mut()
        .resource_scope(|world, mut ui_state: Mut<UiState>| {
            egui::TopBottomPanel::top("menu_bar")
                .frame(Frame::side_top_panel(&ctx.style()))
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
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
                            /// SAFETY: This function is unsafe because it interacts with the DOM directly.
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
                            mus.supplementary_panel_state.is_on_bottom =
                                !mus.supplementary_panel_state.is_on_bottom;
                        }
                        if ui.button("A").clicked() {
                            mus.supplementary_panel_state.expanded =
                                !mus.supplementary_panel_state.expanded;
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
                            if !world
                                .resource::<ApplicationSettings>()
                                .show_performance_stats
                            {
                                return;
                            }
                            ui.monospace(format!("eFPS: {:>6.1}", 1.0 / frame_time));
                            ui.monospace(format!("{:>5.2} ms/f", 1e3 * frame_time));
                        });
                    });
                });

            ctx.style_mut(|s| {
                s.visuals.widgets.noninteractive.bg_stroke.color = old_bg_stroke_color
            });

            let supplementary_panel_content = |ui: &mut Ui| {
                let focused_tab = ui_state
                    .dock_state
                    .find_active_focused()
                    .map(|(_, tab)| tab);
                let mut side_panel_viewer = SidePanelViewer { world, focused_tab };
                let mut style = egui_dock::Style::from_egui(ui.style());
                style.tab.tab_body.inner_margin = Margin::same(1);
                style.tab.tab_body.corner_radius = CornerRadius::ZERO;
                style.tab.tab_body.stroke.width = 0.0;
                style.tab_bar.corner_radius = CornerRadius::ZERO;
                DockArea::new(&mut side_panel_state.dock_state)
                    .id(Id::new("Side panel stuff"))
                    .draggable_tabs(false)
                    .show_leaf_close_all_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .style(style)
                    .show_inside(ui, &mut side_panel_viewer);
            };

            if mus.supplementary_panel_state.is_on_bottom {
                egui::TopBottomPanel::bottom("TreeView")
                    .frame(egui::Frame::new())
                    .resizable(false)
                    .exact_height(ctx.used_size().y / 2.5)
                    .show_animated(
                        ctx,
                        mus.supplementary_panel_state.expanded,
                        supplementary_panel_content,
                    );
            } else {
                egui::SidePanel::left("TreeView")
                    .frame(egui::Frame::new())
                    .default_width(ctx.used_size().x / 4.0)
                    .show_animated(
                        &ctx,
                        mus.supplementary_panel_state.expanded,
                        supplementary_panel_content,
                    );
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
                    let focused_id = ui_state
                        .dock_state
                        .find_active_focused()
                        .map(|(_, tab)| tab.id());
                    ui.ctx().data_mut(|map| {
                        let previously_focused =
                            map.get_temp_mut_or(Id::new("previously focused leaf"), focused_id);
                        if *previously_focused == focused_id {
                            return;
                        } else {
                            *previously_focused = focused_id;
                            let opacity: &mut f32 =
                                map.get_temp_mut_or(Id::new("side panel opacity"), 0.0);
                            *opacity = 0.0;
                        }
                    });
                    let mut tab_viewer = AppTabViewer {
                        world,
                        ctx,
                        focused_id,
                    };
                    let mut style = egui_dock::Style::from_egui(ui.style());
                    style.tab.tab_body.inner_margin = Margin::same(0);
                    style.tab.tab_body.corner_radius = CornerRadius::ZERO;
                    style.tab.tab_body.stroke.width = 0.0;
                    style.tab.active.outline_color = Color32::TRANSPARENT;
                    style.tab.inactive.outline_color = Color32::TRANSPARENT;
                    style.tab.focused.outline_color = Color32::TRANSPARENT;
                    style.tab.hovered.outline_color = Color32::TRANSPARENT;
                    style.tab.inactive_with_kb_focus.outline_color = Color32::TRANSPARENT;
                    style.tab.active_with_kb_focus.outline_color = Color32::TRANSPARENT;
                    style.tab.focused_with_kb_focus.text_color = Color32::TRANSPARENT;
                    style.tab.active.corner_radius = CornerRadius::ZERO;
                    style.tab.inactive.corner_radius = CornerRadius::ZERO;
                    style.tab.focused.corner_radius = CornerRadius::ZERO;
                    style.tab.hovered.corner_radius = CornerRadius::ZERO;
                    style.tab.inactive_with_kb_focus.corner_radius = CornerRadius::ZERO;
                    style.tab.active_with_kb_focus.corner_radius = CornerRadius::ZERO;
                    style.tab.focused_with_kb_focus.corner_radius = CornerRadius::ZERO;
                    style.tab_bar.corner_radius = CornerRadius::ZERO;
                    style.tab_bar.hline_color = Color32::TRANSPARENT;
                    style.tab.hline_below_active_tab_name = true;
                    style.overlay.selection_corner_radius = CornerRadius::same(4);
                    style.tab_bar.height = 32.0;
                    // place a button on the bottom left corner for expanding and collapsing the side panel.
                    let left_bottom = ui.max_rect().left_bottom();
                    let shift = egui::Vec2 { x: 20.0, y: -40.0 };
                    DockArea::new(&mut ui_state.dock_state)
                        .style(style)
                        .show_inside(ui, &mut tab_viewer);
                    let res = ui.place(
                        Rect::from_two_pos(left_bottom, left_bottom + shift),
                        |ui: &mut Ui| {
                            let (resp, painter) =
                                ui.allocate_painter(ui.available_size(), Sense::click());
                            let rect = resp.rect;
                            painter.add(Shape::convex_polygon(
                                vec![
                                    rect.left_bottom() + egui::Vec2 { x: 0.0, y: 0.0 },
                                    rect.left_bottom() + egui::Vec2 { x: 0.0, y: -40.0 },
                                    rect.left_bottom() + egui::Vec2 { x: 20.0, y: -20.0 },
                                    rect.left_bottom() + egui::Vec2 { x: 20.0, y: 0.0 },
                                ],
                                ui.visuals().widgets.hovered.bg_fill,
                                Stroke::NONE,
                            ));
                            resp
                        },
                    );
                    if res.clicked() {
                        mus.supplementary_panel_state.expanded =
                            !mus.supplementary_panel_state.expanded;
                    }
                });
        });
    app.bevy_app.world_mut().insert_resource(mus);
    app.bevy_app.world_mut().insert_resource(side_panel_state);
    Ok(())
}

/// Apply custom fonts to the egui context
pub fn apply_custom_fonts(ctx: &egui::Context) {
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
