use std::iter;
use std::ops::Range;

use ecow::EcoVec;
use egui::scroll_area::{ScrollAreaOutput, ScrollBarVisibility};
use egui::*;
use either::Either;
use itertools::{Itertools, chain, izip};
use paiagram_core::route::{
    RouteTimetable, RouteTimetableEntry, RouteTimetableExport, StationDisplayMode, StationRecord,
};
use paiagram_core::{RouteKey, TripKey};
use paiagram_rw::ExportObject;
use serde::{Deserialize, Serialize};

use crate::UiCommand;
use crate::font::TIMETABLTE_TEXT_STYLE;
use crate::tabs::trip::TripTab;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct RouteTimetableTab {
    /// Scroll offset shared by the row header, column header and main grid.
    scroll: Vec2,
    route_key: RouteKey,
    #[serde(skip)]
    cache: RouteTimetable,
    show_estimates: bool,
    show_inbound: bool,
}

impl PartialEq for RouteTimetableTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_key == other.route_key
    }
}

impl RouteTimetableTab {
    pub fn new(route_key: RouteKey) -> Self {
        Self {
            scroll: Vec2::ZERO,
            route_key,
            cache: RouteTimetable::default(),
            show_estimates: false,
            show_inbound: true,
        }
    }
}

impl super::Tab for RouteTimetableTab {
    const NAME: &'static str = "Route timetable";
    fn main_display(&mut self, app: &mut crate::App, ui: &mut Ui) {
        puffin::profile_function!();
        let Some(route) = app.routes.get(&self.route_key) else {
            ui.centered_and_justified(|ui| ui.heading("Route does not exist"));
            return;
        };

        ui.horizontal(|ui| {
            puffin::profile_scope!("timetable_toolbar");
            if ui.button("refresh cache").clicked() {
                self.cache = route.intervals.populate_route_timetable(&app.world);
            }
            if ui.button("Export .csv").clicked() {
                puffin::profile_scope!("export_csv");
                RouteTimetableExport(self.cache.clone(), app.world.clone())
                    .write_to_file(Default::default());
            }
            ui.checkbox(&mut self.show_estimates, "Show estimates");
            ui.checkbox(&mut self.show_inbound, "Show Inbound");
        });

        let cell_size: Vec2 = vec2(50.0, 20.0);

        // remove spacing between buttons
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        // force floating to avoid bouncing ui when scrolled to the end
        ui.style_mut().spacing.scroll.floating = true;
        // Remove button background
        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;

        let (station_records, columns) = if self.show_inbound {
            let records = self.cache.station_records.make_mut().iter_mut();
            let iter = records.map(|(stn, inbound_mode, _)| (&*stn, inbound_mode));
            (Either::Left(iter), self.cache.inbound_columns.as_slice())
        } else {
            let records = self.cache.station_records.make_mut().iter_mut().rev();
            let iter = records.map(|(stn, _, outbound_mode)| (&*stn, outbound_mode));
            (Either::Right(iter), self.cache.outbound_columns.as_slice())
        };

        Panel::left("left panel")
            .frame(Frame::NONE)
            .drag_to_open(true)
            .default_size(cell_size.x * 2.0)
            .show(ui, |ui| {
                puffin::profile_scope!("station_header");
                self.scroll = show_station_header(app, ui, cell_size, station_records, self.scroll);
            });

        Panel::top("top panel").frame(Frame::NONE).show(ui, |ui| {
            puffin::profile_scope!("trip_header");
            self.scroll = show_trip_header(app, ui, cell_size, columns, self.scroll);
        });

        let station_records = if self.show_inbound {
            let iter = self.cache.station_records.iter().map(|(r, mode, _)| (r, mode));
            Either::Left(iter)
        } else {
            let iter = self.cache.station_records.iter().rev().map(|(r, _, mode)| (r, mode));
            Either::Right(iter)
        };

        CentralPanel::no_frame().show(ui, |ui| {
            puffin::profile_scope!("timetable_grid");
            let (scroll, paint_params) = show_grid(
                self.show_estimates,
                app,
                ui,
                cell_size,
                station_records,
                columns,
                self.scroll,
            );
            self.scroll = scroll;
            if let Some((pos, record)) = paint_params {
                let stroke = Stroke::new(2.0, Color32::RED);
                ui.painter().vline(pos.x, ui.clip_rect().y_range(), stroke);
                ui.painter().hline(ui.clip_rect().x_range(), pos.y, stroke);
                let name = record
                    .station_key(app.graph.nodes())
                    .and_then(|key| app.stations.get(key))
                    .map_or_default(|wfc| wfc.name.as_str());
                ui.painter().text(
                    pos,
                    Align2::LEFT_BOTTOM,
                    name,
                    FontId::proportional(13.0),
                    ui.visuals().text_color(),
                );
            }
        });
    }
}

/// Left panel: the station names and their per-row display modes.
fn show_station_header<'a, 'world>(
    app: &'world crate::App,
    ui: &mut Ui,
    cell_size: Vec2,
    station_records: impl Iterator<Item = (&'a StationRecord, &'a mut StationDisplayMode)>,
    scroll: Vec2,
) -> Vec2 {
    puffin::profile_function!();
    let cell_size = vec2(ui.available_width(), cell_size.y);
    ui.add_sized(cell_size, Label::new("Stations").truncate());
    let scroll_area = ScrollArea::vertical()
        .auto_shrink(false)
        .vertical_scroll_offset(scroll.y)
        .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            for (record, display_mode) in station_records {
                let station_name = record
                    .station_key(app.world.graph.nodes())
                    .and_then(|key| app.world.stations.get(key))
                    .map_or_default(|wfc| wfc.name.as_str());
                let mut cell_size = cell_size;
                cell_size.y *= display_mode.bits().count_ones().at_least(1) as f32;
                let row_type_atoms = ContainerAtom::new(Atoms::from_iter(
                    display_mode
                        .iter()
                        .map(|mode| match mode {
                            StationDisplayMode::ARRIVAL => Atom::from("Ar."),
                            StationDisplayMode::TRACK => Atom::from("Tr."),
                            StationDisplayMode::DEPARTURE => Atom::from("De."),
                            _ => Atom::from("??"),
                        })
                        .map(|atm| {
                            atm.atom_grow(false).atom_shrink(false).atom_align(Align2::RIGHT_TOP)
                        }),
                ))
                .align2(Align2::RIGHT_CENTER)
                .direction(Direction::TopDown);
                let mut button = Button::new(Atoms::from_iter(iter::chain(
                    iter::once(
                        Atom::from(station_name).atom_grow(true).atom_align(Align2::LEFT_CENTER),
                    ),
                    row_type_atoms.into_atoms(),
                )))
                .truncate()
                .min_size(cell_size)
                .atom_ui(ui);
                button.set_intrinsic_size(cell_size);
                Tooltip::for_enabled(&button).show(|ui| ui.label(station_name));
                Popup::menu(&button).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(
                    |ui| {
                        let mut disp_bigflag = |flag: StationDisplayMode, atoms: &str| {
                            let mut flag_bool = display_mode.contains(flag);
                            // disallow removing if it is the only activated flag
                            ui.add_enabled(
                                !(display_mode.bits().count_ones() == 1 && flag_bool),
                                Checkbox::new(&mut flag_bool, atoms),
                            );
                            display_mode.set(flag, flag_bool);
                        };
                        disp_bigflag(StationDisplayMode::ARRIVAL, "Arrival");
                        disp_bigflag(StationDisplayMode::TRACK, "Track");
                        disp_bigflag(StationDisplayMode::DEPARTURE, "Departure");
                    },
                );
            }
        });
    Vec2 {
        x: scroll.x,
        y: scroll_area.state.clamped_offset().y,
    }
}

/// Top panel: trip names, following the grid's horizontal scroll.
fn show_trip_header(
    app: &mut crate::App,
    ui: &mut Ui,
    cell_size: Vec2,
    columns: &[(TripKey, EcoVec<RouteTimetableEntry>)],
    scroll: Vec2,
) -> Vec2 {
    puffin::profile_function!();
    let scroll_area = ScrollArea::horizontal()
        .auto_shrink([false, true])
        .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
        .horizontal_scroll_offset(scroll.x);
    let scroll_area = show_grid_viewport(
        scroll_area,
        ui,
        cell_size,
        1,
        columns.len(),
        |ui, rect, _row_range, col_range| {
            for (col_idx, (key, _)) in columns[col_range].iter().enumerate() {
                let name = app.world.trips.get(key).map_or_default(|wfc| wfc.name.as_str());
                let fill = app
                    .world
                    .trips
                    .get(key)
                    .and_then(|wfc| wfc.service_class)
                    .and_then(|class_key| app.world.service_classes.get(&class_key))
                    .map(|wfc| wfc.style.color)
                    .unwrap_or(ui.visuals().faint_bg_color);
                let cell_rect = Rect::from_min_size(
                    pos2(rect.left() + col_idx as f32 * cell_size.x, rect.top()),
                    cell_size,
                );
                ui.visuals_mut().widgets.inactive.corner_radius = 0.into();
                ui.visuals_mut().widgets.hovered.corner_radius = 0.into();
                ui.visuals_mut().widgets.active.corner_radius = 0.into();
                ui.visuals_mut().widgets.inactive.weak_bg_fill = fill;
                ui.visuals_mut().widgets.hovered.weak_bg_fill = fill.gamma_multiply(0.5);
                ui.visuals_mut().widgets.active.weak_bg_fill = fill.gamma_multiply(0.25);
                ui.spacing_mut().button_padding = Vec2::ZERO;
                // ui.visuals_mut().widgets.inactive.bg_stroke;
                ui.visuals_mut().widgets.hovered.bg_stroke = Stroke {
                    width: 1.0,
                    color: fill,
                };
                ui.visuals_mut().widgets.active.bg_stroke = Stroke {
                    width: 1.0,
                    color: fill.gamma_multiply(0.5),
                };
                let button = Button::new(RichText::new(name).color(if fill.intensity() > 0.5 {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                }))
                .truncate();

                if ui
                    .place(cell_rect, button)
                    .on_hover_text(format!("Click to open the trip\n{name}"))
                    .clicked()
                {
                    app.ui_action_queue.push(UiCommand::OpenOrFocus(super::MainTab::Trip(
                        TripTab::new(*key),
                    )))
                }
            }
        },
    );
    Vec2 {
        x: scroll_area.state.clamped_offset().x,
        y: scroll.y,
    }
}

/// Center panel: the timetable grid itself.
fn show_grid<'a>(
    show_estimates: bool,
    app: &crate::App,
    ui: &mut Ui,
    cell_size: Vec2,
    station_records: impl Iterator<Item = (&'a StationRecord, &'a StationDisplayMode)> + Clone,
    columns: &[(TripKey, EcoVec<RouteTimetableEntry>)],
    scroll: Vec2,
) -> (Vec2, Option<(Pos2, &'a StationRecord)>) {
    puffin::profile_function!();
    // window stroke for drawing strokes
    let stroke = ui.visuals().window_stroke;

    let scroll_area = ScrollArea::both()
        .auto_shrink(false)
        .vertical_scroll_offset(scroll.y)
        .horizontal_scroll_offset(scroll.x)
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible);

    let row_count =
        station_records.clone().map(|(_, it)| it.bits().count_ones().at_least(1) as usize).sum();

    let scroll_area = show_grid_viewport(
        scroll_area,
        ui,
        cell_size,
        row_count,
        columns.len(),
        |ui, rect, row_range, col_range| {
            puffin::profile_scope!("grid_cells");
            for idx in station_records
                .clone()
                .flat_map(|(_, disp_mode)| disp_mode.iter())
                .skip(row_range.start)
                .take(row_range.clone().count())
                .tuple_windows()
                .enumerate()
                .filter_map(|(idx, (prev, curr))| {
                    use StationDisplayMode as Sdm;
                    match [prev, curr] {
                        [Sdm::ARRIVAL, Sdm::ARRIVAL] => false,
                        [Sdm::ARRIVAL, Sdm::TRACK] => true,
                        [Sdm::ARRIVAL, Sdm::DEPARTURE] => true,
                        [Sdm::DEPARTURE, Sdm::ARRIVAL] => false,
                        [Sdm::DEPARTURE, Sdm::TRACK] => true,
                        [Sdm::DEPARTURE, Sdm::DEPARTURE] => false,
                        [Sdm::TRACK, _] => true,
                        _ => false,
                    }
                    .then_some(idx)
                })
            {
                let y = rect.top() + (idx + 1) as f32 * cell_size.y;
                ui.painter().hline(rect.x_range(), y, stroke);
            }
            let col_iter = columns[col_range].iter().enumerate();
            col_iter.fold(None, |paint_pos, (col_idx, (_, section))| {
                let left = rect.left() + col_idx as f32 * cell_size.x;
                let entry_iter = izip!(
                    station_records.clone(),
                    chain!(
                        section.iter().copied().map(Either::Left),
                        iter::once(Either::Right(())),
                        iter::repeat(Either::Left(RouteTimetableEntry::NotInOperation)),
                    ),
                )
                .flat_map(|((station_record, display_mode), entry)| {
                    display_mode.iter().map(move |disp_mode| (station_record, disp_mode, entry))
                })
                .scan(None, |disp_progress, (station_record, disp_mode, entry)| {
                    let ret = if disp_mode == StationDisplayMode::TRACK
                        && let Either::Left(RouteTimetableEntry::Active((_, entry))) = entry
                    {
                        let track_name = app
                            .world
                            .graph
                            .nodes()
                            .get(&entry.node_key())
                            .map_or("?".into(), |wfc| wfc.name.as_str());
                        Atom::from(track_name).atom_shrink(true)
                    } else {
                        let text_to_display = match entry {
                            Either::Left(RouteTimetableEntry::Active((es, entry))) => {
                                *disp_progress = Some(true);
                                match [show_estimates, disp_mode == StationDisplayMode::ARRIVAL] {
                                    [true, true] => es.arr.to_oud2_string(false),
                                    [true, false] => es.dep.to_oud2_string(false),
                                    [false, true] => entry.to_oud2_string_arr(false),
                                    [false, false] => entry.to_oud2_string_dep(false),
                                }
                            }
                            Either::Right(()) if *disp_progress == Some(true) => {
                                *disp_progress = Some(false);
                                "=====".into()
                            }
                            Either::Left(RouteTimetableEntry::NotInOperation) | _ => {
                                if *disp_progress == Some(true) {
                                    "||".into()
                                } else {
                                    "..".into()
                                }
                            }
                        };
                        Atom::from(
                            WidgetText::Text(text_to_display)
                                .text_style(TIMETABLTE_TEXT_STYLE.clone()),
                        )
                        .atom_align(Align2::RIGHT_CENTER)
                    };
                    Some((station_record, ret))
                })
                .skip(row_range.start)
                .take(row_range.clone().count());

                // offset by one cell
                ui.painter().vline(left + cell_size.x, rect.y_range(), stroke);

                entry_iter.enumerate().fold(paint_pos, |paint_pos, (row, (stn_record, atoms))| {
                    let top = rect.top() + row as f32 * cell_size.y;
                    let cell_rect = Rect::from_min_size(pos2(left, top), cell_size);
                    ui.spacing_mut().button_padding = Vec2::ZERO;
                    ui.place(cell_rect, WidgetAtom::new(atoms));
                    paint_pos.or_else(|| {
                        ui.input(|r| r.pointer.hover_pos())
                            .map_or(false, |pos| cell_rect.contains(pos))
                            .then_some((pos2(left + cell_size.x, top + cell_size.y), stn_record))
                    })
                })
            })
        },
    );
    (scroll_area.state.clamped_offset(), scroll_area.inner)
}

/// Like [`ScrollArea::show_viewport`], but specialized for a grid of uniformly-sized cells.
///
/// The callback only has to lay out the visible cells: it receives the screen-space rect
/// covering the visible range together with the row and column index ranges that intersect the
/// viewport.
fn show_grid_viewport<R>(
    scroll_area: ScrollArea,
    ui: &mut Ui,
    cell_size: Vec2,
    total_rows: usize,
    total_cols: usize,
    add_contents: impl FnOnce(&mut Ui, Rect, Range<usize>, Range<usize>) -> R,
) -> ScrollAreaOutput<R> {
    puffin::profile_function!();
    let row_pitch = cell_size.y;
    let col_pitch = cell_size.x;

    scroll_area.show_viewport(ui, |ui, viewport| {
        ui.set_height(row_pitch * total_rows as f32);
        ui.set_width(col_pitch * total_cols as f32);

        let row_range = visible_range(viewport.y_range(), row_pitch, total_rows);
        let col_range = visible_range(viewport.x_range(), col_pitch, total_cols);

        let origin = ui.max_rect().min;
        let rect = Rect::from_min_max(
            origin
                + vec2(
                    col_range.start as f32 * col_pitch,
                    row_range.start as f32 * row_pitch,
                ),
            origin
                + vec2(
                    col_range.end as f32 * col_pitch,
                    row_range.end as f32 * row_pitch,
                ),
        );

        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            add_contents(ui, rect, row_range, col_range)
        })
        .inner
    })
}

/// The indices intersecting `viewport` for evenly spaced cells of the given pitch, padded by one
/// cell on either side so that partially visible cells are included.
fn visible_range(viewport: Rangef, pitch: f32, total: usize) -> Range<usize> {
    let mut min = (viewport.min / pitch).floor().max(0.0) as usize;
    let mut max = (viewport.max / pitch).ceil() as usize + 1;
    if max > total {
        let diff = max.saturating_sub(min);
        max = total;
        min = total.saturating_sub(diff);
    }
    min..max
}
