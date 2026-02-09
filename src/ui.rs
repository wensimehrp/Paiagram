//! # UI
//! Module for the user interface.

pub mod tabs;
mod widgets;

use bevy::prelude::*;
use egui::{Context, CornerRadius, Frame, Id, Margin, ScrollArea, Ui};
use egui_dock::{DockArea, DockState, TabViewer};
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use tabs::{Tab, all_tabs::*};

use crate::{
    import::{self, LoadOuDiaSecond, LoadQETRC},
    route::Route,
    settings::UserPreferences,
    trip::Trip,
    vehicle::Vehicle,
};

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainUiState>()
            .init_resource::<AdditionalUiState>()
            .add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin)
            .add_message::<OpenOrFocus>()
            .add_systems(Update, open_or_focus_tab.run_if(on_message::<OpenOrFocus>));
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
            // MainTab::Graph($t) => $body,
            MainTab::Inspector($t) => $body,
            MainTab::Trip($t) => $body,
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
    // Graph(GraphTab),
    Inspector(InspectorTab),
    Trip(TripTab),
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
        ] {
            if ui.button(s).clicked() {
                self.world.write_message(OpenOrFocus(t));
                ui.close();
            }
        }
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

pub fn show_ui(ctx: &Context, world: &mut World) {
    world.run_system_cached_with(sync_ui, ctx).unwrap();
    egui::TopBottomPanel::top("top panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // TODO: add rfd file reading
            let res = ui.button("More...");
            egui::Popup::menu(&res).show(|ui| {
                if ui.button("Read OUD2").clicked() {
                    world.commands().trigger(crate::rw::read::ReadFile {
                        title: "Load OuDiaSecond Files".to_string(),
                        extensions: vec![(
                            "OuDiaSecond Files".to_string(),
                            vec!["oud2".to_string()],
                        )],
                        callback: |c, s| {
                            c.trigger(LoadOuDiaSecond {
                                content: String::from_utf8(s).unwrap(),
                            });
                        },
                    });
                }
                if ui.button("Read qETRC").clicked() {
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
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "my_font".to_owned());
    ctx.set_fonts(fonts);
}
