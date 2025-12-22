use bevy::ecs::world::World;
use egui::{Id, Response, Ui, WidgetText};

use crate::interface::StatusBarState;

pub mod classes;
pub mod diagram;
pub mod displayed_lines;
pub mod minesweeper;
pub mod services;
pub mod settings;
pub mod start;
pub mod station_timetable;
pub mod tree_view;
pub mod vehicle;

pub mod all_tabs {
    pub use super::classes::ClassesTab;
    pub use super::diagram::DiagramTab;
    pub use super::displayed_lines::DisplayedLinesTab;
    pub use super::minesweeper::MinesweeperTab;
    pub use super::services::ServicesTab;
    pub use super::settings::SettingsTab;
    pub use super::start::StartTab;
    pub use super::station_timetable::StationTimetableTab;
    pub use super::vehicle::VehicleTab;
}

/// The page cache. Lots of r/w, few insertions, good locality, fast executions.
#[derive(Debug)]
pub struct PageCache<K, V>
where
    K: PartialEq + Ord,
{
    keys: Vec<K>,
    vals: Vec<V>,
}

const LINEAR_THRESHOLD: usize = 64;

impl<K, V> PageCache<K, V>
where
    K: PartialEq + Ord,
{
    /// Get the page cache or insert with a custom value.
    pub fn get_mut_or_insert_with<F>(&mut self, query_key: K, make_val: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        if self.keys.len() < LINEAR_THRESHOLD
            && let Some(idx) = self.keys.iter().position(|e| *e == query_key)
        {
            return &mut self.vals[idx];
        }
        return match self.keys.binary_search(&query_key) {
            Ok(idx) => &mut self.vals[idx],
            Err(idx) => {
                self.keys.insert(idx, query_key);
                self.vals.insert(idx, make_val());
                return &mut self.vals[idx];
            }
        };
    }
}

impl<K, V> Default for PageCache<K, V>
where
    K: PartialEq + Ord,
{
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            vals: Vec::new(),
        }
    }
}

pub trait Tab {
    const ICON: &'static str = "🖳";
    const NAME: &'static str;
    fn main_display(&mut self, world: &mut World, ui: &mut Ui);
    fn edit_display(&mut self, _world: &mut World, ui: &mut Ui) {
        ui.label(Self::NAME);
        ui.label("This tab hasn't implemented Edit display yet.");
        ui.label("This is considered a bug. Feel free to open a ticket on GitHub!");
    }
    fn display_display(&mut self, _world: &mut World, ui: &mut Ui) {
        ui.label(Self::NAME);
        ui.label("This tab hasn't implemented Details display yet.");
        ui.label("This is considered a bug. Feel free to open a ticket on GitHub!");
    }
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
    fn on_tab_button(&self, world: &mut World, response: &Response) {
        if response.hovered() {
            let title_text = self.title();
            let s = &mut world.resource_mut::<StatusBarState>().tooltip;
            s.clear();
            s.push_str(Self::ICON);
            s.push(' ');
            s.push_str(title_text.text());
        }
    }
    fn id(&self) -> Id {
        Id::new(Self::NAME)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [true; 2]
    }
}
