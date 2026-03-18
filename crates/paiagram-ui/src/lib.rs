//! # UI
//! Module for the user interface.

mod actions;
mod command_palette;
pub mod export_typst_diagram;
pub mod save;
pub mod tabs;
mod widgets;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use bevy::prelude::*;
use chrono::{Local, Timelike};
use egui::{
    Context, Frame, Key, KeyboardShortcut, Modifiers, OpenUrl, Response, RichText, ScrollArea,
    Stroke, Ui,
};
use egui_i18n::tr;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile, TileId, Tiles, Tree, UiResponse,
};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};
use paiagram_core::colors::PredefinedColor;
use paiagram_core::import::LoadLlt;
use paiagram_core::station::StationQuery;
use paiagram_rw::read::CallbackFn;
use serde::{Deserialize, Serialize};
use tabs::{Tab, all_tabs::*};

use paiagram_core::units::time::Tick;
use paiagram_core::{
    entry::{EntryEstimate, IsDerivedEntry},
    import::{DownloadFile, LoadGTFS, LoadOuDia, LoadQETRC},
    route::Route,
    settings::UserPreferences,
    trip::{ConvertDerivedEntryToExplicit, Trip, TripSchedule},
    units::time::TimetableTime,
    vehicle::Vehicle,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainUiState>()
            .init_resource::<AdditionalUiState>()
            .init_resource::<SelectedItems>()
            .init_resource::<FrameTimeHistory>()
            .init_resource::<GlobalTimer>()
            .init_resource::<UiModal>()
            .init_resource::<command_palette::CommandPalette>()
            .add_plugins((
                bevy_inspector_egui::DefaultInspectorConfigPlugin,
                actions::ActionsPlugin,
            ))
            .add_message::<OpenOrFocus>()
            .add_systems(
                Update,
                (
                    open_or_focus_tab.run_if(on_message::<OpenOrFocus>),
                    save::apply_loaded_scene
                        .run_if(resource_exists::<paiagram_rw::save::LoadedScene>),
                    update_timer,
                ),
            );
    }
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimetableEntrySelection {
    pub entry: Entity,
    pub parent: Entity,
}

#[derive(Reflect, Clone, Copy, PartialEq, Hash, Debug)]
pub(crate) struct IntervalSelection {
    pub source: Entity,
    pub target: Entity,
}

#[derive(Reflect, Clone, Copy, Debug)]
pub(crate) struct StationSelection {
    pub station: Entity,
}

impl PartialEq for StationSelection {
    fn eq(&self, other: &Self) -> bool {
        self.station == other.station
    }
}

#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingRouteSelection {
    pub prev_station: Entity,
}

#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub(crate) struct ExtendingTripSelection {
    pub entry: Entity,
    pub previous_pos: Option<(TimetableTime, usize)>,
    pub last_time: Option<TimetableTime>,
    pub current_entry: Option<Entity>,
}

pub(crate) fn display_entry_info(
    (InMut(ui), InRef(selected_entries)): (InMut<Ui>, InRef<[TimetableEntrySelection]>),
    mut commands: Commands,
    mut selected_items: ResMut<SelectedItems>,
    is_derived_q: Query<(), With<IsDerivedEntry>>,
    mut names_q: Query<&mut Name>,
    schedule_q: Query<&TripSchedule, With<Trip>>,
    entry_q: Query<&EntryEstimate>,
    mut open_or_focus: MessageWriter<OpenOrFocus>,
) {
    for (idx, TimetableEntrySelection { entry, parent }) in
        selected_entries.iter().copied().enumerate()
    {
        ui.strong(format!("Entry {}", idx + 1));

        let is_derived = is_derived_q.get(entry).is_ok();
        if is_derived && ui.button("Convert to explicit").clicked() {
            commands.trigger(ConvertDerivedEntryToExplicit { entity: entry });
        } else if !is_derived && ui.button("Delete").clicked() {
            commands.entity(entry).despawn();
        }

        if let Ok(mut name) = names_q.get_mut(parent) {
            name.mutate(|n| {
                ui.text_edit_singleline(n);
            });
        }

        if ui.button("Open trip view").clicked() {
            open_or_focus.write(OpenOrFocus(crate::MainTab::Trip(TripTab::new(parent))));
        }

        ui.separator();
    }

    if selected_entries.len() == 1 {
        let parent = selected_entries[0].parent;
        if ui.button("Extend").clicked() {
            let mut last_time = None;
            if let Ok(schedule) = schedule_q.get(parent)
                && let Some(time) = schedule.iter().rev().find_map(|e| entry_q.get(*e).ok())
            {
                last_time = Some(time.dep);
            }
            *selected_items = SelectedItems::ExtendingTrip(ExtendingTripSelection {
                entry: parent,
                previous_pos: None,
                current_entry: None,
                last_time,
            });
        }
    }
}

pub(crate) fn display_station_info(
    (InMut(ui), InRef(selected_stations)): (InMut<Ui>, InRef<[StationSelection]>),
    station_q: Query<StationQuery>,
    mut commands: Commands,
) {
    for station in station_q.iter_many(selected_stations.iter().map(|it| it.station)) {
        ui.label(station.name.as_ref());
    }
    if selected_stations.len() >= 2 && ui.button("Create new route").clicked() {
        let stations: Vec<_> = selected_stations.iter().map(|it| it.station).collect();
        commands.spawn((
            Name::new("New Route"),
            Route {
                lengths: vec![10.0; stations.len()],
                stops: stations,
            },
        ));
    }
}

#[derive(Reflect, Resource, Clone, PartialEq, Debug)]
#[reflect(Resource)]
pub(crate) enum SelectedItems {
    None,
    TimetableEntries(Vec<TimetableEntrySelection>),
    Intervals(Vec<IntervalSelection>),
    Stations(Vec<StationSelection>),
    ExtendingRoute(ExtendingRouteSelection),
    ExtendingTrip(ExtendingTripSelection),
}

impl SelectedItems {
    pub(crate) fn add_entry(&mut self, item: SelectedItem) {
        fn toggle_vec<T: PartialEq>(v: &mut Vec<T>, item: T) -> bool {
            if let Some(idx) = v.iter().position(|entry| *entry == item) {
                v.remove(idx);
            } else {
                v.push(item);
            }
            v.is_empty()
        }

        match item {
            SelectedItem::None => {}
            SelectedItem::TimetableEntries(it) => {
                if let Self::TimetableEntries(v) = self {
                    if toggle_vec(v, it) {
                        *self = Self::None;
                    }
                } else if matches!(self, Self::None) {
                    *self = Self::TimetableEntries(vec![it]);
                }
            }
            SelectedItem::Intervals(it) => {
                if let Self::Intervals(v) = self {
                    if toggle_vec(v, it) {
                        *self = Self::None;
                    }
                } else if matches!(self, Self::None) {
                    *self = Self::Intervals(vec![it]);
                }
            }
            SelectedItem::Stations(it) => {
                if let Self::Stations(v) = self {
                    if toggle_vec(v, it) {
                        *self = Self::None;
                    }
                } else if matches!(self, Self::None) {
                    *self = Self::Stations(vec![it]);
                }
            }
        }
    }

    pub(crate) fn set_or_reset(&mut self, item: SelectedItem) {
        match item {
            SelectedItem::None => *self = Self::None,
            SelectedItem::TimetableEntries(it) => *self = Self::TimetableEntries(vec![it]),
            SelectedItem::Intervals(it) => *self = Self::Intervals(vec![it]),
            SelectedItem::Stations(it) => *self = Self::Stations(vec![it]),
        }
    }

    pub(crate) fn station_selection(&self) -> &[StationSelection] {
        match self {
            Self::Stations(s) => s.as_slice(),
            _ => &[],
        }
    }

    pub(crate) fn entry_selection(&self) -> &[TimetableEntrySelection] {
        match self {
            Self::TimetableEntries(s) => s.as_slice(),
            _ => &[],
        }
    }
}

pub(crate) enum SelectedItem {
    None,
    TimetableEntries(TimetableEntrySelection),
    Intervals(IntervalSelection),
    Stations(StationSelection),
}

impl SelectedItem {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl Default for SelectedItems {
    fn default() -> Self {
        Self::None
    }
}

impl Default for SelectedItem {
    fn default() -> Self {
        Self::None
    }
}

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

#[derive(Resource)]
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

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct GlobalTimer {
    value: AtomicI64,
    locker: AtomicU64,
    animation_speed: f64,
    animation_playing: bool,
    sync_to_real_time: bool,
}

fn update_timer(mut timer: ResMut<GlobalTimer>, time: Res<Time<Real>>) {
    if !timer.is_locked() && timer.sync_to_real_time {
        let now = Local::now();
        let seconds = now.num_seconds_from_midnight() as f64;
        let rest = now.nanosecond() as f64 / 1_000_000_000 as f64;
        timer.animation_speed = 1.0;
        timer.animation_playing = true;
        timer.write_seconds(seconds + rest);
    } else if timer.animation_playing && !timer.is_locked() {
        let mut seconds = timer.read_seconds();
        seconds += timer.animation_speed * time.delta_secs_f64();
        timer.write_seconds(seconds);
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
            MainTab::Diagram($t) => $body,
            MainTab::Settings($t) => $body,
            MainTab::Classes($t) => $body,
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
    Diagram(DiagramTab),
    Settings(SettingsTab),
    Classes(ClassesTab),
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
#[reflect(opaque, Resource, Serialize, Deserialize, MapEntities)]
pub struct MainUiState {
    #[deref]
    tree: Tree<MainTab>,
    maximized: Option<TileId>,
}

impl MainUiState {
    pub fn push_to_focused_leaf(&mut self, new_pane: MainTab) -> TileId {
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
}

impl Default for MainUiState {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs("main", vec![MainTab::Start(StartTab::default())]),
            maximized: None,
        }
    }
}

impl MapEntities for MainUiState {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        for pane in self
            .tree
            .tiles
            .iter_mut()
            .filter_map(|(_, p)| if let Tile::Pane(p) = p { Some(p) } else { None })
        {
            pane.map_entities(entity_mapper);
        }
    }
}

#[derive(Message)]
struct OpenOrFocus(MainTab);

fn open_or_focus_tab(
    mut messages: MessageReader<OpenOrFocus>,
    mut mus: ResMut<MainUiState>,
    mut aus: ResMut<AdditionalUiState>,
) {
    for msg in messages.read() {
        let pane = &msg.0; // your pane data

        let focused_id = if let Some(tile_id) = mus.tree.tiles.find_pane(pane) {
            // Already exists → just focus it
            mus.make_active(|id, _| id == tile_id);
            mus.set_visible(tile_id, true);
            tile_id
        } else {
            // New pane → add it to the currently focused container
            mus.push_to_focused_leaf(pane.clone())
        };
        aus.focused_id = Some(focused_id);
    }
}

struct MainTabViewer<'w> {
    world: &'w mut World,
    last_focused_id: &'w mut Option<TileId>,
    last_maximized_id: &'w mut Option<TileId>,
}

impl<'w> MainTabViewer<'w> {
    fn add_popup(&mut self, ui: &mut Ui) {
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
}

impl<'w> Behavior<MainTab> for MainTabViewer<'w> {
    fn tab_title_for_pane(&mut self, pane: &MainTab) -> egui::WidgetText {
        for_all_tabs!(pane, p, p.title())
    }
    fn on_tab_button(
        &mut self,
        _tiles: &Tiles<MainTab>,
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
        for_all_tabs!(tab, t, t.main_display(self.world, ui));
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
struct AdditionalUiState {
    #[deref]
    tree: Tree<AdditionalTab>,
    focused_id: Option<TileId>,
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
        }
    }
}

struct AdditionalTabViewer<'w> {
    world: &'w mut World,
    focused_tab: Option<&'w mut MainTab>,
}

impl<'w> egui_tiles::Behavior<AdditionalTab> for AdditionalTabViewer<'w> {
    fn tab_title_for_pane(&mut self, tab: &AdditionalTab) -> egui::WidgetText {
        match *tab {
            AdditionalTab::Edit => "Edit",
            AdditionalTab::Properties => "Properties",
            AdditionalTab::Export => "Export",
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
                ui.label("Nothing focused");
                return;
            };
            match *tab {
                AdditionalTab::Edit => {
                    for_all_tabs!(focused, t, t.edit_display(self.world, ui));
                }
                AdditionalTab::Properties => {
                    for_all_tabs!(focused, t, t.display_display(self.world, ui));
                }
                AdditionalTab::Export => {
                    for_all_tabs!(focused, t, t.export_display(self.world, ui));
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

pub fn show_ui(ctx: &Context, world: &mut World, cpu_time: Option<f32>) {
    world.run_system_cached_with(sync_ui, ctx).unwrap();
    world.resource_scope(|world, mut modal: Mut<UiModal>| {
        let Some(m) = &mut modal.0 else { return };
        let modal_response = egui::Modal::new(m.id()).show(ctx, |ui| m.display(ui, world));
        if modal_response.should_close() {
            modal.0 = None
        }
    });

    // check if ctrl+p clicked
    world.resource_scope(
        |world, mut command_palette: Mut<command_palette::CommandPalette>| {
            if ctx
                .input_mut(|r| r.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::P)))
            {
                command_palette.toggle();
            };
            command_palette.show(ctx, world);
        },
    );

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
                    let mut read_file = |name: &str, extensions: &[&str], cb: CallbackFn| {
                        if ui.button(tr!("read-file-prompt", {name: name})).clicked() {
                            world.commands().trigger(paiagram_rw::read::ReadFile {
                                title: tr!("read-file-title", {name: name}),
                                extensions: vec![(
                                    tr!("read-file-filetype", {name: name}),
                                    extensions.iter().map(|s| s.to_string()).collect(),
                                )],
                                callback: cb,
                            });
                        }
                    };
                    read_file("OuDia", &["oud"], |c, s| {
                        c.trigger(LoadOuDia::original(s));
                    });
                    read_file("OuDiaSecond", &["oud2"], |c, s| {
                        c.trigger(LoadOuDia::second(String::from_utf8(s).unwrap()));
                    });
                    read_file("qETRC/pyETRC", &["pyetgr", "json"], |c, s| {
                        c.trigger(LoadQETRC {
                            content: String::from_utf8(s).unwrap(),
                        });
                    });
                    read_file("GTFS", &["zip"], |c, s| {
                        c.trigger(LoadGTFS { content: s });
                    });
                    read_file("LLT", &["json"], |c, s| {
                        c.trigger(LoadLlt {
                            content: String::from_utf8(s).unwrap(),
                        });
                    });
                    ui.separator();
                    if ui.button("Save...").clicked() {
                        save::save(world, "save.paia".to_string());
                    }
                    if ui.button("Read...").clicked() {
                        world.commands().trigger(paiagram_rw::read::ReadFile {
                            title: "Load Save".to_string(),
                            extensions: vec![(
                                "Paiagram Savefiles".to_string(),
                                vec!["paia".to_string()],
                            )],
                            callback: paiagram_rw::save::add_load_candidate_compressed_cbor,
                        });
                    }
                    let developer_mode = world.resource_mut::<UserPreferences>().developer_mode;
                    {
                        if developer_mode && ui.button("Save RON...").clicked() {
                            save::save_ron(world, "saved.ron".to_string());
                        }
                        if developer_mode && ui.button("Read RON...").clicked() {
                            world.commands().trigger(paiagram_rw::read::ReadFile {
                                title: "Load RON Files".to_string(),
                                extensions: vec![(
                                    "RON Files".to_string(),
                                    vec!["ron".to_string()],
                                )],
                                callback: paiagram_rw::save::add_load_candidate_ron,
                            });
                        }
                    }
                });
                let res = ui.button("About");
                egui::Popup::menu(&res).show(|ui| {
                    if ui.button("Documentation").clicked() {
                        ui.ctx()
                            .open_url(OpenUrl::new_tab(if cfg!(target_arch = "wasm32") {
                                "/nightly-docs"
                            } else {
                                "https://paiagram.com/nightly-docs"
                            }));
                    }
                    if cfg!(target_arch = "wasm32") && ui.button("Legal").clicked() {
                        ui.ctx().open_url(OpenUrl::new_tab("./license.html"));
                    }
                });
                let mut frame_time_history = world.resource_mut::<FrameTimeHistory>();
                frame_time_history.push(ui.input(|r| r.stable_dt));
                let average_dt = frame_time_history.average_dt();
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
                        color: PredefinedColor::Blue.get(ui.visuals().dark_mode),
                        width: 3.0,
                    };
                    let max = frame_time_history
                        .previous_n(SAMPLE_COUNT)
                        .fold(0.0_f32, f32::max)
                        .max(f32::EPSILON);
                    let graph_width = SAMPLE_COUNT as f32 * (stroke.width + GAP) - GAP;
                    let graph_height = ui.available_height();
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(graph_width, graph_height),
                        egui::Sense::hover(),
                    );
                    for (idx, f) in frame_time_history.previous_n(SAMPLE_COUNT).enumerate() {
                        let height = rect.height() * (f / max).clamp(0.0, 1.0);
                        let x =
                            rect.right() - idx as f32 * (stroke.width + GAP) - stroke.width * 0.5;
                        let points = [
                            egui::pos2(x, rect.bottom()),
                            egui::pos2(x, rect.bottom() - height),
                        ];
                        ui.painter().line_segment(points, stroke);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut timer = world.resource_mut::<GlobalTimer>();
                    let mut seconds = timer.read_seconds();
                    ui.add_enabled(
                        !timer.sync_to_real_time,
                        egui::Checkbox::new(&mut timer.animation_playing, "Play animation"),
                    );
                    let time_response = ui.add(
                        egui::DragValue::new(&mut seconds)
                            .custom_formatter(|it, _| {
                                format!("{}", TimetableTime::from_hms(0, 0, it as i32))
                            })
                            .custom_parser(|s| TimetableTime::from_str(s).map(|it| it.0 as f64)),
                    );
                    ui.add_enabled(
                        !timer.sync_to_real_time,
                        egui::Slider::new(&mut timer.animation_speed, -500.0..=500.0)
                            .fixed_decimals(1)
                            .text("Speed")
                            .clamping(egui::SliderClamping::Always),
                    );
                    egui::Popup::menu(&time_response).show(|ui| {
                        ui.checkbox(&mut timer.sync_to_real_time, "Sync with system clock");
                    });
                    unsafe {
                        if !timer.sync_to_real_time
                            && time_response.dragged()
                            && timer.try_lock_unchecked(1)
                        {
                            timer.write_seconds(seconds);
                        } else {
                            timer.try_unlock_unchecked(1);
                        }
                    }
                    if timer.animation_playing {
                        ui.ctx().request_repaint();
                    }
                    world.resource_scope(|world, mut history: Mut<actions::ActionHistory>| {
                        if ui
                            .add_enabled(history.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            history.try_undo(world);
                        }
                        if ui
                            .add_enabled(history.can_redo(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            history.try_redo(world);
                        }
                    });
                });
            })
        });
    world.resource_scope(|world, mut aus: Mut<AdditionalUiState>| {
        world.resource_scope(|mut world, mut mus: Mut<MainUiState>| {
            let mut tab_viewer = AdditionalTabViewer {
                world: &mut world,
                focused_tab: aus
                    .focused_id
                    .and_then(|id| mus.tiles.get_mut(id))
                    .and_then(|p| {
                        if let Tile::Pane(pane) = p {
                            Some(pane)
                        } else {
                            None
                        }
                    }),
            };
            egui::SidePanel::right("right panel")
                .frame(Frame::default())
                .show(ctx, |ui| {
                    aus.ui(&mut tab_viewer, ui);
                });
            egui::CentralPanel::default()
                .frame(Frame::default())
                .show(ctx, |ui| {
                    let mut maximized = mus.maximized;
                    if let Some(max_id) = mus.maximized
                        && let Some(Tile::Pane(pane)) = mus.tree.tiles.get_mut(max_id)
                    {
                        let mut tab_viewer = MainTabViewer {
                            world: &mut world,
                            last_focused_id: &mut None,
                            last_maximized_id: &mut None,
                        };
                        egui::TopBottomPanel::top("maximized_top")
                            .exact_height(24.0)
                            .show_inside(ui, |ui| {
                                let res = ui.horizontal(|ui| {
                                    ui.label(tab_viewer.tab_title_for_pane(pane));
                                    ui.label(RichText::new("Maximized view").italics());
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
                            world: &mut world,
                            last_focused_id: &mut aus.focused_id,
                            last_maximized_id: &mut maximized,
                        };
                        mus.tree.ui(&mut tab_viewer, ui);
                    }
                    mus.maximized = maximized;
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
        PathBuf::from("crates/paiagram-ui/assets/fonts/SarasaUiSC-Regular.ttf"),
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
