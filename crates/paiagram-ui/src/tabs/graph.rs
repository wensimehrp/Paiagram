use bevy::prelude::*;
use egui::{
    Align2, Color32, CornerRadius, FontId, Margin, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;
use paiagram_core::graph::{AddIntervalPair, Graph, NodeCoor};
use paiagram_core::interval::IntervalQuery;
use paiagram_core::route::Route;
use paiagram_core::station::{CreateNewStation, StationBundle, StationNamePending, StationQuery};
use paiagram_core::units::distance::Distance;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use walkers::sources::Attribution;

use crate::{IntervalSelection, SelectedItem, SelectedItems, StationSelection, TripSelection};

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

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip, default)]
    underlay_tile_change: Option<underlay::UnderlayTileType>,
    #[serde(skip, default)]
    clicked_coor: Option<(f64, f64)>,
    #[serde(skip, default)]
    new_station_name: String,
    #[serde(skip, default = "default_arrange_iterations")]
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
            clicked_coor: None,
            new_station_name: String::new(),
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
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);
    let attribution = world
        .run_system_cached_with(
            underlay::draw_underlay,
            (&mut painter, &tab.navi, ui, tab.underlay_tile_change),
        )
        .unwrap();
    let mut state = tab.gpu_state.lock();
    if let Some(target_format) = ui.ctx().data(|data| {
        data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(egui::Id::new("wgpu_target_format"))
    }) {
        state.target_format = Some(target_format);
    }
    if let Some(msaa_samples) = ui
        .ctx()
        .data(|data| data.get_temp::<u32>(egui::Id::new("wgpu_msaa_samples")))
    {
        state.msaa_samples = msaa_samples;
    }
    let interact_pos = response
        .clicked()
        .then_some(ui.input(|r| r.pointer.interact_pos()))
        .flatten();
    let (selected_stations, selected_trips) = {
        // let selected_items = world.resource::<SelectedItems>();
        // let station_set: Vec<_> = selected_items
        //     .station_selection()
        //     .iter()
        //     .map(|it| it.station)
        //     .collect();
        // let mut trip_set: Vec<_> = selected_items
        //     .entry_selection()
        //     .iter()
        //     .map(|it| it.parent)
        //     .collect();
        // trip_set.sort_unstable();
        // trip_set.dedup();
        // (station_set, trip_set)
        (Vec::new(), Vec::new())
    };
    let selected_item = world
        .run_system_cached_with(
            push_draw_items,
            (
                ui.visuals().dark_mode,
                &tab.navi,
                &mut state.instances,
                &mut painter,
                interact_pos,
                &selected_stations,
                &selected_trips,
                ui.ctx()
                    .animate_bool(ui.id().with("gugugaga"), tab.navi.zoom > 0.002),
            ),
        )
        .unwrap();
    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);

    if let Some(attribution) = attribution {
        draw_attribution(ui, response.rect, &attribution);
    }
    draw_scale_bar(
        &painter,
        response.rect,
        tab.navi.zoom,
        ui.visuals().text_color(),
    );

    // handle selection
    let selected_items = world.resource_mut::<SelectedItems>().into_inner();
    let shift_pressed = ui.input(|i| i.modifiers.shift);
    if shift_pressed
        && let SelectedItems::Stations(stations) = selected_items
        && stations.len() == 1
        && let Some(hover_pos) = ui.input(|r| r.pointer.hover_pos())
    {
        let entity = stations[0].station;
        let (x, y) = world.get::<Node>(entity).unwrap().coor.to_xy();
        let pos = tab.navi.xy_to_screen_pos(x, y);
        ui.painter()
            .line_segment([pos, hover_pos], Stroke::new(1.0, Color32::BLUE));
    }
    let selected_items = world.resource::<SelectedItems>();
    if let SelectedItems::Stations(stations) = selected_items {
        for station in stations.clone().iter() {
            let coor = world.get::<Node>(station.station).unwrap().coor;
            let (x, y) = coor.to_xy();
            let pos = tab.navi.xy_to_screen_pos(x, y);
            let rect = Rect::from_pos(pos).expand(8.0);
            let res = ui
                .interact(
                    rect,
                    ui.id().with(station.station).with("popup response"),
                    Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            if res.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                let new_pos = pos + res.drag_delta();
                let (x, y) = tab.navi.screen_pos_to_xy(new_pos);
                let new_coor = NodeCoor::from_xy(x, y);
                world.get_mut::<Node>(station.station).unwrap().coor = new_coor;
            }
            let inner = |ui: &mut Ui| {
                ui.set_width(150.0);
                ui.horizontal(|ui| {
                    world.get_mut::<Name>(station.station).unwrap().mutate(|s| {
                        ui.text_edit_singleline(s);
                    });
                    if ui.button("A").clicked() {
                        world
                            .commands()
                            .entity(station.station)
                            .insert(StationNamePending::new(coor));
                    }
                });
                ui.small(coor.to_string());
            };
            egui::Popup::menu(&res)
                .open_memory(Some(egui::SetOpenCommand::Bool(true)))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(inner);
        }
    }
    // TODO: fix the lifetime here
    world.resource_scope(|world, selected_items: Mut<SelectedItems>| {
        let selected_items = selected_items.into_inner();
        match (selected_item, selected_items) {
            (None, SelectedItems::Stations(stations))
                if shift_pressed && response.secondary_clicked() && stations.len() == 1 =>
            {
                let pos = ui.input(|r| r.pointer.interact_pos()).unwrap();
                let (x, y) = tab.navi.screen_pos_to_xy(pos);
                let coor = NodeCoor::from_xy(x, y);
                let prev_station = stations[0];
                let new_station = if ui.input(|r| r.modifiers.alt) {
                    world
                        .commands()
                        .spawn((
                            StationBundle::new("Name Pending".into(), Node { coor }),
                            StationNamePending::new(coor),
                        ))
                        .id()
                } else {
                    world
                        .commands()
                        .spawn(StationBundle::new("WP".into(), Node { coor }))
                        .id()
                };
                stations[0] = StationSelection {
                    station: new_station,
                };
                world.trigger(AddIntervalPair {
                    source: prev_station.station,
                    target: new_station,
                    length: Distance::from_m(1000),
                });
            }
            (Some(SelectedItem::Station(station)), SelectedItems::Stations(stations))
                if shift_pressed && stations.len() == 1 =>
            {
                let prev_station = stations[0];
                stations[0] = station;
                world.trigger(AddIntervalPair {
                    source: prev_station.station,
                    target: station.station,
                    length: Distance::from_m(1000),
                });
            }
            (Some(item), items) => {
                let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                if ctrl_pressed {
                    items.toggle_selection(item);
                } else {
                    items.set_single_selection(item);
                }
            }
            (None, _) => {}
        }
    });
    // enhance highlighted station path
    painter.line(
        tab.highlight_station_intervals
            .iter()
            .copied()
            .map(|entity| {
                let (x, y) = world.get::<Node>(entity).unwrap().coor.to_xy();
                tab.navi.xy_to_screen_pos(x, y)
            })
            .collect(),
        Stroke::new(1.5, Color32::RED),
    );
    // create new station
    if response.secondary_clicked()
        && !shift_pressed
        && let Some(pos) = ui.input(|r| r.pointer.interact_pos())
    {
        tab.clicked_coor = Some(tab.navi.screen_pos_to_xy(pos));
    } else if response.clicked() {
        tab.clicked_coor = None
    }
    if let Some((x, y)) = tab.clicked_coor {
        let pos = tab.navi.xy_to_screen_pos(x, y);
        let rect = Rect::from_pos(pos).expand(8.0);
        ui.painter()
            .circle_filled(pos, 6.0, PredefinedColor::Red.get(ui.visuals().dark_mode));
        let res = ui
            .interact(rect, ui.id().with("popup response"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        if res.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let new_pos = pos + res.drag_delta();
            tab.clicked_coor = Some(tab.navi.screen_pos_to_xy(new_pos));
        }
        let inner = |ui: &mut Ui| {
            ui.set_width(200.0);
            ui.text_edit_singleline(&mut tab.new_station_name);
            let coor = NodeCoor::from_xy(x, y);
            if ui.button("New Station").clicked() {
                tab.clicked_coor = None;
                let name = if tab.new_station_name.is_empty() {
                    None
                } else {
                    Some(tab.new_station_name.clone())
                };
                world.trigger(CreateNewStation { name, coor });
                tab.new_station_name.clear();
            }
            ui.small(coor.to_string());
        };
        egui::Popup::menu(&res)
            .open_memory(Some(egui::SetOpenCommand::Bool(true)))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(inner);
    }
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
        InRef(selected_stations),
        InRef(selected_trips),
        In(text_strength),
    ): (
        In<bool>,
        InRef<GraphNavigation>,
        InMut<Vec<ShapeInstance>>,
        InMut<Painter>,
        In<Option<Pos2>>,
        InRef<[Entity]>,
        InRef<[Entity]>,
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
) -> Option<SelectedItem> {
    buffer.clear();

    let mut selected = SelectedItem::None;

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
    // TODO: handle interval selection
    let selected_interval: Option<IntervalSelection> = None;
    for segment in interval_spatial_index.query_xy_aabb(min_x, min_y, max_x, max_y) {
        let spos = navi.xy_to_screen_pos(segment.p0[0], segment.p0[1]);
        let tpos = navi.xy_to_screen_pos(segment.p1[0], segment.p1[1]);
        buffer.push(gpu_draw::ShapeInstance::segment(spos, tpos, 1.0, color));
    }
    if let Some(i) = selected_interval {
        selected = SelectedItem::Interval(i)
    }

    // prepare candidates
    let candidate_nodes: Vec<Entity> =
        spatial_index.entities_in_xy_aabb(min_x, min_y, max_x, max_y);
    // nodes
    let mut selected_node: Option<StationSelection> = None;

    // draw station selection
    for (_, node, _) in nodes.iter_many(selected_stations) {
        let [x, y] = node.coor.to_xy_arr();
        let pos = navi.xy_to_screen_pos(x, y);
        painter.circle(
            pos,
            SELECTION_RADIUS,
            Color32::RED.gamma_multiply(0.5),
            Stroke::new(1.0, Color32::RED),
        );
    }

    // draw other stations
    for (entity, node, name) in nodes.iter_many(candidate_nodes) {
        let [x, y] = node.coor.to_xy_arr();
        let pos = navi.xy_to_screen_pos(x, y);

        if let Some(interact_pos) = maybe_interact_pos
            && selected_node.is_none()
        {
            let r = Rect::from_pos(pos).expand(STATION_SELECTION_RADIUS);
            if r.contains(interact_pos) {
                selected_node = Some(StationSelection { station: entity })
            };
        }

        buffer.push(gpu_draw::ShapeInstance::circle(pos, 4.0, color));
        draw_name(name.map(Name::as_str), pos, color);
    }
    if let Some(n) = selected_node {
        selected = SelectedItem::Station(n);
    }

    // entries
    let mut selected_entry: Option<TripSelection> = None;
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
        let pos = if query_time <= sample.t1 {
            pos0
        } else if query_time >= sample.t2 {
            pos1
        } else {
            let f = (query_time - sample.t1) / (sample.t2 - sample.t1).max(f64::EPSILON);
            pos0.lerp(pos1, f as f32)
        };

        if let Some(interact_pos) = maybe_interact_pos
            && selected_entry.is_none()
        {
            let r = Rect::from_pos(pos).expand(STATION_SELECTION_RADIUS);
            if r.contains(interact_pos) {
                selected_entry = Some(TripSelection {
                    entries: vec1::vec1![sample.entry1],
                    trip: sample.trip,
                })
            };
        }

        if selected_trips.contains(&sample.trip) {
            painter.circle(
                pos,
                SELECTION_RADIUS,
                Color32::BLUE.gamma_multiply(0.5),
                Stroke::new(1.0, Color32::BLUE),
            );
        }

        buffer.push(gpu_draw::ShapeInstance::stealth_arrow(
            pos0, pos1, pos, color,
        ));
        draw_name(Some(name.as_str()), pos, color);
    }
    if let Some(e) = selected_entry {
        selected = SelectedItem::Trip(e);
    }
    maybe_interact_pos.map(|_| selected)
}
