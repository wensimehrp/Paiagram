//! # UI
//! Module for the user interface.

pub mod tabs;
mod widgets;

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use bevy::prelude::*;
use egui::{Context, CornerRadius, Frame, Id, Margin, ScrollArea, Ui};
use egui_dock::{DockArea, DockState, TabViewer};
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use tabs::{Tab, all_tabs::*};

use crate::units::time::Tick;
use crate::{
    import::{DownloadFile, LoadGTFS, LoadOuDia, LoadQETRC},
    route::Route,
    settings::UserPreferences,
    trip::Trip,
    units::time::TimetableTime,
    vehicle::Vehicle,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainUiState>()
            .init_resource::<AdditionalUiState>()
            .init_resource::<GlobalTimer>()
            .init_resource::<UiModal>()
            .add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin)
            .add_message::<OpenOrFocus>()
            .add_systems(
                Update,
                (open_or_focus_tab.run_if(on_message::<OpenOrFocus>),),
            );
    }
}

// TODO: move to selected item resource
/// The current selected item
#[derive(Reflect, Clone, PartialEq, Eq)]
pub enum SelectedItem {
    /// A timetable entry
    TimetableEntry { entry: Entity, parent: Entity },
    /// An interval connecting two stations
    Interval(Entity, Entity),
    /// A station
    Station(Entity),
    /// Extending a trip
    ExtendingTrip {
        entry: Entity,
        previous_pos: Option<(TimetableTime, usize)>,
        last_time: Option<TimetableTime>,
        current_entry: Option<Entity>,
    },
}

#[derive(Reflect, Resource, Deref, DerefMut)]
#[reflect(Resource)]
pub struct SelectedItems(Vec<SelectedItem>);

enum Modals {
    OpenUrl(String),
}

impl Modals {
    fn id(&self) -> egui::Id {
        match self {
            Self::OpenUrl(_) => "openurl".into(),
        }
    }
    fn display(&mut self, ui: &mut egui::Ui, world: &mut World) {
        match self {
            Self::OpenUrl(buf) => {
                ui.heading("Import from URL");
                ui.label("Download the file from the Internet then import it into Paiagram");
                ui.strong("URL:");
                ui.text_edit_singleline(buf);
                if ui.button("Download and Import").clicked() {
                    world.trigger(DownloadFile { url: buf.clone() });
                    ui.close();
                }
            }
        }
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct UiModal(Option<Modals>);

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct GlobalTimer {
    value: AtomicI64,
    locker: AtomicU64,
    animation_speed: f64,
    animation_playing: bool,
}

impl Default for GlobalTimer {
    fn default() -> Self {
        Self {
            value: AtomicI64::new(0),
            locker: AtomicU64::new(Self::UNLOCKED),
            animation_speed: 10.0,
            animation_playing: false,
        }
    }
}

impl GlobalTimer {
    const UNLOCKED: u64 = u64::MAX;
    pub fn read_ticks(&self) -> Tick {
        Tick(self.value.load(Ordering::Acquire))
    }

    pub fn write_ticks(&self, value: Tick) {
        self.value.store(value.0, Ordering::Release);
    }

    pub fn read_seconds(&self) -> f64 {
        self.read_ticks().as_seconds_f64()
    }

    pub fn write_seconds(&self, value: f64) {
        let ticks_per_second = Tick::from_timetable_time(TimetableTime(1)).0 as f64;
        let ticks = (value * ticks_per_second).round() as i64;
        self.write_ticks(Tick(ticks));
    }

    pub fn is_locked(&self) -> bool {
        self.locker.load(Ordering::Acquire) != Self::UNLOCKED
    }

    pub fn try_lock(&self, id: Entity) -> bool {
        let id_bits = id.to_bits();

        let result = self.locker.compare_exchange(
            Self::UNLOCKED,
            id_bits,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        result.is_ok() || result.unwrap_err() == id_bits
    }

    pub fn try_unlock(&self, id: Entity) {
        let _ = self.locker.compare_exchange(
            id.to_bits(),
            Self::UNLOCKED,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }

    pub fn owner(&self) -> u64 {
        self.locker.load(Ordering::Acquire)
    }

    pub unsafe fn try_lock_unchecked(&self, id: u64) -> bool {
        self.try_lock(Entity::from_bits(id))
    }

    pub unsafe fn try_unlock_unchecked(&self, id: u64) {
        self.try_unlock(Entity::from_bits(id))
    }
}

macro_rules! for_all_tabs {
    ($tab:expr, $t:ident, $body:expr) => {
        match $tab {
            MainTab::Start($t) => $body,
            // MainTab::Vehicle($t) => $body,
            // MainTab::StationTimetable($t) => $body,
            MainTab::Diagram($t) => $body,
            // MainTab::DisplayedLines($t) => $body,
            MainTab::Settings($t) => $body,
            MainTab::Classes($t) => $body,
            // MainTab::Services($t) => $body,
            // MainTab::Minesweeper($t) => $body,
            MainTab::Graph($t) => $body,
            MainTab::Inspector($t) => $body,
            MainTab::Trip($t) => $body,
            MainTab::AllTrips($t) => $body,
            MainTab::PriorityGraph($t) => $body,
        }
    };
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum MainTab {
    Start(StartTab),
    // Vehicle(VehicleTab),
    // StationTimetable(StationTimetableTab),
    Diagram(DiagramTab),
    // DisplayedLines(DisplayedLinesTab),
    Settings(SettingsTab),
    Classes(ClassesTab),
    // Services(ServicesTab),
    // Minesweeper(MinesweeperTab),
    Graph(GraphTab),
    Inspector(InspectorTab),
    Trip(TripTab),
    AllTrips(AllTripsTab),
    PriorityGraph(PriorityGraphTab),
}

impl MapEntities for MainTab {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for_all_tabs!(self, t, t.map_entities(entity_mapper))
    }
}

#[derive(Reflect, Resource, Serialize, Deserialize, Clone, Deref, DerefMut)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
struct MainUiState(DockState<MainTab>);

impl Default for MainUiState {
    fn default() -> Self {
        Self(DockState::new(vec![MainTab::Start(StartTab::default())]))
    }
}

impl MapEntities for MainUiState {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for (_, tab) in self.0.iter_all_tabs_mut() {
            tab.map_entities(entity_mapper);
        }
    }
}

#[derive(Message)]
struct OpenOrFocus(MainTab);

fn open_or_focus_tab(mut tabs: MessageReader<OpenOrFocus>, mut state: ResMut<MainUiState>) {
    for tab in tabs.read() {
        if let Some((surface_index, node_index, tab_index)) = state.find_tab(&tab.0) {
            state.set_active_tab((surface_index, node_index, tab_index));
            state.set_focused_node_and_surface((surface_index, node_index));
        } else {
            state.push_to_focused_leaf(tab.0.clone());
        }
    }
}

struct MainTabViewer<'w> {
    world: &'w mut World,
}

impl<'w> TabViewer for MainTabViewer<'w> {
    type Tab = MainTab;
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        for_all_tabs!(tab, t, t.title())
    }
    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        for_all_tabs!(tab, t, t.main_display(self.world, ui));
    }
    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        for_all_tabs!(tab, t, t.id())
    }
    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        for_all_tabs!(tab, t, t.scroll_bars())
    }
    fn add_popup(
        &mut self,
        ui: &mut Ui,
        _surface: egui_dock::SurfaceIndex,
        _node: egui_dock::NodeIndex,
    ) {
        for (s, t) in [
            ("Start", MainTab::Start(StartTab::default())),
            ("Inspector", MainTab::Inspector(InspectorTab::default())),
            ("Settings", MainTab::Settings(SettingsTab::default())),
            ("Classes", MainTab::Classes(ClassesTab::default())),
            ("Graph", MainTab::Graph(GraphTab::default())),
        ] {
            if ui.button(s).clicked() {
                self.world.write_message(OpenOrFocus(t));
                ui.close();
            }
        }
        ui.menu_button("All Trips", |ui| {
            if ui.button("New Route").clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if let Some(e) = self
                    .world
                    .run_system_cached_with(show_name_button::<Route>, ui)
                    .unwrap()
                {
                    self.world
                        .write_message(OpenOrFocus(MainTab::AllTrips(AllTripsTab::new(e))));
                }
            });
        });
        ui.menu_button("Priority Graph", |ui| {
            if ui.button("New Route").clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if let Some(e) = self
                    .world
                    .run_system_cached_with(show_name_button::<Route>, ui)
                    .unwrap()
                {
                    self.world.write_message(OpenOrFocus(MainTab::PriorityGraph(
                        PriorityGraphTab::new(e),
                    )));
                }
            });
        });
        ui.menu_button("Diagrams", |ui| {
            if ui.button("New Route").clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if let Some(e) = self
                    .world
                    .run_system_cached_with(show_name_button::<Route>, ui)
                    .unwrap()
                {
                    self.world
                        .write_message(OpenOrFocus(MainTab::Diagram(DiagramTab::new(e))));
                }
            });
        });
        ui.menu_button("Trips", |ui| {
            if ui.button("New Trip").clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if let Some(e) = self
                    .world
                    .run_system_cached_with(show_name_button::<Trip>, ui)
                    .unwrap()
                {
                    self.world
                        .write_message(OpenOrFocus(MainTab::Trip(TripTab::new(e))));
                }
            });
        });
        ui.menu_button("Vehicles", |ui| {
            if ui.button("New Vehicle").clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                if let Some(e) = self
                    .world
                    .run_system_cached_with(show_name_button::<Vehicle>, ui)
                    .unwrap()
                {
                    // self.world
                    //     .write_message(OpenTab(MainTab::Diagram(DiagramTab::new(e))));
                }
            });
        });
    }
    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        if matches!(tab, MainTab::Start(_)) {
            true
        } else {
            false
        }
    }
}

fn show_name_button<T: Component>(
    InMut(ui): InMut<Ui>,
    names: Query<(Entity, &Name), With<T>>,
) -> Option<Entity> {
    for (e, name) in names {
        if ui.button(name.as_str()).clicked() {
            ui.close();
            return Some(e);
        }
    }
    return None;
}

#[derive(Serialize, Deserialize, Clone, Copy)]
enum AdditionalTab {
    Edit,
    Properties,
    Export,
}

#[derive(Reflect, Resource, Serialize, Deserialize, Clone, Deref, DerefMut)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
struct AdditionalUiState(DockState<AdditionalTab>);

impl Default for AdditionalUiState {
    fn default() -> Self {
        Self(DockState::new(vec![
            AdditionalTab::Edit,
            AdditionalTab::Properties,
            AdditionalTab::Export,
        ]))
    }
}

struct AdditionalTabViewer<'w> {
    world: &'w mut World,
    focused_tab: Option<&'w mut MainTab>,
}

impl<'w> TabViewer for AdditionalTabViewer<'w> {
    type Tab = AdditionalTab;
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match *tab {
            AdditionalTab::Edit => "Edit",
            AdditionalTab::Properties => "Properties",
            AdditionalTab::Export => "Export",
        }
        .into()
    }
    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        let Some(ref mut focused) = self.focused_tab else {
            ui.label("Nothing focused");
            return;
        };
        match *tab {
            AdditionalTab::Edit => {
                for_all_tabs!(focused, t, t.edit_display(self.world, ui))
            }
            AdditionalTab::Properties => {
                for_all_tabs!(focused, t, t.display_display(self.world, ui))
            }
            AdditionalTab::Export => {
                for_all_tabs!(focused, t, t.export_display(self.world, ui))
            }
        }
    }
}

/// WASM fullscreen toggle
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function toggle_fullscreen(id) {
    if (!document.fullscreenElement) {
        const el = document.getElementById(id);
        if (el?.requestFullscreen) {
            el.requestFullscreen().catch(err => {
                console.error(`Error attempting to enable full-screen mode: ${err.message}`);
            });
        }
    } else {
        if (document.exitFullscreen) {
            document.exitFullscreen();
        }
    }
}
"#)]
extern "C" {
    fn toggle_fullscreen(id: &str);
}

pub fn show_ui(ctx: &Context, world: &mut World) {
    world.run_system_cached_with(sync_ui, ctx).unwrap();
    world.resource_scope(|world, mut modal: Mut<UiModal>| {
        let Some(m) = &mut modal.0 else { return };
        let modal_response = egui::Modal::new(m.id()).show(ctx, |ui| m.display(ui, world));
        if modal_response.should_close() {
            modal.0 = None
        }
    });
    egui::TopBottomPanel::top("top panel")
        .exact_height(32.0)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let res = ui.button("More...");
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Fullscreen").clicked() {
                    let is_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                }
                #[cfg(target_arch = "wasm32")]
                if ui.button("Fullscreen").clicked() {
                    toggle_fullscreen("paiagram_canvas");
                }
                egui::Popup::menu(&res).show(|ui| {
                    if ui.button("Import from URL...").clicked() {
                        world.resource_mut::<UiModal>().0 = Some(Modals::OpenUrl(String::new()));
                    }
                    ui.separator();
                    if ui.button("Read OuDia...").clicked() {
                        world.commands().trigger(crate::rw::read::ReadFile {
                            title: "Load OuDia Files".to_string(),
                            extensions: vec![("OuDia Files".to_string(), vec!["oud".to_string()])],
                            callback: |c, s| {
                                c.trigger(LoadOuDia::original(s));
                            },
                        });
                    }
                    if ui.button("Read OuDiaSecond...").clicked() {
                        world.commands().trigger(crate::rw::read::ReadFile {
                            title: "Load OuDiaSecond Files".to_string(),
                            extensions: vec![(
                                "OuDiaSecond Files".to_string(),
                                vec!["oud2".to_string()],
                            )],
                            callback: |c, s| {
                                c.trigger(LoadOuDia::second(String::from_utf8(s).unwrap()));
                            },
                        });
                    }
                    if ui.button("Read qETRC/pyETRC...").clicked() {
                        world.commands().trigger(crate::rw::read::ReadFile {
                            title: "Load qETRC Files".to_string(),
                            extensions: vec![(
                                "qETRC Files".to_string(),
                                vec!["json".to_string(), "pyetgr".to_string()],
                            )],
                            callback: |c, s| {
                                c.trigger(LoadQETRC {
                                    content: String::from_utf8(s).unwrap(),
                                });
                            },
                        });
                    }
                    if ui.button("Read GTFS...").clicked() {
                        world.commands().trigger(crate::rw::read::ReadFile {
                            title: "Load GTFS Files".to_string(),
                            extensions: vec![("GTFS Files".to_string(), vec!["zip".to_string()])],
                            callback: |c, s| {
                                c.trigger(LoadGTFS { content: s });
                            },
                        });
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut timer = world.resource_mut::<GlobalTimer>();
                    let mut seconds = timer.read_seconds();
                    ui.checkbox(&mut timer.animation_playing, "Play animation");
                    let time_response = ui.add(
                        egui::DragValue::new(&mut seconds)
                            .custom_formatter(|it, _| {
                                format!("{}", TimetableTime::from_hms(0, 0, it as i32))
                            })
                            .custom_parser(|s| TimetableTime::from_str(s).map(|it| it.0 as f64)),
                    );
                    ui.add(
                        egui::Slider::new(&mut timer.animation_speed, -500.0..=500.0)
                            .fixed_decimals(1)
                            .text("Speed")
                            .clamping(egui::SliderClamping::Always),
                    );
                    unsafe {
                        if time_response.dragged() && timer.try_lock_unchecked(1) {
                            timer.write_seconds(seconds);
                        } else {
                            timer.try_unlock_unchecked(1);
                        }
                    }
                    if timer.animation_playing {
                        if !timer.is_locked() {
                            seconds += timer.animation_speed * ui.input(|r| r.stable_dt) as f64;
                            timer.write_seconds(seconds);
                        }
                        ui.ctx().request_repaint();
                    }
                    if ui.button("P").clicked() {}
                    if ui.button("R").clicked() {}
                    if ui.button("B").clicked() {}
                });
            })
        });
    let make_dock_style = |ui: &Ui| {
        let mut s = egui_dock::Style::from_egui(ui.style());
        s.tab.tab_body.inner_margin = Margin::same(0);
        s.tab.tab_body.corner_radius = CornerRadius::ZERO;
        s.tab.tab_body.stroke.width = 0.0;
        s.tab.active.corner_radius = CornerRadius::ZERO;
        s.tab.inactive.corner_radius = CornerRadius::ZERO;
        s.tab.focused.corner_radius = CornerRadius::ZERO;
        s.tab.hovered.corner_radius = CornerRadius::ZERO;
        s.tab.inactive_with_kb_focus.corner_radius = CornerRadius::ZERO;
        s.tab.active_with_kb_focus.corner_radius = CornerRadius::ZERO;
        s.tab.focused_with_kb_focus.corner_radius = CornerRadius::ZERO;
        s.tab_bar.corner_radius = CornerRadius::ZERO;
        s
    };
    world.resource_scope(|world, mut aus: Mut<AdditionalUiState>| {
        world.resource_scope(|mut world, mut mus: Mut<MainUiState>| {
            let mut tab_viewer = AdditionalTabViewer {
                world: &mut world,
                focused_tab: mus.find_active_focused().map(|(_, f)| f),
            };
            egui::SidePanel::right("right panel")
                .frame(Frame::default())
                .show(ctx, |ui| {
                    DockArea::new(&mut aus)
                        .show_close_buttons(false)
                        .show_leaf_close_all_buttons(false)
                        .show_leaf_collapse_buttons(false)
                        .id(Id::new("right panel content"))
                        .style(make_dock_style(ui))
                        .show_inside(ui, &mut tab_viewer);
                });
            let mut tab_viewer = MainTabViewer { world: &mut world };
            egui::CentralPanel::default()
                .frame(Frame::default())
                .show(ctx, |ui| {
                    DockArea::new(&mut mus)
                        .show_leaf_close_all_buttons(false)
                        .id(Id::new("main panel content"))
                        .show_add_buttons(true)
                        .show_add_popup(true)
                        .style(make_dock_style(ui))
                        .show_inside(ui, &mut tab_viewer);
                });
        })
    });
}

fn sync_ui(InRef(ctx): InRef<Context>, preferences: Res<UserPreferences>) {
    if !preferences.is_changed() {
        return;
    }
    if preferences.dark_mode {
        ctx.set_theme(egui::Theme::Dark);
    } else {
        ctx.set_theme(egui::Theme::Light);
    }
}

pub fn apply_custom_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/SarasaUiSC-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "dia_pro".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/DiaPro-Regular.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "my_font".to_owned());
    fonts.families.insert(
        egui::FontFamily::Name("dia_pro".into()),
        vec!["dia_pro".to_owned(), "my_font".to_owned()],
    );
    ctx.set_fonts(fonts);
}
