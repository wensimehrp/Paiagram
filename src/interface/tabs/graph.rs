use super::Tab;
use crate::intervals::{Graph, Interval, Station};
use crate::lines::DisplayedLine;
use crate::rw_data::write::write_text_file;
use crate::vehicles::entries::{TimetableEntry, TimetableEntryCache, VehicleScheduleCache};
use bevy::ecs::entity::EntityHashSet;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use egui::{Color32, Context, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use egui_i18n::tr;
use either::Either::{Left, Right};
use emath::{self, RectTransform};
use moonshine_core::kind::{InsertInstanceWorld, Instance};
use petgraph::Direction;
use petgraph::dot;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use visgraph::layout::force_directed::force_directed_layout;

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

#[derive(Debug, Clone, Copy)]
enum SelectedItem {
    Node(Instance<Station>),
    Edge(Instance<Interval>),
    DisplayedLine(Instance<DisplayedLine>),
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

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        if let Err(e) = world.run_system_cached_with(show_graph, (ui, self)) {
            bevy::log::error!("UI Error while displaying graph page: {}", e)
        }
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        let show_spinner = world
            .query::<&GraphLayoutTask>()
            .iter(world)
            .next()
            .is_some();
        ui.group(|ui| {
            ui.label(tr!("tab-graph-auto-arrange-desc"));
            ui.add(
                egui::Slider::new(&mut self.iterations, 100..=10000)
                    .text(tr!("tab-graph-auto-arrange-iterations")),
            );
            ui.horizontal(|ui| {
                if ui.button(tr!("tab-graph-auto-arrange")).clicked() {
                    if let Err(e) = world.run_system_cached_with(
                        auto_arrange_graph,
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
                        arrange_via_osm,
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
            if let Err(e) = write_text_file(&buffer, "transport_graph.dot") {
                bevy::log::error!("Failed to export graph: {:?}", e);
            }
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

#[derive(Component)]
pub struct GraphLayoutTask(pub Task<(Vec<(Instance<Station>, Pos2)>, Vec<Instance<Station>>)>);

pub fn apply_graph_layout(
    mut commands: Commands,
    mut tasks: Populated<(Entity, &mut GraphLayoutTask)>,
    mut stations: Query<(NameOrEntity, &mut Station)>,
    graph: Res<Graph>,
) {
    for (entity, mut task) in &mut tasks {
        let Some((found, not_found)) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task.0))
        else {
            continue;
        };
        for (station_instance, pos) in found {
            if let Ok((_, mut station)) = stations.get_mut(station_instance.entity()) {
                station.0 = pos;
            }
        }
        let not_found_entities: EntityHashSet = not_found.iter().map(|s| s.entity()).collect();
        // find the connecting edges for stations that were not found
        // then find the average position of their connected stations
        // then assign that position to the not found station
        for station_instance in not_found.iter().copied() {
            info!(
                "Station {:?} is not found in database, arranging via neighbors",
                stations
                    .get(station_instance.entity())
                    .map_or("<Unknown>".to_string(), |(name, _)| name.to_string())
            );
            let Some(node_index) = graph.node_index(station_instance) else {
                error!("Station {:?} not found in graph", station_instance);
                continue;
            };

            let mut valid_neighbor_positions = Vec::new();
            let mut visited = HashSet::new();
            visited.insert(node_index);

            let mut queue = VecDeque::new();
            queue.push_back(node_index);

            while let Some(current_node) = queue.pop_front() {
                for neighbor_index in graph.inner().neighbors_undirected(current_node) {
                    if !visited.insert(neighbor_index) {
                        continue;
                    }
                    let Some(stn_instance) = graph.entity(neighbor_index) else {
                        continue;
                    };
                    if not_found_entities.contains(&stn_instance.entity()) {
                        queue.push_back(neighbor_index);
                    } else if let Ok((_, stn)) = stations.get(stn_instance.entity()) {
                        valid_neighbor_positions.push(stn.0);
                    }
                }
            }

            let average_pos = if valid_neighbor_positions.is_empty() {
                Pos2::new(0.0, 0.0)
            } else {
                let sum_x: f32 = valid_neighbor_positions.iter().map(|p| p.x).sum();
                let sum_y: f32 = valid_neighbor_positions.iter().map(|p| p.y).sum();
                Pos2::new(
                    sum_x / valid_neighbor_positions.len() as f32,
                    sum_y / valid_neighbor_positions.len() as f32,
                )
            };
            if let Ok((_, mut station)) = stations.get_mut(station_instance.entity()) {
                station.0 = average_pos;
            }
        }
        commands.entity(entity).despawn();
        info!("Finished applying graph layout");
    }
}

// TODO: move layout algorithms to a separate module
fn auto_arrange_graph(
    (In(ctx), In(iterations)): (In<Context>, In<u32>),
    mut commands: Commands,
    graph: Res<Graph>,
) {
    let inner = graph.inner().clone();
    let thread_pool = AsyncComputeTaskPool::get();
    let task = thread_pool.spawn(async move {
        let graph_ref = &inner;
        let layout = force_directed_layout(&graph_ref, iterations, 0.1);
        let results = inner
            .node_indices()
            .map(|node| {
                let pos = layout(node);
                (inner[node], Pos2::new(pos.0 * 500.0, pos.1 * 500.0))
            })
            .collect::<Vec<_>>();
        ctx.request_repaint();
        // No stations are "not found" in this method
        (results, Vec::new())
    });
    commands.spawn(GraphLayoutTask(task));
}

#[derive(Deserialize)]
struct OSMResponse {
    elements: Vec<OSMElement>,
}

impl OSMResponse {
    fn get_element_by_name(&self, name: &str) -> Option<&OSMElement> {
        // find the element with the closest matching name
        let mut best_match: Option<(&OSMElement, f64, &str)> = None;

        for element in &self.elements {
            for (k, v) in &element.tags {
                if !k.starts_with("name") {
                    continue;
                }

                if v == name {
                    return Some(element);
                }

                let score = strsim::jaro_winkler(name, v);
                if score > 0.9 {
                    if best_match.as_ref().map_or(true, |&(_, s, _)| score > s) {
                        best_match = Some((element, score, v));
                    }
                }
            }
        }

        if let Some((element, score, matched_name)) = best_match {
            info!(
                "Fuzzy matched '{}' to '{:?}' (score: {:.2})",
                name, matched_name, score
            );
            Some(element)
        } else {
            None
        }
    }
}

#[derive(Deserialize)]
struct OSMElement {
    lat: f64,
    lon: f64,
    tags: HashMap<String, String>,
}

impl OSMElement {
    fn to_pos2(&self) -> Pos2 {
        // Web Mercator projection (EPSG:3857)
        // This preserves angles and local shapes, making the map look "natural".
        let lat_rad = self.lat.to_radians();
        let lon_rad = self.lon.to_radians();

        const EARTH_RADIUS: f64 = 6378137.0;

        let x = EARTH_RADIUS * lon_rad;
        let y = EARTH_RADIUS * ((lat_rad / 2.0) + (std::f64::consts::PI / 4.0)).tan().ln();

        // In Egui, Y increases downwards. Mapping North to smaller Y (Up)
        // and East to larger X (Right).
        Pos2::new(x as f32, -y as f32)
    }
}

// TODO: move all OSM reading related stuff into a separate module
fn arrange_via_osm(
    (In(ctx), In(area_name)): (In<Context>, In<Option<String>>),
    mut commands: Commands,
    station_names: Query<(Instance<Station>, &Name)>,
) {
    const MAX_RETRY_COUNT: usize = 3;
    info!("Arranging graph via OSM with parameters...");
    info!(?area_name);
    let mut task_queue: VecDeque<(_, usize)> = station_names
        .iter()
        .map(|(instance, name)| (instance, name.to_string()))
        .collect::<Vec<_>>()
        .chunks(50)
        .map(|chunk| (chunk.to_vec(), 0))
        .collect();
    let thread_pool = AsyncComputeTaskPool::get();

    let task = thread_pool.spawn(async move {
        let mut found: Vec<(Instance<Station>, Pos2)> = Vec::new();
        let mut not_found: Vec<Instance<Station>> = Vec::new();
        while let Some((task, retry_count)) = task_queue.pop_front() {
            if retry_count >= MAX_RETRY_COUNT {
                error!("Max retry count reached for chunk: {:?}", task);
                for (instance, _) in task {
                    not_found.push(instance);
                }
                continue;
            }
            // Build Overpass Query for the chunk
            let names_regex = task
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join("|");

            let (area_def, area_filter) = match area_name.as_ref() {
                Some(area) => (
                    format!(r#"area[name="{}"]->.searchArea;"#, area),
                    "(area.searchArea)",
                ),
                None => ("".to_string(), ""),
            };

            let query = format!(
                r#"[out:json];{}(node[~"^(railway|public_transport|station|subway|light_rail)$"~"^(station|halt|stop|tram_stop|subway_entrance|monorail_station|light_rail_station|narrow_gauge_station|funicular_station|preserved|disused_station|stop_position|platform|stop_area|subway|railway|tram|yes)$"][~"name(:.*)?"~"^({})$"]{};);out;"#,
                area_def, names_regex, area_filter
            );

            // 2. Fetch data from Overpass API using a POST request to handle large queries
            let url = "https://maps.mail.ru/osm/tools/overpass/api/interpreter";
            let request = ehttp::Request::post(
                url,
                format!("data={}", urlencoding::encode(&query)).into_bytes(),
            );

            let response = match ehttp::fetch_async(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!("Failed to fetch OSM data for chunk: {}", e);
                    task_queue.push_back((task, retry_count + 1));
                    continue;
                }
            };

            let osm_data: OSMResponse = match response.json() {
                Ok(data) => data,
                Err(e) => {
                    error!(
                        "Failed to parse OSM data: {}, response: {:?}",
                        e,
                        response.text()
                    );
                    task_queue.push_back((task, retry_count + 1));
                    continue;
                }
            };

            // 3. Match stations and get positions for this chunk
            for (instance, name) in task {
                if let Some(osm_element) = osm_data.get_element_by_name(&name) {
                    let pos = osm_element.to_pos2();
                    found.push((instance, pos));
                    info!(
                        "Matched station '{}' to OSM element at position {:?}",
                        name, pos
                    );
                } else {
                    warn!("No matching OSM element found for station: {}", name);
                    not_found.push(instance);
                }
            }
        }
        ctx.request_repaint();
        (found, not_found)
    });

    commands.spawn(GraphLayoutTask(task));
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

        /*
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
        */
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
