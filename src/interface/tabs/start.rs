use crate::interface::tabs::tree_view;

use super::Tab;
use bevy::ecs::system::InMut;
use bevy::log::prelude::*;
use egui::{Frame, Response, ScrollArea, Sense, Ui, UiBuilder, Vec2};
const CARD_WIDTH: f32 = 150.0;
const CARD_SIZE: Vec2 = Vec2 {
    x: CARD_WIDTH,
    y: CARD_WIDTH / 2.0 * 3.0,
};
const CARD_SPACING: f32 = 20.0;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct StartTab;

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_start, ui) {
            error!("UI Error while displaying start page: {e}")
        }
    }
    fn edit_display(&self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(tree_view::show_tree_view, ui) {
            error!("UI Errorr while displaying tree view: {e}")
        }
    }
}

fn show_start(InMut(ui): InMut<Ui>) {
    // show a bunch of 3:2 rectangles
    let max_width = ui.available_width();
    ui.set_max_width(max_width);
    ui.style_mut().spacing.item_spacing = Vec2::ZERO;
    ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                diagram_card(ui, |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.label("NEW DIAGRAM");
                    });
                });
            });
            ui.add_space(CARD_SPACING);
            for _ in 1..=(max_width / (CARD_SIZE.x + CARD_SPACING)) as usize {
                diagram_card(ui, |_| {});
                ui.add_space(CARD_SPACING);
            }
        });
    });
}

fn diagram_card<R, F>(ui: &mut Ui, content: F) -> Response
where
    F: FnOnce(&mut Ui) -> R,
{
    let (rect, resp) = ui.allocate_exact_size(CARD_SIZE, Sense::click());
    ui.scope_builder(UiBuilder::new().sense(resp.sense).max_rect(rect), |ui| {
        let response = ui.response();
        let visuals = ui.style().interact(&response);
        let mut stroke = visuals.bg_stroke;
        stroke.width = 1.5;
        Frame::canvas(ui.style())
            .fill(visuals.bg_fill.gamma_multiply(0.4))
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.allocate_ui(ui.available_size(), content)
            });
    })
    .response
}
