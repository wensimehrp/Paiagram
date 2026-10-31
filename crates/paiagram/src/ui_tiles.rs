use egui::*;
use egui_tiles::{
    Behavior, ContainerKind, SimplificationOptions, Tile as EguiTile, TileId, Tiles, Tree,
    UiResponse,
};
use serde::{Deserialize, Serialize};

use crate::tiles::Tile;
use crate::tiles::all_tiles::*;
use crate::{App, UiCommand, UiTile, for_all_tiles};

#[derive(Serialize, Deserialize)]
pub struct UiTiles {
    pub tree: Tree<UiTile>,
    pub maximized: Option<TileId>,
}

impl UiTiles {
    pub fn open_or_focus(&mut self, tile: UiTile) {
        let focused_id = if let Some(tile_id) = self.tree.tiles.find_pane(&tile) {
            // Already exists → just focus it
            self.tree.make_active(|id, _| id == tile_id);
            self.tree.set_visible(tile_id, true);
            tile_id
        } else {
            // New pane → add it to the currently focused container
            self.push_to_focused_leaf(tile)
        };
    }
    fn push_to_focused_leaf(&mut self, new_pane: UiTile) -> TileId {
        let new_id = self.tree.tiles.insert_pane(new_pane);

        // Try to add it to the same container that is currently focused
        if let Some(&active_id) = self.tree.active_tiles().last()
            && let Some(parent_id) = self.tree.tiles.parent_of(active_id)
            && let Some(EguiTile::Container(container)) = self.tree.tiles.get_mut(parent_id)
            && container.kind() == ContainerKind::Tabs
        {
            container.add_child(new_id);
            self.tree.make_active(|id, _| id == new_id);
            return new_id;
        }

        // Fallback: create a new top-level container
        let old_root = self.tree.root;
        let tiles_id = if let Some(old_root) = old_root {
            self.tree.tiles.insert_tab_tile(vec![old_root, new_id])
        } else {
            self.tree.tiles.insert_tab_tile(vec![new_id])
        };
        self.tree.root = Some(tiles_id);
        self.tree.make_active(|id, _| id == new_id);
        new_id
    }
}

impl Default for UiTiles {
    fn default() -> Self {
        Self {
            tree: Tree::new_tabs(
                Id::unique("main"),
                vec![
                    UiTile::Start(StartTile::default()),
                    UiTile::Config(ConfigTile::default()),
                ],
            ),
            maximized: None,
        }
    }
}

pub struct TileViewer<'a> {
    pub app: &'a mut App,
}

impl<'w> Behavior<UiTile> for TileViewer<'w> {
    fn tab_title_for_pane(&mut self, pane: &UiTile) -> egui::WidgetText {
        for_all_tiles!(pane, p, p.title())
    }
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, tile: &mut UiTile) -> UiResponse {
        puffin::profile_function!();
        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0, ui.visuals().panel_fill);
        for_all_tiles!(tile, t, t.main_display(self.app, ui));

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
            flatten_tabs_in_tabs: true,
        }
    }
    fn is_tab_closable(&self, tiles: &Tiles<UiTile>, tile_id: TileId) -> bool {
        match tiles.get(tile_id) {
            None => false,
            Some(EguiTile::Container(_)) => false,
            Some(EguiTile::Pane(UiTile::Start(_))) => false,
            Some(EguiTile::Pane(_)) => true,
        }
    }
    fn tab_bar_trailing_ui(
        &mut self,
        _tiles: &Tiles<UiTile>,
        ui: &mut Ui,
        _tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
    ) {
        ui.menu_button("+", |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                #[rustfmt::skip]
                let tiles: [(_, fn() -> UiTile); 6] = [
                    ("Intervals",       || UiTile::Intervals(IntervalsTile::default())),
                    ("Service Classes", || UiTile::ServiceClasses(ServiceClassesTile::default())),
                    ("Graph",           || UiTile::Graph(GraphTile::default())),
                    ("Extension",       || UiTile::Extension(ExtensionTile::default())),
                    ("Config",          || UiTile::Config(ConfigTile::default())),
                    ("Trip",            || UiTile::Trip(TripTile::default())),
                ];
                for (text, tile) in tiles {
                    if ui.button(text).clicked() {
                        self.app.ui_action_tx.send(UiCommand::OpenOrFocus(tile()));
                    }
                }
                if self.app.preferences.dev_mode && ui.button("Profiler").clicked() {
                    self.app.ui_action_tx.send(UiCommand::OpenOrFocus(UiTile::Profiler(
                        ProfilerTile::default(),
                    )));
                }
                ScrollArea::vertical().show_rows(
                    ui,
                    ui.spacing().interact_size.y,
                    self.app.world.routes.len(),
                    |ui, range| {
                        for (route_key, info) in
                            &self.app.world.routes.skip(range.start).take(range.count())
                        {
                            ui.menu_button(info.name.as_str(), |ui| {
                                if ui.button("Graph").clicked() {
                                    self.app.ui_action_tx.send(UiCommand::OpenOrFocus(
                                        UiTile::Marey(MareyTile::new(*route_key)),
                                    ));
                                }
                                if ui.button("Route Timetable").clicked() {
                                    self.app.ui_action_tx.send(UiCommand::OpenOrFocus(
                                        UiTile::RouteTimetable(RouteTimetableTile::new(*route_key)),
                                    ));
                                }
                            });
                        }
                    },
                );
            })
        });
    }
}
