use egui::*;
use itertools::Itertools;
use paiagram_core::trip::TripSchedule;
use paiagram_core::{Coord, IntervalKey, Node, NodeKey, Station, StationKey, Wfc, World, XyPos};
use paiagram_selection::{SelectedItems, TripSelection};
use walkers::Projector;

use crate::App;
use crate::tiles::map::projection::XyProjection;
use crate::tiles::map::select_content::find_node_within_range;
use crate::trip_schedule_visual::WithVisualMod;

pub fn display_selected_content(
    app: &mut App,
    ui: &mut Ui,
    map_response: &Response,
    projector: &Projector,
) {
    puffin::profile_function!();

    let coord_type = app.graph.coord_type;
    let pos_to_xy = |pos: Pos2| XyProjection::pos_to_xy(projector, pos);
    let xy_to_pos = |xy: XyPos| XyProjection::xy_to_pos(projector, xy);
    let pos_to_coord =
        |pos: Pos2| -> Coord { coord_type.unproject(XyProjection::pos_to_xy(projector, pos)) };
    let coord_to_pos = |coor: Coord| XyProjection::xy_to_pos(projector, coord_type.project(coor));

    // display stuff
    let mut remove_smoothed_node_pos = true;
    let smoothed_node_id = ui.scope_id().with("node pointer animation");
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
                let points = map_trip_lines(&app, schedule, coord_to_pos);
                ui.painter().line(points, Stroke::new(3.0, Color32::GREEN));
            }
        }
        SelectedItems::TripExtension((trip_key, focused_pos))
            if let Some(trip) = app.trips.get(trip_key) =>
        {
            let schedule = trip.schedule.clone().with_pushed_entry(*focused_pos);
            let points = map_trip_lines(&app, schedule, coord_to_pos);
            ui.painter().line(points, Stroke::new(3.0, Color32::GREEN));
        }
        SelectedItems::Nodes(selected_nodes) => {
            remove_smoothed_node_pos = false;
            if let Some(pair_to_connect) = handle_node_selection(
                ui,
                map_response,
                selected_nodes,
                coord_to_pos,
                pos_to_xy,
                xy_to_pos,
                app,
                smoothed_node_id,
            ) {
                let _ = app.mutate(|mut world| {
                    world.graph.connect_nodes(pair_to_connect);
                    Ok(world)
                });
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
                    .map(coord_to_pos)
                    .collect();
                ui.painter().line(points, Stroke::new(3.0, Color32::RED));
            }
        }
        SelectedItems::Coordinate(_) => {
            let SelectedItems::Coordinate((coord, edit_buf)) = &mut app.selected_items else {
                unreachable!()
            };
            let pos = coord_to_pos(*coord);
            let resp = ui
                .interact(
                    Rect::from_pos(pos).expand(10.0),
                    ui.scope_id().with("coordinate interaction"),
                    Sense::drag(),
                )
                .on_hover_and_drag_cursor(CursorIcon::Grabbing)
                .on_hover_cursor(CursorIcon::Grab);
            ui.painter().circle(
                pos,
                5.0,
                Color32::RED.gamma_multiply(0.5),
                Stroke::new(1.5, Color32::RED),
            );
            if resp.dragged()
                && let Some(pointer_pos) = resp.interact_pointer_pos()
            {
                *coord = pos_to_coord(pointer_pos)
            }
            let popup = Popup::from_response(&resp).kind(PopupKind::Menu).show(|ui| {
                ui.add(WidgetAtom::new((
                    RichText::new("New Node").strong().atom_grow(true),
                    coord_type.display(*coord).atom_align(Align2::RIGHT_CENTER),
                )));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(edit_buf);
                    ui.button("Add").clicked()
                })
            });
            // stupid lifetime stuff
            if let Some(inner) = popup
                && inner.inner.inner
            {
                let new_name = std::mem::take(edit_buf);
                let new_coord = *coord;
                app.selected_items = SelectedItems::None;
                let _ = app.mutate(|mut world| {
                    let stn_key = StationKey::new();
                    world.stations.insert(
                        stn_key,
                        Wfc::new(Station {
                            name: new_name.clone().into(),
                            pos: new_coord,
                        }),
                    );
                    world.graph.insert_node(
                        NodeKey::new(),
                        Node {
                            name: new_name.into(),
                            parent: stn_key,
                            pos: new_coord,
                        },
                    );
                    Ok(world)
                });
            }
        }
        _ => {
            // unimplemented
        }
    }
    if remove_smoothed_node_pos {
        ui.data_mut(|data| {
            data.remove::<Pos2>(smoothed_node_id);
        })
    }
}

fn map_trip_lines(
    world: &World,
    schedule: TripSchedule,
    coord_to_pos: impl Fn(Coord) -> Pos2 + Clone,
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
                            .map(coord_to_pos.clone()),
                    )
                } else {
                    v.extend(
                        interval
                            .positions(interval_key, world.graph.nodes())
                            .rev()
                            .map(coord_to_pos.clone()),
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
                    .map(coord_to_pos.clone()),
                );
            }
        }
        v
    })
}

fn handle_node_selection(
    ui: &mut Ui,
    map_response: &Response,
    selected_nodes: &[NodeKey],
    coord_to_pos: impl Fn(Coord) -> Pos2 + Copy,
    pos_to_xy: impl Fn(Pos2) -> XyPos + Copy,
    xy_to_pos: impl Fn(XyPos) -> Pos2 + Copy,
    app: &App,
    smoothed_node_id: Id,
) -> Option<(NodeKey, NodeKey)> {
    for node in selected_nodes.iter().filter_map(|key| app.graph.nodes().get(key)) {
        ui.painter().circle(
            coord_to_pos(node.pos),
            5.0,
            Color32::RED.gamma_multiply(0.5),
            Stroke::new(1.5, Color32::RED),
        );
    }
    let selected = selected_nodes.first()?;
    let node = app.graph.nodes().get(selected)?;
    let pointer_pos = ui.pointer_hover_pos()?;
    let aabb = Rect::from_pos(pointer_pos).expand(200.0);
    let found = find_node_within_range(
        &app.spatial().nodes,
        pos_to_xy(aabb.left_top()),
        pos_to_xy(aabb.right_bottom()),
        pointer_pos,
        xy_to_pos,
    );
    let mut requires_repaint = false;
    let dt = ui.input(|input| input.stable_dt).at_most(0.1);
    let new_pos = found.map_or(pointer_pos, |n| xy_to_pos(n.xy));
    let smoothed_pos = ui.data_mut(|data| {
        let smoothed: &mut Pos2 = data.get_temp_mut_or(smoothed_node_id, new_pos);
        let t = emath::exponential_smooth_factor(0.9, 0.05, dt);
        *smoothed = smoothed.lerp(new_pos, t);
        if smoothed.distance_sq(new_pos) > 0.1 * 0.1 {
            requires_repaint = true;
        }
        *smoothed
    });
    if requires_repaint {
        ui.request_repaint();
    }
    ui.painter().line_segment(
        [coord_to_pos(node.pos), smoothed_pos],
        Stroke::new(2.0, Color32::RED),
    );
    ui.painter().circle(
        smoothed_pos,
        4.0,
        Color32::RED.gamma_multiply(0.5),
        Stroke::new(1.0, Color32::RED),
    );
    let connected_node = found?.node_key;
    map_response.clicked().then_some((*selected, connected_node))
}
