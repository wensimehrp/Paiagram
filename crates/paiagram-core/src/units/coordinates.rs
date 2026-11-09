// SPDX-License-Identifier: MPL-2.0
//! Generic coordinates and the projections used to display them.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

/// The Earth radius defined by EPSG:3857.
const EARTH_RADIUS_METERS: f64 = 6_378_137.0;
/// The largest latitude representable in Web Mercator.
const WEB_MERCATOR_MAX_LAT: f64 = 85.051_128_779_806_59;

/// A generic coordinate. It can be converted to different coordinate systems if required.
///
/// The meaning of the two components depends on the [`CoordType`] of the graph that owns the
/// coordinate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coord(pub i32, pub i32);

impl Coord {
    pub const WGS84_CONVERSION_FACTOR: f64 = 10_000_000.0;
    pub const ZERO: Self = Self(0, 0);
    /// Stores a WGS84 longitude/latitude pair as a coordinate.
    pub fn from_wgs84(lon: f64, lat: f64) -> Self {
        let x = (lon.clamp(-180.0, 180.0) * Self::WGS84_CONVERSION_FACTOR).round();
        let y = (lat.clamp(-90.0, 90.0) * Self::WGS84_CONVERSION_FACTOR).round();
        Self(x as i32, y as i32)
    }
    /// Reads the coordinate as a WGS84 longitude/latitude pair.
    pub fn to_wgs84(self) -> (f64, f64) {
        let Self(x, y) = self;
        (
            (x as f64 / Self::WGS84_CONVERSION_FACTOR).clamp(-180.0, 180.0),
            (y as f64 / Self::WGS84_CONVERSION_FACTOR).clamp(-90.0, 90.0),
        )
    }
    pub const fn from_flat(x: i32, y: i32) -> Self {
        Self(x, y)
    }
}

/// A displayed position on the map.
///
/// This stores a normalized EPSG:3857 (Web Mercator) position scaled by [`XyPos::EPSG_FACTOR`]:
/// the origin is the top-left corner of the world, `x` grows eastwards and `y` grows southwards.
/// It is only used for display; coordinates are the persisted representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct XyPos(pub i32, pub i32);

impl XyPos {
    pub const EPSG_FACTOR: f64 = EARTH_RADIUS_METERS * 2.0 * PI;
    /// Scales a normalized EPSG:3857 position into [`XyPos`].
    pub fn from_epsg(x: f64, y: f64) -> Self {
        Self(
            (x * Self::EPSG_FACTOR).round() as i32,
            (y * Self::EPSG_FACTOR).round() as i32,
        )
    }
    /// Scales this position back to a normalized EPSG:3857 position.
    pub fn to_epsg(self) -> (f64, f64) {
        (
            f64::from(self.0) / Self::EPSG_FACTOR,
            f64::from(self.1) / Self::EPSG_FACTOR,
        )
    }
}

/// How a graph's coordinates are stored and projected for display.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CoordType {
    /// Coordinates are stored in plain XY
    Flat,
    /// Coordinates are stored in WGS84
    Wgs(WgsProjection),
}

impl Default for CoordType {
    fn default() -> Self {
        Self::Wgs(WgsProjection::default())
    }
}

impl CoordType {
    pub const ALL: &'static [Self] = &[
        Self::Wgs(WgsProjection::Mercator),
        Self::Wgs(WgsProjection::GcjAndMercator),
        Self::Flat,
    ];

    pub const fn is_flat(self) -> bool {
        matches!(self, Self::Flat)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Flat => "XY, Flat Projection",
            Self::Wgs(WgsProjection::Mercator) => "WGS84, projected via Web Mercator",
            Self::Wgs(WgsProjection::GcjAndMercator) => {
                "WGS84, converted to GCJ-02 and projected via Web Mercator"
            }
        }
    }

    /// Projects a stored coordinate into a displayed position.
    pub fn project(self, coord: Coord) -> XyPos {
        match self {
            Self::Flat => XyPos(coord.0, coord.1),
            Self::Wgs(projection) => projection.project(coord),
        }
    }

    /// Recovers the stored coordinate from a displayed position.
    pub fn unproject(self, xy: XyPos) -> Coord {
        match self {
            Self::Flat => Coord(xy.0, xy.1),
            Self::Wgs(projection) => projection.unproject(xy),
        }
    }

    /// The distance between two coordinates, in metres.
    pub fn distance_to_meters(self, a: Coord, b: Coord) -> f64 {
        match self {
            Self::Flat => {
                let dx = f64::from(a.0) - f64::from(b.0);
                let dy = f64::from(a.1) - f64::from(b.1);
                dx.hypot(dy)
            }
            Self::Wgs(_) => {
                let (lon1, lat1) = a.to_wgs84();
                let (lon2, lat2) = b.to_wgs84();
                haversine(lon1, lat1, lon2, lat2)
            }
        }
    }

    /// Formats a coordinate for display, using the convention of this coordinate type.
    pub fn display(self, coord: Coord) -> String {
        match self {
            Self::Flat => format!("{}, {}", coord.0, coord.1),
            Self::Wgs(_) => {
                let (lon, lat) = coord.to_wgs84();
                let lat_dir = if lat < 0.0 { 'S' } else { 'N' };
                let lon_dir = if lon < 0.0 { 'W' } else { 'E' };
                format!("{:.5}°{}, {:.5}°{}", lat.abs(), lat_dir, lon.abs(), lon_dir)
            }
        }
    }
}

/// The projection applied to WGS84 coordinates.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum WgsProjection {
    /// Project directly with Web Mercator.
    #[default]
    Mercator,
    /// Translate to GCJ-02 first, then project with Web Mercator.
    GcjAndMercator,
}

impl WgsProjection {
    fn project(self, coord: Coord) -> XyPos {
        let (lon, lat) = coord.to_wgs84();
        let (lon, lat) = match self {
            Self::Mercator => (lon, lat),
            Self::GcjAndMercator => coordtransform::wgs84_to_gcj02(lon, lat),
        };
        let x = (lon.to_radians() / PI + 1.0) / 2.0;
        let lat = lat.clamp(-WEB_MERCATOR_MAX_LAT, WEB_MERCATOR_MAX_LAT);
        let y = 0.5 - lat.to_radians().tan().asinh() / (2.0 * PI);
        XyPos::from_epsg(x, y)
    }

    fn unproject(self, xy: XyPos) -> Coord {
        let (x, y) = xy.to_epsg();
        let lon = ((x * 2.0 - 1.0) * PI).to_degrees();
        let lat = ((1.0 - 2.0 * y) * PI).sinh().atan().to_degrees();
        let (lon, lat) = match self {
            Self::Mercator => (lon, lat),
            Self::GcjAndMercator => coordtransform::gcj02_to_wgs84(lon, lat),
        };
        Coord::from_wgs84(lon, lat)
    }
}

/// The great-circle distance between two WGS84 positions, in metres.
fn haversine(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let lat1 = lat1.to_radians();
    let lat2 = lat2.to_radians();
    let delta_lat = lat2 - lat1;
    let delta_lon = (lon2 - lon1).to_radians();
    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_METERS * c
}
