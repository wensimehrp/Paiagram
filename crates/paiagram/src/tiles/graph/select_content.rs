use egui::*;
use itertools::Itertools;
use paiagram_core::spatial_index::SpatialCache;
use paiagram_core::time::TimetableTime;
use paiagram_core::trip::TripSchedule;
use paiagram_core::{IntervalKey, LonLat, Wgs84LonLat, World};
use paiagram_selection::{SelectedItem, SelectedItems, TripSelection};
use walkers::{MapMemory, Projector, lon_lat};

use crate::App;
use crate::distance_to_line::distance_sq_to_line;
use crate::tiles::graph::map_content::trip_leg_current_pos;
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

    // display stuff
    match &app.selected_items {
        SelectedItems::Trips(trips) => {
            for TripSelection {
                trip_key,
                editing_offset,
            } in trips
            {
                let Some(trip) = app.trips.get(trip_key) else {
                    continue;
                };
                let schedule = trip.schedule.clone().with_entry_shift(*editing_offset);
                let points = map_trip_lines(&app, schedule, lon_lat_to_pos);
                ui.painter().line(points, Stroke::new(3.0, Color32::GREEN));
            }
        }
        SelectedItems::TripExtension((trip_key, focused_pos))
            if let Some(trip) = app.trips.get(trip_key) =>
        {
            let schedule = trip.schedule.clone().with_pushed_entry(*focused_pos);
            let points = map_trip_lines(&app, schedule, lon_lat_to_pos);
            ui.painter().line(points, Stroke::new(3.0, Color32::GREEN));
        }
        SelectedItems::Nodes(nodes) => {
            for node in nodes.iter().filter_map(|key| app.graph.nodes().get(key)) {
                ui.painter().circle(
                    lon_lat_to_pos(node.pos),
                    5.0,
                    Color32::RED.gamma_multiply(0.5),
                    Stroke::new(1.5, Color32::RED),
                );
            }
        }
        // SelectedItems::TripExtension((trip_key, focused_pos)) => {}
        SelectedItems::Intervals(intervals) => {
            for interval_key in intervals {
                let Some(interval) = app.graph.intervals().get(interval_key) else {
                    continue;
                };
                let points: Vec<_> = interval
                    .positions(*interval_key, app.graph.nodes())
                    .map(lon_lat_to_pos)
                    .collect();
                ui.painter().line(points, Stroke::new(3.0, Color32::RED));
            }
        }
        _ => {
            // unimplemented
        }
    }

    // select stuff
    if response.clicked()
        && let Some(pointer_pos) = response.interact_pointer_pos()
        && let SelectedItems::None = app.selected_items.take()
    {
        let area = Rect::from_pos(pointer_pos).expand(SELECTION_RADIUS);
        let coor_min = pos_to_lon_lat(area.left_top());
        let coor_max = pos_to_lon_lat(area.right_bottom());
        _select_map_contents(coor_min, coor_max, pointer_pos, app, lon_lat_to_pos);
    }
}

fn _select_map_contents(
    coor_min: LonLat,
    coor_max: LonLat,
    pointer_pos: Pos2,
    app: &mut App,
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2 + Clone,
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
            nodes
                .get(coor_min, coor_max)
                .map(|node| (node, lon_lat_to_pos(node.coor).distance_sq(pointer_pos)))
                .filter_map(|(node, dist_sq)| {
                    (dist_sq < SELECTION_RADIUS_SQ).then_some((node, dist_sq))
                })
                .min_by(|(_, d1), (_, d2)| f32::total_cmp(d1, d2))
                .map(|(node, _)| SelectedItem::Node(node.node_key))
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
        .unwrap_or(SelectedItem::None);

    app.selected_items.replace(selected);
}

fn map_trip_lines(
    world: &World,
    schedule: TripSchedule,
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2 + Clone,
) -> Vec<Pos2> {
    schedule.estimates(&world.graph, |estimates| {
        let mut v = Vec::new();
        for (prev, curr) in estimates.iter().map(|(_, entry)| entry.node_key()).tuple_windows() {
            let interval_key = IntervalKey::new(prev, curr);
            if let Some(interval) = world.graph.intervals().get(&interval_key) {
                if interval_key.hi == prev {
                    v.extend(
                        interval
                            .positions(interval_key, world.graph.nodes())
                            .map(lon_lat_to_pos.clone()),
                    )
                } else {
                    v.extend(
                        interval
                            .positions(interval_key, world.graph.nodes())
                            .rev()
                            .map(lon_lat_to_pos.clone()),
                    )
                }
            } else {
                v.extend(
                    [
                        world.graph.nodes().get(&prev).map(|wfc| wfc.pos),
                        world.graph.nodes().get(&curr).map(|wfc| wfc.pos),
                    ]
                    .iter()
                    .flatten()
                    .copied()
                    .map(lon_lat_to_pos.clone()),
                );
            }
        }
        v
    })
}
