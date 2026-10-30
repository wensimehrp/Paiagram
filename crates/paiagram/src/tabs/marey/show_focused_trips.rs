use std::iter;

use egui::style::WidgetVisuals;
use egui::*;
use either::Either;
use itertools::{Itertools, chain};
use paiagram_core::route::{BucketPoint, TripBucket};
use paiagram_core::time::TDuration;
use paiagram_core::trip::TravelMode::Flexible;
use paiagram_core::trip::{EstimateEntry, TEntryId, TEstimate, TravelMode, TripSchedule};
use paiagram_core::{CanvasLength, Source, TripKey};

use crate::font::TIMETABLE_FONT_ID;
use crate::tabs::Navigatable;
use crate::tabs::marey::MareyTabNavigation;
use crate::tabs::marey::label_placement::label_placement;
use crate::widgets::buttons;

pub fn show_focused_trip<'a>(
    trip_schedule: &TripSchedule,
    stroke: Stroke,
    source: &mut Source,
    animation_progress: f32,
    ui: &mut Ui,
    tab: &mut super::MareyTab,
    station_heights: &[CanvasLength],
    painter: &mut Painter,
    polylines: TripBucket,
    trip_key: TripKey,
    estimate_base: Option<TEstimate>,
) -> Option<(TEntryId, TEstimate, Either<TDuration, TDuration>)> {
    puffin::profile_function!();
    let text_color = ui.visuals().text_color().gamma_multiply(animation_progress);

    polylines.iter().fold(None, |acc, polyline| {
        let points = draw_polyline(&tab.navi, polyline, station_heights, painter, stroke);
        let ret = show_trip_handles(
            trip_schedule,
            tab,
            source,
            polyline,
            &points,
            text_color,
            ui,
            painter,
            trip_key,
            estimate_base,
        );
        acc.or(ret)
    })
}

/// Draws the path of one trip, without any labels or interactive handles.
///
/// Returns the screen-space positions of the arrival/departure pairs, so that
/// [`show_trip_handles`] can place its handles on top of the line.
pub fn draw_polyline(
    navi: &MareyTabNavigation,
    polyline: &[BucketPoint],
    station_heights: &[CanvasLength],
    painter: &mut Painter,
    stroke: Stroke,
) -> Vec<Pos2> {
    puffin::profile_scope!("trip line");
    let points: Vec<_> = polyline
        .iter()
        .flat_map(|&(estimate, _entry, idx, progress)| {
            let x1 = navi.logical_x_to_screen_x(estimate.arr.to_ticks());
            let x2 = navi.logical_x_to_screen_x(estimate.dep.to_ticks());
            let idx = idx as usize;
            let curr_y = navi.logical_y_to_screen_y(station_heights[idx]);
            let next_y = navi.logical_y_to_screen_y(station_heights[idx + 1]);
            let y = lerp(curr_y..=next_y, progress);
            [Pos2::new(x1, y), Pos2::new(x2, y)]
        })
        .collect();
    painter.line(points.clone(), stroke);
    points
}

/// Draws the arrival/departure labels and the interactive handles of one trip.
///
/// Returns the time patch produced by dragging a handle, if any.
fn show_trip_handles(
    trip_schedule: &TripSchedule,
    tab: &super::MareyTab,
    source: &mut Source,
    polyline: &[BucketPoint],
    points: &[Pos2],
    text_color: Color32,
    ui: &mut Ui,
    mut painter: &mut Painter,
    trip_key: TripKey,
    estimate_base: Option<TEstimate>,
) -> Option<(TEntryId, TEstimate, Either<TDuration, TDuration>)> {
    puffin::profile_scope!("trip handles");
    let (points, _) = points.as_chunks::<2>();
    let handle_iter = iter::zip(
        polyline,
        chain!(iter::once(None), points.iter().map(Some), iter::once(None)).tuple_windows(),
    );

    handle_iter.fold(None, |ret, (a, b)| {
        let (estimate_visual, entry, ..) = a;
        let (prev, curr, next) = b;
        let estimate_entry = estimate_base.unwrap_or(*estimate_visual);
        let &[pos_curr_arr, pos_curr_dep] = curr.unwrap();
        let pos_prev_dep = prev.map_or(pos_curr_arr, |&[_, p]| p);
        let pos_next_arr = next.map_or(pos_curr_dep, |&[p, _]| p);
        // arrival text
        let ((arr_align, arr_direction), (dep_align, dep_direction)) =
            if estimate_visual.arr == estimate_visual.dep {
                let (arr_align, arr_direction) =
                    label_placement(pos_prev_dep, pos_curr_dep, pos_next_arr);
                (
                    (arr_align, arr_direction),
                    (arr_align.flipped(), -arr_direction),
                )
            } else {
                let (dep_align, dep_direction) =
                    label_placement(pos_curr_arr, pos_curr_dep, pos_next_arr);
                (
                    label_placement(pos_prev_dep, pos_curr_arr, pos_curr_dep),
                    (dep_align.flipped(), -dep_direction),
                )
            };

        painter.text(
            pos_curr_arr + arr_direction * 10.0,
            arr_align,
            estimate_visual.arr,
            TIMETABLE_FONT_ID.clone(),
            ui.visuals().strong_text_color(),
        );

        if estimate_visual.arr != estimate_visual.dep {
            painter.text(
                pos_curr_dep + dep_direction * 10.0,
                dep_align,
                estimate_visual.dep,
                TIMETABLE_FONT_ID.clone(),
                ui.visuals().strong_text_color(),
            );
        }

        let EstimateEntry::Pinned(entry) = entry else {
            return ret;
        };

        let button_radius = 8.0;
        let circle_handle_size: f32 = 7.0 / 12.0 * button_radius * 2.0;
        let triangle_handle_size: f32 = 10.0 / 12.0 * button_radius * 2.0;
        let dash_handle_size: f32 = 9.0 / 12.0 * button_radius * 2.0;
        let raw_delta = (button_radius * 2.0)
            - (pos_curr_dep.x - pos_curr_arr.x).clamp(-button_radius * 2.0, button_radius * 2.0);
        let delta_x = (raw_delta / 2.0) * 0.8;
        let extreme_bg_color = ui.visuals().extreme_bg_color;
        let bg_stroke = if ui.visuals().dark_mode {
            Stroke::new(1.0, ui.visuals().text_color())
        } else {
            Stroke::new(2.5, Color32::BLACK)
        };
        let style = ui.style_mut();
        style.visuals.widgets.inactive = WidgetVisuals {
            bg_fill: extreme_bg_color,
            bg_stroke,
            ..style.visuals.widgets.inactive
        };
        style.visuals.widgets.hovered = WidgetVisuals {
            bg_fill: extreme_bg_color.lerp_to_gamma(text_color, 0.25),
            bg_stroke,
            ..style.visuals.widgets.inactive
        };
        style.visuals.widgets.active = WidgetVisuals {
            bg_fill: extreme_bg_color.lerp_to_gamma(text_color, 0.5),
            bg_stroke,
            ..style.visuals.widgets.inactive
        };
        let ret = {
            let arr_pos = pos2(pos_curr_arr.x - delta_x, pos_curr_arr.y);
            let curr_arr_response = ui.allocate_rect(
                Rect::from_pos(arr_pos).expand(button_radius),
                Sense::click_and_drag(),
            );
            let arr_visuals = ui.style().interact(&curr_arr_response);
            let (paint_fn, handle_size): (fn(&mut Painter, Pos2, f32, Stroke, Color32) -> (), f32) =
                match entry.arr_or_pass {
                    TravelMode::At(..) => (buttons::circle_button_shape, circle_handle_size),
                    TravelMode::For(..) => (buttons::dash_button_shape, dash_handle_size),
                    TravelMode::Flexible => (buttons::triangle_button_shape, triangle_handle_size),
                };
            paint_fn(
                &mut painter,
                curr_arr_response.rect.center(),
                handle_size,
                arr_visuals.bg_stroke,
                arr_visuals.bg_fill,
            );
            let delta_t = ui.input(|r| r.pointer.latest_pos()).map_or(TDuration::ZERO, |p| {
                tab.navi.screen_x_to_logical_x(p.x).to_timetable_time() - estimate_entry.arr
            });
            if curr_arr_response.drag_stopped() {
                let _ = source.mutate(|mut world| {
                    let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                    trip.schedule.shift_arr_or_pass(entry.id, delta_t);
                    Ok(world)
                });
            }
            Popup::menu(&curr_arr_response).align(RectAlign::outside(arr_align.flipped())).show(
                |ui| {
                    let [show_at, show_for, show_flexible] = [
                        !matches!(entry.arr_or_pass, TravelMode::At(_)),
                        !matches!(entry.arr_or_pass, TravelMode::For(_)),
                        !matches!(entry.arr_or_pass, TravelMode::Flexible),
                    ];
                    let btn = Button::new("At").right_text(
                        RichText::from(format!("{}", estimate_visual.arr))
                            .font(TIMETABLE_FONT_ID.clone()),
                    );
                    if ui.add_enabled(show_at, btn).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_arr_or_pass_mode(
                                entry.id,
                                TravelMode::At(estimate_visual.arr),
                            );
                            Ok(world)
                        });
                    }
                    let duration = trip_schedule.estimates(&source.graph, |estimates| {
                        trip_schedule.dur_since_prev_stable(estimates, entry.id)
                    });
                    let btn = Button::new("For").right_text(
                        RichText::from(
                            duration.map_or("--:--:--".into(), |d| d.to_string_no_arrow()),
                        )
                        .font(TIMETABLE_FONT_ID.clone()),
                    );
                    if ui.add_enabled(show_for, btn).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_arr_or_pass_mode(
                                entry.id,
                                TravelMode::For(duration.unwrap_or_default()),
                            );
                            Ok(world)
                        });
                    }
                    if ui.add_enabled(show_flexible, Button::new("Flexible")).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_arr_or_pass_mode(entry.id, Flexible);
                            Ok(world)
                        });
                    }
                },
            );
            ret.or(curr_arr_response.dragged().then_some((
                entry.id,
                estimate_entry,
                Either::Left(delta_t),
            )))
        };
        {
            let dep_pos = pos2(pos_curr_dep.x + delta_x, pos_curr_dep.y);
            let curr_dep_response = ui.allocate_rect(
                Rect::from_pos(dep_pos).expand(button_radius),
                Sense::click_and_drag(),
            );
            let dep_visuals = ui.style().interact(&curr_dep_response);
            let (paint_fn, handle_size): (fn(&mut Painter, Pos2, f32, Stroke, Color32) -> (), f32) =
                match entry.dep {
                    None => (buttons::double_triangle, dash_handle_size),
                    Some(TravelMode::At(..)) => (buttons::circle_button_shape, circle_handle_size),
                    Some(TravelMode::For(..)) => (buttons::dash_button_shape, dash_handle_size),
                    Some(TravelMode::Flexible) => {
                        (buttons::triangle_button_shape, triangle_handle_size)
                    }
                };
            paint_fn(
                &mut painter,
                curr_dep_response.rect.center(),
                handle_size,
                dep_visuals.bg_stroke,
                dep_visuals.bg_fill,
            );
            let delta_t = ui.input(|r| r.pointer.latest_pos()).map_or(TDuration::ZERO, |p| {
                tab.navi.screen_x_to_logical_x(p.x).to_timetable_time() - estimate_entry.dep
            });
            if curr_dep_response.drag_stopped() {
                let _ = source.mutate(|mut world| {
                    let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                    trip.schedule.shift_dep(entry.id, delta_t);
                    Ok(world)
                });
            }
            Popup::menu(&curr_dep_response).align(RectAlign::outside(arr_align.flipped())).show(
                |ui| {
                    let [show_at, show_for, show_flexible, show_non_stop] = [
                        !matches!(entry.dep, Some(TravelMode::At(_))),
                        !matches!(entry.dep, Some(TravelMode::For(_))),
                        !matches!(entry.dep, Some(TravelMode::Flexible)),
                        !matches!(entry.dep, None),
                    ];
                    let btn = Button::new("At").right_text(
                        RichText::from(format!("{}", estimate_visual.dep))
                            .font(TIMETABLE_FONT_ID.clone()),
                    );
                    if ui.add_enabled(show_at, btn).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule
                                .set_dep_mode(entry.id, Some(TravelMode::At(estimate_visual.dep)));
                            Ok(world)
                        });
                    }
                    let duration = estimate_visual.duration();
                    let btn = Button::new("For").right_text(
                        RichText::from(duration.to_string_no_arrow())
                            .font(TIMETABLE_FONT_ID.clone()),
                    );
                    if ui.add_enabled(show_for, btn).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_dep_mode(entry.id, Some(TravelMode::For(duration)));
                            Ok(world)
                        });
                    }
                    if ui.add_enabled(show_flexible, Button::new("Flexible")).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_dep_mode(entry.id, Some(Flexible));
                            Ok(world)
                        });
                    }
                    if ui.add_enabled(show_non_stop, Button::new("Non-Stop")).clicked() {
                        let _ = source.mutate(|mut world| {
                            let trip = world.trips.get_mut(&trip_key).ok_or("wtf")?;
                            trip.schedule.set_dep_mode(entry.id, None);
                            Ok(world)
                        });
                    }
                },
            );
            ret.or(curr_dep_response.dragged().then_some((
                entry.id,
                estimate_entry,
                Either::Right(delta_t),
            )))
        }
    })
}
