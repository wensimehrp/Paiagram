use super::Tab;
use bevy::{
    ecs::{name::NameOrEntityItem, query::QueryIter},
    prelude::*,
};
use bevy_inspector_egui::bevy_inspector::ui_for_entity_with_children;
use egui_table::{Column, Table, TableDelegate};
use moonshine_core::prelude::MapEntities;
use paiagram_core::entry::EntryMode;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Copy, PartialEq)]
enum SelectedItem {
    #[default]
    None,
    Entity(Entity),
}

struct EntityTable<'w> {
    name_or_entity_iter: QueryIter<'w, 'w, NameOrEntity, Without<EntryMode>>,
    names_or_entities: Vec<NameOrEntityItem<'w, 'w>>,
    selected: &'w mut SelectedItem,
    start: usize,
}

impl<'w> TableDelegate for EntityTable<'w> {
    fn header_cell_ui(&mut self, _ui: &mut egui::Ui, _cell: &egui_table::HeaderCellInfo) {}
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        let start = info.visible_rows.start as usize;
        let end = info.visible_rows.end as usize;
        self.start = start;
        let count = end.saturating_sub(start).saturating_add(1);
        self.names_or_entities
            .extend(self.name_or_entity_iter.clone().skip(start).take(count));
    }
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let info = &self.names_or_entities[cell.row_nr as usize - self.start];
        ui.selectable_value(
            self.selected,
            SelectedItem::Entity(info.entity),
            if let Some(n) = info.name {
                format!("{} ({})", n, info.entity)
            } else {
                info.entity.to_string()
            },
        );
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct InspectorTab {
    #[serde(skip, default)]
    selected: SelectedItem,
    query: String,
}

impl PartialEq for InspectorTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl MapEntities for InspectorTab {
    fn map_entities<E: EntityMapper>(&mut self, _entity_mapper: &mut E) {}
}

impl Tab for InspectorTab {
    const NAME: &'static str = "Inspector";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        egui::SidePanel::new(egui::panel::Side::Right, ui.id().with("right panel"))
            .exact_width(available_width / 2.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match self.selected {
                    SelectedItem::None => {
                        ui.label("Nothing selected");
                    }
                    SelectedItem::Entity(entity) => {
                        ui_for_entity_with_children(world, entity, ui);
                    }
                })
            });
        let mut binding = world.query_filtered::<NameOrEntity, Without<EntryMode>>();
        let iter = binding.query(world);
        let row_count = iter.count() as u64;
        let res = ui.text_edit_singleline(&mut self.query);
        if res.changed() {
            // TODO: query
        }
        Table::new()
            .num_rows(row_count)
            .columns([Column::new(available_width / 2.0).resizable(false)])
            .show(
                ui,
                &mut EntityTable {
                    name_or_entity_iter: iter.iter(),
                    names_or_entities: Vec::new(),
                    selected: &mut self.selected,
                    start: 0,
                },
            );
    }
}
