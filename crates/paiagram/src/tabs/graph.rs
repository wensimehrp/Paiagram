//! Geographic network editor backed exclusively by Source commands and spatial caches.
use std::sync::Arc;

use ecow::{EcoVec, eco_vec};
use egui::{Align2, Color32, FontId, Frame, Pos2, Rect, Sense, Stroke, Ui, Vec2, WidgetText};
use paiagram_core::spatial::project;
use paiagram_core::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use paiagram_core::*;
use serde::{Deserialize, Serialize};

use super::trip::TripTab;
use super::{MainTab, Navigatable, Tab};
use crate::selection::{SelectedItem, SelectedItems};
use crate::{App, UiCommand};
mod inspector;
mod underlay;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Station(StationKey),
    Node(NodeKey),
    Interval(IntervalKey),
    Trip(TripKey),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub(crate) struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip)]
    underlay: Arc<egui::mutex::Mutex<underlay::UnderlayPainter>>,
    #[serde(skip)]
    selected: Option<Hit>,
    #[serde(skip)]
    coordinate: Option<LonLat>,
    #[serde(skip)]
    drag: Option<(Hit, LonLat, Pos2)>,
    #[serde(skip)]
    route_stations: Vec<StationKey>,
    #[serde(skip)]
    trip_nodes: Vec<NodeKey>,
    #[serde(skip)]
    route: Option<RouteKey>,
    #[serde(skip)]
    route_records: Vec<RouteStationRecord>,
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    fitted: bool,
    #[serde(skip)]
    panel_is_open: bool,
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            underlay_tile_type: underlay::UnderlayTileType::None,
            underlay: Default::default(),
            selected: None,
            coordinate: None,
            drag: None,
            route_stations: Vec::new(),
            trip_nodes: Vec::new(),
            route: None,
            route_records: Vec::new(),
            name: String::new(),
            fitted: false,
            panel_is_open: true,
        }
    }
}
impl PartialEq for GraphTab {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GraphNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: f32,
    visible: Rect,
}
impl Default for GraphNavigation {
    fn default() -> Self {
        Self {
            x_offset: -500.0,
            y_offset: -500.0,
            zoom: 0.5,
            visible: Rect::NOTHING,
        }
    }
}
impl Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 {
        self.zoom
    }
    fn zoom_y(&self) -> f32 {
        self.zoom
    }
    fn set_zoom(&mut self, x: f32, _: f32) {
        self.zoom = x.clamp(0.00001, 20.0);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, x: f64, y: f64) {
        self.x_offset = x;
        self.y_offset = y;
    }
    fn visible_rect(&self) -> Rect {
        self.visible
    }
    fn clamp_zoom(&self, x: f32, _: f32) -> (f32, f32) {
        let z = x.clamp(0.00001, 20.0);
        (z, z)
    }
}

impl GraphNavigation {
    fn screen(&self, p: [f64; 2]) -> Pos2 {
        self.xy_to_screen_pos(p[0], p[1])
    }
    fn fixed_screen(&self, p: XyPos) -> Pos2 {
        let p = XyPosF64::from(p);
        self.screen([p.x, p.y])
    }
    fn coordinate(&self, p: Pos2) -> LonLat {
        let (x, y) = self.screen_pos_to_xy(p);
        Wgs84LonLat::from(XyPosF64::new(x, y)).into()
    }
}

impl Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn title(&self) -> WidgetText {
        egui_i18n::tr!("tab-graph").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        self.route_stations.retain(|k| app.stations.contains_key(*k));
        self.trip_nodes.retain(|k| app.nodes.contains_key(*k));
        if self.selected.is_some_and(|h| !exists(app.snap(), h)) {
            self.selected = None;
        }
        if self.route.is_some_and(|k| !app.routes.contains_key(k)) {
            self.route = None;
        }
        let mut is_open = self.panel_is_open || ui.memory(|mem| mem.everything_is_visible());
        egui::Panel::left(ui.id().with("graph inspector"))
            .default_size(245.0)
            .resizable(true)
            .show_collapsible(ui, &mut is_open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(app, ui));
            });
        self.panel_is_open = is_open;
        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(0))
            .show(ui, |ui| self.map(app, ui));
    }
}

impl GraphTab {
    fn fit(&mut self, world: &WorldSnapshot) {
        let mut points = world
            .stations
            .iter()
            .map(|s| project(*s.pos))
            .chain(world.nodes.iter().map(|n| project(*n.pos)))
            .chain(
                world
                    .intervals
                    .iter()
                    .flat_map(|(_, interval)| interval.nodes.iter().copied().map(project)),
            );
        let Some(first) = points.next() else {
            return;
        };
        let (mut min, mut max) = (first, first);
        for point in points {
            for i in 0..2 {
                min[i] = min[i].min(point[i]);
                max[i] = max[i].max(point[i]);
            }
        }
        let width = self.navi.visible.width().max(100.0) as f64;
        let height = self.navi.visible.height().max(100.0) as f64;
        self.navi.zoom = ((width - 70.0) / (max[0] - min[0]).max(500.0))
            .min((height - 70.0) / (max[1] - min[1]).max(500.0))
            .clamp(0.00001, 20.0) as f32;
        self.navi.x_offset = (min[0] + max[0]) / 2.0 - width / (2.0 * self.navi.zoom as f64);
        self.navi.y_offset = (min[1] + max[1]) / 2.0 - height / (2.0 * self.navi.zoom as f64);
    }

    fn select(&mut self, app: &mut App, hit: Hit) {
        self.selected = Some(hit);
        self.coordinate = None;
        app.selected_items = match hit {
            Hit::Station(k) => SelectedItem::Station(k),
            Hit::Node(k) => SelectedItem::Node(k),
            Hit::Interval(k) => SelectedItem::Interval(k),
            Hit::Trip(k) => SelectedItem::Trip(k),
        }
        .into();
    }

    fn map(&mut self, app: &mut App, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Fit network").clicked() {
                self.fit(app.snap());
            }
            ui.label("Drag to pan · Pinch to zoom · Shift-click nodes to connect");
        });
        let (response, mut painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        self.navi.visible = response.rect;
        if !self.fitted && app.stations.len() > 0 {
            self.fit(app.snap());
            self.fitted = true;
        }
        if response.drag_started()
            && let Some(hit) = self.selected
            && let Some(pos) = position(app.snap(), hit)
            && ui
                .input(|i| i.pointer.press_origin())
                .is_some_and(|p| p.distance(self.navi.screen(project(pos))) < 12.0)
        {
            self.drag = Some((hit, pos, ui.input(|i| i.pointer.press_origin()).unwrap()));
        }
        if self.drag.is_none() && !ui.ctx().egui_wants_keyboard_input() {
            self.navi.handle_navigation(ui, &response);
        }
        let attribution = {
            let mut underlay = self.underlay.lock();
            underlay.update_tile_type(Some(self.underlay_tile_type));
            underlay.draw_underlay(&mut painter, &self.navi, ui)
        };
        let pad = 20.0 / self.navi.zoom as f64;
        let xr = self.navi.visible_x();
        let yr = self.navi.visible_y();
        let min = [xr.start - pad, yr.start - pad];
        let max = [xr.end + pad, yr.end + pad];
        let (geo_min, geo_max) = paiagram_core::spatial::geographic_bounds(min, max);
        let (xy_min, xy_max) = paiagram_core::spatial::projected_bounds(min, max);
        let cursor = response.interact_pointer_pos().or(response.hover_pos());
        let mut candidate: Option<(u8, f32, Hit)> = None;
        let mut offer = |hit, priority, distance: f32| {
            if distance <= 10.0
                && candidate.is_none_or(|(p, d, _)| priority > p || priority == p && distance < d)
            {
                candidate = Some((priority, distance, hit));
            }
        };
        let neutral = ui.visuals().text_color();
        let accent = ui.visuals().selection.bg_fill;
        for edge in app.source.graph_cache().intervals(geo_min, geo_max) {
            for pair in edge.points.windows(2) {
                let a = self.navi.screen(project(pair[0]));
                let b = self.navi.screen(project(pair[1]));
                let selected = self.selected == Some(Hit::Interval(edge.key));
                painter.line_segment(
                    [a, b],
                    Stroke::new(
                        if selected { 3.0 } else { 1.5 },
                        if selected {
                            accent
                        } else {
                            neutral.gamma_multiply(0.55)
                        },
                    ),
                );
                if let Some(p) = cursor {
                    offer(Hit::Interval(edge.key), 0, segment_distance(p, a, b));
                }
                if a.distance(b) > 35.0 {
                    let dir = (b - a).normalized();
                    let mid = a.lerp(b, 0.55);
                    let side = Vec2::new(-dir.y, dir.x);
                    painter.line_segment(
                        [mid - dir * 6.0 + side * 3.0, mid],
                        Stroke::new(1.0, neutral),
                    );
                    painter.line_segment(
                        [mid - dir * 6.0 - side * 3.0, mid],
                        Stroke::new(1.0, neutral),
                    );
                }
            }
        }
        for station in app.source.graph_cache().stations(geo_min, geo_max) {
            let p = self.navi.screen(project(station.point));
            let selected = self.selected == Some(Hit::Station(station.key))
                || self.route_stations.contains(&station.key);
            painter.circle_stroke(
                p,
                if selected { 10.0 } else { 7.0 },
                Stroke::new(2.0, if selected { accent } else { neutral }),
            );
            if let Some(name) = app.stations.query(station.key, |v| v.name.clone()) {
                painter.text(
                    p + Vec2::new(11.0, -12.0),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(13.0),
                    neutral,
                );
            }
            // Station labels remain selectable when the platform occupies the same coordinate.
            if let Some(c) = cursor {
                offer(Hit::Station(station.key), 1, c.distance(p));
                offer(
                    Hit::Station(station.key),
                    3,
                    c.distance(p + Vec2::new(18.0, -12.0)),
                );
            }
        }
        let nodes: Vec<_> = app.source.graph_cache().nodes(geo_min, geo_max).cloned().collect();
        for node in nodes {
            let p = self.navi.screen(project(node.point));
            let Some((platform, name)) =
                app.nodes.query(node.key, |v| (*v.is_platform, v.name.clone()))
            else {
                continue;
            };
            let selected =
                self.selected == Some(Hit::Node(node.key)) || self.trip_nodes.contains(&node.key);
            let color = if selected { accent } else { neutral };
            if platform {
                painter.rect_filled(Rect::from_center_size(p, Vec2::splat(7.0)), 1.0, color);
            } else {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        p + Vec2::new(0.0, -5.0),
                        p + Vec2::new(5.0, 0.0),
                        p + Vec2::new(0.0, 5.0),
                        p + Vec2::new(-5.0, 0.0),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
            if selected {
                painter.circle_stroke(p, 11.0, Stroke::new(2.0, accent));
            }
            if self.navi.zoom > 0.1 && !name.is_empty() {
                painter.text(
                    p + Vec2::new(8.0, 6.0),
                    Align2::LEFT_TOP,
                    name,
                    FontId::proportional(11.0),
                    color,
                );
            }
            if let Some(c) = cursor {
                offer(Hit::Node(node.key), 2, c.distance(p));
            }
        }
        let mut shown = std::collections::HashSet::new();
        for (sample, time) in app.source.graph_cache().trips(
            xy_min,
            xy_max,
            app.timer.ticks().as_seconds_f64(),
            app.settings.repeat_frequency.0 as f64,
        ) {
            if !shown.insert(sample.trip) {
                continue;
            }
            let Some((position, angle)) = sample.position_and_angle(time) else {
                continue;
            };
            let p = self.navi.fixed_screen(position);
            let (name, color) = app
                .trips
                .query(sample.trip, |v| {
                    (
                        v.name.clone(),
                        v.service_class
                            .and_then(|k| app.service_classes.query(k, |c| c.style.color))
                            .unwrap_or(Color32::LIGHT_BLUE),
                    )
                })
                .unwrap();
            if sample.points.windows(2).any(|p| p[0].1 != p[1].1) {
                let dir = Vec2::new(angle.cos() as f32, angle.sin() as f32);
                let side = Vec2::new(-dir.y, dir.x);
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        p + dir * 8.0,
                        p - dir * 5.0 + side * 4.0,
                        p - dir * 5.0 - side * 4.0,
                    ],
                    color,
                    Stroke::NONE,
                ));
            } else {
                painter.circle_filled(p, 5.0, color);
            }
            if self.selected == Some(Hit::Trip(sample.trip)) {
                painter.circle_stroke(p, 10.0, Stroke::new(2.0, accent));
            }
            painter.text(
                p + Vec2::new(8.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(12.0),
                color,
            );
            if let Some(c) = cursor {
                offer(Hit::Trip(sample.trip), 4, c.distance(p));
            }
        }
        if let Some((hit, old, origin)) = self.drag {
            let delta = ui.input(|i| {
                i.pointer
                    .interact_pos()
                    .or(i.pointer.hover_pos())
                    .map(|p| p - origin)
                    .unwrap_or_default()
            });
            let original = self.navi.screen(project(old));
            let pos = self.navi.coordinate(original + delta);
            painter.circle_stroke(original + delta, 12.0, Stroke::new(2.0, accent));
            if response.drag_stopped() {
                if pos != old {
                    app.command_queue.push(move_command(app.snap(), hit, pos));
                }
                self.drag = None;
            }
        }
        if response.clicked() {
            if let Some((_, _, hit)) = candidate {
                if ui.input(|i| i.modifiers.shift) {
                    if let (Some(Hit::Node(a)), Hit::Node(b)) = (self.selected, hit) {
                        if a != b {
                            let cmds = connection_commands(app.snap(), a, b);
                            if !cmds.is_empty() {
                                app.command_queue.push(Command::Macro(cmds.into_boxed_slice()));
                            }
                        }
                    }
                }
                self.select(app, hit);
            } else if let Some(p) = cursor {
                self.coordinate = Some(self.navi.coordinate(p));
                self.selected = None;
                app.selected_items =
                    SelectedItems::Coordinate((self.coordinate.unwrap(), String::new()));
            }
        }
        if let Some(pos) = self.coordinate {
            painter.circle_stroke(
                self.navi.screen(project(pos)),
                8.0,
                Stroke::new(2.0, accent),
            );
        }
        if let Some(attribution) = attribution {
            let rect = Rect::from_min_size(
                response.rect.right_bottom() - Vec2::new(230.0, 22.0),
                Vec2::new(225.0, 20.0),
            );
            ui.put(
                rect,
                egui::Hyperlink::from_label_and_url(
                    format!("© {}", attribution.text),
                    attribution.url,
                ),
            );
        }
        let centre_lat = Wgs84LonLat::from(self.navi.coordinate(response.rect.center())).lat;
        let metres = 100.0 / self.navi.zoom * centre_lat.to_radians().cos() as f32;
        let y = response.rect.bottom() - 14.0;
        let x = response.rect.left() + 12.0;
        painter.line_segment(
            [Pos2::new(x, y), Pos2::new(x + 100.0, y)],
            Stroke::new(2.0, neutral),
        );
        painter.text(
            Pos2::new(x, y - 4.0),
            Align2::LEFT_BOTTOM,
            format!("{metres:.0} m"),
            FontId::proportional(11.0),
            neutral,
        );
    }
}

fn exists(world: &WorldSnapshot, hit: Hit) -> bool {
    match hit {
        Hit::Station(k) => world.stations.contains_key(k),
        Hit::Node(k) => world.nodes.contains_key(k),
        Hit::Interval(k) => world.intervals.contains_key(k),
        Hit::Trip(k) => world.trips.contains_key(k),
    }
}

fn position(world: &WorldSnapshot, hit: Hit) -> Option<LonLat> {
    match hit {
        Hit::Station(k) => world.stations.query(k, |v| *v.pos),
        Hit::Node(k) => world.nodes.query(k, |v| *v.pos),
        _ => None,
    }
}

fn move_command(world: &WorldSnapshot, hit: Hit, pos: LonLat) -> Command {
    match hit {
        Hit::Node(key) => world
            .nodes
            .query(key, |v| Command::ChangeNode {
                key,
                info: NodeInfo {
                    name: v.name.clone(),
                    parent: *v.parent,
                    pos,
                    is_platform: *v.is_platform,
                },
            })
            .unwrap_or_else(Command::new_empty),
        Hit::Station(key) => world
            .stations
            .query(key, |v| {
                let old = project(*v.pos);
                let new = project(pos);
                let mut commands = vec![Command::ChangeStation {
                    key,
                    info: StationInfo {
                        name: v.name.clone(),
                        pos,
                    },
                }];
                for node in v.nodes {
                    if let Some(cmd) = world.nodes.query(*node, |n| {
                        let p = project(*n.pos);
                        Command::ChangeNode {
                            key: *node,
                            info: NodeInfo {
                                name: n.name.clone(),
                                parent: key,
                                pos: Wgs84LonLat::from(XyPosF64::new(
                                    p[0] + new[0] - old[0],
                                    p[1] + new[1] - old[1],
                                ))
                                .into(),
                                is_platform: *n.is_platform,
                            },
                        }
                    }) {
                        commands.push(cmd);
                    }
                }
                Command::Macro(commands.into_boxed_slice())
            })
            .unwrap_or_else(Command::new_empty),
        _ => Command::new_empty(),
    }
}

fn connection_commands(world: &WorldSnapshot, a: NodeKey, b: NodeKey) -> Vec<Command> {
    [(a, b), (b, a)]
        .into_iter()
        .filter(|k| !world.intervals.contains_key(*k))
        .filter_map(|key| {
            Some(Command::AddInterval {
                key,
                info: Interval {
                    nodes: eco_vec![
                        world.nodes.query(key.0, |v| *v.pos)?,
                        world.nodes.query(key.1, |v| *v.pos)?
                    ],
                    length: None,
                    trips: EcoVec::new(),
                },
            })
        })
        .collect()
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let t = if d.length_sq() > 0.0 {
        ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + d * t)
}

fn node_label(world: &WorldSnapshot, key: NodeKey) -> String {
    world
        .nodes
        .query(key, |n| {
            format!(
                "{} / {}",
                world.stations.query(*n.parent, |s| s.name.to_string()).unwrap_or_default(),
                n.name
            )
        })
        .unwrap_or_else(|| "Missing node".into())
}

#[cfg(test)]
mod spatial_tests {
    use super::*;

    #[test]
    fn fit_and_selection_include_the_middle_legs_of_a_curved_interval() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx);
        let mut tab = GraphTab::default();
        let (a, b) = (NodeKey::new(), NodeKey::new());
        let points: EcoVec<LonLat> = [(0.0, 0.0), (0.01, 0.01), (0.02, 0.01), (0.03, 0.0)]
            .into_iter()
            .map(|(x, y)| Wgs84LonLat::new(x, y).into())
            .collect();
        for (key, pos) in [(a, points[0]), (b, *points.last().unwrap())] {
            let station = StationKey::new();
            assert!(app.source.apply_command(Command::AddStation {
                key: station,
                info: StationInfo {
                    name: "S".into(),
                    pos
                }
            }));
            assert!(app.source.apply_command(Command::AddNode {
                key,
                info: NodeInfo {
                    name: "1".into(),
                    parent: station,
                    pos,
                    is_platform: true
                }
            }));
        }
        assert!(app.source.apply_command(Command::AddInterval {
            key: (a, b),
            info: Interval {
                nodes: points.clone(),
                length: None,
                trips: EcoVec::new()
            }
        }));
        let frame = |app: &mut App, tab: &mut GraphTab, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
                    events,
                    ..Default::default()
                },
                |ui| tab.main_display(app, ui),
            )
            .drop_without_applying_deltas();
        };
        frame(&mut app, &mut tab, Vec::new());
        for p in &points {
            assert!(tab.navi.visible.contains(tab.navi.screen(project(*p))));
        }
        let pos =
            tab.navi.screen(project(points[1])).lerp(tab.navi.screen(project(points[2])), 0.5);
        frame(
            &mut app,
            &mut tab,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            &mut tab,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(tab.selected, Some(Hit::Interval((a, b))));
    }
}
