use std::f32::consts::FRAC_PI_4;

use egui::style::WidgetVisuals;
use egui::*;
use itertools::{Itertools, chain};
use paiagram_core::trip::{EstimateEntry, TravelMode};
use paiagram_core::{CanvasLength, TripKey};

use crate::App;
use crate::tabs::Navigatable;
use crate::tabs::diagram::label_placement::label_placement;
use crate::widgets::buttons;

pub fn show_focused_trip(
    trip_key: &TripKey,
    app: &App,
    animation_progress: f32,
    ui: &mut Ui,
    tab: &mut super::DiagramTab,
    station_heights: &[CanvasLength],
    mut painter: &mut Painter,
    response: &Response,
) {
    let Some(trip) = app.source.trips.get(trip_key) else {
        return;
    };
    let stroke = trip.service_class.and_then(|key| app.source.service_classes.get(&key)).map_or(
        Stroke::new(lerp(1.0..=5.0, animation_progress), Color32::GRAY),
        |class| {
            Stroke::new(
                lerp((class.style.width as f32 * 1.0)..=5.0, animation_progress),
                class.style.color,
            )
        },
    );
    let text_color = ui.visuals().text_color().gamma_multiply(animation_progress);
    for polyline in tab.cache.map.get(trip_key).iter().flat_map(|&it| it) {
        let points: Vec<_> = polyline
            .iter()
            .flat_map(|&(estimate, _entry, idx, progress)| {
                let x1 = tab.navi.logical_x_to_screen_x(estimate.arr.to_ticks());
                let x2 = tab.navi.logical_x_to_screen_x(estimate.dep.to_ticks());
                let idx = idx as usize;
                let curr_y = tab.navi.logical_y_to_screen_y(station_heights[idx]);
                let next_y = tab.navi.logical_y_to_screen_y(station_heights[idx + 1]);
                let y = lerp(curr_y..=next_y, progress);
                [Pos2::new(x1, y), Pos2::new(x2, y)]
            })
            .collect();
        if let Some(&first) = points.first()
            && let Some(&last) = points.last()
            && let Some((estimate_first, ..)) = polyline.first()
            && let Some((estimate_last, ..)) = polyline.last()
        {
            let y_max =
                points.iter().fold(first.y, |acc, pos| if pos.y > acc { pos.y } else { acc })
                    + 20.0;
            painter.line(
                vec![first, pos2(first.x, y_max), pos2(last.x, y_max), last],
                Stroke::new(1.0, Color32::DARK_GREEN.gamma_multiply(animation_progress)),
            );
            painter.text(
                pos2(first.x.max(response.rect.left()), y_max) + Vec2::angled(-FRAC_PI_4) * 2.0,
                Align2::LEFT_BOTTOM,
                estimate_first.arr.to_string(),
                FontId::proportional(13.0),
                text_color,
            );
            painter.text(
                pos2(first.x.max(response.rect.left()), y_max) + Vec2::angled(FRAC_PI_4) * 2.0,
                Align2::LEFT_TOP,
                (estimate_last.dep - estimate_first.arr).to_string(),
                FontId::proportional(13.0),
                text_color,
            );
            painter.text(
                pos2(last.x.min(response.rect.right()), y_max)
                    + Vec2::angled(-FRAC_PI_4 * 3.0) * 2.0,
                Align2::RIGHT_BOTTOM,
                (estimate_last.dep).to_string(),
                FontId::proportional(13.0),
                text_color,
            );
        }
        painter.line(points.clone(), stroke);
        let (points, _) = points.as_chunks::<2>();
        for ((estimate, entry, ..), (prev, curr, next)) in std::iter::zip(
            polyline,
            chain!([None], points.iter().map(Some), [None]).tuple_windows(),
        ) {
            let &[pos_curr_arr, pos_curr_dep] = curr.unwrap();
            let pos_prev_dep = prev.map_or(pos_curr_arr, |&[_, p]| p);
            let pos_next_arr = next.map_or(pos_curr_dep, |&[p, _]| p);
            if estimate.arr != estimate.dep {
                let (align, direction) = label_placement(pos_prev_dep, pos_curr_arr, pos_curr_dep);
                painter.text(
                    pos_curr_arr + direction * 4.0,
                    align,
                    estimate.arr.to_string(),
                    FontId::proportional(13.0),
                    text_color,
                );
            }
            let (align, direction) = label_placement(pos_curr_arr, pos_curr_dep, pos_next_arr);
            painter.text(
                pos_curr_dep + direction * 4.0,
                align,
                estimate.dep.to_string(),
                FontId::proportional(13.0),
                text_color,
            );
            let EstimateEntry::Pinned(entry) = entry else {
                continue;
            };
            let button_radius = 8.0;
            let circle_handle_size: f32 = 7.0 / 12.0 * button_radius * 2.0;
            let triangle_handle_size: f32 = 10.0 / 12.0 * button_radius * 2.0;
            let dash_handle_size: f32 = 9.0 / 12.0 * button_radius * 2.0;
            let raw_delta = (button_radius * 2.0)
                - (pos_curr_dep.x - pos_curr_arr.x)
                    .clamp(-button_radius * 2.0, button_radius * 2.0);
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
            {
                let arr_pos = pos2(pos_curr_arr.x - delta_x, pos_curr_arr.y);
                let rect = Rect::from_pos(arr_pos).expand(button_radius);
                let curr_arr_response = ui.allocate_rect(rect, Sense::click_and_drag());
                let arr_visuals = ui.style().interact(&curr_arr_response);
                match entry.arr_or_pass {
                    TravelMode::At(t) => {
                        buttons::circle_button_shape(
                            &mut painter,
                            rect.center(),
                            circle_handle_size,
                            arr_visuals.bg_stroke,
                            arr_visuals.bg_fill,
                        );
                    }
                    TravelMode::For(d) => {
                        buttons::dash_button_shape(
                            &mut painter,
                            rect.center(),
                            dash_handle_size,
                            arr_visuals.bg_stroke,
                            arr_visuals.bg_fill,
                        );
                    }
                    TravelMode::Flexible => {
                        buttons::triangle_button_shape(
                            &mut painter,
                            rect.center(),
                            triangle_handle_size,
                            arr_visuals.bg_stroke,
                            arr_visuals.bg_fill,
                        );
                    }
                }
            }
            {
                let dep_pos = pos2(pos_curr_dep.x + delta_x, pos_curr_dep.y);
                let rect = Rect::from_pos(dep_pos).expand(button_radius);
                let curr_dep_response = ui.allocate_rect(rect, Sense::click_and_drag());
                let dep_visuals = ui.style().interact(&curr_dep_response);
                match entry.dep {
                    None => {
                        buttons::double_triangle(
                            &mut painter,
                            rect.center(),
                            dash_handle_size,
                            dep_visuals.bg_stroke,
                            dep_visuals.bg_fill,
                        );
                    }
                    Some(TravelMode::At(t)) => {
                        buttons::circle_button_shape(
                            &mut painter,
                            rect.center(),
                            circle_handle_size,
                            dep_visuals.bg_stroke,
                            dep_visuals.bg_fill,
                        );
                    }
                    Some(TravelMode::For(d)) => {
                        buttons::dash_button_shape(
                            &mut painter,
                            rect.center(),
                            dash_handle_size,
                            dep_visuals.bg_stroke,
                            dep_visuals.bg_fill,
                        );
                    }
                    Some(TravelMode::Flexible) => {
                        buttons::triangle_button_shape(
                            &mut painter,
                            rect.center(),
                            triangle_handle_size,
                            dep_visuals.bg_stroke,
                            dep_visuals.bg_fill,
                        );
                    }
                }
            }
        }
    }
}
