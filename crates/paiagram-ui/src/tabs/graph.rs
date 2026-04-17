use bevy::prelude::*;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Margin, Painter, Popup,
    PopupCloseBehavior, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;
use paiagram_core::graph::{AddIntervalPair, Graph, NodeCoor};
use paiagram_core::interval::IntervalQuery;
use paiagram_core::route::Route;
use paiagram_core::station::{CreateNewStation, StationNamePending, StationQuery};
use paiagram_core::units::distance::Distance;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use walkers::sources::Attribution;

use crate::{
    IntervalSelection, ModifySelectedItems, SelectedItem, SelectedItems, StationSelection,
    TripSelection,
};

use crate::tabs::graph::gpu_draw::ShapeInstance;
use crate::{GlobalTimer, tabs::Navigatable};
use paiagram_core::{
    colors::PredefinedColor,
    graph::{GraphIntervalSpatialIndex, GraphSpatialIndex, Node},
    settings::ProjectSettings,
    trip::{
        Trip, TripClass, TripSpatialIndex,
        class::{Class, DisplayedStroke},
    },
};

mod gpu_draw;
mod underlay;

/// The state of the graph
enum GraphGlobalState<'a> {
    /// Looking at local state now
    LocalDependent(&'a mut GraphLocalState),
    /// User is selecting some trips
    SelectingTrips(&'a [TripSelection]),
    /// User is selecting some intervals
    SelectingIntervals(&'a [IntervalSelection]),
    /// User is selecting some stations
    SelectingStations(&'a [StationSelection]),
    /// User has only selected one station
    SelectingStation(&'a StationSelection),
}

impl<'a> From<(&'a SelectedItems, &'a mut GraphLocalState)> for GraphGlobalState<'a> {
    fn from((selected_items, local_state): (&'a SelectedItems, &'a mut GraphLocalState)) -> Self {
        match selected_items {
            SelectedItems::None => GraphGlobalState::LocalDependent(local_state),
            SelectedItems::Trips(it) => GraphGlobalState::SelectingTrips(it),
            SelectedItems::Intervals(it) => GraphGlobalState::SelectingIntervals(it),
            SelectedItems::Stations(it) if it.len() == 1 => {
                GraphGlobalState::SelectingStation(it.first())
            }
            SelectedItems::Stations(it) => GraphGlobalState::SelectingStations(it),
            SelectedItems::ExtendingRoute(_it) => GraphGlobalState::LocalDependent(local_state),
            SelectedItems::ExtendingTrip(_it) => GraphGlobalState::LocalDependent(local_state),
        }
    }
}

/// There's something specific to the graph that is unrelated with the global state
#[derive(Default, Clone)]
enum GraphLocalState {
    /// Idle
    #[default]
    Idle,
    /// The user has selected a position. This position may be used to e.g. generate a station
    SelectingPosition {
        pos: (f64, f64),
        name_candidate: String,
    },
}

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip, default)]
    local_state: GraphLocalState,
    #[serde(skip, default)]
    underlay_tile_change: Option<underlay::UnderlayTileType>,
    #[serde(skip, default)]
    arrange_iterations: u32,
    #[serde(skip, default)]
    osm_area_name: String,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuGraphRendererState>>,
    #[serde(skip, default)]
    highlight_station_intervals: Vec<Entity>,
}

fn default_arrange_iterations() -> u32 {
    1000
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            underlay_tile_type: underlay::UnderlayTileType::None,
            underlay_tile_change: None,
            local_state: GraphLocalState::default(),
            arrange_iterations: default_arrange_iterations(),
            osm_area_name: String::new(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuGraphRendererState::default(),
            )),
            highlight_station_intervals: Vec::new(),
        }
    }
}

impl PartialEq for GraphTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GraphNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: f32,
    visible: egui::Rect,
}

impl Default for GraphNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: 1.0,
            visible: egui::Rect::NOTHING,
        }
    }
}

impl super::Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 {
        self.zoom
    }
    fn zoom_y(&self) -> f32 {
        self.zoom
    }
    fn set_zoom(&mut self, zoom_x: f32, _zoom_y: f32) {
        self.zoom = zoom_x;
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = offset_x;
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible
    }
}

impl super::Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| display(self, world, ui));
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        self.underlay_tile_change = ui
            .add(&mut self.underlay_tile_type)
            .changed()
            .then_some(self.underlay_tile_type);
        ui.add(
            egui::Slider::new(&mut self.arrange_iterations, 100..=10000)
                .text(tr!("tab-graph-auto-arrange-iterations")),
        );
        if ui.button(tr!("tab-graph-auto-arrange")).clicked() {
            world
                .run_system_cached_with(
                    paiagram_core::graph::arrange::auto_arrange_graph,
                    (ui.ctx().clone(), self.arrange_iterations),
                )
                .unwrap();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr!("tab-graph-osm-area-name"));
            ui.text_edit_singleline(&mut self.osm_area_name);
        });
        if ui.button(tr!("tab-graph-arrange-via-osm")).clicked() {
            let area_name = if self.osm_area_name.is_empty() {
                None
            } else {
                Some(self.osm_area_name.clone())
            };
            world
                .run_system_cached_with(
                    paiagram_core::graph::arrange::arrange_via_osm,
                    (ui.ctx().clone(), area_name),
                )
                .unwrap();
        }
        if let Some(task) = world.get_resource::<paiagram_core::graph::arrange::GraphLayoutTask>() {
            let (finished, total, queued_retry) = task.progress();
            let mode = match task.kind {
                paiagram_core::graph::arrange::GraphLayoutKind::ForceDirected => {
                    tr!("tab-graph-arrange-mode-force")
                }
                paiagram_core::graph::arrange::GraphLayoutKind::OSM => {
                    tr!("tab-graph-arrange-mode-osm")
                }
            };
            ui.label(tr!(
                "tab-graph-arrange-progress",
                {
                    mode: mode,
                    finished: finished,
                    total: total,
                    queued_retry: queued_retry
                }
            ));
            if total > 0 {
                ui.add(egui::ProgressBar::new(finished as f32 / total as f32));
            }
        }
        ui.separator();
        let selected_sample = world.resource_mut::<SelectedItems>();
        match selected_sample.clone() {
            SelectedItems::None | SelectedItems::Intervals(_) | SelectedItems::ExtendingTrip(_) => {
            }
            SelectedItems::Trips(trips) => {
                // world
                //     .run_system_cached_with(crate::display_entry_info, (ui, entries.as_slice()))
                //     .unwrap();
            }
            SelectedItems::Stations(stations) => {
                world
                    .run_system_cached_with(
                        display_station_info,
                        (
                            ui,
                            stations.as_slice(),
                            &mut self.highlight_station_intervals,
                        ),
                    )
                    .unwrap();
            }
            SelectedItems::ExtendingRoute(r) => {}
        }
    }
}

fn display_station_info(
    (InMut(ui), InRef(selected_stations), InMut(highlight_station_intervals)): (
        InMut<Ui>,
        InRef<[StationSelection]>,
        InMut<Vec<Entity>>,
    ),
    station_q: Query<StationQuery>,
    interval_q: Query<IntervalQuery>,
    graph: Res<Graph>,
    mut commands: Commands,
    mut last_hovered: Local<bool>,
) {
    for station in station_q.iter_many(selected_stations.iter().map(|it| it.station)) {
        ui.label(station.name.as_ref());
    }
    let res = ui.button("Create new route");
    if selected_stations.len() < 2 {
        *last_hovered = res.hovered();
        return;
    }
    let selected_station_entities_iter = selected_stations.iter().map(|it| it.station);
    if res.hovered() ^ *last_hovered
        && let Some((_, points)) =
            graph.route_between_source_waypoint_target(selected_station_entities_iter, &interval_q)
    {
        // refresh
        highlight_station_intervals.clear();
        highlight_station_intervals.extend_from_slice(&points);
    } else if !res.hovered() {
        highlight_station_intervals.clear()
    }
    if res.clicked() {
        commands.spawn((
            Name::new("New Route"),
            Route {
                lengths: vec![10.0; highlight_station_intervals.len()],
                stops: highlight_station_intervals.clone(),
            },
        ));
    }
    *last_hovered = res.hovered();
}

fn display(tab: &mut GraphTab, world: &mut World, ui: &mut egui::Ui) {
    // allocate painter for drawing afterwards
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);

    // fetch attribution info to draw later
    let attribution = world
        .run_system_cached_with(
            underlay::draw_underlay,
            (&mut painter, &tab.navi, ui, tab.underlay_tile_change),
        )
        .unwrap();

    let mut state = tab.gpu_state.lock();
    if let Some(target_format) = ui.data(|data| {
        data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(egui::Id::new("wgpu_target_format"))
    }) {
        state.target_format = Some(target_format);
    }
    if let Some(msaa_samples) =
        ui.data(|data| data.get_temp::<u32>(egui::Id::new("wgpu_msaa_samples")))
    {
        state.msaa_samples = msaa_samples;
    }

    let interact_pos = response
        .clicked()
        .then_some(ui.input(|r| r.pointer.interact_pos()))
        .flatten();

    // push draw items and handle selection
    let selected_item = world
        .run_system_cached_with(
            push_draw_items,
            (
                ui.visuals().dark_mode,
                &tab.navi,
                &mut state.instances,
                &mut painter,
                interact_pos,
                ui.animate_bool(ui.id().with("gugugaga"), tab.navi.zoom > 0.002),
            ),
        )
        .unwrap();

    let shift_pressed = ui.input(|r| r.modifiers.shift);
    match (
        selected_item.clone(),
        ui.input(|r| r.modifiers.command),
        shift_pressed,
    ) {
        (Some(Some(selected_item)), true, _) => {
            world.write_message(ModifySelectedItems::Toggle(selected_item));
        }
        (Some(Some(selected_item)), false, _) => {
            world.write_message(ModifySelectedItems::SetSingle(selected_item));
        }
        (Some(None), true, _) => {
            // do nothing in this case.
        }
        (Some(None), false, true) => {
            // also do nothing in this case.
        }
        (Some(None), false, false) => {
            world.write_message(ModifySelectedItems::Clear);
        }
        (None, _, _) => {
            // do nothing in this case. No interactions, no response
        }
    }

    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);

    // draw the attribution and the scale bar
    if let Some(attribution) = attribution {
        draw_attribution(ui, response.rect, &attribution);
    }
    draw_scale_bar(
        &painter,
        response.rect,
        tab.navi.zoom,
        ui.visuals().text_color(),
    );

    world.resource_scope(|world, selected_items: Mut<SelectedItems>| {
        let state: GraphGlobalState<'_> = (selected_items.as_ref(), &mut tab.local_state).into();
        let interact_pos = response
            .clicked()
            .then(|| ui.input(|r| r.pointer.interact_pos()))
            .flatten();

        let mut new_state: Option<GraphLocalState> = None;

        let mut display_station_info = |ui: &mut Ui, station_entity: Entity| {
            let coor = world.get::<Node>(station_entity).unwrap().coor;
            let (x, y) = coor.to_xy();
            let pos = tab.navi.xy_to_screen_pos(x, y);
            let rect = Rect::from_pos(pos).expand(8.0);
            let res = ui
                .allocate_rect(rect, Sense::drag())
                .on_hover_cursor(CursorIcon::Grab);
            if res.dragged() {
                ui.set_cursor_icon(egui::CursorIcon::Grabbing);
                let new_pos = pos + res.drag_delta();
                let (x, y) = tab.navi.screen_pos_to_xy(new_pos);
                let new_coor = NodeCoor::from_xy(x, y);
                world.get_mut::<Node>(station_entity).unwrap().coor = new_coor;
            }
            Popup::menu(&res)
                .open(true)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_width(150.0);
                    ui.horizontal(|ui| {
                        world.get_mut::<Name>(station_entity).unwrap().mutate(|s| {
                            ui.text_edit_singleline(s);
                        });
                        if ui.button("A").clicked() {
                            world
                                .commands()
                                .entity(station_entity)
                                .insert(StationNamePending::new(coor));
                        }
                    });
                    ui.small(coor.to_string());
                });
        };

        match state {
            GraphGlobalState::LocalDependent(GraphLocalState::Idle)
                if let Some(interact_pos) = interact_pos =>
            {
                let pos = tab.navi.screen_pos_to_xy(interact_pos);
                new_state = Some(GraphLocalState::SelectingPosition {
                    pos,
                    name_candidate: String::new(),
                })
            }
            GraphGlobalState::LocalDependent(GraphLocalState::Idle) => {
                // there's no interaction! in this case do nothing.
            }
            GraphGlobalState::LocalDependent(GraphLocalState::SelectingPosition {
                pos,
                name_candidate,
            }) => {
                let screen_pos = tab.navi.xy_to_screen_pos(pos.0, pos.1);
                let rect = Rect::from_pos(screen_pos).expand(6.0);
                painter.rect(
                    rect,
                    0,
                    Color32::RED.gamma_multiply(0.5),
                    Stroke::new(1.0, Color32::RED),
                    egui::StrokeKind::Middle,
                );
                let res = ui
                    .allocate_rect(rect, Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if res.dragged() {
                    ui.set_cursor_icon(egui::CursorIcon::Grabbing);
                    let new_pos = screen_pos + res.drag_delta();
                    *pos = tab.navi.screen_pos_to_xy(new_pos);
                }

                Popup::menu(&res)
                    .open(true)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_width(200.0);
                        ui.text_edit_singleline(name_candidate);
                        let coor = NodeCoor::from_xy(pos.0, pos.1);
                        if ui.button("New Station").clicked() {
                            let name = (!name_candidate.is_empty()).then(|| name_candidate.clone());
                            world.trigger(CreateNewStation { name, coor });
                            new_state = Some(GraphLocalState::Idle);
                        }
                        ui.small(coor.to_string());
                    });

                if selected_item.is_some() {
                    new_state = Some(GraphLocalState::Idle);
                }
            }
            GraphGlobalState::SelectingTrips(it) => {
                // TODO
            }
            GraphGlobalState::SelectingIntervals(it) => {
                // TODO
            }
            GraphGlobalState::SelectingStations(stations) => {
                for station in stations {
                    display_station_info(ui, station.station);
                }
            }
            GraphGlobalState::SelectingStation(station) => {
                display_station_info(ui, station.station);
                // check if shift is down
                if shift_pressed && let Some(cursor_pos) = ui.input(|r| r.pointer.hover_pos()) {
                    let coor = world.get::<Node>(station.station).unwrap().coor;
                    let (x, y) = coor.to_xy();
                    let station_pos = tab.navi.xy_to_screen_pos(x, y);
                    painter.line_segment([station_pos, cursor_pos], Stroke::new(1.0, Color32::RED));
                    if let Some(Some(SelectedItem::Station(selected))) = selected_item {
                        world.trigger(AddIntervalPair {
                            source: station.station,
                            target: selected.station,
                            length: Distance::from_m(1000),
                        });
                    }
                }
            }
        }
        if let Some(new_state) = new_state {
            tab.local_state = new_state
        }
    });
}

fn draw_scale_bar(painter: &Painter, viewport: Rect, zoom: f32, color: egui::Color32) {
    if zoom <= 0.0 || !viewport.is_positive() {
        return;
    }

    let desired_px = 120.0f64;
    let meters_per_px = 1.0 / zoom as f64;
    let raw_meters = desired_px * meters_per_px;
    let bar_meters = round_to_1_2_5(raw_meters).max(1.0);
    let bar_px = (bar_meters as f32 * zoom).max(1.0);

    let margin = 10.0;
    let baseline_y = viewport.bottom() - margin;
    let left_x = viewport.left() + margin;
    let right_x = left_x + bar_px;

    let stroke = Stroke::new(1.6, color);
    painter.line_segment(
        [
            Pos2::new(left_x, baseline_y),
            Pos2::new(right_x, baseline_y),
        ],
        stroke,
    );

    let tick_len = 7.0;
    painter.line_segment(
        [
            Pos2::new(left_x, baseline_y),
            Pos2::new(left_x, baseline_y - tick_len),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(right_x, baseline_y),
            Pos2::new(right_x, baseline_y - tick_len),
        ],
        stroke,
    );

    let mid_tick_len = 5.0;
    for fraction in [0.25f32, 0.5, 0.75] {
        let x = left_x + bar_px * fraction;
        painter.line_segment(
            [
                Pos2::new(x, baseline_y),
                Pos2::new(x, baseline_y - mid_tick_len),
            ],
            stroke,
        );
    }

    painter.text(
        Pos2::new(left_x, baseline_y - tick_len - 3.0),
        Align2::LEFT_BOTTOM,
        format_scale_label(bar_meters),
        FontId::proportional(12.0),
        color,
    );
}

fn round_to_1_2_5(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let exponent = value.log10().floor();
    let base = 10.0f64.powf(exponent);
    let normalized = value / base;
    let rounded = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    rounded * base
}

fn format_scale_label(meters: f64) -> String {
    if meters >= 1000.0 {
        let km = meters / 1000.0;
        if (km - km.round()).abs() < 1e-6 {
            format!("{:.0} km", km)
        } else {
            format!("{:.1} km", km)
        }
    } else {
        format!("{:.0} m", meters)
    }
}

fn draw_attribution(ui: &mut Ui, viewport: Rect, attribution: &Attribution) {
    let margin = 6.0;
    let font_id = FontId::proportional(13.0);
    let color = ui.style().visuals.hyperlink_color;
    let text = format!("© {}", attribution.text);
    let galley = ui.painter().layout_no_wrap(text.clone(), font_id, color);
    let size = galley.size();
    let min = Pos2::new(
        viewport.right() - margin - size.x,
        viewport.bottom() - margin - size.y,
    );
    let rect = Rect::from_min_size(min, size);
    let mut r = CornerRadius::ZERO;
    r.nw = 4;
    ui.painter()
        .rect_filled(rect.expand(margin), r, Color32::WHITE.gamma_multiply(0.5));
    ui.put(
        rect,
        egui::Hyperlink::from_label_and_url(text, attribution.url).open_in_new_tab(true),
    );
}

fn push_draw_items(
    (
        In(is_dark),
        InRef(navi),
        InMut(buffer),
        InMut(painter),
        In(maybe_interact_pos),
        In(text_strength),
    ): (
        In<bool>,
        InRef<GraphNavigation>,
        InMut<Vec<ShapeInstance>>,
        InMut<Painter>,
        In<Option<Pos2>>,
        In<f32>,
    ),
    nodes: Query<(Entity, &Node, Option<&Name>)>,
    spatial_index: Res<GraphSpatialIndex>,
    interval_spatial_index: Res<GraphIntervalSpatialIndex>,
    trip_spatial_index: Res<TripSpatialIndex>,
    settings: Res<ProjectSettings>,
    trip_meta_q: Query<(&Name, &TripClass), With<Trip>>,
    stroke_q: Query<&DisplayedStroke, With<Class>>,
    timer: Res<GlobalTimer>,
    selected_items: Res<SelectedItems>,
) -> Option<Option<SelectedItem>> {
    buffer.clear();

    let mut binding = GraphLocalState::Idle;
    let state: GraphGlobalState<'_> = (selected_items.as_ref(), &mut binding).into();

    let selection_strength = painter.ctx().animate_bool_responsive(
        Id::new("graph selection animation"),
        !matches!(
            state,
            GraphGlobalState::LocalDependent(GraphLocalState::Idle)
        ),
    );

    // prepare time
    let time = timer.read_seconds();
    let repeat_time = settings.repeat_frequency.0 as f64;
    let query_time = if repeat_time > 0.0 {
        time.rem_euclid(repeat_time)
    } else {
        time
    };

    let draw_name = |name: Option<&str>, pos: Pos2, color: Color32| {
        if text_strength > 0.05
            && let Some(name) = name
        {
            painter.text(
                pos + Vec2 { x: 7.0, y: 0.0 },
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(13.0),
                color.gamma_multiply(text_strength),
            );
        }
    };

    // in the case of pushing a selected item the interface only allows pushing if:
    // there is interaction i.e. maybe_interaction_pos is Some AND there aren't any
    // previously selected items AND one of the following:
    //   1. the current state is idle, OR
    //   2. the current state's items matches the pushed item's type.
    //      e.g. SelectingStations and StationSelection
    let mut selected_item: Option<SelectedItem> = None;
    macro_rules! push_selected_item {
        ($f:expr, $p:pat) => {
            if let Some(interact_pos) = maybe_interact_pos
                && selected_item.is_none()
                && matches!(
                    state,
                    GraphGlobalState::LocalDependent(GraphLocalState::Idle) | $p
                )
                && let Some(candidate_item) = $f(interact_pos)
            {
                selected_item = Some(candidate_item);
            }
        };
    }

    // prepare visuals
    let color = PredefinedColor::Neutral.get(is_dark);
    let margin_x = 12.0 / navi.zoom_x().max(f32::EPSILON) as f64;
    let margin_y = 12.0 / navi.zoom_y().max(f32::EPSILON) as f64;
    let visible_x = navi.visible_x();
    let visible_y = navi.visible_y();
    let min_x = visible_x.start - margin_x;
    let max_x = visible_x.end + margin_x;
    let min_y = visible_y.start - margin_y;
    let max_y = visible_y.end + margin_y;

    const STATION_SELECTION_RADIUS: f32 = 10.0;
    const SELECTION_RADIUS: f32 = 10.0;

    // intervals
    // TODO: interval selection
    for segment in interval_spatial_index.query_xy_aabb(min_x, min_y, max_x, max_y) {
        let spos = navi.xy_to_screen_pos(segment.p0[0], segment.p0[1]);
        let tpos = navi.xy_to_screen_pos(segment.p1[0], segment.p1[1]);
        buffer.push(gpu_draw::ShapeInstance::segment(spos, tpos, 1.0, color));
    }

    // prepare candidates
    let candidate_nodes: Vec<Entity> =
        spatial_index.entities_in_xy_aabb(min_x, min_y, max_x, max_y);

    // draw station selection
    let selected = match state {
        GraphGlobalState::SelectingStations(it) => it,
        GraphGlobalState::SelectingStation(it) => std::slice::from_ref(it),
        _ => &[],
    };
    for (_, node, _) in nodes.iter_many(selected.into_iter().map(|it| it.station)) {
        let [x, y] = node.coor.to_xy_arr();
        let pos = navi.xy_to_screen_pos(x, y);
        painter.circle(
            pos,
            SELECTION_RADIUS,
            Color32::RED
                .gamma_multiply(0.5)
                .gamma_multiply(selection_strength),
            Stroke::new(1.0, Color32::RED.gamma_multiply(selection_strength)),
        );
    }

    // draw other stations
    for (station_entity, node, name) in nodes.iter_many(candidate_nodes) {
        let [x, y] = node.coor.to_xy_arr();
        let station_screen_pos = navi.xy_to_screen_pos(x, y);
        push_selected_item!(
            |pos| {
                let r = Rect::from_pos(station_screen_pos).expand(STATION_SELECTION_RADIUS);
                r.contains(pos)
                    .then_some(SelectedItem::Station(StationSelection {
                        station: station_entity,
                    }))
            },
            GraphGlobalState::SelectingStations(_) | GraphGlobalState::SelectingStation(_)
        );

        buffer.push(gpu_draw::ShapeInstance::circle(
            station_screen_pos,
            4.0,
            color,
        ));
        draw_name(name.map(Name::as_str), station_screen_pos, color);
    }

    // entries
    for sample in
        trip_spatial_index.query_xy_time(min_x..=max_x, min_y..=max_y, query_time..=query_time)
    {
        let (name, trip_class) = trip_meta_q
            .get(sample.trip)
            .expect("Trips should have a name and a class");
        let stroke = stroke_q
            .get(trip_class.entity())
            .expect("Classes should have a stroke");
        let color = stroke.color.get(is_dark);

        let pos0 = navi.xy_to_screen_pos(sample.p0[0], sample.p0[1]);
        let pos1 = navi.xy_to_screen_pos(sample.p1[0], sample.p1[1]);
        let entry_pos = if query_time <= sample.t1 {
            pos0
        } else if query_time >= sample.t2 {
            pos1
        } else {
            let f = (query_time - sample.t1) / (sample.t2 - sample.t1).max(f64::EPSILON);
            pos0.lerp(pos1, f as f32)
        };

        push_selected_item!(
            |pos| {
                let r = Rect::from_pos(entry_pos).expand(STATION_SELECTION_RADIUS);
                r.contains(pos).then_some(SelectedItem::Trip(TripSelection {
                    entries: vec1::vec1![sample.entry1],
                    trip: sample.trip,
                }))
            },
            GraphGlobalState::SelectingTrips(_)
        );

        if let GraphGlobalState::SelectingTrips(trips) = state
            && trips.iter().any(|it| it.trip == sample.trip)
        {
            painter.circle(
                entry_pos,
                SELECTION_RADIUS,
                Color32::BLUE
                    .gamma_multiply(0.5)
                    .gamma_multiply(selection_strength),
                Stroke::new(1.0, Color32::BLUE.gamma_multiply(selection_strength)),
            );
        }

        buffer.push(gpu_draw::ShapeInstance::stealth_arrow(
            pos0, pos1, entry_pos, color,
        ));
        draw_name(Some(name.as_str()), entry_pos, color);
    }

    maybe_interact_pos.map(|_| selected_item)
}
