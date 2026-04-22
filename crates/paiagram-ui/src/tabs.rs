use bevy::ecs::entity::MapEntities;
use bevy::ecs::world::World;
use egui::emath;
use egui::{Id, Key, NumExt, Response, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use std::borrow::Cow;

pub(crate) mod classes;
pub(crate) mod diagram;
pub(crate) mod graph;
pub(crate) mod inspector;
pub(crate) mod priority_graph;
pub(crate) mod route_timetable;
pub(crate) mod settings;
pub(crate) mod start;
pub(crate) mod station;
pub(crate) mod text;
pub(crate) mod trip;

pub mod all_tabs {
    pub(crate) use super::classes::ClassesTab;
    pub(crate) use super::diagram::DiagramTab;
    pub(crate) use super::graph::GraphTab;
    pub(crate) use super::inspector::InspectorTab;
    pub(crate) use super::priority_graph::PriorityGraphTab;
    pub(crate) use super::route_timetable::RouteTimetableTab;
    pub(crate) use super::settings::SettingsTab;
    pub(crate) use super::start::StartTab;
    pub(crate) use super::station::StationTab;
    pub(crate) use super::text::TextTab;
    pub(crate) use super::trip::TripTab;
}

fn handle_keyboard_navigation(ui: &Ui) -> Vec2 {
    const PAN_SPEED: f32 = 500.0;
    let key_pan_delta = ui.ctx().input(|input| {
        let mut delta = Vec2::ZERO;
        if input.key_down(Key::ArrowUp) {
            delta.y += PAN_SPEED;
        }
        if input.key_down(Key::ArrowDown) {
            delta.y -= PAN_SPEED;
        }
        if input.key_down(Key::ArrowLeft) {
            delta.x += PAN_SPEED;
        }
        if input.key_down(Key::ArrowRight) {
            delta.x -= PAN_SPEED;
        }
        delta.x *= input.stable_dt;
        delta.y *= input.stable_dt;
        delta
    });
    let dt = ui.ctx().input(|input| input.stable_dt).at_most(0.1);
    let mut requires_repaint = false;
    let smoothed_delta = ui.ctx().data_mut(|data| {
        let smoothed_delta: &mut Vec2 =
            data.get_temp_mut_or(ui.id().with("keyboard pan info"), vec2(0.0, 0.0));
        let t = emath::exponential_smooth_factor(0.9, 0.3, dt);
        *smoothed_delta = emath::lerp(*smoothed_delta..=key_pan_delta, t);
        let diff = (*smoothed_delta - key_pan_delta).length();
        if diff < 0.01 {
            *smoothed_delta = key_pan_delta
        } else {
            requires_repaint = true
        }
        *smoothed_delta
    });
    if requires_repaint {
        ui.ctx().request_repaint();
    }
    smoothed_delta
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
    fn screen_x_to_logical_x(&self, screen_x: f32) -> Self::XOffset {
        let rect = self.visible_rect();
        let x = self.offset_x() + (screen_x - rect.left()) as f64 * self.x_per_screen_unit_f64();
        x.into()
    }
    fn logical_x_to_screen_x(&self, logical_x: Self::XOffset) -> f32 {
        let rect = self.visible_rect();
        let logical_x = logical_x.into();
        rect.left() + ((logical_x - self.offset_x()) / self.x_per_screen_unit_f64()) as f32
    }
    fn screen_y_to_logical_y(&self, screen_y: f32) -> Self::YOffset {
        let rect = self.visible_rect();
        let y = self.offset_y() + (screen_y - rect.top()) as f64 * self.y_per_screen_unit_f64();
        y.into()
    }
    fn logical_y_to_screen_y(&self, logical_y: Self::YOffset) -> f32 {
        let rect = self.visible_rect();
        let logical_y = logical_y.into();
        rect.top() + ((logical_y - self.offset_y()) / self.y_per_screen_unit_f64()) as f32
    }
    fn screen_pos_to_xy(&self, pos: egui::Pos2) -> (Self::XOffset, Self::YOffset) {
        (
            self.screen_x_to_logical_x(pos.x),
            self.screen_y_to_logical_y(pos.y),
        )
    }
    fn xy_to_screen_pos(&self, x: Self::XOffset, y: Self::YOffset) -> egui::Pos2 {
        let screen_x = self.logical_x_to_screen_x(x);
        let screen_y = self.logical_y_to_screen_y(y);
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
    fn x_per_screen_unit_f64(&self) -> f64 {
        1.0 / self.zoom_x().max(f32::EPSILON) as f64
    }
    fn y_per_screen_unit_f64(&self) -> f64 {
        1.0 / self.zoom_y().max(f32::EPSILON) as f64
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
            egui::Vec2::splat(ui.input(|input| input.zoom_delta()))
        };
        let pan_delta = handle_keyboard_navigation(ui)
            + response.drag_delta()
            + ui.input(|input| input.smooth_scroll_delta());
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
            let new_offset_x = self.offset_x() - ticks_per_screen_unit * pan_delta.x as f64;
            let new_offset_y = self.offset_y() - pan_delta.y as f64 / self.zoom_y() as f64;
            moved |= pan_delta.x.abs() >= 0.01;
            moved |= pan_delta.y.abs() >= 0.01;
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
}
