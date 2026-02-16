use bevy::prelude::*;
use egui::{Align2, Color32, FontId, Margin, Painter, Rect, Sense, Stroke, Vec2};
use instant::Instant;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{
    colors::PredefinedColor,
    graph::{GraphIntervalSpatialIndex, GraphSpatialIndex, Node},
    settings::ProjectSettings,
    trip::{
        Trip, TripClass, TripSpatialIndex,
        class::{Class, DisplayedStroke},
    },
    ui::{GlobalTimer, tabs::Navigatable},
};

mod gpu_draw;

#[derive(Clone)]
struct GraphLabel {
    pos: egui::Pos2,
    text: String,
    color: egui::Color32,
}

#[derive(Default, Clone)]
struct GraphDrawItems {
    shapes: Vec<gpu_draw::ShapeSpec>,
    labels: Vec<GraphLabel>,
}

#[derive(Default, Clone)]
struct CollectTimings {
    index_query_ms: f32,
    nodes_ms: f32,
    edges_ms: f32,
    trips_ms: f32,
    label_cull_ms: f32,
}

#[derive(Default, Clone)]
struct CollectedGraphDraw {
    items: GraphDrawItems,
    timings: CollectTimings,
}

#[derive(Default, Clone)]
struct GraphPerf {
    collect_ms: f32,
    collect_index_query_ms: f32,
    collect_nodes_ms: f32,
    collect_edges_ms: f32,
    collect_trips_ms: f32,
    collect_label_cull_ms: f32,
    gpu_upload_ms: f32,
    text_ms: f32,
    frame_ms: f32,
    shape_count: usize,
    label_count: usize,
}

fn smooth_ms(previous: f32, new_value: f32) -> f32 {
    if previous <= 0.0 {
        new_value
    } else {
        previous * 0.8 + new_value * 0.2
    }
}

fn segment_visible(viewport: Rect, a: egui::Pos2, b: egui::Pos2) -> bool {
    if viewport.contains(a) || viewport.contains(b) {
        return true;
    }
    let min_x = a.x.min(b.x);
    let max_x = a.x.max(b.x);
    let min_y = a.y.min(b.y);
    let max_y = a.y.max(b.y);
    let seg_rect = Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y));
    viewport.intersects(seg_rect)
}

#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct GraphTab {
    navi: GraphNavigation,
    #[serde(skip, default = "default_arrange_iterations")]
    arrange_iterations: u32,
    #[serde(skip, default)]
    osm_area_name: String,
    #[serde(skip, default)]
    show_perf: bool,
    #[serde(skip, default)]
    perf: GraphPerf,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuGraphRendererState>>,
}

fn default_arrange_iterations() -> u32 {
    1000
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            arrange_iterations: default_arrange_iterations(),
            osm_area_name: String::new(),
            show_perf: false,
            perf: GraphPerf::default(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuGraphRendererState::default(),
            )),
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
    fn offset_y(&self) -> f32 {
        self.y_offset as f32
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f32) {
        self.x_offset = offset_x;
        self.y_offset = offset_y as f64
    }
    fn x_from_f64(&self, value: f64) -> Self::XOffset {
        value
    }
    fn x_to_f64(&self, value: Self::XOffset) -> f64 {
        value
    }
    fn y_from_f32(&self, value: f32) -> Self::YOffset {
        value as f64
    }
    fn y_to_f32(&self, value: Self::YOffset) -> f32 {
        value as f32
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
        ui.add(egui::Slider::new(&mut self.arrange_iterations, 100..=10000).text("Iterations"));
        if ui.button("Arrange graph").clicked() {
            world
                .run_system_cached_with(
                    crate::graph::arrange::auto_arrange_graph,
                    (ui.ctx().clone(), self.arrange_iterations),
                )
                .unwrap();
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("OSM area:");
            ui.text_edit_singleline(&mut self.osm_area_name);
        });
        if ui.button("Arrange via OSM").clicked() {
            let area_name = if self.osm_area_name.is_empty() {
                None
            } else {
                Some(self.osm_area_name.clone())
            };
            world
                .run_system_cached_with(
                    crate::graph::arrange::arrange_via_osm,
                    (ui.ctx().clone(), area_name),
                )
                .unwrap();
        }
        if let Some(task) = world.get_resource::<crate::graph::arrange::GraphLayoutTask>() {
            let (finished, total, queued_retry) = task.progress();
            let mode = match task.kind {
                crate::graph::arrange::GraphLayoutKind::ForceDirected => "Force",
                crate::graph::arrange::GraphLayoutKind::OSM => "OSM",
            };
            ui.label(format!(
                "Arrange ({mode}) progress: {finished}/{total} | retry queued: {queued_retry}"
            ));
            if total > 0 {
                ui.add(egui::ProgressBar::new(finished as f32 / total as f32));
            }
        }
        ui.separator();
        ui.checkbox(&mut self.show_perf, "Show perf");
        if self.show_perf {
            ui.monospace(format!(
                "CPU collect: {:.2} ms\n  - Index query: {:.2} ms\n  - Nodes: {:.2} ms\n  - Edges: {:.2} ms\n  - Trips: {:.2} ms\n  - Label cull: {:.2} ms\nGPU upload prep: {:.2} ms\nText draw: {:.2} ms\nFrame total: {:.2} ms\nShapes: {}\nLabels: {}",
                self.perf.collect_ms,
                self.perf.collect_index_query_ms,
                self.perf.collect_nodes_ms,
                self.perf.collect_edges_ms,
                self.perf.collect_trips_ms,
                self.perf.collect_label_cull_ms,
                self.perf.gpu_upload_ms,
                self.perf.text_ms,
                self.perf.frame_ms,
                self.perf.shape_count,
                self.perf.label_count,
            ));
        }
    }
}

fn display(tab: &mut GraphTab, world: &mut World, ui: &mut egui::Ui) {
    let frame_start = Instant::now();
    let (response, painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);
    draw_world_grid(
        &painter,
        tab.navi.visible,
        Vec2 {
            x: tab.navi.x_offset as f32,
            y: tab.navi.y_offset as f32,
        },
        tab.navi.zoom,
    );

    let collect_start = Instant::now();
    let collected = world
        .run_system_cached_with(collect_draw_items, (ui.visuals().dark_mode, &tab.navi))
        .unwrap();
    tab.perf.collect_ms = smooth_ms(
        tab.perf.collect_ms,
        collect_start.elapsed().as_secs_f32() * 1000.0,
    );
    tab.perf.collect_index_query_ms = smooth_ms(
        tab.perf.collect_index_query_ms,
        collected.timings.index_query_ms,
    );
    tab.perf.collect_nodes_ms = smooth_ms(tab.perf.collect_nodes_ms, collected.timings.nodes_ms);
    tab.perf.collect_edges_ms = smooth_ms(tab.perf.collect_edges_ms, collected.timings.edges_ms);
    tab.perf.collect_trips_ms = smooth_ms(tab.perf.collect_trips_ms, collected.timings.trips_ms);
    tab.perf.collect_label_cull_ms = smooth_ms(
        tab.perf.collect_label_cull_ms,
        collected.timings.label_cull_ms,
    );
    let draw_items = collected.items;
    tab.perf.shape_count = draw_items.shapes.len();
    tab.perf.label_count = draw_items.labels.len();

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
    let gpu_upload_start = Instant::now();
    gpu_draw::write_instances(&draw_items.shapes, &mut state);
    tab.perf.gpu_upload_ms = smooth_ms(
        tab.perf.gpu_upload_ms,
        gpu_upload_start.elapsed().as_secs_f32() * 1000.0,
    );
    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);

    let text_start = Instant::now();
    for label in &draw_items.labels {
        painter.text(
            label.pos,
            Align2::LEFT_CENTER,
            &label.text,
            FontId::proportional(13.0),
            label.color,
        );
    }
    tab.perf.text_ms = smooth_ms(
        tab.perf.text_ms,
        text_start.elapsed().as_secs_f32() * 1000.0,
    );
    tab.perf.frame_ms = smooth_ms(
        tab.perf.frame_ms,
        frame_start.elapsed().as_secs_f32() * 1000.0,
    );
}

fn draw_world_grid(painter: &Painter, viewport: Rect, offset: Vec2, zoom: f32) {
    if zoom <= 0.0 {
        return;
    }

    // Transitions like diagram.rs: Linear fade between MIN and MAX screen spacing
    const MIN_WIDTH: f32 = 32.0;
    const MAX_WIDTH: f32 = 120.0;

    // Use a neutral gray without querying visuals
    let base_color = Color32::from_gray(160);

    for p in ((-5)..=5).rev() {
        let spacing = 10.0f32.powi(p);
        let screen_spacing = spacing * zoom;

        // Strength calculation identical to diagram.rs (1.5 scaling factor)
        let strength =
            ((screen_spacing * 1.5 - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)).clamp(0.0, 1.0);
        if strength <= 0.0 {
            continue;
        }

        let stroke = Stroke::new(0.6, base_color.gamma_multiply(strength));

        // Vertical lines
        let mut n = (offset.x / spacing).floor();
        loop {
            let world_x = n * spacing;
            let screen_x_rel = (world_x - offset.x) * zoom;
            if screen_x_rel > viewport.width() {
                break;
            }
            if screen_x_rel >= 0.0 {
                painter.vline(viewport.left() + screen_x_rel, viewport.y_range(), stroke);
            }
            n += 1.0;
        }

        // Horizontal lines
        let mut m = (offset.y / spacing).floor();
        loop {
            let world_y = m * spacing;
            let screen_y_rel = (world_y - offset.y) * zoom;
            if screen_y_rel > viewport.height() {
                break;
            }
            if screen_y_rel >= 0.0 {
                painter.hline(viewport.x_range(), viewport.top() + screen_y_rel, stroke);
            }
            m += 1.0;
        }
    }
}

fn collect_draw_items(
    (In(is_dark), InRef(navi)): (In<bool>, InRef<GraphNavigation>),
    nodes: Query<(Entity, &Node, Option<&Name>)>,
    spatial_index: Res<GraphSpatialIndex>,
    interval_spatial_index: Res<GraphIntervalSpatialIndex>,
    trip_spatial_index: Res<TripSpatialIndex>,
    settings: Res<ProjectSettings>,
    trip_meta_q: Query<(&Name, &TripClass), With<Trip>>,
    stroke_q: Query<&DisplayedStroke, With<Class>>,
    timer: Res<GlobalTimer>,
) -> CollectedGraphDraw {
    let time = timer.read_seconds();
    let repeat_time = settings.repeat_frequency.0 as f64;
    let query_time = if repeat_time > 0.0 {
        time.rem_euclid(repeat_time)
    } else {
        time
    };
    let mut out = GraphDrawItems::default();
    let mut timings = CollectTimings::default();
    let color = PredefinedColor::Neutral.get(is_dark);
    let view = navi.visible_rect();
    let view_expanded = view.expand2(Vec2::splat(12.0));
    let margin_x = 12.0 / navi.zoom_x().max(f32::EPSILON) as f64;
    let margin_y = 12.0 / navi.zoom_y().max(f32::EPSILON) as f64;
    let visible_x = navi.visible_x();
    let visible_y = navi.visible_y();
    let min_x = visible_x.start - margin_x;
    let max_x = visible_x.end + margin_x;
    let min_y = visible_y.start - margin_y;
    let max_y = visible_y.end + margin_y;

    let index_query_start = Instant::now();
    let candidate_nodes: Vec<Entity> = if spatial_index.is_empty() {
        nodes.iter().map(|(entity, _, _)| entity).collect()
    } else {
        spatial_index.entities_in_xy_aabb(min_x, min_y, max_x, max_y)
    };
    timings.index_query_ms = index_query_start.elapsed().as_secs_f32() * 1000.0;

    let mut screen_pos_by_entity: HashMap<Entity, egui::Pos2> = HashMap::new();
    let mut visible_nodes: HashSet<Entity> = HashSet::new();

    let nodes_start = Instant::now();
    for entity in candidate_nodes {
        let Ok((_, node, name)) = nodes.get(entity) else {
            continue;
        };
        let x = node.pos.x();
        let y = node.pos.y();
        let pos = navi.xy_to_screen_pos(x, y);
        screen_pos_by_entity.insert(entity, pos);

        if !view_expanded.contains(pos) {
            continue;
        }
        visible_nodes.insert(entity);
        out.shapes
            .push(gpu_draw::ShapeSpec::circle(pos, 6.0, color));
        if let Some(name) = name {
            out.labels.push(GraphLabel {
                pos: pos + Vec2 { x: 7.0, y: 0.0 },
                text: name.to_string(),
                color,
            });
        }
    }
    timings.nodes_ms = nodes_start.elapsed().as_secs_f32() * 1000.0;

    let edges_start = Instant::now();
    let mut rendered_intervals: HashSet<Entity> = HashSet::new();
    for segment in interval_spatial_index.query_xy_aabb(min_x, min_y, max_x, max_y) {
        if !rendered_intervals.insert(segment.interval) {
            continue;
        }
        let spos = navi.xy_to_screen_pos(segment.p0[0], segment.p0[1]);
        let tpos = navi.xy_to_screen_pos(segment.p1[0], segment.p1[1]);
        if !segment_visible(view_expanded, spos, tpos) {
            continue;
        }
        out.shapes
            .push(gpu_draw::ShapeSpec::segment(spos, tpos, 1.0, color));
    }
    timings.edges_ms = edges_start.elapsed().as_secs_f32() * 1000.0;

    let trips_start = Instant::now();
    for sample in
        trip_spatial_index.query_xy_time(min_x..=max_x, min_y..=max_y, query_time..=query_time)
    {
        let Ok((name, trip_class)) = trip_meta_q.get(sample.trip) else {
            continue;
        };
        let Ok(stroke) = stroke_q.get(trip_class.entity()) else {
            continue;
        };
        let color = stroke.color.get(is_dark);
        let pos = navi.xy_to_screen_pos(sample.x, sample.y);
        if !view_expanded.contains(pos) {
            continue;
        }
        out.shapes
            .push(gpu_draw::ShapeSpec::circle(pos, 6.0, color));
        out.labels.push(GraphLabel {
            pos: pos + Vec2 { x: 7.0, y: 0.0 },
            text: name.to_string(),
            color,
        });
    }
    timings.trips_ms = trips_start.elapsed().as_secs_f32() * 1000.0;

    timings.label_cull_ms = 0.0;

    CollectedGraphDraw {
        items: out,
        timings,
    }
}
