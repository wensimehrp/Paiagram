use egui::*;
use itertools::Itertools;
use paiagram_core::trip::TripSchedule;
use paiagram_core::{
    IntervalKey, LonLat, Node, NodeKey, Station, StationKey, Wfc, Wgs84LonLat, World,
};
use paiagram_selection::{SelectedItems, TripSelection};
use walkers::{MapMemory, Projector, lon_lat};

use crate::App;
use crate::trip_schedule_visual::WithVisualMod;

pub fn display_selected_content(
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
            if let Some(to_connect) = handle_node_selection(ui, nodes, lon_lat_to_pos, app) {}
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
        SelectedItems::Coordinate(_) => {
            let SelectedItems::Coordinate((coord, edit_buf)) = &mut app.selected_items else {
                unreachable!()
            };
            let pos = lon_lat_to_pos(*coord);
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
                *coord = pos_to_lon_lat(pointer_pos)
            }
            let popup = Popup::from_response(&resp).kind(PopupKind::Menu).show(|ui| {
                ui.add(WidgetAtom::new((
                    RichText::new("New Node").strong().atom_grow(true),
                    Wgs84LonLat::from(*coord).to_string().atom_align(Align2::RIGHT_CENTER),
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

fn handle_node_selection(
    ui: &mut Ui,
    nodes: &[NodeKey],
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2 + Clone,
    app: &App,
) -> Option<NodeKey> {
    for node in nodes.iter().filter_map(|key| app.graph.nodes().get(key)) {
        ui.painter().circle(
            lon_lat_to_pos(node.pos),
            5.0,
            Color32::RED.gamma_multiply(0.5),
            Stroke::new(1.5, Color32::RED),
        );
    }
    let selected = nodes.first()?;
    if nodes.len() > 0 {
        return None;
    };
    let mut requires_repaint = false;
    // let new_pos = station_heights[station_idx];
    // let smoothed_logical_y = ui.data_mut(|data| {
    //     let smoothed: &mut CanvasLength =
    //         data.get_temp_mut_or(ui.scope_id().with("pointer animation"), new_logical_y);
    //     let t = emath::exponential_smooth_factor(0.9, 0.05, dt);
    //     *smoothed = CanvasLength(lerp(smoothed.0..=new_logical_y.0, t as f64));
    //     if (*smoothed - new_logical_y).0.abs() > 0.1 {
    //         requires_repaint = true;
    //     }
    //     *smoothed
    // });
    if requires_repaint {
        ui.request_repaint();
    }
    None
}
