use std::collections::HashMap;

use crate::graph::{Node, NodePos};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use serde::Deserialize;

pub(super) struct FetchNamePlugin;

impl Plugin for FetchNamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fetch_station_name);
    }
}

/// Stations with this marker component is created with a default name, and the name would be
/// fetched via OSM services
#[derive(Component)]
pub(super) struct StationNamePending(Task<Option<(String, NodePos)>>);

#[derive(Deserialize)]
struct OSMResponse {
    elements: Vec<OSMResponseInner>,
}

// TODO: unify the networking parts
#[derive(Deserialize)]
struct OSMResponseInner {
    lon: Option<f64>,
    lat: Option<f64>,
    center: Option<OSMCenter>,
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct OSMCenter {
    lon: f64,
    lat: f64,
}

impl StationNamePending {
    pub fn new(coor: NodePos) -> Self {
        let task = AsyncComputeTaskPool::get().spawn(Self::fetch(coor));
        Self(task)
    }
    async fn fetch(coor: NodePos) -> Option<(String, NodePos)> {
        let NodePos { lon, lat } = coor;
        const RADIUS_METERS: u32 = 1000;
        const MAX_RETRY_COUNT: usize = 3;
        const OVERPASS_ENDPOINTS: [&str; 2] = [
            "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
            "https://overpass-api.de/api/interpreter",
        ];
        let query = format!(
            r#"
[out:json][timeout:25];
nwr[~"^(railway|public_transport|station|subway|light_rail)$"~"^(station|halt|stop|tram_stop|subway_entrance|monorail_station|light_rail_station|narrow_gauge_station|funicular_station|preserved|disused_station|stop_position|platform|stop_area|subway|railway|tram)$"](around:{RADIUS_METERS}, {lat}, {lon});
out center;
"#
        );

        let mut osm_data: Option<OSMResponse> = None;

        'breakpoint: for i in 1..=MAX_RETRY_COUNT {
            for &endpoint in &OVERPASS_ENDPOINTS {
                info!("Arranging ({coor}) via OSM... ({i}/{MAX_RETRY_COUNT})");
                let request = ehttp::Request::post(
                    endpoint,
                    format!("data={}", urlencoding::encode(&query)).into_bytes(),
                );
                let response = match ehttp::fetch_async(request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        warn!("OSM request failed: {e}");
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
                        osm_data = Some(data);
                        break 'breakpoint;
                    }
                    Err(e) => {
                        warn!(?e)
                    }
                };
            }
        }
        let Some(osm_data) = osm_data else {
            return None;
        };
        osm_data
            .elements
            .into_iter()
            .filter_map(|mut data| {
                let name = data.tags.remove("name")?;
                let pos = match (data.lon, data.lat, data.center) {
                    (Some(lon), Some(lat), _) => NodePos { lon, lat },
                    (_, _, Some(center)) => NodePos {
                        lon: center.lon,
                        lat: center.lat,
                    },
                    _ => return None,
                };
                Some((name, pos))
            })
            .min_by(|(_, pos_a), (_, pos_b)| {
                let dist_a = (pos_a.lon - lon).powi(2) + (pos_a.lat - lat).powi(2);
                let dist_b = (pos_b.lon - lon).powi(2) + (pos_b.lat - lat).powi(2);
                dist_a.total_cmp(&dist_b)
            })
    }
}

fn fetch_station_name(
    mut pending_entries: Query<(Entity, &mut Node, &mut Name, &mut StationNamePending)>,
    mut commands: Commands,
) {
    for (entity, mut node, mut name, mut pending_name) in pending_entries.iter_mut() {
        let Some(found) = block_on(poll_once(&mut pending_name.0)) else {
            continue;
        };
        if let Some((found_name, found_pos)) = found {
            name.set(found_name);
            node.pos = found_pos;
        } else {
            name.set("Name Not Found")
        };
        commands.entity(entity).remove::<StationNamePending>();
    }
}
