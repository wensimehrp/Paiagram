use egui::*;
use itertools::Itertools;
use paiagram_core::spatial_index::{NodesRTree, SpatialCache};
use paiagram_core::time::TimetableTime;
use paiagram_core::trip::TripSchedule;
use paiagram_core::{IntervalKey, LonLat, NodeKey, Wgs84LonLat, World};
use paiagram_selection::{SelectedItem, SelectedItems, TripSelection};
use walkers::{MapMemory, Projector, lon_lat};

use crate::App;
use crate::distance_to_line::distance_sq_to_line;
use crate::tiles::map::map_content::trip_leg_current_pos;
use crate::trip_schedule_visual::WithVisualMod;

const SELECTION_RADIUS: f32 = 10.0;
const SELECTION_RADIUS_SQ: f32 = SELECTION_RADIUS * SELECTION_RADIUS;

pub fn select_map_contents(
    app: &mut App,
    ui: &mut Ui,
    response: &Response,
    projector: &Projector,
    _map_memory: &MapMemory,
) {
    puffin::profile_function!();

    let pos_to_lon_lat = |pos: Pos2| -> LonLat {
        let coor = projector.unproject(pos.to_vec2());
        Wgs84LonLat {
            lon: coor.x(),
            lat: coor.y(),
        }
        .into()
    };

    let lon_lat_to_pos = |coor: LonLat| {
        let wgs84_lon_lat: Wgs84LonLat = coor.into();
        projector.project(lon_lat(wgs84_lon_lat.lon, wgs84_lon_lat.lat)).to_pos2()
    };

    // select stuff
    if response.clicked()
        && let Some(pointer_pos) = response.interact_pointer_pos()
        && let SelectedItems::None = app.selected_items.take()
    {
        let area = Rect::from_pos(pointer_pos).expand(SELECTION_RADIUS);
        let coor_min = pos_to_lon_lat(area.left_top());
        let coor_max = pos_to_lon_lat(area.right_bottom());
        _select_map_contents(
            coor_min,
            coor_max,
            pointer_pos,
            app,
            lon_lat_to_pos,
            pos_to_lon_lat,
        );
    }
}

fn _select_map_contents(
    coor_min: LonLat,
    coor_max: LonLat,
    pointer_pos: Pos2,
    app: &mut App,
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2 + Clone,
    pos_to_lon_lat: impl Fn(Pos2) -> LonLat,
) {
    puffin::profile_function!();

    let SpatialCache {
        intervals,
        nodes,
        trip_legs,
    } = app.spatial();

    let curr_time = TimetableTime(app.timer.seconds() as i32);

    let selected = trip_legs
        .get(coor_min, coor_max, curr_time)
        .filter_map(|leg| {
            trip_leg_current_pos(leg.clone(), app.timer.seconds(), lon_lat_to_pos.clone()).and_then(
                |pos| {
                    let dist_sq = pos.distance_sq(pointer_pos);
                    (dist_sq < SELECTION_RADIUS_SQ).then_some((leg, dist_sq))
                },
            )
        })
        .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
        .map(|(leg, _)| SelectedItem::Trip(leg.trip_key.into()))
        .or_else(|| {
            find_node_within_range(
                nodes,
                coor_min,
                coor_max,
                pointer_pos,
                lon_lat_to_pos.clone(),
            )
            .map(SelectedItem::Node)
        })
        .or_else(|| {
            intervals
                .get(coor_min, coor_max)
                .flat_map(|interval| {
                    interval
                        .nodes
                        .iter()
                        .copied()
                        .map(lon_lat_to_pos.clone())
                        .tuple_windows()
                        .zip(std::iter::repeat(interval))
                })
                .map(|((prev, curr), interval)| {
                    (interval, distance_sq_to_line((prev, curr), pointer_pos))
                })
                .filter_map(|(interval, dist_sq)| {
                    (dist_sq < SELECTION_RADIUS_SQ).then_some((interval, dist_sq))
                })
                .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
                .map(|(interval, _)| SelectedItem::Interval(interval.interval_key))
        })
        .map_or(
            SelectedItems::Coordinate((pos_to_lon_lat(pointer_pos), String::new())),
            SelectedItems::from,
        );

    app.selected_items = selected;
}

pub fn find_node_within_range(
    nodes: &NodesRTree,
    coor_min: LonLat,
    coor_max: LonLat,
    pointer_pos: Pos2,
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2 + Clone,
) -> Option<NodeKey> {
    nodes
        .get(coor_min, coor_max)
        .map(|node| (node, lon_lat_to_pos(node.coor).distance_sq(pointer_pos)))
        .filter_map(|(node, dist_sq)| (dist_sq < SELECTION_RADIUS_SQ).then_some((node, dist_sq)))
        .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
        .map(|(n, _)| n.node_key)
}
