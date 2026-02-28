use std::borrow::Cow;

use bevy::ecs::world::World;
use egui::{Id, Response, Ui, WidgetText};
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;

// use paiagram_core::interface::StatusBarState;

pub mod classes;
// pub mod diagram;
pub mod diagram;
// pub mod displayed_lines;
pub mod graph;
pub mod inspector;
// pub mod minesweeper;
// pub mod overview;
// pub mod services;
pub mod all_trips;
pub mod priority_graph;
pub mod settings;
pub mod start;
pub mod trip;
// pub mod station_timetable;
// pub mod tree_view;
// pub mod vehicle;

pub mod all_tabs {
    pub use super::classes::ClassesTab;
    pub use super::diagram::DiagramTab;
    // pub use super::displayed_lines::DisplayedLinesTab;
    pub use super::graph::GraphTab;
    pub use super::inspector::InspectorTab;
    // pub use super::minesweeper::MinesweeperTab;
    // pub use super::overview::OverviewTab;
    // pub use super::services::ServicesTab;
    pub use super::all_trips::AllTripsTab;
    pub use super::priority_graph::PriorityGraphTab;
    pub use super::settings::SettingsTab;
    pub use super::start::StartTab;
    pub use super::trip::TripTab;
    // pub use super::station_timetable::StationTimetableTab;
    // pub use super::vehicle::VehicleTab;
}

pub trait Navigatable {
    type XOffset: Into<f64> + From<f64>;
    type YOffset: Into<f64> + From<f64>;
    fn zoom_x(&self) -> f32;
    fn zoom_y(&self) -> f32;
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32);
    fn offset_x(&self) -> f64;
    fn offset_y(&self) -> f64;
    fn set_offset(&mut self, offset_x: f64, offset_y: f64);
    fn screen_pos_to_xy(&self, pos: egui::Pos2) -> (Self::XOffset, Self::YOffset) {
        let rect = self.visible_rect();
        let x_per_screen_unit = self.x_per_screen_unit().into();
        let y_per_screen_unit = self.y_per_screen_unit().into();
        let x = self.offset_x() + (pos.x - rect.left()) as f64 * x_per_screen_unit;
        let y = self.offset_y() + (pos.y - rect.top()) as f64 * y_per_screen_unit;
        (x.into(), y.into())
    }
    fn xy_to_screen_pos(&self, x: Self::XOffset, y: Self::YOffset) -> egui::Pos2 {
        let rect = self.visible_rect();
        let x_per_screen_unit = self.x_per_screen_unit().into();
        let y_per_screen_unit = self.y_per_screen_unit().into();
        let x = x.into();
        let y = y.into();
        let screen_x = rect.left() + ((x - self.offset_x()) / x_per_screen_unit) as f32;
        let screen_y = rect.top() + ((y - self.offset_y()) / y_per_screen_unit) as f32;
        egui::Pos2::new(screen_x, screen_y)
    }
    fn visible_rect(&self) -> egui::Rect;
    fn visible_x(&self) -> std::ops::Range<Self::XOffset> {
        let width = self.visible_rect().width() as f64;
        let start = self.offset_x();
        let end = start + width * self.x_per_screen_unit().into();
        start.into()..end.into()
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let height = self.visible_rect().height() as f64;
        let start = self.offset_y();
        let end = start + height * self.y_per_screen_unit().into();
        start.into()..end.into()
    }
    fn x_per_screen_unit(&self) -> Self::XOffset {
        (1.0 / self.zoom_x().max(f32::EPSILON) as f64).into()
    }
    fn y_per_screen_unit(&self) -> Self::YOffset {
        (1.0 / self.zoom_y().max(f32::EPSILON) as f64).into()
    }
    fn allow_axis_zoom(&self) -> bool {
        false
    }
    /// Handles the navigation of the navigatable component.
    /// Returns true if there are any user input
    fn handle_navigation(&mut self, ui: &mut Ui, response: &Response) -> bool {
        let mut moved = response.dragged();
        let started_pos = ui
            .ctx()
            .input(|i| i.pointer.press_origin().or(i.pointer.hover_pos()));
        let zoom_delta = if self.allow_axis_zoom() {
            ui.input(|input| input.zoom_delta_2d())
        } else {
            let zoom_delta = ui.input(|input| input.zoom_delta());
            egui::vec2(zoom_delta, zoom_delta)
        };
        let scroll_delta = ui.input(|input| input.smooth_scroll_delta);
        let zooming = (zoom_delta.x - 1.0).abs() > 0.001 || (zoom_delta.y - 1.0).abs() > 0.001;

        if zooming
            && ui.ui_contains_pointer()
            && let Some(pos) = response.hover_pos()
        {
            moved |= zooming;
            let old_zoom_x = self.zoom_x();
            let old_zoom_y = self.zoom_y();
            let mut new_zoom_x = old_zoom_x * zoom_delta.x;
            let mut new_zoom_y = old_zoom_y * zoom_delta.y;
            let (clamped_x, clamped_y) = self.clamp_zoom(new_zoom_x, new_zoom_y);
            new_zoom_x = clamped_x;
            new_zoom_y = clamped_y;

            let rel_pos = (pos - response.rect.min) / response.rect.size();
            let world_width_before = response.rect.width() as f64 / old_zoom_x as f64;
            let world_width_after = response.rect.width() as f64 / new_zoom_x as f64;
            let world_pos_before_x = self.offset_x() + rel_pos.x as f64 * world_width_before;
            let new_offset_x = world_pos_before_x - rel_pos.x as f64 * world_width_after;

            let world_height_before = response.rect.height() as f64 / old_zoom_y as f64;
            let world_height_after = response.rect.height() as f64 / new_zoom_y as f64;
            let world_pos_before_y = self.offset_y() + rel_pos.y as f64 * world_height_before;
            let new_offset_y = world_pos_before_y - rel_pos.y as f64 * world_height_after;

            self.set_zoom(new_zoom_x, new_zoom_y);
            self.set_offset(new_offset_x, new_offset_y);
        }
        // if ui.ui_contains_pointer() || ui.input(|r| r.any_touches()) {
        if let Some(started_pos) = started_pos
            && response.rect.contains(started_pos)
        {
            let ticks_per_screen_unit = 1.0 / self.zoom_x() as f64;
            let pan_delta = response.drag_delta() + scroll_delta;
            let new_offset_x = self.offset_x() - ticks_per_screen_unit * pan_delta.x as f64;
            let new_offset_y = self.offset_y() - pan_delta.y as f64 / self.zoom_y() as f64;
            moved |= scroll_delta.x != 0.0;
            moved |= scroll_delta.y != 0.0;
            self.set_offset(new_offset_x, new_offset_y);
        }

        self.post_navigation(response);
        return moved;
    }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x, zoom_y)
    }
    fn post_navigation(&mut self, _response: &Response) {}
}

pub trait Tab: MapEntities {
    /// The internal name of the tab used for identification. This must be a static string.
    /// The actual displayed name could be different based on e.g. the localization or other contents.
    const NAME: &'static str;
    /// Called before rendering the tab.
    fn pre_render(&mut self, _world: &mut World) {}
    /// Called after rendering the tab.
    fn post_render(&mut self, _world: &mut World) {}
    /// The main display of the tab.
    fn main_display(&mut self, world: &mut World, ui: &mut Ui);
    /// The edit display
    fn edit_display(&mut self, _world: &mut World, ui: &mut Ui) {
        ui.label(Self::NAME);
        ui.label(tr!("side-panel-edit-fallback-1"));
        ui.label(tr!("side-panel-edit-fallback-2"));
    }
    /// The details display
    fn display_display(&mut self, _world: &mut World, ui: &mut Ui) {
        ui.label(Self::NAME);
        ui.label(tr!("side-panel-details-fallback-1"));
        ui.label(tr!("side-panel-details-fallback-2"));
    }
    /// The export display
    fn export_display(&mut self, _world: &mut World, ui: &mut Ui) {
        ui.label(Self::NAME);
        ui.label(tr!("side-panel-export-fallback-1"));
        ui.label(tr!("side-panel-export-fallback-2"));
    }
    /// The title of the tab
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
    /// What to do when clicking on the tab button
    fn on_tab_button(&self, _world: &mut World, _response: &Response) {
        // if response.hovered() {
        //     let title_text = self.title();
        //     let s = &mut world.resource_mut::<StatusBarState>().tooltip;
        //     s.clear();
        //     s.push_str(self.icon().as_ref());
        //     s.push(' ');
        //     s.push_str(title_text.text());
        // }
    }
    /// The id of the tab
    fn id(&self) -> Id {
        Id::new(Self::NAME)
    }
    /// Whether if the tab allows scrolling
    fn scroll_bars(&self) -> [bool; 2] {
        [true; 2]
    }
    /// The frame of the tab
    fn frame(&self) -> egui::Frame {
        egui::Frame::default().inner_margin(egui::Margin::same(6))
    }
    /// The icon of the tab
    fn icon(&self) -> Cow<'static, str> {
        "🖳".into()
    }
    /// The rendering order of the tab. Lower = higher priority
    fn rendering_order(&self) -> isize {
        0
    }
}
