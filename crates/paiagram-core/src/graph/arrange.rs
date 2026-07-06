use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graphmap::DiGraphMap;
use serde::Deserialize;

use crate::units::coordinates::Wgs84LonLat;
use crate::{IntervalKey, LonLat, StationKey};

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

fn name_tag_weight(key: &str) -> f64 {
    match key {
        "name" => 0.06,
        _ if key.starts_with("name:") => 0.05,
        "official_name" => 0.04,
        _ if key.starts_with("official_name:") => 0.04,
        "short_name" => 0.03,
        _ if key.starts_with("short_name:") => 0.03,
        "loc_name" => 0.02,
        _ if key.starts_with("loc_name:") => 0.02,
        "alt_name" => 0.01,
        _ if key.starts_with("alt_name:") => 0.01,
        "old_name" => 0.0,
        _ if key.starts_with("old_name:") => 0.0,
        _ => -1.0,
    }
}

fn station_kind_weight(tags: &HashMap<String, String>) -> f64 {
    let railway_weight: f64 = match tags.get("railway").map(String::as_str) {
        Some("station") => 0.60,
        Some("halt") => 0.55,
        Some("tram_stop") => 0.45,
        Some("stop") => 0.40,
        Some("light_rail") | Some("subway") | Some("monorail_station") => 0.40,
        Some("stop_position") => 0.20,
        Some("platform") => 0.15,
        Some("disused_station") | Some("preserved") => 0.10,
        Some(_) | None => 0.0,
    };
    let public_transport_weight: f64 = match tags.get("public_transport").map(String::as_str) {
        Some("station") => 0.50,
        Some("stop_area") => 0.35,
        Some("platform") => 0.20,
        Some("stop_position") => 0.15,
        Some(_) | None => 0.0,
    };
    let station_weight: f64 = match tags.get("station").map(String::as_str) {
        Some("subway") | Some("light_rail") => 0.20,
        Some(_) | None => 0.0,
    };
    railway_weight
        .max(public_transport_weight)
        .max(station_weight)
}

#[derive(Deserialize)]
struct OSMResponse {
    elements: Vec<OSMElement>,
}

#[derive(Deserialize)]
struct OSMElement {
    lat: Option<f64>,
    lon: Option<f64>,
    center: Option<OSMCenter>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct OSMCenter {
    lat: f64,
    lon: f64,
}

impl OSMElement {
    fn wgs84_coor(&self) -> Option<Wgs84LonLat> {
        match (self.lon, self.lat, self.center.as_ref()) {
            (Some(lon), Some(lat), _) => Some(Wgs84LonLat::new(lon, lat)),
            (_, _, Some(center)) => Some(Wgs84LonLat::new(center.lon, center.lat)),
            _ => None,
        }
    }
}

fn best_name_match<'a>(
    elements: &'a [OSMElement],
    station_name: &str,
) -> Option<&'a OSMElement> {
    let mut best: Option<(&OSMElement, f64)> = None;
    for element in elements {
        if element.wgs84_coor().is_none() {
            continue;
        }
        let base_weight = station_kind_weight(&element.tags);
        for (key, value) in &element.tags {
            let name_weight = name_tag_weight(key);
            if name_weight < 0.0 {
                continue;
            }

            let score = if value == station_name {
                2.0 + base_weight + name_weight
            } else {
                let similarity = strsim::jaro_winkler(station_name, value);
                if similarity <= 0.9 {
                    continue;
                }
                similarity + base_weight + name_weight
            };

            if best
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
    graph: &DiGraphMap<StationKey, IntervalKey>,
    known_positions: &mut HashMap<StationKey, LonLat>,
    all_stations: &[StationKey],
) -> usize {
    let mut fallback_count = 0usize;
    for &station in all_stations {
        if known_positions.contains_key(&station) {
            continue;
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut found_neighbor_positions = Vec::new();

        queue.push_back(station);
        visited.insert(station);

        while let Some(current) = queue.pop_front() {
            for neighbor in graph.neighbors(current) {
                if !visited.insert(neighbor) {
                    continue;
                }
                if let Some(coor) = known_positions.get(&neighbor) {
                    found_neighbor_positions.push(*coor);
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
            .map(|p| p.lon as f64)
            .sum::<f64>()
            / count;
        let avg_lat = found_neighbor_positions
            .iter()
            .map(|p| p.lat as f64)
            .sum::<f64>()
            / count;
        known_positions.insert(
            station,
            LonLat {
                lon: avg_lon.round() as i32,
                lat: avg_lat.round() as i32,
            },
        );
        fallback_count += 1;
    }

    fallback_count
}

/// Arrange stations via OSM Overpass API.
///
/// Returns a map from station key to its geographic coordinate on success.
pub async fn arrange_via_osm(
    stations: Vec<(StationKey, String)>,
    graph: &DiGraphMap<StationKey, IntervalKey>,
    area_name: Option<&str>,
    ctx: Option<&egui::Context>,
) -> HashMap<StationKey, LonLat> {
    const MAX_RETRY_COUNT: usize = 3;
    const OVERPASS_ENDPOINTS: [&str; 2] = [
        "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
        "https://overpass-api.de/api/interpreter",
    ];
    let total = stations.len();
    let station_keys: Vec<StationKey> = stations.iter().map(|(key, _)| *key).collect();

    log::info!(
        "Starting OSM arrange: stations={}, area={}",
        total,
        area_name.unwrap_or("<global>")
    );

    let (area_def, area_filter) = match area_name {
        Some(area) => {
            if area.len() == 2 && area.chars().all(|c| c.is_ascii_alphabetic()) {
                let country_code = area.to_uppercase();
                log::info!(target: "paiagram", "country_code: {country_code:?}");
                (
                    format!(r#"area["ISO3166-1"="{country_code}"]->.searchArea;"#),
                    "(area.searchArea)",
                )
            } else {
                log::info!(target: "paiagram", "area: {area:?}");
                (
                    format!(r#"area[name="{}"]->.searchArea;"#, area),
                    "(area.searchArea)",
                )
            }
        }
        None => (String::new(), ""),
    };

    let mut known_positions: HashMap<StationKey, LonLat> = HashMap::new();
    let mut task_queue: VecDeque<(Vec<(StationKey, String)>, usize)> = stations
        .chunks(100)
        .map(|chunk| (chunk.to_vec(), 0))
        .collect();

    while let Some((chunk, retry_count)) = task_queue.pop_front() {
        if retry_count >= MAX_RETRY_COUNT {
            continue;
        }

        let names_regex = chunk
            .iter()
            .map(|(_, name)| escape_overpass_regex(name))
            .collect::<Vec<_>>()
            .join("|");

        let query = format!(
            r#"[out:json];{area_def}(node[~"^(railway|public_transport|station|subway|light_rail)$"~"^(station|halt|stop|tram_stop|subway_entrance|monorail_station|light_rail_station|narrow_gauge_station|funicular_station|preserved|disused_station|stop_position|platform|stop_area|subway|railway|tram|yes)$"][~"name(:.*)?"~"^({names_regex})$"]{area_filter};);out;"#,
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
                    log::warn!(
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
                log::warn!(
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
                    log::info!(
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
                    log::warn!(
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
            task_queue.push_back((chunk, retry_count + 1));
            continue;
        };

        let chunk_size = chunk.len();
        let mut matched_count = 0usize;
        for (key, name) in chunk {
            if let Some(element) = best_name_match(&osm_data.elements, &name) {
                if let Some(wgs84) = element.wgs84_coor() {
                    known_positions.insert(key, LonLat::from(wgs84));
                    matched_count += 1;
                }
            }
        }
        log::info!(
            "OSM chunk processed: matched={}/{}, progress={}/{}",
            matched_count,
            chunk_size,
            known_positions.len(),
            total
        );
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    }

    let fallback_count =
        fill_unmatched_via_neighbors(graph, &mut known_positions, &station_keys);
    log::info!(
        "OSM neighbour fallback applied: fallback_mapped={}, total_mapped={}/{}",
        fallback_count,
        known_positions.len(),
        total
    );

    known_positions
}
