use std::num::NonZeroU32;
use std::sync::Arc;

use ecow::EcoVec;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Margin, Painter, Popup,
    PopupCloseBehavior, Pos2, Rect, Sense, Stroke, Ui, Vec2, WidgetText,
};
use egui_i18n::tr;
use paiagram_core::colors::PredefinedColor;
use paiagram_core::{Command, IntervalKey, LonLat, StationKey};
use paiagram_core::trip::{TEntry, TravelMode};
use serde::{Deserialize, Serialize};

use super::{AppState, Navigatable, Tab};
use crate::{
    CoordinateSelection, SelectedItem, SelectedItems, StationSelection,
};
mod underlay;

/// The state of the graph
enum GraphState<'a> {
    Idle,
    SelectingStations(&'a [StationSelection]),
    SelectingStation(&'a StationSelection),
    SelectingCoordinate(&'a mut CoordinateSelection),
    SelectingTrips,
    SelectingIntervals,
}

impl<'a> From<&'a mut SelectedItems> for GraphState<'a> {
    fn from(selected_items: &'a mut SelectedItems) -> Self {
        match selected_items {
            SelectedItems::Trips(_) => GraphState::SelectingTrips,
            SelectedItems::Intervals(_) => GraphState::SelectingIntervals,
            SelectedItems::Stations(it) => {
                if it.len() == 1 {
                    GraphState::SelectingStation(it.first())
                } else {
                    GraphState::SelectingStations(it)
                }
            }
            SelectedItems::Coordinate(it) => GraphState::SelectingCoordinate(it),
            _ => GraphState::Idle,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip, default)]
    underlay_tile_change: Option<underlay::UnderlayTileType>,
    #[serde(skip, default)]
    arrange_iterations: u32,
    #[serde(skip, default)]
    osm_area_name: String,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<GpuGraphStatePlaceholder>>,
    #[serde(skip, default)]
    highlight_station_intervals: Vec<StationKey>,
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            underlay_tile_type: underlay::UnderlayTileType::None,
            underlay_tile_change: None,
            arrange_iterations: 1000,
            osm_area_name: String::new(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                GpuGraphStatePlaceholder::default(),
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct GraphNavigation {
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

impl Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 { self.zoom }
    fn zoom_y(&self) -> f32 { self.zoom }
    fn set_zoom(&mut self, zoom_x: f32, _zoom_y: f32) { self.zoom = zoom_x; }
    fn offset_x(&self) -> f64 { self.x_offset }
    fn offset_y(&self) -> f64 { self.y_offset }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) { self.x_offset = offset_x; self.y_offset = offset_y; }
    fn visible_rect(&self) -> egui::Rect { self.visible }
}

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn title(&self) -> WidgetText { tr!("tab-graph").into() }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| display(self, app, ui));
    }
    fn edit_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        ui.add(
            egui::Slider::new(&mut self.arrange_iterations, 100..=10000)
                .text(tr!("tab-graph-auto-arrange-iterations")),
        );
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr!("tab-graph-osm-area-name"));
            ui.text_edit_singleline(&mut self.osm_area_name);
        });
        ui.separator();
        // Show selected stations info and route creation
        match &app.selected_items {
            SelectedItems::Stations(stations) => {
                for sel in stations.iter() {
                    if let Some(view) = app.source.stations.get_view(sel.station) {
                        ui.label(view.name.as_str());
                    }
                }
                let route_btn = ui.add_enabled(
                    stations.len() >= 2,
                    egui::Button::new(tr!("graph-create-new-route")),
                );
                if route_btn.clicked() && stations.len() >= 2 {
                    let route_stations: Vec<_> = stations.iter().map(|s| s.station).collect();
                    let rk = paiagram_core::RouteKey::new();
                    app.source.apply_command(Command::AddRoute {
                        key: rk,
                        view: paiagram_core::RouteView {
                            name: "New Route".into(),
                            stations: route_stations.into(),
                        },
                    });
                }
            }
            _ => {}
        }
    }
}

fn display(tab: &mut GraphTab, app: &mut AppState, ui: &mut egui::Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);

    // Draw underlay
    let attribution = underlay::draw_underlay(&mut painter, &tab.navi, ui, tab.underlay_tile_change);

    let is_dark = ui.visuals().dark_mode;
    let color = PredefinedColor::Neutral.get(is_dark);

    let interact_pos = response
        .clicked()
        .then(|| ui.input(|r| r.pointer.interact_pos()))
        .flatten();

    let shift_pressed = ui.input(|r| r.modifiers.shift);
    let ctrl_pressed = ui.input(|r| r.modifiers.command);

    // Show station intervals (draw all, egui clips to viewport)
    for start_key in app.source.stations.keys() {
        let Some(start_view) = app.source.stations.get_view(*start_key) else { continue; };
        let start_pos = tab.navi.xy_to_screen_pos(start_view.pos.lon as f64, start_view.pos.lat as f64);

        let neighbors: Vec<StationKey> = (*app.source).graph.neighbors(*start_key).collect();
        for neighbor_key in neighbors {
            let Some(neighbor_view) = app.source.stations.get_view(neighbor_key) else { continue; };
            let neighbor_pos = tab.navi.xy_to_screen_pos(neighbor_view.pos.lon as f64, neighbor_view.pos.lat as f64);

            let dir = (neighbor_pos - start_pos).normalized();
            let perp = egui::Vec2::new(-dir.y, dir.x);
            let gap = 2.0;
            painter.line_segment([start_pos + perp * gap, neighbor_pos + perp * gap], Stroke::new(1.0, color));
            painter.line_segment([start_pos - perp * gap, neighbor_pos - perp * gap], Stroke::new(1.0, color));
        }
    }

    // Show intervals from IntervalCollection
    for ik in app.source.intervals.keys() {
        if let Some(view) = app.source.intervals.get_view(*ik) {
            for pair in view.nodes.windows(2) {
                let p0 = tab.navi.xy_to_screen_pos(pair[0].lon as f64, pair[0].lat as f64);
                let p1 = tab.navi.xy_to_screen_pos(pair[1].lon as f64, pair[1].lat as f64);
                let d = (p1 - p0).normalized();
                let n = egui::Vec2::new(-d.y, d.x);
                let gap = 1.5;
                painter.line_segment([p0 + n * gap, p1 + n * gap], Stroke::new(0.5, color));
                painter.line_segment([p0 - n * gap, p1 - n * gap], Stroke::new(0.5, color));
            }
        }
    }

    // Draw trip positions at current time
    let current_time = app.timer.read_seconds();
    let repeat_time = app.project_settings.repeat_frequency.0 as f64;
    let query_time = if repeat_time > 0.0 { current_time.rem_euclid(repeat_time) } else { current_time };

    // Build station position map: StationKey -> (lon, lat)
    let mut stn_pos: std::collections::HashMap<paiagram_core::StationKey, (f64, f64)> = std::collections::HashMap::new();
    for sk in app.source.stations.keys() {
        if let Some(v) = app.source.stations.get_view(*sk) {
            stn_pos.insert(*sk, (v.pos.lon as f64, v.pos.lat as f64));
        }
    }

    // For each trip, find the segment active at query_time
    macro_rules! ttime_to_secs {
        ($t:expr) => { $t.0 as f64 };
    }
    for tk in app.source.trips.keys() {
        let Some(view) = app.source.trips.get_view(*tk) else { continue; };
        let entries = view.schedule.entries();
        for pair in entries.windows(2) {
            let (stn_a, ta, stn_b, tb) = match (&pair[0], &pair[1]) {
                (TEntry::Pinned { stn: s1, arr: TravelMode::At(a), .. },
                 TEntry::Pinned { stn: s2, arr: TravelMode::At(b), .. }) => (*s1, *a, *s2, *b),
                _ => continue,
            };
            let Some(&pa) = stn_pos.get(&stn_a) else { continue; };
            let Some(&pb) = stn_pos.get(&stn_b) else { continue; };
            let t1 = ttime_to_secs!(ta);
            let t2 = ttime_to_secs!(tb);
            if t2 <= t1 { continue; }
            let q = query_time.rem_euclid(repeat_time.max(1.0));
            if q < t1 || q > t2 { continue; }

            // Interpolate position
            let f = (q - t1) / (t2 - t1);
            let pos_x = pa.0 + (pb.0 - pa.0) * f;
            let pos_y = pa.1 + (pb.1 - pa.1) * f;
            let screen_a = tab.navi.xy_to_screen_pos(pa.0, pa.1);
            let screen_b = tab.navi.xy_to_screen_pos(pb.0, pb.1);
            let screen_pos = tab.navi.xy_to_screen_pos(pos_x, pos_y);

            // Get class color
            let trip_color = view.class
                .and_then(|ck| app.source.classes.get_view(ck))
                .map(|cv| cv.style.color)
                .unwrap_or(Color32::GRAY);

            // Stealth arrow only (track lines are rendered separately) (matching original GPU shader geometry, rendered in software)
            let dir = (screen_b - screen_a).normalized();
            let perp = egui::Vec2::new(-dir.y, dir.x);
            let arrow_len = 14.0;
            let stealth = 0.2;
            let tip_x = arrow_len * (1.0 - stealth) * 0.5;
            let left_x = -arrow_len * (1.0 + stealth) * 0.5;
            let indent_x = -arrow_len * (1.0 - stealth) * 0.5;
            let arrow_width = arrow_len * (12.0 / 14.0);
            let half_w = arrow_width * 0.5;

            // Two triangles forming a stealth arrowhead
            let tip = screen_pos + dir * tip_x;
            let left_w = screen_pos + dir * left_x + perp * half_w;
            let indent = screen_pos + dir * indent_x;
            let right_w = screen_pos + dir * left_x + perp * -half_w;
            let white_uv = egui::epaint::WHITE_UV;
            let mut mesh = egui::Mesh::default();
            let idx = mesh.vertices.len() as u32;
            for p in [tip, left_w, indent, right_w] {
                mesh.vertices.push(egui::epaint::Vertex { pos: p, uv: white_uv, color: trip_color });
            }
            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx, idx + 2, idx + 3);
            painter.add(egui::Shape::mesh(mesh));

            painter.text(screen_pos + egui::Vec2::new(7.0, -7.0), Align2::LEFT_CENTER,
                &view.name, FontId::proportional(13.0), trip_color);
        }
    }

    // Draw stations
    let mut selected_item: Option<SelectedItem> = None;
    let mut stations_visible: Vec<(StationKey, LonLat, String)> = Vec::new();

    for sk in app.source.stations.keys() {
        if let Some(view) = app.source.stations.get_view(*sk) {
            let pos = tab.navi.xy_to_screen_pos(view.pos.lon as f64, view.pos.lat as f64);
            if !response.rect.contains(pos) { continue; }
            stations_visible.push((*sk, view.pos, view.name.to_string()));

            // Check click
            if let Some(ipos) = interact_pos {
                if (pos - ipos).length() < 10.0 {
                    selected_item = Some(SelectedItem::Station(StationSelection { station: *sk }));
                }
            }

            // Draw station dot
            painter.circle_filled(pos, 4.0, color);

            // Draw name
            painter.text(
                pos + Vec2::new(7.0, 0.0),
                Align2::LEFT_CENTER,
                &view.name,
                FontId::proportional(13.0),
                color,
            );
        }
    }

    // Handle shift+click interval creation BEFORE updating selection
    // so we can detect the second station being clicked while one is already selected
    let clicked_station_key = selected_item.as_ref().and_then(|s| {
        if let SelectedItem::Station(st) = s { Some(st.station) } else { None }
    });
    if shift_pressed {
        if let Some(csk) = clicked_station_key {
            if let SelectedItems::Stations(ref stations) = app.selected_items {
                if stations.len() == 1 && stations[0].station != csk {
                    let from_pos = app.source.stations.query(stations[0].station, |b| *b.pos).unwrap_or(LonLat { lon: 0, lat: 0 });
                    let to_pos = app.source.stations.query(csk, |b| *b.pos).unwrap_or(LonLat { lon: 0, lat: 0 });
                    app.source.apply_command(Command::AddInterval {
                        key: IntervalKey::new(),
                        view: paiagram_core::IntervalView {
                            nodes: EcoVec::from(vec![from_pos, to_pos]),
                            length: NonZeroU32::new(1000),
                        },
                        from: Some(stations[0].station),
                        to: Some(csk),
                    });
                }
            }
        }
    }

    let hit_station = selected_item.is_some();
    let empty_click = interact_pos.is_some() && !hit_station;

    if let Some(sel) = selected_item {
        if ctrl_pressed {
            app.selected_items.toggle_selection(sel);
        } else {
            app.selected_items.set_single_selection(sel);
        }
    } else if empty_click {
        if matches!(app.selected_items, SelectedItems::Coordinate(_)) {
            if !ctrl_pressed && !shift_pressed {
                app.selected_items = SelectedItems::None;
            }
        } else if !ctrl_pressed && !shift_pressed {
            let (x, y) = tab.navi.screen_pos_to_xy(interact_pos.unwrap());
            app.selected_items.set_single_selection(SelectedItem::Coordinate(
                CoordinateSelection {
                    pos: LonLat { lon: x as i32, lat: y as i32 },
                    name_candidate: String::new(),
                },
            ));
        }
    }

    if let SelectedItems::Coordinate(ref mut coord_sel) = app.selected_items {
        let screen_pos = tab.navi.xy_to_screen_pos(coord_sel.pos.lon as f64, coord_sel.pos.lat as f64);
        let rect = Rect::from_pos(screen_pos).expand(6.0);
        painter.rect(rect, 0, Color32::RED.gamma_multiply(0.5), Stroke::new(1.0, Color32::RED), egui::StrokeKind::Middle);
        let res = ui.allocate_rect(rect, Sense::drag()).on_hover_cursor(CursorIcon::Grab);
        if res.dragged() {
            let new_pos = screen_pos + res.drag_delta();
            let (x, y) = tab.navi.screen_pos_to_xy(new_pos);
            coord_sel.pos = LonLat { lon: x as i32, lat: y as i32 };
        }
        let name_ptr = &mut coord_sel.name_candidate as *mut String;
        let pos_copy = coord_sel.pos;
        Popup::menu(&res)
            .open(true)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(200.0);
                let name = unsafe { &mut *name_ptr };
                ui.text_edit_singleline(name);
                if ui.button(tr!("graph-new-station")).clicked() {
                    let key = StationKey::new();
                    app.source.apply_command(Command::AddStation {
                        key,
                        name: if name.is_empty() { "New Station".into() } else { name.as_str().into() },
                        pos: pos_copy,
                    });
                    app.selected_items = SelectedItems::None;
                }
                ui.small(format!("lon:{} lat:{}", pos_copy.lon, pos_copy.lat));
            });
    }

    if let SelectedItems::Stations(ref stations) = app.selected_items {
        for sel in stations.iter() {
            if let Some(view) = app.source.stations.get_view(sel.station) {
                let pos = tab.navi.xy_to_screen_pos(view.pos.lon as f64, view.pos.lat as f64);
                painter.circle(pos, 10.0, Color32::RED.gamma_multiply(0.5), Stroke::new(1.0, Color32::RED));
            }
        }

        if stations.len() == 1 {
            let sk = stations[0].station;
            if let Some(view) = app.source.stations.get_view(sk) {
                let screen_pos = tab.navi.xy_to_screen_pos(view.pos.lon as f64, view.pos.lat as f64);
                let rect = Rect::from_pos(screen_pos).expand(8.0);
                let res = ui.allocate_rect(rect, Sense::drag()).on_hover_cursor(CursorIcon::Grab);
                if res.dragged() {
                    let new_pos = screen_pos + res.drag_delta();
                    let (x, y) = tab.navi.screen_pos_to_xy(new_pos);
                    app.source.apply_command(Command::AddStation {
                        key: sk,
                        name: view.name.clone(),
                        pos: LonLat { lon: x as i32, lat: y as i32 },
                    });
                }
                let sk_ptr = &sk as *const StationKey;
                Popup::menu(&res)
                    .open(true)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_width(150.0);
                        let sk = unsafe { *sk_ptr };
                        if let Some(v) = app.source.stations.get_view(sk) {
                            let mut name = v.name.to_string();
                            ui.text_edit_singleline(&mut name);
                            if name != v.name.as_str() {
                                app.source.apply_command(Command::RenameStation {
                                    key: sk,
                                    name: name.into(),
                                });
                            }
                            ui.small(format!("lon:{} lat:{}", v.pos.lon, v.pos.lat));
                        }
                    });

            }
        }

        // Shift+click preview (when exactly 1 station selected, shift held, hovering)
        if stations.len() == 1 && shift_pressed {
            if let Some(cursor_pos) = ui.input(|r| r.pointer.hover_pos()) {
                if let Some(view) = app.source.stations.get_view(stations[0].station) {
                    let sp = tab.navi.xy_to_screen_pos(view.pos.lon as f64, view.pos.lat as f64);
                    painter.line_segment([sp, cursor_pos], Stroke::new(1.0, Color32::RED));
                }
            }
        }
    }

    // Draw scale bar
    if let Some((text, url)) = attribution {
        draw_attribution(ui, response.rect, &text, &url);
    }
    draw_scale_bar(&painter, response.rect, tab.navi.zoom, ui.visuals().text_color());
}

fn draw_scale_bar(painter: &Painter, viewport: Rect, zoom: f32, color: egui::Color32) {
    if zoom <= 0.0 || !viewport.is_positive() { return; }
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
    painter.line_segment([Pos2::new(left_x, baseline_y), Pos2::new(right_x, baseline_y)], stroke);
    let tick_len = 7.0;
    painter.line_segment([Pos2::new(left_x, baseline_y), Pos2::new(left_x, baseline_y - tick_len)], stroke);
    painter.line_segment([Pos2::new(right_x, baseline_y), Pos2::new(right_x, baseline_y - tick_len)], stroke);
    let mid_tick_len = 5.0;
    for fraction in [0.25f32, 0.5, 0.75] {
        let x = left_x + bar_px * fraction;
        painter.line_segment([Pos2::new(x, baseline_y), Pos2::new(x, baseline_y - mid_tick_len)], stroke);
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
    if value <= 0.0 { return 0.0; }
    let exponent = value.log10().floor();
    let base = 10.0f64.powf(exponent);
    let normalized = value / base;
    let rounded = if normalized <= 1.0 { 1.0 } else if normalized <= 2.0 { 2.0 } else if normalized <= 5.0 { 5.0 } else { 10.0 };
    rounded * base
}

fn format_scale_label(meters: f64) -> String {
    if meters >= 1000.0 {
        let km = meters / 1000.0;
        if (km - km.round()).abs() < 1e-6 { format!("{:.0} km", km) } else { format!("{:.1} km", km) }
    } else {
        format!("{:.0} m", meters)
    }
}

fn draw_attribution(ui: &mut Ui, viewport: Rect, text: &str, url: &str) {
    let margin = 6.0;
    let font_id = FontId::proportional(13.0);
    let color = ui.style().visuals.hyperlink_color;
    let label = format!("© {}", text);
    let galley = ui.painter().layout_no_wrap(label.clone(), font_id, color);
    let size = galley.size();
    let min = Pos2::new(viewport.right() - margin - size.x, viewport.bottom() - margin - size.y);
    let rect = Rect::from_min_size(min, size);
    let mut r = CornerRadius::ZERO;
    r.nw = 4;
    ui.painter().rect_filled(rect.expand(margin), r, Color32::WHITE.gamma_multiply(0.5));
    ui.put(rect, egui::Hyperlink::from_label_and_url(label, url).open_in_new_tab(true));
}

#[derive(Default)]
struct GpuGraphStatePlaceholder;
