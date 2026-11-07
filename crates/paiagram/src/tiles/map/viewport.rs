//! Module for centering the viewport

use egui::Vec2;
use itertools::Itertools;
use paiagram_core::{Wgs84LonLat, World};
use walkers::{MapMemory, MercatorProjection, Pixels, Projection, lon_lat};

use crate::tiles::map::autonavi::MercatorProjectionGcj02;

/// Calculate the median center of projected coordinates
fn get_median_center(coords: impl Iterator<Item = (f64, f64)> + Clone) -> (f64, f64) {
    let xs: Vec<_> = coords.clone().map(|(x, _)| x).sorted_unstable_by(f64::total_cmp).collect();
    let ys: Vec<_> = coords.map(|(_, y)| y).sorted_unstable_by(f64::total_cmp).collect();
    let median = |vals: &[f64]| {
        let len = vals.len();
        let half = len / 2;
        if len == 0 {
            // unfortunately this means navigating to the middle of the Atlantics
            0.0
        } else if len % 2 == 0 {
            (vals[half] + vals[half - 1]) / 2.0
        } else {
            vals[half]
        }
    };
    (median(&xs), median(&ys))
}

/// Calculate the range of coordinates w/o outliers outside of the upper fence
fn get_nodes_range(coords: impl Iterator<Item = (f64, f64)> + Clone) -> [(f64, f64); 2] {
    let (median_x, median_y) = get_median_center(coords.clone());
    let coords: Vec<_> = coords
        .map(|(cx, cy)| {
            let distance_sq = (median_x - cx).powi(2) + (median_y - cy).powi(2);
            (cx, cy, distance_sq)
        })
        .sorted_unstable_by(|(_, _, a), (_, _, b)| f64::total_cmp(a, b))
        .collect();

    let len = coords.len();
    if len == 0 {
        return [(0.0, 0.0); 2];
    }

    let quartile_dist = |q_num: usize| -> f64 {
        let idx = (len * q_num) / 4;
        coords[idx].2.sqrt()
    };

    let q1 = quartile_dist(1);
    let q3_idx = (len * 3) / 4;
    let q3 = coords[q3_idx].2.sqrt();
    let iqr = q3 - q1;

    let max_dist_sq = (q3 + 1.5 * iqr).powi(2).max(coords[q3_idx].2);

    coords.iter().take_while(|&&(_, _, d_sq)| d_sq <= max_dist_sq).fold(
        [(median_x, median_y); 2],
        |[(min_x, max_x), (min_y, max_y)], &(cx, cy, _)| {
            [
                (min_x.min(cx), max_x.max(cx)),
                (min_y.min(cy), max_y.max(cy)),
            ]
        },
    )
}

/// The fraction of the viewport the nodes' bounding box is allowed to occupy after fitting.
const FIT_PADDING: f64 = 0.9;

/// Build a [`MapMemory`] that centers and zooms the map so that the graph's nodes fit inside a
/// viewport of `viewport` size (in screen points).
pub(super) fn fit_viewport_to_nodes(world: &World, is_autonavi: bool, viewport: Vec2) -> MapMemory {
    let mut map_memory = MapMemory::default();

    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return map_memory;
    }

    let projection: Box<dyn Projection> = if is_autonavi {
        Box::new(MercatorProjectionGcj02)
    } else {
        Box::new(MercatorProjection)
    };

    let zoom = map_memory.zoom();
    let coords: Vec<(f64, f64)> = world
        .graph
        .nodes()
        .values()
        .map(|node| {
            let pos: Wgs84LonLat = node.pos.into();
            let projected = projection.project(lon_lat(pos.lon, pos.lat), zoom);
            (projected.x(), projected.y())
        })
        .collect();
    if coords.is_empty() {
        return map_memory;
    }

    let [(min_x, max_x), (min_y, max_y)] = get_nodes_range(coords.iter().copied());
    let (center_x, center_y) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let span_x = max_x - min_x;
    let span_y = max_y - min_y;

    // The projection scales linearly with 2^zoom, so the zoom needed to fit a span is a
    // logarithmic function of the ratio between the viewport and that span.
    let fit_scale = |viewport_extent: f32, span: f64| -> f64 {
        if span > f64::EPSILON {
            f64::from(viewport_extent) / span * FIT_PADDING
        } else {
            f64::INFINITY
        }
    };

    let zoom_delta = fit_scale(viewport.x, span_x).min(fit_scale(viewport.y, span_y)).log2();

    let center = projection.unproject(Pixels::new(center_x, center_y), zoom);
    map_memory.center_at(center);

    if !zoom_delta.is_finite() {
        // All nodes project onto (almost) the same point, so there is no meaningful bounding
        // box to fit. Keep the current zoom and only center on the nodes.
        return map_memory;
    }

    let max_zoom = if is_autonavi { 18.0 } else { 19.0 };
    let fitted_zoom = (zoom + zoom_delta).clamp(0.0, max_zoom);
    let _ = map_memory.set_zoom(fitted_zoom);
    map_memory
}
