//! Definitions for the user interface.

mod settings;
mod tabs;
mod widgets;

/// Initialize the i18n system: load embedded translation bundles and set the
/// default language. Must be called once before any `tr!()` macro is used.
pub fn init_i18n(locale: Option<&str>) {
    // FTL files are embedded at compile time so the approach works on all
    // targets (native, wasm) without runtime filesystem dependencies.
    let en_ca = include_str!("../assets/locales/en-CA.ftl");
    let zh_hans = include_str!("../assets/locales/zh-Hans.ftl");

    let _ = egui_i18n::load_translations_from_text("en-CA", en_ca);
    let _ = egui_i18n::load_translations_from_text("zh-Hans", zh_hans);

    egui_i18n::set_fallback("en-CA");
    egui_i18n::set_language(locale.unwrap_or("en-CA"));
}

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
use paiagram_core::colors::{DisplayedColor, PredefinedColor};
use paiagram_core::settings::{ProjectSettings, UserPreferences};
use paiagram_core::units::time::{Tick, TimetableTime};
use paiagram_core::{LonLat, Source, StationKey, TripKey, WorldSnapshot};
use serde::{Deserialize, Serialize};
use tabs::Tab;
use tabs::all_tabs::*;
use vec1::{Vec1, vec1};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::widgets::TimeDragValue;

/// The main application struct, holding all state.
pub struct App {
    pub source: Source,
    main_ui: MainUiState,
    additional_ui: AdditionalUiState,
    timer: GlobalTimer,
    frame_time: FrameTimeHistory,
    pub preferences: UserPreferences,
    pub project_settings: ProjectSettings,
    theme_changed: bool,
    pending_open_tabs: Vec<MainTab>,
    pub selected_items: SelectedItems,
    pub pending_import:
        std::sync::Arc<std::sync::Mutex<Vec<paiagram_core::Command>>>,
}

impl std::ops::Deref for App {
    type Target = Source;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl std::ops::DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.source
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            source: Source::default(),
            main_ui: MainUiState::default(),
            additional_ui: AdditionalUiState::default(),
            timer: GlobalTimer::default(),
            frame_time: FrameTimeHistory::default(),
            preferences: UserPreferences::default(),
            project_settings: ProjectSettings::default(),
            theme_changed: true,
            pending_open_tabs: Vec::new(),
            pending_import: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            selected_items: SelectedItems::None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TripSelection {
    pub(crate) trip: TripKey,
}

impl PartialEq for TripSelection {
    fn eq(&self, other: &Self) -> bool {
        self.trip == other.trip
    }
}

#[derive(Clone, Copy, PartialEq, Hash, Debug, Eq, PartialOrd, Ord)]
pub(crate) struct StationPairSelection {
    pub(crate) source: StationKey,
    pub(crate) target: StationKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StationSelection {
    pub(crate) station: StationKey,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingRouteSelection {
    pub(crate) prev_station: StationKey,
}

// Extending or creating a new trip
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingTripSelection {
    // The current focused trip
    pub(crate) trip: TripKey,
    // previous position on the canvas
    pub(crate) previous_pos: Option<(TimetableTime, usize)>,
    pub(crate) last_time: Option<TimetableTime>,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct CoordinateSelection {
    pub(crate) coor: LonLat,
    pub(crate) name_candidate: String,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SelectedItems {
    None,
    Trips(Vec1<TripSelection>),
    StationPairs(Vec1<StationPairSelection>),
    Stations(Vec1<StationSelection>),
    ExtendingRoute(ExtendingRouteSelection),
    ExtendingTrip(ExtendingTripSelection),
    Coordinate(CoordinateSelection),
}

#[derive(Clone)]
pub(crate) enum ModifySelectedItems {
    Toggle(SelectedItem),
    SetSingle(SelectedItem),
    Clear,
}

#[derive(Clone, Default)]
pub(crate) enum SelectedItem {
    #[default]
    None,
    Trip(TripSelection),
    StationPair(StationPairSelection),
    Station(StationSelection),
    Coordinate(CoordinateSelection),
}

impl Default for SelectedItems {
    fn default() -> Self {
        Self::None
    }
}


struct FrameTimeHistory {
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

impl GlobalTimer {
    const UNLOCKED: u64 = u64::MAX;

    fn update(&mut self) {
        if !self.is_locked() && self.sync_to_real_time {
            let now = Local::now();
            let seconds = now.num_seconds_from_midnight() as f64;
            let rest = now.nanosecond() as f64 / 1_000_000_000 as f64;
            self.animation_speed = 1.0;
            self.animation_playing = true;
            self.write_seconds(seconds + rest);
        } else if self.animation_playing && !self.is_locked() {
            let seconds = self.read_seconds();
            self.write_seconds(seconds);
        }
    }

    pub(crate) fn read_ticks(&self) -> Tick {
        Tick(self.value.load(Ordering::Acquire))
    }

    pub(crate) fn write_ticks(&self, value: Tick) {
        self.value.store(value.0, Ordering::Release);
    }

    pub(crate) fn read_seconds(&self) -> f64 {
        self.read_ticks().as_seconds_f64()
    }

    pub(crate) fn write_seconds(&self, value: f64) {
        let ticks_per_second = Tick::from_timetable_time(TimetableTime(1)).0 as f64;
        let ticks = (value * ticks_per_second).round() as i64;
        self.write_ticks(Tick(ticks));
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.locker.load(Ordering::Acquire) != Self::UNLOCKED
    }

    pub(crate) fn try_lock(&self, id: u64) -> bool {
        let result = self.locker.compare_exchange(
            Self::UNLOCKED,
            id,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        result.is_ok() || result.unwrap_err() == id
    }

    pub(crate) fn try_unlock(&self, id: u64) {
        let _ = self.locker.compare_exchange(
            id,
            Self::UNLOCKED,
            Ordering::Release,
            Ordering::Relaxed,
        );
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
    tree: Tree<MainTab>,
    maximized: Option<TileId>,
}

impl MainUiState {
    pub(crate) fn push_to_focused_leaf(&mut self, new_pane: MainTab) -> TileId {
        let new_id = self.tree.tiles.insert_pane(new_pane);

        // Try to add it to the same Tabs container that is currently focused
        if let Some(&active_id) = self.tree.active_tiles().last()
            && let Some(parent_id) = self.tree.tiles.parent_of(active_id)
            && let Some(Tile::Container(container)) = self.tree.tiles.get_mut(parent_id)
            && container.kind() == ContainerKind::Tabs
        {
            container.add_child(new_id);
            self.tree.make_active(|id, _| id == new_id);
            return new_id;
        }

        // Fallback: create a new top-level Tabs container
        let old_root = self.tree.root;
        let tabs_id = if let Some(old_root) = old_root {
            self.tree.tiles.insert_tab_tile(vec![old_root, new_id])
        } else {
            self.tree.tiles.insert_tab_tile(vec![new_id])
        };
        self.tree.root = Some(tabs_id);
        self.tree.make_active(|id, _| id == new_id);
        new_id
    }

    fn open_or_focus(&mut self, pane: MainTab) -> TileId {
        if let Some(tile_id) = self.tree.tiles.find_pane(&pane) {
            self.tree.make_active(|id, _| id == tile_id);
            self.tree.set_visible(tile_id, true);
            tile_id
        } else {
            self.push_to_focused_leaf(pane)
        }
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

struct MainTabViewer<'a> {
    app: &'a mut App,
    last_focused_id: &'a mut Option<TileId>,
    last_maximized_id: &'a mut Option<TileId>,
}

impl<'a> MainTabViewer<'a> {
    fn add_popup(&mut self, ui: &mut Ui) {
        let tab_definitions: [(&str, _); 4] = [
            (&tr!("tab-start"), MainTab::Start(StartTab::default())),
            (&tr!("tab-settings"), MainTab::Settings(SettingsTab)),
            (&tr!("tab-classes"), MainTab::Classes(ClassesTab::default())),
            (&tr!("tab-graph"), MainTab::Graph(GraphTab::default())),
        ];
        for (s, t) in tab_definitions {
            if ui.button(s).clicked() {
                self.app.open_or_focus(t);
                ui.close();
            }
        }

        ui.menu_button(tr!("menu-route-timetable"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (key, name) in self.app.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self
                            .app
                            .open_or_focus(MainTab::RouteTimetable(RouteTimetableTab::new(key)));
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button(tr!("menu-priority-graph"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (key, name) in self.app.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self
                            .app
                            .open_or_focus(MainTab::PriorityGraph(PriorityGraphTab::new(key)));
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button(tr!("menu-text"), |ui| {
            if ui.button(tr!("menu-project-remarks")).clicked() {
                self
                    .app
                    .open_or_focus(MainTab::Text(TextTab::default()));
            }
        });

        ui.menu_button(tr!("menu-diagrams"), |ui| {
            if ui.button(tr!("menu-new-route")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (key, name) in self.app.routes_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self
                            .app
                            .open_or_focus(MainTab::Diagram(DiagramTab::new(key)));
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button(tr!("menu-trips"), |ui| {
            if ui.button(tr!("menu-new-trip")).clicked() {}
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| {
                for (key, name) in self.app.trips_iter() {
                    if ui.button(name.as_str()).clicked() {
                        self
                            .app
                            .open_or_focus(MainTab::Trip(TripTab::new(key)));
                        ui.close();
                    }
                }
            });
        });
    }
}

impl<'a> Behavior<MainTab> for MainTabViewer<'a> {
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
            *self.last_focused_id = Some(tile_id);
        }
        button_response
    }

    fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, tab: &mut MainTab) -> UiResponse {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        for_all_tabs!(tab, t, t.main_display(self.app, ui));
        if let Some(pos) = ui.input(|i| i.pointer.press_origin())
            && ui.clip_rect().shrink(10.0).contains(pos)
        {
            *self.last_focused_id = Some(tile_id)
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
        // maximize
        if ui.button("M").clicked() {
            *self.last_maximized_id = *self.last_focused_id;
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
                DisplayedColor::from_seed(for_all_tab_types!(tab, NAME)).into_color32(visuals.dark_mode)
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
                DisplayedColor::from_seed(for_all_tab_types!(tab, NAME)).into_color32(visuals.dark_mode)
            }
        };
        Stroke::new(
            1.0,
            base.gamma_multiply(if state.active { 1.0 } else { 0.7 }),
        )
    }

    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> Stroke {
        Stroke::new(1.0, Color32::TRANSPARENT)
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

struct AdditionalTabViewer<'a> {
    app: &'a mut App,
    focused_tab: Option<&'a mut MainTab>,
}

impl<'a> egui_tiles::Behavior<AdditionalTab> for AdditionalTabViewer<'a> {
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
            let Some(ref mut focused) = self.focused_tab else {
                ui.label(tr!("menu-nothing-focused"));
                return;
            };
            match *tab {
                AdditionalTab::Edit => {
                    for_all_tabs!(focused, t, t.edit_display(self.app, ui));
                }
                AdditionalTab::Properties => {
                    for_all_tabs!(focused, t, t.display_display(self.app, ui));
                }
                AdditionalTab::Export => {
                    for_all_tabs!(focused, t, t.export_display(self.app, ui));
                }
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

// File dialog & save/load temporarily disabled.
// Dependencies (rfd, futures-lite, cbor4ii, ron) need explicit addition first.

impl App {
    /// Get a clone of the current world snapshot.
    fn snapshot_clone(&self) -> WorldSnapshot {
        WorldSnapshot::clone(&**self)
    }

    pub fn open_or_focus(&mut self, tab: MainTab) {
        self.pending_open_tabs.push(tab);
    }

    pub fn modify_selected_items(&mut self, action: ModifySelectedItems) {
        match action {
            ModifySelectedItems::SetSingle(item) => {
                self.selected_items = match item {
                    SelectedItem::None => SelectedItems::None,
                    SelectedItem::Trip(t) => SelectedItems::Trips(vec1![t]),
                    SelectedItem::StationPair(p) => SelectedItems::StationPairs(vec1![p]),
                    SelectedItem::Station(s) => SelectedItems::Stations(vec1![s]),
                    SelectedItem::Coordinate(c) => SelectedItems::Coordinate(c),
                };
            }
            ModifySelectedItems::Toggle(item) => {
                match (&mut self.selected_items, item) {
                    (SelectedItems::Trips(list), SelectedItem::Trip(t)) => {
                        if let Some(pos) = list.iter().position(|x| *x == t) {
                            list.remove(pos);
                        } else {
                            list.push(t);
                        }
                    }
                    (SelectedItems::Stations(list), SelectedItem::Station(s)) => {
                        if let Some(pos) = list.iter().position(|x| *x == s) {
                            list.remove(pos);
                        } else {
                            list.push(s);
                        }
                    }
                    (SelectedItems::StationPairs(list), SelectedItem::StationPair(p)) => {
                        if let Some(pos) = list.iter().position(|x| *x == p) {
                            list.remove(pos);
                        } else {
                            list.push(p);
                        }
                    }
                    _ => {}
                }
            }
            ModifySelectedItems::Clear => {
                self.selected_items = SelectedItems::None;
            }
        }
    }

    fn process_pending_tabs(&mut self) {
        let tabs = std::mem::take(&mut self.pending_open_tabs);
        for tab in tabs {
            let focused_id = self.main_ui.open_or_focus(tab);
            self.additional_ui.focused_id = Some(focused_id);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_file(
        &self,
        dialog: rfd::AsyncFileDialog,
        parse: fn(Vec<u8>) -> Result<Vec<paiagram_core::Command>, String>,
    ) {
        // Use an AtomicBool to signal that commands are ready
        let pending = self.pending_import.clone();
        std::thread::spawn(move || {
            let Some(file) = futures_lite::future::block_on(dialog.pick_file()) else {
                return;
            };
            let content = futures_lite::future::block_on(file.read());
            match parse(content) {
                Ok(cmds) => {
                    let mut queue = pending.lock().unwrap();
                    for cmd in cmds {
                        queue.push(cmd);
                    }
                }
                Err(e) => {
                    log::error!("Import error: {e}");
                }
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save(&self) {
        let snapshot = WorldSnapshot::clone(&**self);
        paiagram_rw::save::serialize_compressed_cbor(snapshot, "save.paia".to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_ron(&self) {
        let snapshot = WorldSnapshot::clone(&**self);
        paiagram_rw::save::serialize_ron(snapshot, "saved.ron".to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_file(&self, dialog: rfd::AsyncFileDialog) {
        let pending = self.pending_import.clone();
        std::thread::spawn(move || {
            let Some(file) = futures_lite::future::block_on(dialog.pick_file()) else {
                return;
            };
            let content = futures_lite::future::block_on(file.read());
            // Decompress lz4 frame then deserialize CBOR
            use std::io::Read;
            let mut decoder = lz4_flex::frame::FrameDecoder::new(&content[..]);
            let mut decompressed = Vec::new();
            let snapshot = decoder
                .read_to_end(&mut decompressed)
                .ok()
                .and_then(|_| cbor4ii::serde::from_slice::<paiagram_core::WorldSnapshot>(&decompressed).ok());
            if let Some(snapshot) = snapshot {
                let mut queue = pending.lock().unwrap();
                queue.push(paiagram_core::Command::LoadWorld {
                    snapshot: Box::new(snapshot),
                });
            } else {
                log::error!("Failed to load save: corrupt or invalid format");
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_ron_file(&self, dialog: rfd::AsyncFileDialog) {
        let pending = self.pending_import.clone();
        std::thread::spawn(move || {
            let Some(file) = futures_lite::future::block_on(dialog.pick_file()) else {
                return;
            };
            let content = futures_lite::future::block_on(file.read());
            let s = String::from_utf8_lossy(&content);
            match ron::de::from_str::<paiagram_core::WorldSnapshot>(&s) {
                Ok(snapshot) => {
                    let mut queue = pending.lock().unwrap();
                    queue.push(paiagram_core::Command::LoadWorld {
                        snapshot: Box::new(snapshot),
                    });
                }
                Err(e) => log::error!("Failed to load RON: {e}"),
            }
        });
    }

    fn sync_ui(&mut self, ctx: &Context) {
        if !self.theme_changed {
            return;
        }
        self.theme_changed = false;
        if self.preferences.dark_mode {
            ctx.set_theme(egui::Theme::Dark);
        } else {
            ctx.set_theme(egui::Theme::Light);
        }
    }

    pub fn show_ui(&mut self, ui: &mut Ui, cpu_time: Option<f32>) {
        self.sync_ui(ui.ctx());

        self.preferences.dark_mode = match ui.system_theme() {
            None => self.preferences.dark_mode,
            Some(egui::Theme::Dark) => true,
            Some(egui::Theme::Light) => false,
        };

        self.process_pending_tabs();

        // Process pending import/load commands from background threads
        {
            let mut pending = self.pending_import.lock().unwrap();
            for cmd in pending.drain(..) {
                let _ = self.source.apply_command(cmd);
            }
        }

        if ui.input_mut(|r| r.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::P))) {
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
                        if cfg!(not(target_arch = "wasm32"))
                            && ui.button("Import from URL...").clicked()
                        {
                        }

                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            ui.separator();
                            if ui.button(tr!("read-file-prompt", {name: "OuDia"})).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("read-file-title", {name: "OuDia"}))
                                    .add_filter("OuDia", &["oud"]);
                                self.import_file(dialog, |bytes| {
                                    paiagram_core::import::oudia::parse_oud(&bytes)
                                });
                            }
                            if ui.button(tr!("read-file-prompt", {name: "OuDiaSecond"})).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("read-file-title", {name: "OuDiaSecond"}))
                                    .add_filter("OuDiaSecond", &["oud2"]);
                                self.import_file(dialog, |bytes| {
                                    let s = String::from_utf8_lossy(&bytes);
                                    paiagram_core::import::oudia::parse_oud2(&s)
                                });
                            }
                            if ui.button(tr!("read-file-prompt", {name: "qETRC/pyETRC"})).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("read-file-title", {name: "qETRC/pyETRC"}))
                                    .add_filter("qETRC/pyETRC", &["pyetgr", "json"]);
                                self.import_file(dialog, |bytes| {
                                    let s = String::from_utf8_lossy(&bytes);
                                    paiagram_core::import::qetrc::load_qetrc(&s)
                                });
                            }
                            if ui.button(tr!("read-file-prompt", {name: "GTFS"})).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("read-file-title", {name: "GTFS"}))
                                    .add_filter("GTFS", &["zip"]);
                                self.import_file(dialog, |bytes| {
                                    paiagram_core::import::gtfs::load_gtfs_static(&bytes)
                                });
                            }
                            if ui.button(tr!("read-file-prompt", {name: "LLT"})).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("read-file-title", {name: "LLT"}))
                                    .add_filter("LLT", &["json"]);
                                self.import_file(dialog, |bytes| {
                                    let s = String::from_utf8_lossy(&bytes);
                                    paiagram_core::import::llt::load_llt(&s)
                                });
                            }
                            ui.separator();
                            if ui.button(tr!("menu-save")).clicked() {
                                self.save();
                            }
                            if ui.button(tr!("menu-read")).clicked() {
                                let dialog = rfd::AsyncFileDialog::new()
                                    .set_title(tr!("menu-load-save"))
                                    .add_filter(tr!("menu-paiagram-savefiles"), &["paia"]);
                                self.load_file(dialog);
                            }
                            if self.preferences.developer_mode {
                                if ui.button(tr!("menu-save-ron")).clicked() {
                                    self.save_ron();
                                }
                                if ui.button(tr!("menu-read-ron")).clicked() {
                                    let dialog = rfd::AsyncFileDialog::new()
                                        .set_title(tr!("menu-load-ron-files"))
                                        .add_filter(tr!("menu-ron-files"), &["ron"]);
                                    self.load_ron_file(dialog);
                                }
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

                    if self.preferences.developer_mode {
                        self.frame_time.push(ui.input(|r| r.stable_dt));
                        let average_dt = self.frame_time.average_dt();
                        ui.monospace(format!("FPS: {:6.2}", 1.0_f32 / average_dt));
                        ui.monospace(format!("FRAME: {:5.2}ms", average_dt * 1000.0_f32));
                        ui.monospace(format!(
                            "CPU: {:5.2}ms",
                            cpu_time.unwrap_or(0.0) * 1000.0_f32
                        ));
                        ui.horizontal(|ui| {
                            const GAP: f32 = 4.0;
                            const SAMPLE_COUNT: usize = 32;
                            let stroke = Stroke {
                                color: PredefinedColor::Blue.into_color32(ui.visuals().dark_mode),
                                width: 3.0,
                            };
                            let max = self
                                .frame_time
                                .previous_n(SAMPLE_COUNT)
                                .fold(0.0_f32, f32::max)
                                .max(f32::EPSILON);
                            let graph_width = SAMPLE_COUNT as f32 * (stroke.width + GAP) - GAP;
                            let graph_height = ui.available_height();
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(graph_width, graph_height),
                                egui::Sense::hover(),
                            );
                            for (idx, f) in self.frame_time.previous_n(SAMPLE_COUNT).enumerate() {
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
                        let can_undo = self.source.undoable();
                        let can_redo = self.source.redoable();
                        if ui
                            .add_enabled(can_undo, egui::Button::new(tr!("menu-undo")))
                            .clicked()
                        {
                            self.source.undo();
                        }
                        if ui
                            .add_enabled(can_redo, egui::Button::new(tr!("menu-redo")))
                            .clicked()
                        {
                            self.source.redo();
                        }
                        ui.checkbox(&mut self.additional_ui.expanded, "");
                        const GIT_REV_SHORT: &str =
                            git_version::git_version!(fallback = "unknown");
                        const GH_LINK: &str = git_version::git_version!(
                            args = ["--always", "--abbrev=40"],
                            prefix = "https://github.com/wensimehrp/Paiagram/commit/",
                            fallback = "https://github.com/wensimehrp/Paiagram/"
                        );
                        ui.hyperlink_to(GIT_REV_SHORT, GH_LINK);
                    });
                })
            });

        Panel::bottom("bottom panel")
            .exact_size(24.0)
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let ticks_in_cycle = self.project_settings.repeat_frequency.to_ticks();
                    let mut time = self.timer.read_ticks().to_timetable_time();

                    ui.add_enabled(
                        !self.timer.sync_to_real_time,
                        egui::Checkbox::new(&mut self.timer.animation_playing, ""),
                    );
                    let time_response = ui.add(TimeDragValue(&mut time));
                    ui.add_enabled(
                        !self.timer.sync_to_real_time,
                        egui::DragValue::new(&mut self.timer.animation_speed)
                            .fixed_decimals(1)
                            .suffix("×"),
                    );
                    egui::Popup::menu(&time_response).show(|ui| {
                        ui.checkbox(
                            &mut self.timer.sync_to_real_time,
                            tr!("menu-sync-system-clock"),
                        );
                    });

                    const TIMER_LOCK_ID: u64 = 1;
                    if !self.timer.sync_to_real_time
                        && time_response.dragged()
                        && self.timer.try_lock(TIMER_LOCK_ID)
                    {
                        self.timer.write_ticks(Tick::from_timetable_time(time));
                    } else {
                        self.timer.try_unlock(TIMER_LOCK_ID);
                    }

                    if self.timer.animation_playing {
                        ui.ctx().request_repaint();
                    }

                    let (_id, rect) = ui.allocate_space(ui.available_size());
                    let progress_stroke = ui.visuals().window_stroke();
                    ui.painter()
                        .hline(rect.x_range(), rect.center().y, progress_stroke);
                    let amount_of_ticks = 24;
                    for i in 0..(amount_of_ticks + 1) {
                        let progress = (1.0 / amount_of_ticks as f32) * i as f32;
                        let x = egui::lerp(rect.left()..=rect.right(), progress);
                        let y_range = if i % 4 == 0 {
                            rect.y_range()
                        } else {
                            rect.y_range().shrink(5.0)
                        };
                        ui.painter().vline(x, y_range, progress_stroke);
                    }
                    let indicator_stroke = Stroke::new(1.5, Color32::RED);
                    let progress = self.timer.read_ticks().normalized_with(ticks_in_cycle);
                    let progress = progress.0 as f32 / ticks_in_cycle.0 as f32;
                    ui.painter().vline(
                        egui::lerp(rect.left()..=rect.right(), progress),
                        rect.y_range(),
                        indicator_stroke,
                    );
                })
            });

        // Side panel and central panel
        let expanded = self.additional_ui.expanded;

        // Use raw pointers to avoid borrow conflicts in synchronous egui closures.
        // SAFETY: The closures run synchronously within the show_* calls,
        // so the pointers remain valid and the borrows are disjoint in practice.
        let right_tree: *mut Tree<AdditionalTab> = &mut self.additional_ui.tree;
        let right_app: *mut App = self as *mut App;
        let right_focused: *mut Option<TileId> = &mut self.additional_ui.focused_id;
        let main_tree: *mut Tree<MainTab> = &mut self.main_ui.tree;
        Panel::right("right panel")
            .frame(Frame::default())
            .show_animated_inside(ui, expanded, |ui| {
                let tree = unsafe { &mut *right_tree };
                let app = unsafe { &mut *right_app };
                let focused_id = unsafe { &mut *right_focused };
                let main_tree = unsafe { &mut *main_tree };
                let focused_tab = focused_id
                    .and_then(|id| main_tree.tiles.get_mut(id))
                    .and_then(|p| if let Tile::Pane(pane) = p { Some(pane) } else { None });
                let mut aux = AdditionalTabViewer {
                    app,
                    focused_tab,
                };
                tree.ui(&mut aux, ui);
            });

        let main_tree: *mut Tree<MainTab> = &mut self.main_ui.tree;
        let main_app: *mut App = self as *mut App;
        let main_focused: *mut Option<TileId> = &mut self.additional_ui.focused_id;
        egui::CentralPanel::default()
            .frame(Frame::default())
            .show_inside(ui, |ui| {
                let app = unsafe { &mut *main_app };
                let tree = unsafe { &mut *main_tree };
                let focused_id = unsafe { &mut *main_focused };
                let mut maximized = app.main_ui.maximized;
                if let Some(max_id) = app.main_ui.maximized
                    && let Some(Tile::Pane(pane)) = tree.tiles.get_mut(max_id)
                {
                    let mut tab_viewer = MainTabViewer {
                        app,
                        last_focused_id: &mut None,
                        last_maximized_id: &mut None,
                    };
                    Panel::top("maximized_top")
                        .exact_size(24.0)
                        .show_inside(ui, |ui| {
                            let res = ui.horizontal(|ui| {
                                ui.label(tab_viewer.tab_title_for_pane(pane));
                                ui.label(RichText::new(tr!("menu-maximized-view")).italics());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| ui.button("x"),
                                )
                                .inner
                            });
                            if res.inner.clicked() {
                                maximized = None
                            }
                        });
                    let _ = tab_viewer.pane_ui(ui, max_id, pane);
                } else {
                    let mut tab_viewer = MainTabViewer {
                        app,
                        last_focused_id: focused_id,
                        last_maximized_id: &mut maximized,
                    };
                    tree.ui(&mut tab_viewer, ui);
                }
                app.main_ui.maximized = maximized;
            });
    }
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

    fonts.font_data.insert(
        "dia_pro".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/DiaPro-Regular.ttf"
        ))),
    );

    if has_sarasa {
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "my_font".to_owned());
    }

    let mut dia_pro_family = vec!["dia_pro".to_owned()];
    if has_sarasa {
        dia_pro_family.push("my_font".to_owned());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("dia_pro".into()), dia_pro_family);

    fonts
}

#[cfg(not(target_arch = "wasm32"))]
fn load_sarasa_local() -> Option<Vec<u8>> {
    let mut candidates = vec![
        PathBuf::from("assets/fonts/SarasaUiSC-Regular.ttf"),
        PathBuf::from("crates/paiagram/assets/fonts/SarasaUiSC-Regular.ttf"),
    ];

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.push(parent.join("assets/fonts/SarasaUiSC-Regular.ttf"));
    }

    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }

    None
}

#[cfg(target_arch = "wasm32")]
fn download_sarasa_and_apply(ctx: Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(window) = eframe::web_sys::window() else {
            return;
        };

        let Ok(response) =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_str("SarasaUiSC-Regular.ttf"))
                .await
        else {
            return;
        };

        let Ok(response) = response.dyn_into::<eframe::web_sys::Response>() else {
            return;
        };

        if !response.ok() {
            return;
        }

        let Ok(array_buffer_promise) = response.array_buffer() else {
            return;
        };

        let Ok(array_buffer) = wasm_bindgen_futures::JsFuture::from(array_buffer_promise).await
        else {
            return;
        };

        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        ctx.set_fonts(build_font_definitions(Some(bytes)));
    });
}
