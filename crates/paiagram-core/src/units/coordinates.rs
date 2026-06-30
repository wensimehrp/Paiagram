use serde::{Deserialize, Serialize};

use crate::Distance;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wgs84LonLat {
    pub lon: f64,
    pub lat: f64,
}

impl Wgs84LonLat {
    pub fn new(lon: f64, lat: f64) -> Self {
        Self {
            lon: lon.clamp(-180.0, 180.0),
            lat: lat.clamp(-90.0, 90.0),
        }
    }
    pub fn distance_to_meters(self, other: Self) -> f64 {
        // Convert degrees to radians
        let self_lat_rad = self.lat.to_radians();
        let self_lon_rad = self.lon.to_radians();
        let other_lat_rad = other.lat.to_radians();
        let other_lon_rad = other.lon.to_radians();

        // Differences in coordinates
        let delta_lat = other_lat_rad - self_lat_rad;
        let delta_lon = other_lon_rad - self_lon_rad;

        // Haversine formula
        let a = (delta_lat / 2.0).sin().powi(2)
            + self_lat_rad.cos() * other_lat_rad.cos() * (delta_lon / 2.0).sin().powi(2);

        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        XyPos::EARTH_RADIUS_METERS * c
    }
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
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
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

impl std::fmt::Display for Wgs84LonLat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lat_dir = if self.lat < 0.0 { 'S' } else { 'N' };
        let lon_dir = if self.lon < 0.0 { 'W' } else { 'E' };
        write!(
            f,
            "{:.4}°{}, {:.4}°{}",
            self.lat.abs(),
            lat_dir,
            self.lon.abs(),
            lon_dir
        )
    }
}
