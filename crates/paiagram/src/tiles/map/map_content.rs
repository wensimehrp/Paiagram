use egui::emath::inverse_lerp;
use egui::*;
use paiagram_core::spatial_index::{IntervalRTreeObject, NodeRTreeObject, TripLegRTreeObject};
use paiagram_core::time::TimetableTime;
use paiagram_core::{LonLat, Wgs84LonLat};
use walkers::{MapMemory, Projector, lon_lat};

use crate::App;
use crate::tiles::map::gpu_draw::GraphCallback;
use crate::tiles::map::graph_intervals::graph_intervals::IntervalPoint;
use crate::tiles::map::graph_nodes::graph_nodes::NodePoint;

const ICON_HALF_SIZE: f32 = 18.0;

/// Decodes the embedded trip icon and uploads it as an egui texture. Every trip uses the same
/// texture for now, so egui batches all icon quads into one draw call.
pub fn load_trip_icon(ctx: &Context) -> TextureHandle {
    let decoded = image::load_from_memory(include_bytes!("../../../../../assets/raicho_485.png"))
        .expect("the embedded trip icon must be a valid PNG")
        .into_rgba8();

    // The icon is drawn from one square quad, so pad it with transparent pixels instead of
    // stretching it. This preserves the original aspect ratio.
    let side = decoded.width().max(decoded.height());
    let mut rgba = image::RgbaImage::new(side, side);
    let x = i64::from((side - decoded.width()) / 2);
    let y = i64::from((side - decoded.height()) / 2);
    image::imageops::overlay(&mut rgba, &decoded, x, y);

    let image = ColorImage::from_rgba_unmultiplied([side as usize, side as usize], rgba.as_raw());
    ctx.load_texture("graph_trip_icon", image, TextureOptions::LINEAR)
}

pub fn render_map_contents(
    app: &App,
    trip_icon: TextureId,
    ui: &mut Ui,
    response: &Response,
    projector: &Projector,
    map_memory: &MapMemory,
) {
    puffin::profile_function!();

    const DISPLAY_ZOOM_THRESHOLD: f64 = 8.0;
    let strength = ui.animate_bool_responsive(
        ui.scope_id().with("text display animation"),
        map_memory.zoom() > DISPLAY_ZOOM_THRESHOLD,
    );

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

    let coor_min = pos_to_lon_lat(response.rect.left_top());
    let coor_max = pos_to_lon_lat(response.rect.right_bottom());

    // Intervals
    let mut callback = GraphCallback::new();
    {
        puffin::profile_scope!("map_intervals");
        callback.populate_uniforms(response.rect);
        for IntervalRTreeObject {
            interval_key: _,
            nodes,
        } in app.spatial().intervals.get(coor_min, coor_max)
        {
            let Some((last, rest)) = nodes.split_last() else {
                continue;
            };
            callback.interval_points.extend(
                rest.iter()
                    .copied()
                    .map(lon_lat_to_pos)
                    .map(|pos| IntervalPoint::new((pos - response.rect.left_top()).into(), 1)),
            );
            let pos = lon_lat_to_pos(*last) - response.rect.left_top();
            callback.interval_points.push(IntervalPoint::new(pos.into(), 0));
        }
    }

    // Nodes
    {
        puffin::profile_scope!("draw_nodes");
        let node_text_color = ui.visuals().strong_text_color().gamma_multiply(strength);
        for NodeRTreeObject { node_key, coor } in app.spatial().nodes.get(coor_min, coor_max) {
            let Some(node) = app.graph.nodes().get(&node_key) else {
                continue;
            };
            let draw_pos = lon_lat_to_pos(coor);
            callback.node_points.push(NodePoint::new(
                (draw_pos - response.rect.left_top()).into(),
                u32::from_le_bytes(Color32::GRAY.to_array()),
            ));
            if strength > 0.1 {
                ui.painter().text(
                    draw_pos + vec2(0.0, 4.0),
                    Align2::CENTER_TOP,
                    node.name.as_str(),
                    FontId::proportional(13.0),
                    node_text_color,
                );
            }
        }
    }

    // Trip legs
    {
        let tint = Color32::WHITE.gamma_multiply(strength);
        puffin::profile_scope!("draw_trip_legs");
        let map_trip_legs = app
            .spatial()
            .trip_legs
            .get(
                coor_min,
                coor_max,
                TimetableTime(app.timer.seconds() as i32),
            )
            .filter_map(|trip_leg| {
                app.trips.get(&trip_leg.trip_key).zip(trip_leg_current_pos(
                    trip_leg,
                    app.timer.seconds(),
                    lon_lat_to_pos,
                ))
            })
            .map(|(trip, draw_pos)| {
                (
                    trip,
                    draw_pos,
                    app.get_service_class_stroke_or_default(trip.service_class).color,
                )
            });
        if strength > 0.1 {
            ui.painter().add(callback.paint_callback(response.rect));
            // simply use egui's image features
            for (trip, draw_pos, fill) in map_trip_legs {
                ui.painter().image(
                    trip_icon,
                    Rect::from_center_size(draw_pos, Vec2::splat(ICON_HALF_SIZE * 2.0)),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    tint,
                );
                ui.painter().text(
                    draw_pos + Vec2::new(0.0, -ICON_HALF_SIZE),
                    Align2::CENTER_BOTTOM,
                    trip.name.as_str(),
                    FontId::proportional(13.0),
                    fill.gamma_multiply(strength),
                );
            }
        } else {
            for (_, draw_pos, fill) in map_trip_legs {
                callback.node_points.push(NodePoint::new(
                    (draw_pos - response.rect.left_top()).into(),
                    u32::from_le_bytes(fill.to_array()),
                ));
            }
            ui.painter().add(callback.paint_callback(response.rect));
        }
    }
}

pub fn trip_leg_current_pos(
    TripLegRTreeObject {
        interval,
        is_hi_to_lo,
        trip_key: _,
        curr_arr: _,
        curr_dep,
        next_arr,
    }: TripLegRTreeObject,
    current_secs: f64,
    lon_lat_to_pos: impl Fn(LonLat) -> Pos2,
) -> Option<Pos2> {
    let curr_progress = {
        let t1 = curr_dep.0 as f64;
        let t2 = next_arr.0 as f64;
        // curr arr time is simply out of range.
        // Just clamp the value so it stays at the station...
        inverse_lerp(t1..=t2, current_secs).unwrap_or(0.0).at_least(0.0) as f32
    };
    // current_secs is a float while t1 and t2 are i32
    // this leaves a ~1 in-app second gap that duplicates entries.
    // simply remove the entry when progress > 1.0.
    if curr_progress > 1.0 {
        return None;
    }
    let curr_progress = if is_hi_to_lo {
        curr_progress
    } else {
        1.0 - curr_progress
    };
    // Simple O(n) find
    let prev_idx: usize = interval
        .iter()
        .enumerate()
        .find_map(|(idx, &(_, interval_progress))| {
            (curr_progress >= interval_progress).then_some(idx)
        })
        .unwrap_or(0);

    let (prev_coor, prev_progress) = interval[prev_idx];
    let (next_coor, next_progress) = interval[prev_idx + 1];
    let normalized_progress =
        inverse_lerp(prev_progress..=next_progress, curr_progress).unwrap_or(0.0);
    let prev_pos = lon_lat_to_pos(prev_coor);
    let next_pos = lon_lat_to_pos(next_coor);
    Some(prev_pos.lerp(next_pos, normalized_progress))
}
