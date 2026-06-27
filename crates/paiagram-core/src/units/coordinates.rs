use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wgs84LonLat {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LonLat {
    pub lon: i32,
    pub lat: i32,
}

impl LonLat {
    const CONVERSION_FACTOR_F64: f64 = 10_000_000.0;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XyPos {
    pub x: f64,
    pub y: f64,
}

impl XyPos {
    // EPSG:3857
    /// The constant defined in EPSG:3857
    const EARTH_RADIUS_METERS: f64 = 6_378_137.0;
    const WEB_MERCATOR_MAX_LAT: f64 = 85.051_128_779_806_59;
}

impl From<Wgs84LonLat> for LonLat {
    fn from(value: Wgs84LonLat) -> Self {
        let lon = value.lon.clamp(-180.0, 180.0) * Self::CONVERSION_FACTOR_F64;
        let lon = lon.round() as i32;
        let lat = value.lat.clamp(-90.0, 90.0) * Self::CONVERSION_FACTOR_F64;
        let lat = lat.round() as i32;
        Self { lon, lat }
    }
}

impl From<LonLat> for Wgs84LonLat {
    fn from(value: LonLat) -> Self {
        let lon = value.lon as f64 / LonLat::CONVERSION_FACTOR_F64;
        let lon = lon.clamp(-180.0, 180.0);
        let lat = value.lat as f64 / LonLat::CONVERSION_FACTOR_F64;
        let lat = lat.clamp(-90.0, 90.0);
        Self { lon, lat }
    }
}

impl From<Wgs84LonLat> for XyPos {
    fn from(value: Wgs84LonLat) -> Self {
        let x = Self::EARTH_RADIUS_METERS * value.lon.to_radians();
        let lat = value
            .lat
            .clamp(-Self::WEB_MERCATOR_MAX_LAT, Self::WEB_MERCATOR_MAX_LAT);
        let lat_rad = lat.to_radians();
        let y =
            -Self::EARTH_RADIUS_METERS * (std::f64::consts::FRAC_PI_4 + lat_rad / 2.0).tan().ln();
        Self { x, y }
    }
}

impl From<XyPos> for Wgs84LonLat {
    fn from(value: XyPos) -> Self {
        let lon = (value.x / XyPos::EARTH_RADIUS_METERS).to_degrees();
        let lat = (2.0 * (-value.y / XyPos::EARTH_RADIUS_METERS).exp().atan()
            - std::f64::consts::FRAC_PI_2)
            .to_degrees();
        Self { lon, lat }
    }
}
