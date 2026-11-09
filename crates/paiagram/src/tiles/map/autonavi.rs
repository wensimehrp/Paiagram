// SPDX-License-Identifier: MPL-2.0
//! Tile provider module for AutoNavi (Amap, or 高德)

use walkers::TileId;
use walkers::sources::{Attribution, TileSource};

pub struct AutoNavi;

impl TileSource for AutoNavi {
    fn tile_url(&self, tile_id: TileId) -> String {
        let z = tile_id.zoom;
        let x = tile_id.x;
        let y = tile_id.y;

        // Cycle through subdomains 01-04 based on the x coordinate
        let subdomain = (x % 4) + 1;

        // style=8 is the standard road map
        format!(
            "https://webrd0{subdomain}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}"
        )
    }
    fn attribution(&self) -> Attribution {
        Attribution {
            text: "AutoNavi (Amap)",
            url: "https://www.amap.com/",
            logo_dark: None,
            logo_light: None,
        }
    }
    fn max_zoom(&self) -> u8 {
        18
    }
}
