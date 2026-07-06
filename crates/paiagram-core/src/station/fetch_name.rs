use std::collections::HashMap;

use serde::Deserialize;

use crate::LonLat;

#[derive(Deserialize)]
struct OSMResponse {
    elements: Vec<OSMResponseInner>,
}

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

/// Fetch a station name from OSM given a rough coordinate.
/// Returns the name and the more precise coordinate if found.
pub async fn fetch_station_name(coor: LonLat) -> Option<(String, LonLat)> {
    let lon_f64 = coor.lon as f64 / LonLat::CONVERSION_FACTOR_F64;
    let lat_f64 = coor.lat as f64 / LonLat::CONVERSION_FACTOR_F64;

    const RADIUS_METERS: u32 = 1000;
    const MAX_RETRY_COUNT: usize = 3;
    const OVERPASS_ENDPOINTS: [&str; 2] = [
        "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
        "https://overpass-api.de/api/interpreter",
    ];
    let query = format!(
        r#"
[out:json][timeout:25];
nwr[~"^(railway|public_transport|station|subway|light_rail)$"~"^(station|halt|stop|tram_stop|subway_entrance|monorail_station|light_rail_station|narrow_gauge_station|funicular_station|preserved|disused_station|stop_position|platform|stop_area|subway|railway|tram)$"](around:{RADIUS_METERS}, {lat_f64}, {lon_f64});
out center;
"#
    );

    let mut osm_data: Option<OSMResponse> = None;

    'breakpoint: for _i in 1..=MAX_RETRY_COUNT {
        for &endpoint in &OVERPASS_ENDPOINTS {
            log::info!(
                "Fetching name of ({lon_f64}, {lat_f64}) via OSM... ({_i}/{MAX_RETRY_COUNT})"
            );
            let request = ehttp::Request::post(
                endpoint,
                format!("data={}", urlencoding::encode(&query)).into_bytes(),
            );
            let response = match ehttp::fetch_async(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    log::warn!("OSM request failed: {e}");
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
                    osm_data = Some(data);
                    break 'breakpoint;
                }
                Err(e) => {
                    log::warn!("OSM response parse failed: {e:?}")
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
            let (lon, lat) = match (data.lon, data.lat, data.center) {
                (Some(lon), Some(lat), _) => (lon, lat),
                (_, _, Some(center)) => (center.lon, center.lat),
                _ => return None,
            };
            Some((name, LonLat::from(crate::Wgs84LonLat::new(lon, lat))))
        })
        .min_by(|(_, coor_a), (_, coor_b)| {
            let dist_a = (coor_a.lon as f64 - lon_f64).powi(2)
                + (coor_a.lat as f64 - lat_f64).powi(2);
            let dist_b = (coor_b.lon as f64 - lon_f64).powi(2)
                + (coor_b.lat as f64 - lat_f64).powi(2);
            dist_a.total_cmp(&dist_b)
        })
}
