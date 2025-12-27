use super::Tab;
use crate::intervals::{Graph, IntervalGraphType, Station};
use crate::rw_data::write::write_text_file;
use bevy::prelude::*;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use egui_i18n::tr;
use emath::{self, RectTransform};
use petgraph::Direction::Outgoing;
use petgraph::dot;
use petgraph::visit::EdgeRef;
use visgraph::Orientation::TopToBottom;
use visgraph::layout::hierarchical::hierarchical_layout;

#[derive(Debug, Clone, Copy)]
pub struct GraphTab {
    zoom: f32,
    translation: Vec2,
    selected_item: Option<SelectedItem>,
}

#[derive(Debug, Clone, Copy)]
enum SelectedItem {
    Node(Entity),
    Edge(Entity),
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            translation: Vec2::ZERO,
            selected_item: None,
        }
    }
}

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        if let Err(e) = world.run_system_cached_with(show_graph, (ui, self)) {
            bevy::log::error!("UI Error while displaying graph page: {}", e)
        }
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        if ui.button("Auto-arrange Graph").clicked() {
            if let Err(e) = world.run_system_cached(auto_arrange_graph) {
                error!("Error while auto-arranging graph: {}", e);
            }
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-graph").into()
    }
    fn export_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        let mut buffer = String::with_capacity(4096);
        if ui.button("Export Graph as DOT file").clicked() {
            if let Err(e) = world.run_system_cached_with(make_dot_string, &mut buffer) {
                bevy::log::error!("Error while generating DOT string: {}", e);
                return;
            }
            if let Err(e) = write_text_file(&buffer, "transport_graph.dot") {
                bevy::log::error!("Failed to export graph: {:?}", e);
            }
        }
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

fn auto_arrange_graph(graph: Res<Graph>, mut stations: Query<&mut Station>) {
    const SHIFT_FACTOR: f32 = 500.0;
    let inner = &graph.inner;
    let layout = hierarchical_layout(&inner, TopToBottom);
    for node in inner.node_indices() {
        let pos = layout(node);
        if let Ok(mut station) = stations.get_mut(graph.entity(node).unwrap()) {
            station.0 = Pos2::new(pos.0 * SHIFT_FACTOR, pos.1 * SHIFT_FACTOR);
        }
    }
}

fn make_dot_string(InMut(buffer): InMut<String>, graph: Res<Graph>, names: Query<&Name>) {
    let get_node_attr = |_, (_, entity): (_, &Entity)| {
        format!(
            r#"label = "{}""#,
            names
                .get(*entity)
                .map_or("<Unknown>".to_string(), |name| name.to_string())
        )
    };
    let get_edge_attr = |_, _| String::new();
    let dot_string = dot::Dot::with_attr_getters(
        &graph.inner,
        &[dot::Config::EdgeNoLabel, dot::Config::NodeNoLabel],
        &get_edge_attr,
        &get_node_attr,
    );
    buffer.clear();
    buffer.push_str(&format!("{:?}", dot_string));
}

fn show_graph(
    (InMut(ui), mut state): (InMut<egui::Ui>, InMut<GraphTab>),
    graph: Res<Graph>,
    mut stations: Query<(&Name, &mut Station)>,
) {
    const EDGE_OFFSET: f32 = 10.0;
    let selected_strength = ui.ctx().animate_bool(
        ui.id().with("background animation"),
        state.selected_item.is_some(),
    );
    let selected_strength_ease = ui.ctx().animate_bool_with_time_and_easing(
        ui.id().with("selected item animation"),
        state.selected_item.is_some(),
        0.2,
        emath::easing::quadratic_out,
    );
    let mut focused_pos: Option<(Pos2, Pos2)> = None;
    // Iterate over the graph and see what's in it
    egui::Frame::canvas(&ui.style()).show(ui, |ui| {
        // Draw lines between stations with shifted positions
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        let world_rect = Rect::from_min_size(
            Pos2::new(state.translation.x, state.translation.y),
            Vec2::new(
                response.rect.width() / state.zoom,
                response.rect.height() / state.zoom,
            ),
        );
        if response.clicked() {
            state.selected_item = None;
        }
        let to_screen = RectTransform::from_to(world_rect, response.rect);
        // draw edges
        for (from, to, weight) in graph.inner.node_indices().flat_map(|n| {
            graph.inner.edges_directed(n, Outgoing).map(|a| {
                (
                    graph.entity(a.source()).unwrap(),
                    graph.entity(a.target()).unwrap(),
                    a.weight(),
                )
            })
        }) {
            let Ok((_, from_station)) = stations.get(from) else {
                continue;
            };
            let Ok((_, to_station)) = stations.get(to) else {
                continue;
            };
            let from = from_station.0;
            let to = to_station.0;
            // shift the two points to its left by EDGE_OFFSET pixels
            let direction = (to - from).normalized();
            let angle = direction.y.atan2(direction.x) + std::f32::consts::FRAC_PI_2;
            let offset = Vec2::new(angle.cos(), angle.sin()) * EDGE_OFFSET / state.zoom;
            let from = from + offset;
            let to = to + offset;
            painter.line_segment(
                [to_screen * from, to_screen * to],
                Stroke::new(1.0, Color32::LIGHT_BLUE),
            );
        }
        // draw nodes after edges
        for node in graph.inner.node_indices().map(|n| graph.entity(n).unwrap()) {
            let Ok((name, mut station)) = stations.get_mut(node) else {
                continue;
            };
            let pos = &mut station.0;
            let galley = painter.layout_no_wrap(
                name.to_string(),
                egui::FontId::proportional(13.0),
                ui.visuals().text_color(),
            );
            painter.galley(
                {
                    let pos = to_screen * *pos;
                    let offset = Vec2::new(15.0, -galley.size().y / 2.0);
                    pos + offset
                },
                galley,
                ui.visuals().text_color(),
            );
            ui.place(
                Rect::from_pos(to_screen * *pos).expand(10.0),
                |ui: &mut Ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                    let fill = if resp.hovered() {
                        Color32::YELLOW
                    } else {
                        Color32::LIGHT_GREEN
                    };
                    if resp.clicked() {
                        state.selected_item = Some(SelectedItem::Node(node));
                    }
                    if matches!(state.selected_item, Some(SelectedItem::Node(n)) if n == node) {
                        focused_pos = Some((*pos, Pos2::ZERO));
                    }
                    ui.painter().circle_filled(to_screen * *pos, 10.0, fill);
                    if resp.dragged() {
                        *pos += resp.drag_delta() / state.zoom;
                    }
                    resp
                },
            );
        }
        painter.rect_filled(response.rect, 0, {
            let amt = (selected_strength * 180.0) as u8;
            if ui.ctx().theme().default_visuals().dark_mode {
                Color32::from_black_alpha(amt)
            } else {
                Color32::from_white_alpha(amt)
            }
        });
        if let (Some(SelectedItem::Node(_)), Some((station_pos, _))) =
            (state.selected_item, focused_pos)
        {
            painter.circle(
                to_screen * station_pos,
                12.0 + 10.0 * (1.0 - selected_strength_ease),
                Color32::RED.gamma_multiply(0.5 * selected_strength_ease),
                Stroke::new(2.0, Color32::RED.gamma_multiply(selected_strength_ease)),
            );
            painter.circle_filled(to_screen * station_pos, 10.0, Color32::LIGHT_RED);
        }
        // handle zooming and panning
        let mut zoom_delta: f32 = 1.0;
        let mut translation_delta: Vec2 = Vec2::default();
        ui.input(|input| {
            zoom_delta = input.zoom_delta();
            translation_delta = input.translation_delta();
        });
        if let Some(pos) = response.hover_pos() {
            let old_zoom = state.zoom;
            let new_zoom = state.zoom * zoom_delta;
            let rel_pos = (pos - response.rect.min) / response.rect.size();

            let world_width_before = response.rect.width() / old_zoom;
            let world_width_after = response.rect.width() / new_zoom;
            let world_pos_before_x = state.translation.x + rel_pos.x * world_width_before;
            let new_translation_x = world_pos_before_x - rel_pos.x * world_width_after;

            let world_height_before = response.rect.height() / old_zoom;
            let world_height_after = response.rect.height() / new_zoom;
            let world_pos_before_y = state.translation.y + rel_pos.y * world_height_before;
            let new_translation_y = world_pos_before_y - rel_pos.y * world_height_after;

            state.zoom = new_zoom;
            state.translation = Vec2::new(new_translation_x, new_translation_y);
        }
        let zoom = state.zoom;
        state.translation -= (translation_delta + response.drag_delta()) / zoom;
    });
}
