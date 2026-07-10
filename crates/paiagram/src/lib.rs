//! Definitions for the user interface.

mod command_palette;
pub mod export_typst_diagram;
pub mod save;
mod tabs;
mod widgets;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use chrono::{Local, Timelike};
use egui::{
    Color32, Context, Frame, Key, KeyboardShortcut, Modifiers, OpenUrl, Panel, Response, RichText,
    ScrollArea, Stroke, Ui,
};
use egui_i18n::tr;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
use std::collections::VecDeque;
use paiagram_core::colors::{DisplayedColor, PredefinedColor};
use paiagram_core::settings::{ProjectSettings, UserPreferences};
use paiagram_core::trip::{TEntry, TripSchedule};
use paiagram_core::units::time::{Tick, TimetableTime};
use paiagram_core::{Command, StationKey, TripKey, WorldSnapshot};
use serde::{Deserialize, Serialize};
use tabs::Tab;
use tabs::all_tabs::*;
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub use tabs::AppState;

use crate::tabs::text::TextMessage;
use crate::widgets::TimeDragValue;

/// The truth of the application.
#[derive(Clone)]
pub(crate) enum PendingTabOp {
    Open(MainTab),
}

pub struct PaiagramApp {
    pub state: AppState,
}

impl PaiagramApp {
    pub fn new() -> Self {
        Self {
            state: AppState {
                source: paiagram_core::Source::default(),
                preferences: UserPreferences::default(),
                project_settings: ProjectSettings::default(),
                selected_items: SelectedItems::None,
                timer: GlobalTimer::default(),
                main_ui: MainUiState::default(),
                additional_ui: AdditionalUiState::default(),
                modal: UiModal(None),
                frame_time_history: FrameTimeHistory::default(),
                pending_tabs: VecDeque::new(),
                loaded_scene: None,
                load_error: None,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TripSelection {
    pub(crate) trip: TripKey,
    pub(crate) entries: Vec1<usize>, // Entry indices within the trip's schedule
}

impl PartialEq for TripSelection {
    fn eq(&self, other: &Self) -> bool {
        self.trip == other.trip
    }
}

#[derive(Clone, Copy, PartialEq, Hash, Debug, Eq, PartialOrd, Ord)]
pub(crate) struct IntervalSelection {
    pub(crate) source_key: StationKey,
    pub(crate) target_key: StationKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StationSelection {
    pub(crate) station: StationKey,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingRouteSelection {
    pub(crate) prev_station: StationKey,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingTripSelection {
    pub(crate) trip: TripKey,
    pub(crate) previous_pos: Option<(Tick, usize)>,
    pub(crate) last_time: Option<TimetableTime>,
    pub(crate) current_entry: Option<usize>,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct CoordinateSelection {
    pub(crate) pos: paiagram_core::LonLat,
    pub(crate) name_candidate: String,
}

#[derive(Clone, PartialEq, Debug)]
#[non_exhaustive]
pub(crate) enum SelectedItems {
    None,
    Trips(Vec1<TripSelection>),
    Intervals(Vec1<IntervalSelection>),
    Stations(Vec1<StationSelection>),
    ExtendingRoute(ExtendingRouteSelection),
    ExtendingTrip(ExtendingTripSelection),
    Coordinate(CoordinateSelection),
}

impl SelectedItems {
    fn toggle_entries<T: Ord + Clone>(
        entries: &mut Vec1<T>,
        incoming: impl Iterator<Item = T>,
    ) -> bool {
        entries.extend(incoming);
        entries.sort_unstable();

        let mut result = Vec::new();
        let mut i = 0;
        let data = entries.as_vec();

        while i < data.len() {
            let mut j = i + 1;
            while j < data.len() && data[j] == data[i] {
                j += 1;
            }
            if (j - i) % 2 != 0 {
                result.push(data[i].clone());
            }
            i = j;
        }

        if result.is_empty() {
            true
        } else {
            *entries = Vec1::try_from_vec(result).unwrap();
            false
        }
    }

    pub(crate) fn toggle_selection(&mut self, item: SelectedItem) {
        let mut should_reset = false;
        match (item, &mut *self) {
            (SelectedItem::None, _) => return,
            (SelectedItem::Trip(sel), Self::Trips(it)) => {
                it.sort_unstable_by_key(|t| t.trip);
                match it.binary_search_by_key(&sel.trip, |t| t.trip) {
                    Ok(idx) => {
                        if Self::toggle_entries(&mut it[idx].entries, sel.entries.into_iter()) {
                            if it.len() == 1 {
                                should_reset = true;
                            } else {
                                it.remove(idx);
                            }
                        }
                    }
                    Err(idx) => {
                        it.insert(idx, sel);
                    }
                }
            }
            (SelectedItem::Interval(sel), Self::Intervals(it)) => {
                should_reset = Self::toggle_entries(it, std::iter::once(sel));
            }
            (SelectedItem::Station(sel), Self::Stations(it)) => {
                should_reset = Self::toggle_entries(it, std::iter::once(sel));
            }
            _ => {}
        }
        if should_reset {
            *self = SelectedItems::None;
        }
    }

    pub(crate) fn set_single_selection(&mut self, item: SelectedItem) {
        match item {
            SelectedItem::None => *self = Self::None,
            SelectedItem::Trip(it) => *self = Self::Trips(vec1![it]),
            SelectedItem::Interval(it) => *self = Self::Intervals(vec1![it]),
            SelectedItem::Station(it) => *self = Self::Stations(vec1![it]),
            SelectedItem::Coordinate(pos) => *self = Self::Coordinate(pos),
        }
    }
}

#[derive(Clone)]
#[non_exhaustive]
pub(crate) enum SelectedItem {
    None,
    Trip(TripSelection),
    Interval(IntervalSelection),
    Station(StationSelection),
    Coordinate(CoordinateSelection),
}

impl Default for SelectedItems { fn default() -> Self { Self::None } }
impl Default for SelectedItem { fn default() -> Self { Self::None } }

enum Modals {
    OpenUrl(String),
}

impl Modals {
    fn id(&self) -> egui::Id {
        match self {
            Self::OpenUrl(_) => "openurl".into(),
        }
    }
    fn display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        match self {
            Self::OpenUrl(buf) => {
                ui.heading(tr!("menu-import-url-heading"));
                ui.label(tr!("menu-import-url-desc"));
                ui.strong(tr!("menu-url-label"));
                ui.text_edit_singleline(buf);
                if ui.button(tr!("menu-download-and-import")).clicked() {
                    let url = buf.clone();
                    // TODO: trigger download via paiagram-core's import
                    // app.source.import_url(url);
                    ui.close();
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct UiModal(pub(crate) Option<Modals>);

#[derive(Clone)]
pub(crate) struct FrameTimeHistory {
    values: [f32; Self::CAPACITY],
    next_index: usize,
}

impl FrameTimeHistory {
    const CAPACITY: usize = 255;
    fn push(&mut self, dt_seconds: f32) {
        self.values[self.next_index] = dt_seconds;
        self.next_index = (self.next_index + 1) % Self::CAPACITY;
    }
    fn average_dt(&self) -> f32 {
        let sum: f32 = self.values.iter().sum();
        sum / Self::CAPACITY as f32
    }
    fn previous_n(&self, n: usize) -> impl Iterator<Item = f32> {
        let count = n.min(Self::CAPACITY);
        (0..count).map(move |i| {
            let index = (self.next_index + Self::CAPACITY - 1 - i) % Self::CAPACITY;
            self.values[index]
        })
    }
}

impl Default for FrameTimeHistory {
    fn default() -> Self {
        Self {
            values: [0.0; Self::CAPACITY],
            next_index: 0,
        }
    }
}

pub(crate) struct GlobalTimer {
    value: AtomicI64,
    locker: AtomicU64,
    animation_speed: f64,
    animation_playing: bool,
    sync_to_real_time: bool,
}

impl Clone for GlobalTimer {
    fn clone(&self) -> Self {
        Self {
            value: AtomicI64::new(self.value.load(Ordering::Acquire)),
            locker: AtomicU64::new(self.locker.load(Ordering::Acquire)),
            animation_speed: self.animation_speed,
            animation_playing: self.animation_playing,
            sync_to_real_time: self.sync_to_real_time,
        }
    }
}

impl Default for GlobalTimer {
    fn default() -> Self {
        Self {
            value: AtomicI64::new(0),
            locker: AtomicU64::new(Self::UNLOCKED),
            animation_speed: 10.0,
            animation_playing: false,
            sync_to_real_time: false,
        }
    }
}

impl GlobalTimer {
    const UNLOCKED: u64 = u64::MAX;
    pub(crate) fn read_ticks(&self) -> Tick { Tick(self.value.load(Ordering::Acquire)) }
    pub(crate) fn write_ticks(&self, value: Tick) { self.value.store(value.0, Ordering::Release); }
    pub(crate) fn read_seconds(&self) -> f64 { self.read_ticks().as_seconds_f64() }
    pub(crate) fn write_seconds(&self, value: f64) {
        let ticks_per_second = Tick::from_timetable_time(TimetableTime(1)).0 as f64;
        let ticks = (value * ticks_per_second).round() as i64;
        self.write_ticks(Tick(ticks));
    }
    pub(crate) fn is_locked(&self) -> bool { self.locker.load(Ordering::Acquire) != Self::UNLOCKED }
    pub(crate) fn lock(&self) -> bool {
        self.locker.compare_exchange(Self::UNLOCKED, 1, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }
    pub(crate) fn unlock(&self) {
        let _ = self.locker.compare_exchange(1, Self::UNLOCKED, Ordering::Release, Ordering::Relaxed);
    }
}

fn update_timer(timer: &mut GlobalTimer, dt_seconds: f64) {
    if !timer.is_locked() && timer.sync_to_real_time {
        let now = Local::now();
        let seconds = now.num_seconds_from_midnight() as f64;
        let rest = now.nanosecond() as f64 / 1_000_000_000.0;
        timer.animation_speed = 1.0;
        timer.animation_playing = true;
        timer.write_seconds(seconds + rest);
    } else if timer.animation_playing && !timer.is_locked() {
        let mut seconds = timer.read_seconds();
        seconds += timer.animation_speed * dt_seconds;
        timer.write_seconds(seconds);
    }
}

macro_rules! for_all_tabs {
    ($tab:expr, $t:ident, $body:expr) => {
        match $tab {
            MainTab::Start($t) => $body,
            MainTab::Diagram($t) => $body,
            MainTab::Settings($t) => $body,
            MainTab::Classes($t) => $body,
            MainTab::Graph($t) => $body,
            MainTab::Trip($t) => $body,
            MainTab::RouteTimetable($t) => $body,
            MainTab::PriorityGraph($t) => $body,
            MainTab::Text($t) => $body,
            MainTab::Station($t) => $body,
        }
    };
}

macro_rules! for_all_tab_types {
    ($tab:expr, $body:ident) => {
        match $tab {
            MainTab::Start(_) => StartTab::$body,
            MainTab::Diagram(_) => DiagramTab::$body,
            MainTab::Settings(_) => SettingsTab::$body,
            MainTab::Classes(_) => ClassesTab::$body,
            MainTab::Graph(_) => GraphTab::$body,
            MainTab::Trip(_) => TripTab::$body,
            MainTab::RouteTimetable(_) => RouteTimetableTab::$body,
            MainTab::PriorityGraph(_) => PriorityGraphTab::$body,
            MainTab::Text(_) => TextTab::$body,
            MainTab::Station(_) => StationTab::$body,
        }
    };
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) enum MainTab {
    Start(StartTab),
    Diagram(DiagramTab),
    Settings(SettingsTab),
    Classes(ClassesTab),
    Graph(GraphTab),
    Trip(TripTab),
    RouteTimetable(RouteTimetableTab),
    PriorityGraph(PriorityGraphTab),
    Text(TextTab),
    Station(StationTab),
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct MainUiState {
    pub tree: Tree<MainTab>,
    pub maximized: Option<TileId>,
}

impl MainUiState {
    pub(crate) fn push_to_focused_leaf(&mut self, new_pane: MainTab) -> TileId {
        let new_id = self.tree.tiles.insert_pane(new_pane);
        let mut activated = false;
        if let Some(root_id) = self.tree.root {
            if let Some(Tile::Container(container)) = self.tree.tiles.get_mut(root_id) {
                container.add_child(new_id);
                if let egui_tiles::Container::Tabs(tabs) = container {
                    tabs.active = Some(new_id);
                    activated = true;
                }
            }
        }
        if !activated {
            let tabs_id = self.tree.tiles.insert_tab_tile(vec![new_id]);
            self.tree.root = Some(tabs_id);
        }
        new_id
    }
}

impl Default for MainUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs("main", vec![MainTab::Start(StartTab::default())]),
            maximized: None,
        }
    }
}



#[derive(Serialize, Deserialize, Clone, Copy)]
enum AdditionalTab {
    Edit,
    Properties,
    Export,
}

#[derive(Serialize, Deserialize, Clone)]
struct AdditionalUiState {
    tree: Tree<AdditionalTab>,
    focused_id: Option<TileId>,
    expanded: bool,
}

impl Default for AdditionalUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs(
                "additional",
                vec![
                    AdditionalTab::Edit,
                    AdditionalTab::Properties,
                    AdditionalTab::Export,
                ],
            ),
            focused_id: None,
            expanded: true,
        }
    }
}

struct MainTabViewer {
    app: *mut AppState,
    last_focused_id: *mut Option<TileId>,
    last_maximized_id: *mut Option<TileId>,
}
unsafe impl Send for MainTabViewer {}
unsafe impl Sync for MainTabViewer {}

impl MainTabViewer {
    fn app(&mut self) -> &mut AppState {
        unsafe { &mut *self.app }
    }
    fn last_focused(&mut self) -> &mut Option<TileId> {
        unsafe { &mut *self.last_focused_id }
    }
    fn last_maximized(&mut self) -> &mut Option<TileId> {
        unsafe { &mut *self.last_maximized_id }
    }
    fn add_popup(&mut self, ui: &mut Ui) {
        let tab_definitions: [(&str, _); 4] = [
            (&tr!("tab-start"), MainTab::Start(StartTab::default())),
            (&tr!("tab-settings"), MainTab::Settings(SettingsTab)),
            (&tr!("tab-classes"), MainTab::Classes(ClassesTab::default())),
            (&tr!("tab-graph"), MainTab::Graph(GraphTab::default())),
        ];
        for (s, t) in tab_definitions {
            if ui.button(s).clicked() {
                self.app().pending_tabs.push_back(PendingTabOp::Open(t));
                ui.close();
            }
        }
        ui.menu_button(tr!("menu-route-timetable"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (rk, name) in self.app().source.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self.open_tab_or_close(ui, MainTab::RouteTimetable(RouteTimetableTab::new(rk)));
                    }
                }
            });
        });
        ui.menu_button(tr!("menu-priority-graph"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (rk, name) in self.app().source.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self.open_tab_or_close(ui, MainTab::PriorityGraph(PriorityGraphTab::new(rk)));
                    }
                }
            });
        });
        ui.menu_button(tr!("menu-diagrams"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (rk, name) in self.app().source.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self.open_tab_or_close(ui, MainTab::Diagram(DiagramTab::new(rk)));
                    }
                }
            });
        });
        ui.menu_button(tr!("menu-trips"), |ui| {
            if ui.button(tr!("menu-new-trip")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (tk, name) in self.app().source.trips_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self.open_tab_or_close(ui, MainTab::Trip(TripTab::new(tk)));
                    }
                }
            });
        });
        ui.menu_button(tr!("menu-text"), |ui| {
            if ui.button(tr!("menu-new-text-message")).clicked() {
                // TODO: implement text messages
            }
            ui.separator();
            if ui.button(tr!("menu-project-remarks")).clicked() {
                self.open_tab_or_close(ui, MainTab::Text(TextTab::new(None)));
            }
        });
    }

    fn open_tab_or_close(&mut self, ui: &mut Ui, tab: MainTab) {
        self.app().pending_tabs.push_back(PendingTabOp::Open(tab));
        ui.close();
    }
}

impl Behavior<MainTab> for MainTabViewer {
    fn tab_title_for_pane(&mut self, pane: &MainTab) -> egui::WidgetText {
        for_all_tabs!(pane, p, p.title())
    }
    fn on_tab_button(
        &mut self,
        _tiles: &mut Tiles<MainTab>,
        tile_id: TileId,
        button_response: Response,
    ) -> Response {
        if button_response.clicked() || button_response.dragged() {
            *self.last_focused() = Some(tile_id);
        }
        button_response
    }
    fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, tab: &mut MainTab) -> UiResponse {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        for_all_tabs!(tab, t, t.main_display(self.app(), ui));
        if let Some(pos) = ui.input(|i| i.pointer.press_origin())
            && ui.clip_rect().shrink(10.0).contains(pos)
        {
            *self.last_focused() = Some(tile_id)
        }
        Default::default()
    }
    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: false,
            prune_single_child_containers: true,
            all_panes_must_have_tabs: true,
            join_nested_linear_containers: true,
        }
    }
    fn is_tab_closable(&self, tiles: &Tiles<MainTab>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            None => false,
            Some(Tile::Container(_)) => false,
            Some(Tile::Pane(MainTab::Start(_))) => false,
            Some(Tile::Pane(_)) => true,
        }
    }
    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<MainTab>,
        ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if ui.button("M").clicked() {
            *self.last_maximized() = *self.last_focused();
        }
        let res = ui.button("+");
        egui::Popup::menu(&res).show(|ui| {
            self.add_popup(ui);
        });
    }
    fn tab_bg_color(
        &self,
        visuals: &egui::Visuals,
        tiles: &Tiles<MainTab>,
        tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> Color32 {
        let base = match tiles.get(tile_id) {
            None | Some(Tile::Container(_)) => visuals.panel_fill,
            Some(Tile::Pane(tab)) => {
                DisplayedColor::from_seed(for_all_tab_types!(tab, NAME)).get(visuals.dark_mode)
            }
        };
        base.gamma_multiply(if state.active { 0.7 } else { 0.2 })
    }
    fn tab_outline_stroke(
        &self,
        visuals: &egui::Visuals,
        tiles: &Tiles<MainTab>,
        tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> Stroke {
        let base = match tiles.get(tile_id) {
            None | Some(Tile::Container(_)) => visuals.panel_fill,
            Some(Tile::Pane(tab)) => {
                DisplayedColor::from_seed(for_all_tab_types!(tab, NAME)).get(visuals.dark_mode)
            }
        };
        Stroke::new(1.0, base.gamma_multiply(if state.active { 1.0 } else { 0.7 }))
    }
    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> Stroke {
        Stroke::new(1.0, Color32::TRANSPARENT)
    }
}

struct AdditionalTabViewer {
    focused: Option<*mut MainTab>,
    app_ptr: *mut AppState,
}
unsafe impl Send for AdditionalTabViewer {}
unsafe impl Sync for AdditionalTabViewer {}

impl Behavior<AdditionalTab> for AdditionalTabViewer {
    fn tab_title_for_pane(&mut self, tab: &AdditionalTab) -> egui::WidgetText {
        match *tab {
            AdditionalTab::Edit => tr!("side-panel-edit"),
            AdditionalTab::Properties => tr!("side-panel-details"),
            AdditionalTab::Export => tr!("side-panel-export"),
        }
        .into()
    }
    fn pane_ui(
        &mut self,
        ui: &mut Ui,
        _tile_id: egui_tiles::TileId,
        tab: &mut AdditionalTab,
    ) -> egui_tiles::UiResponse {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
            let Some(focused_ptr) = self.focused else {
                ui.label(tr!("menu-nothing-focused"));
                return;
            };
            let focused = unsafe { &mut *focused_ptr };
            let app = unsafe { &mut *self.app_ptr };
            match *tab {
                AdditionalTab::Edit => for_all_tabs!(focused, t, t.edit_display(app, ui)),
                AdditionalTab::Properties => for_all_tabs!(focused, t, t.display_display(app, ui)),
                AdditionalTab::Export => for_all_tabs!(focused, t, t.export_display(app, ui)),
            }
        });
        Default::default()
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


fn process_pending_tabs(app: &mut AppState) {
    use egui_tiles::{Container, Tile};
    // Handle loaded scene from file thread
    if let Some(bytes) = crate::save::take_loaded_file() {
        let result = crate::save::apply_loaded_scene(app, &bytes);
        if let Err(e) = result {
            app.load_error = Some(format!("Load failed: {}", e));
        }
    }
    if let Some(bytes) = app.loaded_scene.take() {
        let result = crate::save::apply_loaded_scene(app, &bytes);
        if let Err(e) = result {
            app.load_error = Some(format!("Load failed: {}", e));
        }
    }
    while let Some(op) = app.pending_tabs.pop_front() {
        match op {
            PendingTabOp::Open(tab) => {
                if let Some(id) = app.main_ui.tree.tiles.find_pane(&tab) {
                    app.main_ui.tree.set_visible(id, true);
                    app.additional_ui.focused_id = Some(id);
                    continue;
                }
                let new_id = app.main_ui.tree.tiles.insert_pane(tab);
                let root_id = app.main_ui.tree.root;
                if let Some(root_id) = root_id {
                    if let Some(Tile::Container(Container::Tabs(tabs))) = app.main_ui.tree.tiles.get_mut(root_id) {
                        tabs.children.push(new_id);
                        tabs.active = Some(new_id);
                        app.additional_ui.focused_id = Some(new_id);
                        continue;
                    }
                }
                let tabs_id = app.main_ui.tree.tiles.insert_tab_tile(vec![new_id]);
                app.main_ui.tree.root = Some(tabs_id);
                app.additional_ui.focused_id = Some(new_id);
            }
        }
    }
}

pub fn show_ui(ui: &mut Ui, app: &mut AppState, cpu_time: Option<f32>) {
    process_pending_tabs(app);
    // sync theme
    if app.preferences.dark_mode {
        ui.ctx().set_theme(egui::Theme::Dark);
    } else {
        ui.ctx().set_theme(egui::Theme::Light);
    }

    // update timer
    let dt = ui.input(|r| r.stable_dt);
    update_timer(&mut app.timer, dt as f64);

    // Modal
    {
        let modal_open = app.modal.0.is_some();
        if modal_open {
            let r = egui::Modal::new(egui::Id::new("modal")).show(ui.ctx(), |ui| {
                ui.label("Modal content");
            });
            if r.should_close() {
                app.modal.0 = None;
            }
        }
    }

    // Show load error if any
    if let Some(err) = &app.load_error {
        let err_str = err.clone();
        let r = egui::Modal::new(egui::Id::new("load_error")).show(ui.ctx(), |ui| {
            ui.heading("Load Error");
            ui.label(&err_str);
            if ui.button("OK").clicked() {
                app.load_error = None;
            }
        });
        if r.should_close() {
            app.load_error = None;
        }
    }

    // Command palette
    if ui.input_mut(|r| r.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::P))) {
        // TODO: commit palette toggle
    }

    Panel::top("top panel")
        .exact_size(32.0)
        .show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let res = ui.button("More...");
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Fullscreen").clicked() {
                    let is_fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
                }
                #[cfg(target_arch = "wasm32")]
                if ui.button("Fullscreen").clicked() {
                    toggle_fullscreen("paiagram_canvas");
                }
                egui::Popup::menu(&res).show(|ui| {
                    if ui.button("Import from URL...").clicked() {
                        app.modal.0 = Some(Modals::OpenUrl(String::new()));
                    }
                    ui.separator();
                    let mut read_file = |name: &str, extensions: &[&str], _handle: fn(Vec<u8>)| {
                        if ui.button(tr!("read-file-prompt", {name: name})).clicked() {
                            // TODO: implement file reading
                        }
                    };
                    read_file("OuDia", &["oud"], |_| {});
                    read_file("OuDiaSecond", &["oud2"], |_| {});
                    read_file("qETRC/pyETRC", &["pyetgr", "json"], |_| {});
                    read_file("GTFS", &["zip"], |_| {});
                    read_file("LLT", &["json"], |_| {});
                    ui.separator();
                    if ui.button("Save...").clicked() {
                        save::save(app, "save.paia".to_string());
                    }
                    if ui.button("Read...").clicked() {
                        save::spawn_load_thread();
                    }
                    if app.preferences.developer_mode {
                        if ui.button("Save RON...").clicked() {
                            save::save_ron(app, "saved.ron".to_string());
                        }
                    }
                });
                let res = ui.button(tr!("menu-about"));
                egui::Popup::menu(&res).show(|ui| {
                    if ui.button(tr!("menu-documentation")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab(if cfg!(target_arch = "wasm32") {
                            "/docs"
                        } else {
                            "https://paiagram.com/docs"
                        }));
                    }
                    if cfg!(target_arch = "wasm32") && ui.button(tr!("menu-legal")).clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("./license.html"));
                    }
                });
                if app.preferences.developer_mode {
                    app.frame_time_history.push(ui.input(|r| r.stable_dt));
                    let average_dt = app.frame_time_history.average_dt();
                    ui.monospace(format!("FPS: {:6.2}", 1.0_f32 / average_dt));
                    ui.monospace(format!("FRAME: {:5.2}ms", average_dt * 1000.0_f32));
                    ui.monospace(format!("CPU: {:5.2}ms", cpu_time.unwrap_or(0.0) * 1000.0_f32));
                    ui.horizontal(|ui| {
                        const GAP: f32 = 4.0;
                        const SAMPLE_COUNT: usize = 32;
                        let stroke = Stroke {
                            color: PredefinedColor::Blue.get(ui.visuals().dark_mode),
                            width: 3.0,
                        };
                        let max = app.frame_time_history.previous_n(SAMPLE_COUNT)
                            .fold(0.0_f32, f32::max)
                            .max(f32::EPSILON);
                        let graph_width = SAMPLE_COUNT as f32 * (stroke.width + GAP) - GAP;
                        let graph_height = ui.available_height();
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(graph_width, graph_height),
                            egui::Sense::hover(),
                        );
                        for (idx, f) in app.frame_time_history.previous_n(SAMPLE_COUNT).enumerate() {
                            let height = rect.height() * (f / max).clamp(0.0, 1.0);
                            let x = rect.right()
                                - idx as f32 * (stroke.width + GAP)
                                - stroke.width * 0.5;
                            let points = [
                                egui::pos2(x, rect.bottom()),
                                egui::pos2(x, rect.bottom() - height),
                            ];
                            ui.painter().line_segment(points, stroke);
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(app.source.undoable(), egui::Button::new(tr!("menu-undo"))).clicked() {
                        app.source.undo();
                    }
                    if ui.add_enabled(app.source.redoable(), egui::Button::new(tr!("menu-redo"))).clicked() {
                        app.source.redo();
                    }
                    ui.checkbox(&mut app.additional_ui.expanded, "");
                });
            })
        });

    Panel::bottom("bottom panel")
        .exact_size(24.0)
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let ticks_in_cycle = app.project_settings.repeat_frequency.to_ticks();
                let mut time = app.timer.read_ticks().to_timetable_time();
                ui.add_enabled(
                    !app.timer.sync_to_real_time,
                    egui::Checkbox::new(&mut app.timer.animation_playing, ""),
                );
                let time_response = ui.add(TimeDragValue(&mut time));
                ui.add_enabled(
                    !app.timer.sync_to_real_time,
                    egui::DragValue::new(&mut app.timer.animation_speed)
                        .fixed_decimals(1)
                        .suffix("×"),
                );
                egui::Popup::menu(&time_response).show(|ui| {
                    ui.checkbox(&mut app.timer.sync_to_real_time, tr!("menu-sync-system-clock"));
                });
                if !app.timer.sync_to_real_time && time_response.dragged() && app.timer.lock() {
                    app.timer.write_ticks(Tick::from_timetable_time(time));
                } else {
                    app.timer.unlock();
                }
                if app.timer.animation_playing {
                    ui.ctx().request_repaint();
                }

                let (_id, rect) = ui.allocate_space(ui.available_size());
                let progress_stroke = ui.visuals().window_stroke();
                ui.painter().hline(rect.x_range(), rect.center().y, progress_stroke);
                let amount_of_ticks = 24;
                for i in 0..(amount_of_ticks + 1) {
                    let progress = (1.0 / amount_of_ticks as f32) * i as f32;
                    let x = egui::emath::lerp(rect.left()..=rect.right(), progress);
                    let y_range = if i % 4 == 0 { rect.y_range() } else { rect.y_range().shrink(5.0) };
                    ui.painter().vline(x, y_range, progress_stroke);
                }
                let indicator_stroke = Stroke::new(1.5, Color32::RED);
                let progress = app.timer.read_ticks().normalized_with(ticks_in_cycle);
                let progress = progress.0 as f32 / ticks_in_cycle.0 as f32;
                ui.painter().vline(
                    egui::emath::lerp(rect.left()..=rect.right(), progress),
                    rect.y_range(),
                    indicator_stroke,
                );
            })
        });

    // Right panel (must be before CentralPanel)
    let expanded = app.additional_ui.expanded;
    let focused_slot = &mut app.additional_ui.focused_id as *mut Option<TileId>;
    let mut maximized = app.main_ui.maximized;
    {
        let additional_tiles = &mut app.additional_ui.tree as *mut Tree<AdditionalTab>;
        Panel::right("right panel")
            .frame(Frame::default())
            .show_animated_inside(ui, expanded, |ui| {
                let fid = unsafe { &*focused_slot };
                let focused_tab = fid.and_then(|fid| {
                    app.main_ui.tree.tiles.get_mut(fid).and_then(|t| {
                        if let Tile::Pane(pane) = t { Some(pane as *mut MainTab) } else { None }
                    })
                });
                let mut additional_viewer = AdditionalTabViewer {
                    focused: focused_tab,
                    app_ptr: app as *mut AppState,
                };
                let tree = unsafe { &mut *additional_tiles };
                tree.ui(&mut additional_viewer, ui);
            });
    }

    // Central panel
    egui::CentralPanel::default()
        .frame(Frame::default())
        .show_inside(ui, |ui| {
            if let Some(max_id) = app.main_ui.maximized {
                if let Some(Tile::Pane(pane)) = app.main_ui.tree.tiles.get_mut(max_id) {
                    let title = for_all_tabs!(pane, t, t.title());
                    drop(pane);
                    let mut tv = MainTabViewer {
                        app: app as *mut AppState,
                        last_focused_id: std::ptr::null_mut(),
                        last_maximized_id: &mut maximized as *mut Option<TileId>,
                    };
                    Panel::top("maximized_top")
                        .exact_size(24.0)
                        .show_inside(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(title);
                                ui.label(RichText::new(tr!("menu-maximized-view")).italics());
                                if ui.button("x").clicked() { maximized = None; }
                            });
                        });
                    if let Some(Tile::Pane(pane)) = app.main_ui.tree.tiles.get_mut(max_id) {
                        let _ = tv.pane_ui(ui, max_id, pane);
                    }
                    return;
                }
            }
            let mut tv = MainTabViewer {
                app: app as *mut AppState,
                last_focused_id: focused_slot,
                last_maximized_id: &mut maximized as *mut Option<TileId>,
            };
            app.main_ui.tree.ui(&mut tv, ui);
        });
    app.main_ui.maximized = maximized;
}

pub fn apply_custom_fonts(ctx: &Context) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let sarasa = load_sarasa_local();
        ctx.set_fonts(build_font_definitions(sarasa));
    }
    #[cfg(target_arch = "wasm32")]
    {
        ctx.set_fonts(build_font_definitions(None));
        download_sarasa_and_apply(ctx.clone());
    }
}

fn build_font_definitions(sarasa: Option<Vec<u8>>) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let has_sarasa = sarasa.is_some();
    if let Some(bytes) = sarasa {
        fonts.font_data.insert(
            "my_font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
    }
    if has_sarasa {
        fonts.families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "my_font".to_owned());
    }
    let mut dia_pro_family = vec!["dia_pro".to_owned()];
    if has_sarasa { dia_pro_family.push("my_font".to_owned()); }
    fonts.families.insert(egui::FontFamily::Name("dia_pro".into()), dia_pro_family);
    fonts
}

#[cfg(not(target_arch = "wasm32"))]
fn load_sarasa_local() -> Option<Vec<u8>> {
    let mut candidates = vec![
        PathBuf::from("assets/fonts/SarasaUiSC-Regular.ttf"),
        PathBuf::from("crates/paiagram-ui/assets/fonts/SarasaUiSC-Regular.ttf"),
    ];
    if let Ok(exe) = std::env::current_exe() && let Some(parent) = exe.parent() {
        candidates.push(parent.join("assets/fonts/SarasaUiSC-Regular.ttf"));
    }
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) { return Some(bytes); }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn download_sarasa_and_apply(ctx: Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(window) = eframe::web_sys::window() else { return; };
        let Ok(response) = wasm_bindgen_futures::JsFuture::from(
            window.fetch_with_str("SarasaUiSC-Regular.ttf"),
        ).await else { return; };
        let Ok(response) = response.dyn_into::<eframe::web_sys::Response>() else { return; };
        if !response.ok() { return; }
        let Ok(array_buffer) = wasm_bindgen_futures::JsFuture::from(
            response.array_buffer().unwrap(),
        ).await else { return; };
        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        ctx.set_fonts(build_font_definitions(Some(bytes)));
    });
}
