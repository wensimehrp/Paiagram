use egui::*;
use paiagram_core::RouteKey;
use paiagram_core::trip::{TEntry, TravelMode};
use serde::{Deserialize, Serialize};

use crate::font::TIMETABLTE_TEXT_STYLE;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct RouteTimetableTab {
    /// Scroll offset shared by the row header, column header and main grid.
    scroll: Vec2,
}

impl Default for RouteTimetableTab {
    fn default() -> Self {
        Self { scroll: Vec2::ZERO }
    }
}

impl super::Tab for RouteTimetableTab {
    const NAME: &'static str = "Route timetable";
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        let cell_size: Vec2 = vec2(45.0, 20.0);
        let total_rows: usize = 100;
        let total_cols: usize = app.trips.len();

        // Like `ScrollArea::show_rows`, but for the columns: only the visible columns are laid
        // out. Each cell has the same size, so we can jump straight to the visible range.
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let spacing = ui.spacing().item_spacing;
        let col_pitch = cell_size.x + spacing.x;
        let row_pitch = cell_size.y + spacing.y;
        let grid_height = row_pitch * total_rows as f32 - spacing.y;

        // All three scroll areas share this offset. Each starts from it, and whichever one the
        // user scrolled reports back an updated offset for the others to follow.
        let mut scroll = self.scroll;

        // window stroke for drawing strokes
        let stroke = ui.visuals().window_stroke;

        // Row header: follows the grid's vertical scroll.
        let left = Panel::left(ui.id().with("left panel"))
            .frame(Frame::NONE)
            .drag_to_open(true)
            .show(ui, |ui| {
                let max_size = [ui.available_width(), cell_size.y];
                ui.add_sized(max_size, Label::new("Stations").truncate());
                ScrollArea::vertical()
                    .auto_shrink(false)
                    .vertical_scroll_offset(scroll.y)
                    .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            for i in 0..total_rows {
                                ui.add_sized(max_size, Label::new(format!("{}", i)).truncate());
                            }
                        })
                    })
            });
        scroll.y = left.inner.state.offset.y;

        // Column header: follows the grid's horizontal scroll.
        let top = Panel::top(ui.id().with("top panel")).frame(Frame::NONE).show(ui, |ui| {
            ScrollArea::horizontal()
                .auto_shrink([false, true])
                .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
                .horizontal_scroll_offset(scroll.x)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for trip in app.trips.iter() {
                            ui.add_sized(cell_size, Label::new(trip.name.as_str()).truncate());
                        }
                    })
                })
        });
        scroll.x = top.inner.state.offset.x;

        // Main grid: scrolls both ways; whichever area moved last wins for the next frame.
        let center = CentralPanel::no_frame().show(ui, |ui| {
            ScrollArea::both()
                .auto_shrink(false)
                .vertical_scroll_offset(scroll.y)
                .horizontal_scroll_offset(scroll.x)
                .show_viewport(ui, |ui, viewport| {
                    ui.set_height(grid_height);
                    ui.set_width(col_pitch * total_cols as f32);

                    let mut min_col = (viewport.min.x / col_pitch).floor() as usize;
                    let mut max_col = (viewport.max.x / col_pitch).ceil() as usize + 1;
                    if max_col > total_cols {
                        let diff = max_col.saturating_sub(min_col);
                        max_col = total_cols;
                        min_col = total_cols.saturating_sub(diff);
                    }

                    let x_min = ui.max_rect().left() + min_col as f32 * col_pitch;
                    let x_max = ui.max_rect().left() + max_col as f32 * col_pitch;
                    let rect = Rect::from_x_y_ranges(x_min..=x_max, ui.max_rect().y_range());

                    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                        ui.horizontal_top(|ui| {
                            for trip in app.trips.iter().skip(min_col).take(max_col) {
                                ui.vertical(|ui| {
                                    for entry in trip.schedule.entries().iter().take(total_rows) {
                                        let disp = match match entry {
                                            TEntry::Derived { .. } => TravelMode::Flexible,
                                            TEntry::PinnedStop { dep, .. } => *dep,
                                            TEntry::PinnedPass { pass, .. } => *pass,
                                        } {
                                            TravelMode::Flexible => "..".into(),
                                            TravelMode::At(t) => t.to_oud2_str(false),
                                            TravelMode::For(d) => d.to_string_no_arrow(),
                                        };
                                        ui.add_sized(
                                            cell_size,
                                            Label::new(
                                                WidgetText::Text(disp)
                                                    .text_style(TIMETABLTE_TEXT_STYLE.clone()),
                                            ),
                                        );
                                    }
                                });
                            }
                        });
                        for col in min_col..max_col {
                            let x = rect.left() + (col - min_col + 1) as f32 * col_pitch;
                            ui.painter().vline(x, rect.y_range(), stroke);
                        }
                    });
                })
        });
        scroll = center.inner.state.offset;

        self.scroll = scroll;
    }
    fn title(&self) -> WidgetText {
        "Route Timetable".into()
    }
}
