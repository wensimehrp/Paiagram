use egui::*;
use itertools::Itertools;
use paiagram_core::spatial_index::{NodeRTreeObject, NodesRTree, SpatialCache};
use paiagram_core::time::TimetableTime;
use paiagram_core::{Coord, XyPos};
use paiagram_selection::{SelectedItem, SelectedItems};
use walkers::Projector;

use crate::App;
use crate::distance_to_line::distance_sq_to_line;
use crate::tiles::map::map_content::trip_leg_current_pos;
use crate::tiles::map::projection::XyProjection;

const SELECTION_RADIUS: f32 = 10.0;
const SELECTION_RADIUS_SQ: f32 = SELECTION_RADIUS * SELECTION_RADIUS;

pub fn select_map_contents(app: &mut App, ui: &mut Ui, response: &Response, projector: &Projector) {
    puffin::profile_function!();

    let coord_type = app.graph.coord_type;
    let pos_to_xy = |pos: Pos2| XyProjection::pos_to_xy(projector, pos);
    let xy_to_pos = |xy: XyPos| XyProjection::xy_to_pos(projector, xy);
    let pos_to_coord =
        |pos: Pos2| -> Coord { coord_type.unproject(XyProjection::pos_to_xy(projector, pos)) };

    // select stuff
    if response.clicked()
        && let Some(pointer_pos) = response.interact_pointer_pos()
        && let SelectedItems::None = app.selected_items.take()
    {
        let area = Rect::from_pos(pointer_pos).expand(SELECTION_RADIUS);
        let xy_min = pos_to_xy(area.left_top());
        let xy_max = pos_to_xy(area.right_bottom());
        _select_map_contents(xy_min, xy_max, pointer_pos, app, xy_to_pos, pos_to_coord);
    }
}

fn _select_map_contents(
    xy_min: XyPos,
    xy_max: XyPos,
    pointer_pos: Pos2,
    app: &mut App,
    xy_to_pos: impl Fn(XyPos) -> Pos2 + Clone,
    pos_to_coord: impl Fn(Pos2) -> Coord,
) {
    puffin::profile_function!();

    let SpatialCache {
        intervals,
        nodes,
        trip_legs,
    } = app.spatial();

    let curr_time = TimetableTime(app.timer.seconds() as i32);

    let selected = trip_legs
        .get(xy_min, xy_max, curr_time)
        .filter_map(|leg| {
            trip_leg_current_pos(leg.clone(), app.timer.seconds(), xy_to_pos.clone()).and_then(
                |pos| {
                    let dist_sq = pos.distance_sq(pointer_pos);
                    (dist_sq < SELECTION_RADIUS_SQ).then_some((leg, dist_sq))
                },
            )
        })
        .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
        .map(|(leg, _)| SelectedItem::Trip(leg.trip_key.into()))
        .or_else(|| {
            find_node_within_range(nodes, xy_min, xy_max, pointer_pos, xy_to_pos.clone())
                .map(|n| SelectedItem::Node(n.node_key))
        })
        .or_else(|| {
            intervals
                .get(xy_min, xy_max)
                .flat_map(|interval| {
                    interval
                        .nodes
                        .iter()
                        .copied()
                        .map(|(xy, _)| xy)
                        .map(xy_to_pos.clone())
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
            SelectedItems::Coordinate((pos_to_coord(pointer_pos), String::new())),
            SelectedItems::from,
        );

    app.selected_items = selected;
}

pub fn find_node_within_range(
    nodes: &NodesRTree,
    xy_min: XyPos,
    xy_max: XyPos,
    pointer_pos: Pos2,
    xy_to_pos: impl Fn(XyPos) -> Pos2 + Clone,
) -> Option<NodeRTreeObject> {
    nodes
        .get(xy_min, xy_max)
        .map(|node| (node, xy_to_pos(node.xy).distance_sq(pointer_pos)))
        .filter_map(|(node, dist_sq)| (dist_sq < SELECTION_RADIUS_SQ).then_some((node, dist_sq)))
        .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
        .map(|(n, _)| n)
}
