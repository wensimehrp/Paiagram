use super::Tab;
use crate::graph::{Graph, Interval, Station};
use crate::lines::DisplayedLine;
use crate::rw_data::write::write_text_file;
use crate::vehicles::entries::{TimetableEntry, TimetableEntryCache, VehicleScheduleCache};
use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use egui::{Color32, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use egui_i18n::tr;
use either::Either::{Left, Right};
use emath::{self, RectTransform};
use moonshine_core::kind::{InsertInstanceWorld, Instance};
use petgraph::Direction;
use petgraph::dot;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

// TODO: display scale on ui.
// TODO: implement snapping and alignment guides when moving stations
#[derive(Clone, Serialize, Deserialize)]
pub struct GraphTab {
    zoom: f32,
    translation: Vec2,
    #[serde(skip)]
    selected_item: Option<SelectedItem>,
    #[serde(skip)]
    edit_mode: Option<EditMode>,
    animation_counter: f32,
    animation_playing: bool,
    iterations: u32,
    query_region_buffer: String,
}

#[derive(Debug, Clone, Copy)]
enum EditMode {
    EditDisplayedLine(Instance<DisplayedLine>),
}

impl MapEntities for EditMode {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        match self {
            EditMode::EditDisplayedLine(line) => line.map_entities(entity_mapper),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SelectedItem {
    Node(Instance<Station>),
    Edge(Instance<Interval>),
    DisplayedLine(Instance<DisplayedLine>),
}

impl MapEntities for SelectedItem {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        match self {
            SelectedItem::Node(node) => node.map_entities(entity_mapper),
            SelectedItem::Edge(edge) => edge.map_entities(entity_mapper),
            SelectedItem::DisplayedLine(line) => line.map_entities(entity_mapper),
        }
    }
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            translation: Vec2::ZERO,
            selected_item: None,
            edit_mode: None,
            animation_playing: false,
            animation_counter: 0.0,
            iterations: 3000,
            query_region_buffer: String::new(),
        }
    }
}

impl MapEntities for GraphTab {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some(selected_item) = &mut self.selected_item {
            selected_item.map_entities(entity_mapper);
        }
        if let Some(edit_mode) = &mut self.edit_mode {
            edit_mode.map_entities(entity_mapper);
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
        let show_spinner = false;
        ui.group(|ui| {
            ui.label(tr!("tab-graph-auto-arrange-desc"));
            ui.add(
                egui::Slider::new(&mut self.iterations, 100..=10000)
                    .text(tr!("tab-graph-auto-arrange-iterations")),
            );
            ui.horizontal(|ui| {
                if ui.button(tr!("tab-graph-auto-arrange")).clicked() {
                    if let Err(e) = world.run_system_cached_with(
                        crate::graph::arrange::auto_arrange_graph,
                        (ui.ctx().clone(), self.iterations),
                    ) {
                        error!("Error while auto-arranging graph: {}", e);
                    }
                }
                if show_spinner {
                    ui.add(egui::Spinner::new());
                };
            });
            ui.separator();
            ui.label(tr!("tab-graph-arrange-via-osm-desc"));
            ui.horizontal(|ui| {
                if ui.button(tr!("tab-graph-arrange-via-osm-terms")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl {
                        url: "https://osmfoundation.org/wiki/Terms_of_Use".into(),
                        new_tab: true,
                    });
                }
                if ui.button(tr!("tab-graph-arrange-via-osm")).clicked() {
                    if let Err(e) = world.run_system_cached_with(
                        crate::graph::arrange::arrange_via_osm,
                        (
                            ui.ctx().clone(),
                            if self.query_region_buffer.is_empty() {
                                None
                            } else {
                                Some(self.query_region_buffer.clone())
                            },
                        ),
                    ) {
                        error!("Error while arranging graph via OSM: {}", e);
                    }
                }
                // add a progress bar here
                if show_spinner {
                    ui.add(egui::Spinner::new());
                };
            });
            ui.horizontal(|ui| {
                ui.label(tr!("tab-graph-osm-area-name"));
                ui.text_edit_singleline(&mut self.query_region_buffer);
            })
        });
        ui.group(|ui| {
            ui.label(tr!("tab-graph-animation"));
            ui.label(tr!("tab-graph-animation-desc"));
            ui.horizontal(|ui| {
                if ui
                    .button(if self.animation_playing { "⏸" } else { "►" })
                    .clicked()
                {
                    self.animation_playing = !self.animation_playing;
                }
                if ui.button("⏮").clicked() {
                    self.animation_counter = 0.0;
                }
                ui.add(
                    egui::Slider::new(
                        &mut self.animation_counter,
                        (-86400.0 * 2.0)..=(86400.0 * 2.0),
                    )
                    .text("Time"),
                );
            })
        });
        match self.selected_item {
            None => {
                ui.group(|ui| {
                    ui.label(tr!("tab-graph-new-displayed-line-desc"));
                    if !ui.button(tr!("tab-graph-new-displayed-line")).clicked() {
                        return;
                    }
                    let new_displayed_line = world
                        .spawn((Name::new(tr!("new-displayed-line")),))
                        .insert_instance(DisplayedLine::new(vec![]))
                        .into();
                    self.edit_mode = Some(EditMode::EditDisplayedLine(new_displayed_line));
                    self.selected_item = Some(SelectedItem::DisplayedLine(new_displayed_line));
                });
            }
            Some(SelectedItem::DisplayedLine(e)) => {
                ui.group(|ui| {
                    if let Err(e) = world.run_system_cached_with(display_displayed_line, (ui, e)) {
                        bevy::log::error!("UI Error while displaying displayed line editor: {}", e)
                    }
                    if ui.button(tr!("done")).clicked() {
                        // check if the displayed line is empty
                        // if so, delete it
                        if let Ok((_, line)) = world
                            .query::<(&Name, &DisplayedLine)>()
                            .get(world, e.entity())
                        {
                            if line.stations().is_empty() {
                                world.entity_mut(e.entity()).despawn();
                            }
                        }
                        self.edit_mode = None;
                        self.selected_item = None;
                    }
                });
            }
            _ => {}
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-graph").into()
    }
    fn export_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        let mut buffer = String::with_capacity(32768);
        if ui.button("Export Graph as DOT file").clicked() {
            if let Err(e) = world.run_system_cached_with(make_dot_string, &mut buffer) {
                bevy::log::error!("Error while generating DOT string: {}", e);
                return;
            }

            let data = std::mem::take(&mut buffer);
            let filename = "transport_graph.dot".to_string();

            bevy::tasks::IoTaskPool::get()
                .spawn(async move {
                    if let Err(e) = write_text_file(data, filename).await {
                        bevy::log::error!("Failed to export graph: {:?}", e);
                    }
                })
                .detach();
        }
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

fn display_displayed_line(
    (InMut(ui), In(entity)): (InMut<Ui>, In<Instance<DisplayedLine>>),
    displayed_lines: Query<(&Name, &DisplayedLine)>,
    stations: Query<&Name, With<Station>>,
) {
    let Ok((name, line)) = displayed_lines.get(entity.entity()) else {
        return;
    };
    ui.heading(name.as_str());
    for (i, (station_entity, _)) in line.stations().iter().enumerate() {
        let Some(station_name) = stations.get(station_entity.entity()).ok() else {
            continue;
        };
        ui.horizontal(|ui| {
            ui.label(format!("{}.", i + 1));
            ui.label(station_name.as_str());
        });
    }
}

fn make_dot_string(InMut(buffer): InMut<String>, graph: Res<Graph>, names: Query<&Name>) {
    let get_node_attr = |_, (_, entity): (_, &Instance<Station>)| {
        format!(
            r#"label = "{}""#,
            names
                .get(entity.entity())
                .map_or("<Unknown>".to_string(), |name| name.to_string())
        )
    };
    let get_edge_attr = |_, _| String::new();
    let dot_string = dot::Dot::with_attr_getters(
        graph.inner(),
        &[dot::Config::EdgeNoLabel, dot::Config::NodeNoLabel],
        &get_edge_attr,
        &get_node_attr,
    );
    buffer.clear();
    buffer.push_str(&format!("{:?}", dot_string));
}

fn draw_line_spline(
    painter: &egui::Painter,
    to_screen: RectTransform,
    viewport: Rect,
    stations_list: &[(Instance<Station>, f32)],
    stations: &Query<(&Name, &Station)>,
) {
    let n = stations_list.len();
    if n < 2 {
        return;
    }

    // Find the range of visible stations to optimize rendering
    let mut first_visible = None;
    let mut last_visible = None;
    for (i, (entity, _)) in stations_list.iter().enumerate() {
        if let Ok((_, s)) = stations.get(entity.entity()) {
            if viewport.expand(100.0).contains(to_screen * s.0) {
                if first_visible.is_none() {
                    first_visible = Some(i);
                }
                last_visible = Some(i);
            }
        }
    }

    let (Some(start_idx), Some(end_idx)) = (first_visible, last_visible) else {
        return;
    };

    // Expand the range by 3 points on each side as requested
    let render_start = start_idx.saturating_sub(3);
    let render_end = (end_idx + 3).min(n - 1);

    let mut previous = stations
        .get(stations_list[render_start].0.entity())
        .map(|(_, s)| to_screen * s.0)
        .unwrap_or(Pos2::ZERO);

    for i in render_start..render_end {
        let p1_world = stations
            .get(stations_list[i].0.entity())
            .map(|(_, s)| s.0)
            .unwrap_or(Pos2::ZERO);
        let p2_world = stations
            .get(stations_list[i + 1].0.entity())
            .map(|(_, s)| s.0)
            .unwrap_or(Pos2::ZERO);

        let p0 = if i > 0 {
            to_screen
                * stations
                    .get(stations_list[i - 1].0.entity())
                    .map(|(_, s)| s.0)
                    .unwrap_or(Pos2::ZERO)
        } else {
            to_screen * p1_world
        };
        let p1 = to_screen * p1_world;
        let p2 = to_screen * p2_world;
        let p3 = if i + 2 < n {
            to_screen
                * stations
                    .get(stations_list[i + 2].0.entity())
                    .map(|(_, s)| s.0)
                    .unwrap_or(Pos2::ZERO)
        } else {
            p2
        };

        let num_samples =
            ((p3.distance(p2) + p2.distance(p1) + p1.distance(p0)) as usize / 20).max(1);

        let v0 = bevy::math::Vec2::new(p0.x, p0.y);
        let v1 = bevy::math::Vec2::new(p1.x, p1.y);
        let v2 = bevy::math::Vec2::new(p2.x, p2.y);
        let v3 = bevy::math::Vec2::new(p3.x, p3.y);

        for j in 1..=num_samples {
            let t = j as f32 / num_samples as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let pos_v = 0.5
                * ((2.0 * v1)
                    + (-v0 + v2) * t
                    + (2.0 * v0 - 5.0 * v1 + 4.0 * v2 - v3) * t2
                    + (-v0 + 3.0 * v1 - 3.0 * v2 + v3) * t3);
            let pos = Pos2::new(pos_v.x, pos_v.y);
            painter.line_segment([previous, pos], Stroke::new(4.0, Color32::LIGHT_YELLOW));
            previous = pos;
        }
    }
}

fn show_graph(
    (InMut(ui), mut state): (InMut<egui::Ui>, InMut<GraphTab>),
    graph: Res<Graph>,
    mut displayed_lines: Query<(Instance<DisplayedLine>, &mut DisplayedLine)>,
    mut stations: Query<(&Name, &mut Station)>,
    schedules: Query<&VehicleScheduleCache>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    time: Res<Time>,
) {
    if state.animation_playing {
        state.animation_counter += time.delta_secs() * 10.0;
        ui.ctx().request_repaint();
    }
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
        if response.clicked() && !state.edit_mode.is_some() {
            state.selected_item = None;
        }
        let to_screen = RectTransform::from_to(world_rect, response.rect);
        draw_world_grid(&painter, response.rect, state.translation, state.zoom);
        // draw edges
        for (from, to, _weight) in graph.inner().node_indices().flat_map(|n| {
            graph
                .inner()
                .edges_directed(n, Direction::Outgoing)
                .map(|a| {
                    (
                        graph.entity(a.source()).unwrap(),
                        graph.entity(a.target()).unwrap(),
                        a.weight(),
                    )
                })
        }) {
            let Ok((_, from_station)) = stations.get(from.entity()) else {
                continue;
            };
            let Ok((_, to_station)) = stations.get(to.entity()) else {
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
        for node in graph
            .inner()
            .node_indices()
            .map(|n| graph.entity(n).unwrap())
        {
            let Ok((name, mut station)) = stations.get_mut(node.entity()) else {
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
                    let (_rect, resp) =
                        ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                    let fill = if resp.hovered() {
                        Color32::YELLOW
                    } else {
                        Color32::LIGHT_GREEN
                    };
                    match (state.edit_mode, resp.clicked()) {
                        (_, false) => {}
                        (None, true) => {
                            state.selected_item = Some(SelectedItem::Node(node));
                        }
                        (Some(EditMode::EditDisplayedLine(e)), true) => {
                            if let Ok((_, mut line)) = displayed_lines.get_mut(e.entity()) {
                                if let Err(e) = line.push((node, 0.0)) {
                                    error!("Failed to add station to line: {:?}", e);
                                }
                            }
                        }
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

        let stations_readonly = stations.as_readonly();
        displayed_lines
            .as_readonly()
            .par_iter()
            .for_each(|(_line_entity, line)| {
                draw_line_spline(
                    &painter,
                    to_screen,
                    response.rect,
                    line.stations(),
                    &stations_readonly,
                );
            });

        if state.animation_playing {
            for section in schedules.iter().filter_map(|s| {
                s.position(state.animation_counter, |e| timetable_entries.get(e).ok())
            }) {
                match section {
                    Left((from_entity, to_entity, progress)) => {
                        let Ok((_, from_station)) = stations.get(from_entity) else {
                            continue;
                        };
                        let Ok((_, to_station)) = stations.get(to_entity) else {
                            continue;
                        };
                        let from_pos = to_screen * from_station.0;
                        let to_pos = to_screen * to_station.0;
                        // shift the from and to positions to its left by EDGE_OFFSET pixels
                        let direction = (to_pos - from_pos).normalized();
                        let angle = direction.y.atan2(direction.x) + std::f32::consts::FRAC_PI_2;
                        let offset = Vec2::new(angle.cos(), angle.sin()) * EDGE_OFFSET;
                        let from_pos = from_pos + offset;
                        let to_pos = to_pos + offset;
                        painter.circle_filled(
                            from_pos.lerp(to_pos, progress),
                            6.0,
                            Color32::from_rgb(100, 200, 100),
                        );
                    }
                    Right(_station_pos) => {}
                };
            }
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
            let zoom = state.zoom;
            state.translation -= (translation_delta + response.drag_delta()) / zoom;
        }
    });
}

fn draw_world_grid(painter: &Painter, viewport: Rect, offset: Vec2, zoom: f32) {
    if zoom <= 0.0 {
        return;
    }

    // Transitions like diagram.rs: Linear fade between MIN and MAX screen spacing
    const MIN_WIDTH: f32 = 32.0;
    const MAX_WIDTH: f32 = 120.0;

    // Use a neutral gray without querying visuals
    let base_color = Color32::from_gray(160);

    for p in ((-5)..=5).rev() {
        let spacing = 10.0f32.powi(p);
        let screen_spacing = spacing * zoom;

        // Strength calculation identical to diagram.rs (1.5 scaling factor)
        let strength =
            ((screen_spacing * 1.5 - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)).clamp(0.0, 1.0);
        if strength <= 0.0 {
            continue;
        }

        let stroke = Stroke::new(0.6, base_color.gamma_multiply(strength));

        // Vertical lines
        let mut n = (offset.x / spacing).floor();
        loop {
            let world_x = n * spacing;
            let screen_x_rel = (world_x - offset.x) * zoom;
            if screen_x_rel > viewport.width() {
                break;
            }
            if screen_x_rel >= 0.0 {
                painter.vline(viewport.left() + screen_x_rel, viewport.y_range(), stroke);
            }
            n += 1.0;
        }

        // Horizontal lines
        let mut m = (offset.y / spacing).floor();
        loop {
            let world_y = m * spacing;
            let screen_y_rel = (world_y - offset.y) * zoom;
            if screen_y_rel > viewport.height() {
                break;
            }
            if screen_y_rel >= 0.0 {
                painter.hline(viewport.x_range(), viewport.top() + screen_y_rel, stroke);
            }
            m += 1.0;
        }
    }
}
