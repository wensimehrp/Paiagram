// SPDX-License-Identifier: MPL-2.0
//! A dummy [`Projection`] for positions that are already projected.
//!
//! `walkers` expects a geographic [`Position`] and projects it into the global pixel space.
//! Paiagram instead stores a [`Coord`] and projects it into an [`XyPos`] with the graph's
//! [`CoordType`] before it reaches `walkers`. Since an [`XyPos`] is just a
//! scaled Mercator position, this projection is a single scale from [`XyPos`] to pixels.

use egui::Pos2;
use paiagram_core::XyPos;
use walkers::{Pixels, Position, Projection, Projector};

/// The size of one tile at zoom 0, as assumed by `walkers`.
const TILE_SIZE: f64 = 256.0;

/// A projection for positions that are already projected into [`XyPos`].
#[derive(Clone, Copy, Debug, Default)]
pub struct XyProjection;

impl XyProjection {
    /// Wraps an [`XyPos`] into the [`Position`] this projection understands.
    pub fn to_position(XyPos(x, y): XyPos) -> Position {
        Position::new(f64::from(x), f64::from(y))
    }

    /// Extracts the [`XyPos`] from a [`Position`] produced by this projection.
    pub fn from_position(position: Position) -> XyPos {
        XyPos(position.x().round() as i32, position.y().round() as i32)
    }

    /// Projects an already-projected position onto the screen.
    pub fn xy_to_pos(projector: &Projector, xy: XyPos) -> Pos2 {
        projector.project(Self::to_position(xy)).to_pos2()
    }

    /// Recovers a projected position from a screen position, through `walkers`' projector.
    pub fn pos_to_xy(projector: &Projector, pos: Pos2) -> XyPos {
        Self::from_position(projector.unproject(pos.to_vec2()))
    }

    /// How many pixels one [`XyPos`] unit spans at the given zoom.
    fn pixels_per_unit(zoom: f64) -> f64 {
        2f64.powf(zoom) * TILE_SIZE / XyPos::EPSG_FACTOR
    }

    /// The latitude that Web Mercator maps to the given `y` coordinate.
    fn latitude_from_y(y: f64) -> f64 {
        let y = y / XyPos::EPSG_FACTOR;
        ((1.0 - 2.0 * y) * std::f64::consts::PI).sinh().atan().to_degrees()
    }
}

impl Projection for XyProjection {
    fn project(&self, position: Position, zoom: f64) -> Pixels {
        let scale = Self::pixels_per_unit(zoom);
        Pixels::new(position.x() * scale, position.y() * scale)
    }

    fn unproject(&self, pixels: Pixels, zoom: f64) -> Position {
        let scale = Self::pixels_per_unit(zoom);
        Position::new(pixels.x() / scale, pixels.y() / scale)
    }

    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f64 {
        let latitude = Self::latitude_from_y(position.y());
        Self::pixels_per_unit(zoom) / latitude.to_radians().cos()
    }
}
