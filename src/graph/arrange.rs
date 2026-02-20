use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future::poll_once};
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use petgraph::graph::NodeIndex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use visgraph::layout::force_directed::force_directed_layout;

use super::{Graph, Node, NodePos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphLayoutKind {
    ForceDirected,
    OSM,
}

#[derive(Resource)]
pub struct GraphLayoutTask {
    pub task: Task<Vec<(Entity, NodePos)>>,
    finished: Arc<AtomicUsize>,
    queued_for_retry: Arc<AtomicUsize>,
    pub total: usize,
    pub kind: GraphLayoutKind,
}

impl GraphLayoutTask {
    fn new(
        task: Task<Vec<(Entity, NodePos)>>,
        finished: Arc<AtomicUsize>,
        queued_for_retry: Arc<AtomicUsize>,
        total: usize,
        kind: GraphLayoutKind,
    ) -> Self {
        Self {
            task,
            finished,
            queued_for_retry,
            total,
            kind,
        }
    }

    pub fn progress(&self) -> (usize, usize, usize) {
        (
            self.finished.load(Ordering::Relaxed),
            self.total,
            self.queued_for_retry.load(Ordering::Relaxed),
        )
    }
}

pub fn apply_graph_layout_task(
    mut commands: Commands,
    task: Option<ResMut<GraphLayoutTask>>,
    mut nodes: Query<&mut Node>,
) {
    let Some(mut task) = task else {
        return;
    };
    let Some(found) = block_on(poll_once(&mut task.task)) else {
        return;
    };
    for (entity, pos) in found {
        let Ok(mut node) = nodes.get_mut(entity) else {
            continue;
        };
        node.pos = pos;
    }
    let (finished, total, queued_for_retry) = task.progress();
    info!(
        "Graph arrange completed: mode={:?}, mapped={finished}/{total}, retry_queued={queued_for_retry}",
        task.kind
    );
    commands.remove_resource::<GraphLayoutTask>();
}

pub fn apply_force_directed_layout(
    In(iterations): In<u32>,
    graph_map: Res<Graph>,
    mut nodes: Query<&mut Node>,
) {
    let graph: petgraph::Graph<_, _, _, usize> = graph_map.map.clone().into_graph();
    let binding = &graph;
    let entity_map: EntityHashMap<NodeIndex<usize>> = graph
        .node_indices()
        .map(|idx| (*graph.node_weight(idx).unwrap(), idx))
        .collect();
    let layout = force_directed_layout(&binding, iterations, 0.1);

    for node_entity in graph_map.nodes() {
        let Some(&idx) = entity_map.get(&node_entity) else {
            continue;
        };
        let Ok(mut pos) = nodes.get_mut(node_entity) else {
            continue;
        };
        let (nx, ny) = layout(idx);
        pos.pos = NodePos::new_xy(nx as f64, ny as f64);
    }
}

pub fn auto_arrange_graph(
    (In(ctx), In(iterations)): (In<egui::Context>, In<u32>),
    mut commands: Commands,
    graph_map: Res<Graph>,
) {
    let graph: petgraph::Graph<_, _, _, usize> = graph_map.map.clone().into_graph();
    let total = graph.node_count();
    let finished = Arc::new(AtomicUsize::new(0));
    let queued_for_retry = Arc::new(AtomicUsize::new(0));
    let finished_in_task = Arc::clone(&finished);

    info!(
        "Starting force-directed arrange: nodes={}, iterations={}",
        total, iterations
    );

    let task = AsyncComputeTaskPool::get().spawn(async move {
        let binding = &graph;
        let layout = force_directed_layout(&binding, iterations, 0.1);
        let out: Vec<(Entity, NodePos)> = graph
            .node_indices()
            .map(|idx| {
                let (x, y) = layout(idx);
                (
                    *graph.node_weight(idx).unwrap(),
                    NodePos::new_xy(x as f64, y as f64),
                )
            })
            .collect();
        finished_in_task.store(total, Ordering::Relaxed);
        ctx.request_repaint();
        out
    });

    commands.insert_resource(GraphLayoutTask::new(
        task,
        finished,
        queued_for_retry,
        total,
        GraphLayoutKind::ForceDirected,
    ));
}

#[derive(Deserialize)]
struct OSMResponse {
    elements: Vec<OSMElement>,
}

#[derive(Deserialize)]
struct OSMElement {
    lat: f64,
    lon: f64,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
}

fn escape_overpass_regex(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn best_name_match<'a>(elements: &'a [OSMElement], station_name: &str) -> Option<&'a OSMElement> {
    let mut best: Option<(&OSMElement, f64)> = None;
    for element in elements {
        for (key, value) in &element.tags {
            if !key.starts_with("name") {
                continue;
            }
            if value == station_name {
                return Some(element);
            }
            let score = strsim::jaro_winkler(station_name, value);
            if score > 0.9
                && best
                    .as_ref()
                    .is_none_or(|(_, best_score)| score > *best_score)
            {
                best = Some((element, score));
            }
        }
    }
    best.map(|(element, _)| element)
}

fn fill_unmatched_via_neighbors(
    graph: &petgraph::Graph<Entity, Entity, petgraph::Directed, usize>,
    known_positions: &mut HashMap<Entity, NodePos>,
    all_stations: &[Entity],
) -> usize {
    let entity_to_index: HashMap<Entity, NodeIndex<usize>> = graph
        .node_indices()
        .map(|idx| (*graph.node_weight(idx).unwrap(), idx))
        .collect();

    let mut fallback_count = 0usize;
    for &station in all_stations {
        if known_positions.contains_key(&station) {
            continue;
        }
        let Some(&start_idx) = entity_to_index.get(&station) else {
            continue;
        };

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut found_neighbor_positions = Vec::new();

        queue.push_back(start_idx);
        visited.insert(start_idx);

        while let Some(current) = queue.pop_front() {
            for neighbor in graph.neighbors_undirected(current) {
                if !visited.insert(neighbor) {
                    continue;
                }
                let neighbor_entity = *graph.node_weight(neighbor).unwrap();
                if let Some(pos) = known_positions.get(&neighbor_entity) {
                    found_neighbor_positions.push(*pos);
                } else {
                    queue.push_back(neighbor);
                }
            }
        }

        if found_neighbor_positions.is_empty() {
            continue;
        }

        let count = found_neighbor_positions.len() as f64;
        let avg_lon = found_neighbor_positions
            .iter()
            .map(|p| p.lon)
            .sum::<f64>()
            / count;
        let avg_lat = found_neighbor_positions
            .iter()
            .map(|p| p.lat)
            .sum::<f64>()
            / count;
        known_positions.insert(station, NodePos::new_lon_lat(avg_lon, avg_lat));
        fallback_count += 1;
    }

    fallback_count
}

pub fn arrange_via_osm(
    (In(ctx), In(area_name)): (In<egui::Context>, In<Option<String>>),
    mut commands: Commands,
    graph_map: Res<Graph>,
    station_names: Query<(Entity, &Name), With<crate::station::Station>>,
) {
    const MAX_RETRY_COUNT: usize = 3;
    const OVERPASS_ENDPOINTS: [&str; 2] = [
        "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
        "https://overpass-api.de/api/interpreter",
    ];
    let stations: Vec<(Entity, String)> = station_names
        .iter()
        .map(|(entity, name)| (entity, name.to_string()))
        .collect();
    let total = stations.len();
    let station_entities: Vec<Entity> = stations.iter().map(|(entity, _)| *entity).collect();
    let graph: petgraph::Graph<_, _, _, usize> = graph_map.map.clone().into_graph();

    info!(
        "Starting OSM arrange: stations={}, area={}",
        total,
        area_name.as_deref().unwrap_or("<global>")
    );

    let finished = Arc::new(AtomicUsize::new(0));
    let queued_for_retry = Arc::new(AtomicUsize::new(0));
    let finished_in_task = Arc::clone(&finished);
    let queued_in_task = Arc::clone(&queued_for_retry);

    let mut task_queue: VecDeque<(Vec<(Entity, String)>, usize)> = stations
        .chunks(50)
        .map(|chunk| (chunk.to_vec(), 0))
        .collect();

    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut known_positions: HashMap<Entity, NodePos> = HashMap::new();

        while let Some((chunk, retry_count)) = task_queue.pop_front() {
            if retry_count >= MAX_RETRY_COUNT {
                finished_in_task.fetch_add(chunk.len(), Ordering::Relaxed);
                continue;
            }

            let names_regex = chunk
                .iter()
                .map(|(_, name)| escape_overpass_regex(name))
                .collect::<Vec<_>>()
                .join("|");

            let (area_def, area_filter) = match area_name.as_ref() {
                Some(area) => (
                    format!(r#"area[name="{}"]->.searchArea;"#, area),
                    "(area.searchArea)",
                ),
                None => (String::new(), ""),
            };

            let query = format!(
                r#"[out:json];{}(node[~"^(railway|public_transport|station|subway|light_rail)$"~"^(station|halt|stop|tram_stop|subway_entrance|monorail_station|light_rail_station|narrow_gauge_station|funicular_station|preserved|disused_station|stop_position|platform|stop_area|subway|railway|tram|yes)$"][~"name(:.*)?"~"^({})$"]{};);out;"#,
                area_def, names_regex, area_filter
            );

            let mut osm_data: Option<OSMResponse> = None;
            for endpoint in OVERPASS_ENDPOINTS {
                let request = ehttp::Request::post(
                    endpoint,
                    format!("data={}", urlencoding::encode(&query)).into_bytes(),
                );

                let response = match ehttp::fetch_async(request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        warn!(
                            "OSM request failed: endpoint={}, chunk(size={}), retry={}/{} ({:?})",
                            endpoint,
                            chunk.len(),
                            retry_count + 1,
                            MAX_RETRY_COUNT,
                            e
                        );
                        continue;
                    }
                };

                if !response.ok {
                    let body_preview = response
                        .text()
                        .map(|t| t.chars().take(200).collect::<String>())
                        .unwrap_or_else(|| "<non-utf8>".to_string());
                    warn!(
                        "OSM bad response: endpoint={}, status={} {}, content_type={:?}, body_preview={:?}",
                        endpoint,
                        response.status,
                        response.status_text,
                        response.content_type(),
                        body_preview
                    );
                    continue;
                }

                match response.json() {
                    Ok(data) => {
                        info!(
                            "OSM chunk fetched: endpoint={}, chunk(size={}), retry={}/{}",
                            endpoint,
                            chunk.len(),
                            retry_count,
                            MAX_RETRY_COUNT
                        );
                        osm_data = Some(data);
                        break;
                    }
                    Err(e) => {
                        let body_preview = response
                            .text()
                            .map(|t| t.chars().take(200).collect::<String>())
                            .unwrap_or_else(|| "<non-utf8>".to_string());
                        warn!(
                            "OSM response parse failed: endpoint={}, chunk(size={}), retry={}/{} ({:?}), content_type={:?}, body_preview={:?}",
                            endpoint,
                            chunk.len(),
                            retry_count + 1,
                            MAX_RETRY_COUNT,
                            e,
                            response.content_type(),
                            body_preview
                        );
                    }
                }
            }

            let Some(osm_data) = osm_data else {
                queued_in_task.fetch_add(chunk.len(), Ordering::Relaxed);
                task_queue.push_back((chunk, retry_count + 1));
                continue;
            };

            let chunk_size = chunk.len();
            let mut matched_count = 0usize;
            for (entity, name) in chunk {
                if let Some(element) = best_name_match(&osm_data.elements, &name) {
                    known_positions.insert(entity, NodePos::new_lon_lat(element.lon, element.lat));
                    matched_count += 1;
                }
                finished_in_task.fetch_add(1, Ordering::Relaxed);
            }
            info!(
                "OSM chunk processed: matched={}/{}, progress={}/{}",
                matched_count,
                chunk_size,
                finished_in_task.load(Ordering::Relaxed),
                total
            );
            ctx.request_repaint();
        }

        let fallback_count = fill_unmatched_via_neighbors(&graph, &mut known_positions, &station_entities);
        info!(
            "OSM neighbour fallback applied: fallback_mapped={}, total_mapped={}/{}",
            fallback_count,
            known_positions.len(),
            total
        );

        known_positions.into_iter().collect()
    });

    commands.insert_resource(GraphLayoutTask::new(
        task,
        finished,
        queued_for_retry,
        total,
        GraphLayoutKind::OSM,
    ));
}
