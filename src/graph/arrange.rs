use crate::graph::{Graph, Station};
use bevy::ecs::entity::EntityHashSet;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use egui::{Context, Pos2};
use moonshine_core::prelude::*;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use visgraph::layout::force_directed::force_directed_layout;

#[derive(Resource)]
pub struct GraphLayoutTask {
    pub task: Task<(Vec<(Instance<Station>, Pos2)>, Vec<Instance<Station>>)>,
    pub finished: AtomicUsize,
    pub queued_for_retry: AtomicUsize,
    pub total: usize,
}

impl GraphLayoutTask {
    pub fn in_progress(&self) -> usize {
        let finished = self.finished.load(Ordering::Relaxed);
        self.total.saturating_sub(finished)
    }
}

impl GraphLayoutTask {
    pub fn new(
        task: Task<(Vec<(Instance<Station>, Pos2)>, Vec<Instance<Station>>)>,
        total: usize,
    ) -> Self {
        Self {
            task,
            finished: AtomicUsize::new(0),
            queued_for_retry: AtomicUsize::new(0),
            total,
        }
    }

    pub fn finish(&self, amount: usize) {
        self.finished.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn finished_count(&self) -> usize {
        self.finished.load(Ordering::Relaxed)
    }

    pub fn queue_retry(&self, amount: usize) {
        self.queued_for_retry.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn queued_for_retry_count(&self) -> usize {
        self.queued_for_retry.load(Ordering::Relaxed)
    }
}

/// Apply the graph layout once the task is complete
/// This system should only be run if the [`GraphLayoutTask`] resource exists
pub(super) fn apply_graph_layout(
    mut task: ResMut<GraphLayoutTask>,
    mut stations: Query<(NameOrEntity, &mut Station)>,
    mut commands: Commands,
    graph: Res<Graph>,
) {
    let Some((found, not_found)) =
        bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task.task))
    else {
        return;
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
    // cleanup the task resource
    commands.remove_resource::<GraphLayoutTask>();
    info!("Finished applying graph layout");
}

// TODO: move layout algorithms to a separate module
pub fn auto_arrange_graph(
    (In(ctx), In(iterations)): (In<Context>, In<u32>),
    mut commands: Commands,
    graph: Res<Graph>,
) {
    info!("Auto arranging graph with {} iterations", iterations);
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
    commands.insert_resource(GraphLayoutTask::new(task, graph.inner().node_count()));
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
pub fn arrange_via_osm(
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

    let async_task = async move {
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
    };

    let task = thread_pool.spawn(async_task);
    commands.insert_resource(GraphLayoutTask::new(task, station_names.iter().len()));
}
