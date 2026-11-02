// SPDX-License-Identifier: MPL-2.0
//! Projection and provider module for AutoNavi (Amap, or 高德)

use walkers::sources::{Attribution, TileSource};
use walkers::{MercatorProjection, Pixels, Position, Projection, TileId};

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

pub struct MercatorProjectionGcj02;

impl Projection for MercatorProjectionGcj02 {
    fn project(&self, position: Position, zoom: f64) -> Pixels {
        let (x, y) = coordtransform::wgs84_to_gcj02(position.x(), position.y());
        Projection::project(&MercatorProjection, Position::new(x, y), zoom)
    }

    fn unproject(&self, pixels: Pixels, zoom: f64) -> Position {
        let pos = Projection::unproject(&MercatorProjection, pixels, zoom);
        let (x, y) = coordtransform::gcj02_to_wgs84(pos.x(), pos.y());
        Position::new(x, y)
    }

    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f64 {
        let (x, y) = coordtransform::wgs84_to_gcj02(position.x(), position.y());
        Projection::scale_pixel_per_meter(&MercatorProjection, Position::new(x, y), zoom)
    }
}
