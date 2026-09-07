use ecow::EcoVec;
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, WidgetText, pos2, vec2};
use paiagram_core::diagram::{Diagram, DiagramPoint};
use paiagram_core::time::{TimetableDuration, TimetableTime};
use paiagram_core::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use paiagram_core::{Command, NodeKey, RouteKey, TripInfo, TripKey};
use serde::{Deserialize, Serialize};

use super::trip::TripTab;
use super::{MainTab, Tab};
use crate::{App, UiCommand};

#[derive(Clone)]
struct Drag {
    trip: TripKey,
    point: DiagramPoint,
    departure: bool,
    seconds: i32,
    origin_x: f32,
    revision: u64,
}

/// Navigation is saved with the tab; geometry and in-progress edits are transient.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct DiagramTab {
    route: RouteKey,
    offset: [f64; 2],
    scale: [f64; 2],
    follow: bool,
    repeat: bool,
    #[serde(skip)]
    cache: Option<(u64, Diagram)>,
    #[serde(skip)]
    selected: Option<(TripKey, TEntryId)>,
    #[serde(skip)]
    drag: Option<Drag>,
    #[serde(default)]
    fitted: bool,
    #[serde(skip)]
    drafting: bool,
    #[serde(skip)]
    draft: EcoVec<(NodeKey, i32)>,
    #[serde(skip)]
    viewport: Option<Rect>,
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl DiagramTab {
    pub(crate) fn new(route: RouteKey) -> Self {
        Self {
            route,
            offset: [0.0, -20.0],
            scale: [0.05, 1.0],
            follow: false,
            repeat: false,
            cache: None,
            selected: None,
            drag: None,
            fitted: false,
            drafting: false,
            draft: EcoVec::new(),
            viewport: None,
        }
    }

    fn fit(&mut self, diagram: &Diagram, rect: Rect, now: f64) {
        let mut first = f64::INFINITY;
        let mut last = f64::NEG_INFINITY;
        for point in diagram.trips.iter().flat_map(|t| t.parts.iter()).flatten() {
            first = first.min(point.time.arr.0 as f64).min(point.time.dep.0 as f64);
            last = last.max(point.time.arr.0 as f64).max(point.time.dep.0 as f64);
        }
        if !first.is_finite() {
            first = now;
            last = now + 3600.0;
        }
        let span = (last - first).max(1800.0);
        self.offset = [first - span * 0.05, -20.0];
        self.scale = [
            (rect.width() as f64 / (span * 1.1)).clamp(0.00001, 20.0),
            (rect.height() as f64 / (diagram.rows.last().map_or(0.0, |r| r.position) + 40.0))
                .clamp(0.01, 20.0),
        ];
        self.fitted = true;
    }

    fn screen(&self, rect: Rect, time: f64, position: f64) -> Pos2 {
        pos2(
            rect.left() + ((time - self.offset[0]) * self.scale[0]) as f32,
            rect.top() + ((position - self.offset[1]) * self.scale[1]) as f32,
        )
    }

    fn inspector(&mut self, app: &mut App, ui: &mut Ui, diagram: &Diagram) {
        if self.drafting {
            ui.label(
                "Click station rows to add timed stops. The first platform is used initially.",
            );
            for (i, (node, seconds)) in self.draft.make_mut().iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    let station = app.nodes.query(*node, |n| *n.parent);
                    egui::ComboBox::from_id_salt("platform")
                        .selected_text(
                            app.nodes.query(*node, |n| n.name.to_string()).unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for n in app
                                .nodes
                                .iter()
                                .filter(|n| Some(*n.parent) == station && *n.is_platform)
                            {
                                if diagram.rows.iter().any(|r| r.nodes.contains(&n.key)) {
                                    ui.selectable_value(node, n.key, n.name.as_str());
                                }
                            }
                        });
                    ui.add(time_value(seconds));
                    ui.small(time_label(*seconds as i64));
                });
            }
            if ui.button("Remove last stop").clicked() {
                self.draft.pop();
            }
            let valid = valid_draft(app, self.route, &self.draft);
            if ui.add_enabled(valid, egui::Button::new("Create trip")).clicked() {
                let key = TripKey::new();
                let entries = draft_entries(app, self.route, &self.draft).unwrap();
                self.selected = entries.first().map(|e| (key, e.id()));
                app.command_queue.push(Command::AddTrip {
                    key,
                    info: TripInfo {
                        name: "New trip".into(),
                        schedule: TripSchedule::new(entries),
                        service_class: None,
                        vehicles: Default::default(),
                    },
                });
                self.drafting = false;
                self.draft.clear();
            }
            if !valid {
                ui.small(
                    "Choose at least two stops in time order with a directed path between them.",
                );
            }
            return;
        }
        let Some((key, id)) = self.selected else {
            ui.label("Select a trip or a stop to edit its timetable.");
            ui.small("Drag arrival or departure handles to change time. Drag empty space to pan; Ctrl + scroll zooms time, Shift + scroll zooms station spacing. Escape cancels a drag.");
            return;
        };
        let Some(trip) = diagram.trips.iter().find(|t| t.key == key) else {
            self.selected = None;
            return;
        };
        ui.heading(trip.name.as_str());
        if ui.button("Open timetable").clicked() {
            app.ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Trip(TripTab::new(key))));
        }
        let Some(point) = trip.parts.iter().flatten().find(|p| p.entry.id() == id) else {
            return;
        };
        ui.label(
            app.nodes.query(point.entry.node_key(), |n| n.name.to_string()).unwrap_or_default(),
        );
        let mut entry = point.entry;
        let mut changed = false;
        match &mut entry {
            TEntry::Pinned { arr, dep, .. } => {
                changed |= time_editor(ui, "Arrival", arr, point.time.arr);
                changed |= time_editor(ui, "Departure", dep, point.time.dep);
            }
            TEntry::PinnedNonStop { node, pass, .. } => {
                changed |= time_editor(ui, "Pass", pass, point.time.arr);
                if ui.button("Make a stop").clicked() {
                    entry = TEntry::Pinned {
                        node: *node,
                        id,
                        arr: *pass,
                        dep: TravelMode::For(TimetableDuration(0)),
                        external: false,
                    };
                    changed = true;
                }
            }
            TEntry::Derived { node, .. } => {
                ui.label("Estimated passing time");
                ui.label(time_label(point.time.arr.0 as i64));
                if ui.button("Pin passing time").clicked() {
                    entry = TEntry::PinnedNonStop {
                        node: *node,
                        id,
                        pass: TravelMode::At(point.time.arr),
                        external: false,
                    };
                    changed = true;
                }
            }
        }
        if changed {
            app.command_queue.push(Command::ChangeTripEntry {
                key,
                id,
                new_entry: entry,
            });
        }
        if ui.button("Remove timetable entry").clicked() {
            app.command_queue.push(Command::RemoveTripEntry { key, id });
            self.selected = None;
        }
    }
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn title(&self) -> WidgetText {
        self.cache
            .as_ref()
            .map(|(_, d)| format!("Diagram · {}", d.name))
            .unwrap_or_else(|| "Diagram".into())
            .into()
    }
    fn id(&self) -> Id {
        Id::new((Self::NAME, self.route))
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        let revision = app.source.revision();
        if self.cache.as_ref().is_none_or(|(r, _)| *r != revision) {
            self.cache = Diagram::build(app.snap(), self.route).map(|d| (revision, d));
        }
        let Some((_, diagram)) = &self.cache else {
            ui.label("This route no longer exists. Restore it with Undo, or open another route from the + menu.");
            return;
        };
        let diagram = diagram.clone();
        if self.drag.as_ref().is_some_and(|d| d.revision != revision) {
            self.drag = None;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag = None;
            self.drafting = false;
            self.draft.clear();
        }
        let mut fit = false;
        ui.horizontal(|ui| {
            ui.heading(diagram.name.as_str());
            fit = ui.button("Fit").clicked();
            ui.checkbox(&mut self.follow, "Follow clock");
            ui.checkbox(&mut self.repeat, "Repeat services");
            if ui.selectable_label(self.drafting, "Draw trip").clicked() {
                self.drafting = !self.drafting;
                self.draft.clear();
                self.drag = None;
            }
        });
        egui::Panel::right(ui.id().with("diagram editor")).default_size(225.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.inspector(app, ui, &diagram));
        });
        if diagram.rows.is_empty() {
            ui.label("Add stations to this route in the Graph tab.");
            return;
        }
        let (area, response) = ui.allocate_exact_size(
            ui.available_size().max(vec2(1.0, 1.0)),
            Sense::click_and_drag(),
        );
        let rect = Rect::from_min_max(
            area.min + vec2(135.0_f32.min(area.width() / 3.0), 28.0),
            area.max,
        );
        self.viewport = Some(rect);
        if !rect.is_positive() {
            return;
        }
        let now = app.timer.ticks().as_seconds_f64();
        if fit || !self.fitted {
            self.fit(&diagram, rect, now);
        }
        if self.follow {
            self.offset[0] = now - rect.width() as f64 / self.scale[0] * 0.3;
            ui.ctx().request_repaint();
        }
        if response.hovered() && self.drag.is_none() {
            let (scroll, modifiers, pointer, pinch) = ui.input(|i| {
                (
                    i.smooth_scroll_delta,
                    i.modifiers,
                    i.pointer.hover_pos(),
                    i.zoom_delta(),
                )
            });
            if let Some(pointer) = pointer {
                let axis = if modifiers.shift { 1 } else { 0 };
                let factor = if modifiers.ctrl || modifiers.command || modifiers.shift {
                    (scroll.y as f64 * 0.005).exp()
                } else {
                    pinch as f64
                };
                if factor != 1.0 {
                    let px = if axis == 0 {
                        pointer.x - rect.left()
                    } else {
                        pointer.y - rect.top()
                    } as f64;
                    let anchor = self.offset[axis] + px / self.scale[axis];
                    self.scale[axis] = (self.scale[axis] * factor).clamp(0.00001, 20.0);
                    self.offset[axis] = anchor - px / self.scale[axis];
                    self.follow = false;
                } else {
                    self.offset[0] -= scroll.x as f64 / self.scale[0];
                    self.offset[1] -= scroll.y as f64 / self.scale[1];
                }
            }
        }
        if let Some(drag) = &mut self.drag {
            drag.seconds =
                (ui.input(|i| i.pointer.latest_pos()).map_or(0.0, |p| p.x - drag.origin_x) as f64
                    / self.scale[0])
                    .round()
                    .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        }
        let mut preview = None;
        if let Some(drag) = &self.drag {
            if let Some(cmd) = drag_command(drag) {
                let mut snap = app.snap().clone();
                if snap.apply_command(cmd).is_some() {
                    preview = Diagram::build(&snap, self.route);
                }
            }
        }
        let displayed = preview.as_ref().unwrap_or(&diagram);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let text = ui.visuals().text_color();
        let faint = ui.visuals().widgets.noninteractive.bg_stroke;
        let step = [
            1_i64, 5, 15, 30, 60, 300, 900, 1800, 3600, 21600, 86400, 604800,
        ]
        .into_iter()
        .find(|s| *s as f64 * self.scale[0] >= 70.0)
        .unwrap_or(604800);
        let begin = (self.offset[0] / step as f64).floor() as i64;
        let count =
            ((rect.width() as f64 / self.scale[0] / step as f64).ceil() as usize + 2).min(500);
        for i in 0..count {
            let seconds = begin.saturating_add(i as i64).saturating_mul(step);
            let x = self.screen(rect, seconds as f64, 0.0).x;
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], faint);
            if x >= rect.left() && x <= rect.right() {
                ui.painter().text(
                    pos2(x, rect.top() - 5.0),
                    Align2::CENTER_BOTTOM,
                    time_label(seconds),
                    FontId::proportional(11.0),
                    text,
                );
            }
        }
        ui.painter().text(
            pos2(rect.left() - 8.0, rect.top() - 5.0),
            Align2::RIGHT_BOTTOM,
            "Station · km",
            FontId::proportional(11.0),
            text,
        );
        for row in &diagram.rows {
            let y = self.screen(rect, 0.0, row.position).y;
            if y >= rect.top() && y <= rect.bottom() {
                painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], faint);
                let label = row.milestone.map_or_else(
                    || row.name.to_string(),
                    |m| format!("{}  {:.1}", row.name, m / 1000.0),
                );
                ui.painter_at(area).text(
                    pos2(rect.left() - 8.0, y),
                    Align2::RIGHT_CENTER,
                    label,
                    FontId::proportional(12.0),
                    text,
                );
            }
        }
        let clock_x = self.screen(rect, now, 0.0).x;
        painter.line_segment(
            [pos2(clock_x, rect.top()), pos2(clock_x, rect.bottom())],
            Stroke::new(1.0, Color32::LIGHT_RED),
        );
        let pointer = (if response.drag_started() {
            ui.input(|i| i.pointer.press_origin())
        } else {
            response.hover_pos()
        })
        .filter(|p| rect.contains(*p));
        let mut hit: Option<(f32, TripKey, DiagramPoint, bool)> = None;
        let period = app.settings.repeat_frequency.0 as f64;
        let end = self.offset[0] + rect.width() as f64 / self.scale[0];
        let mut repeat_limited = false;
        for trip in &displayed.trips {
            let selected = self.selected.is_some_and(|(key, _)| key == trip.key);
            let color = trip.style.map_or(Color32::from_rgb(80, 170, 240), |s| s.color);
            let stroke = Stroke::new(
                trip.style.map_or(1.5, |s| s.width.max(1) as f32)
                    + if selected { 1.5 } else { 0.0 },
                color,
            );
            for part in &trip.parts {
                let min =
                    part.iter().map(|p| p.time.arr.0.min(p.time.dep.0)).min().unwrap_or(0) as f64;
                let max =
                    part.iter().map(|p| p.time.arr.0.max(p.time.dep.0)).max().unwrap_or(0) as f64;
                let (start_cycle, end_cycle) = if self.repeat && period > 0.0 {
                    (
                        ((self.offset[0] - max) / period).ceil() as i64,
                        ((end - min) / period).floor() as i64,
                    )
                } else {
                    (0, 0)
                };
                repeat_limited |= end_cycle.saturating_sub(start_cycle) > 256;
                // Limit pathological zoom-out/repetition work while retaining complete paths.
                for cycle in start_cycle..=end_cycle.min(start_cycle.saturating_add(256)) {
                    let shift = cycle as f64 * period;
                    let mut previous = None;
                    for &point in part {
                        let a = self.screen(rect, point.time.arr.0 as f64 + shift, point.position);
                        let b = self.screen(rect, point.time.dep.0 as f64 + shift, point.position);
                        for (from, to) in
                            previous.map(|p| (p, a)).into_iter().chain(std::iter::once((a, b)))
                        {
                            painter.line_segment([from, to], stroke);
                            if let Some(p) = pointer {
                                let distance = segment_distance(p, from, to);
                                if distance < 6.0
                                    && hit.as_ref().is_none_or(|h| distance + 2.0 < h.0)
                                {
                                    hit = Some((distance + 2.0, trip.key, point, false));
                                }
                            }
                        }
                        for (p, departure) in [(a, false), (b, true)] {
                            if departure
                                && (a == b || !matches!(point.entry, TEntry::Pinned { .. }))
                            {
                                continue;
                            }
                            if selected {
                                painter.circle_filled(p, 3.5, color);
                            }
                            if let Some(pointer) = pointer {
                                let distance = pointer.distance(p);
                                if distance < 8.0 && hit.as_ref().is_none_or(|h| distance < h.0) {
                                    hit = Some((distance, trip.key, point, departure));
                                }
                            }
                        }
                        if part.len() == 1 {
                            painter.circle_stroke(a, 3.0, stroke);
                        }
                        previous = Some(b);
                    }
                }
            }
        }
        if repeat_limited {
            painter.text(
                rect.right_bottom() - vec2(8.0, 8.0),
                Align2::RIGHT_BOTTOM,
                "Zoom in to see all repeated services",
                FontId::proportional(12.0),
                text,
            );
        }
        for &(node, time) in &self.draft {
            if let Some(row) = diagram.rows.iter().find(|r| r.nodes.contains(&node)) {
                painter.circle_filled(
                    self.screen(rect, time as f64, row.position),
                    5.0,
                    Color32::LIGHT_GREEN,
                );
            }
        }
        if let Some((_, key, point, departure)) = hit {
            let name = displayed
                .trips
                .iter()
                .find(|t| t.key == key)
                .map(|t| t.name.as_str())
                .unwrap_or("");
            response.clone().on_hover_text(format!(
                "{name}\n{}: {}\nDrag to edit; estimated times become pinned.",
                if departure {
                    "Departure"
                } else {
                    "Arrival / pass"
                },
                time_label(if departure {
                    point.time.dep.0
                } else {
                    point.time.arr.0
                } as i64)
            ));
            if !self.drafting && (response.clicked() || response.drag_started()) {
                self.selected = Some((key, point.entry.id()));
                app.selected_items = crate::selection::SelectedItem::Trip(key).into();
                if response.drag_started() {
                    self.follow = false;
                    self.drag = Some(Drag {
                        trip: key,
                        point,
                        departure,
                        seconds: 0,
                        origin_x: pointer.unwrap().x,
                        revision,
                    });
                }
            }
            if response.double_clicked() && !self.drafting {
                app.ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Trip(TripTab::new(key))));
            }
        }
        if response.clicked() && !self.drafting && hit.is_none() {
            self.selected = None;
            app.selected_items = crate::SelectedItems::None;
        }
        if response.clicked() && self.drafting {
            if let Some(p) = pointer {
                if let Some(row) = diagram.rows.iter().min_by(|a, b| {
                    (self.screen(rect, 0.0, a.position).y - p.y)
                        .abs()
                        .total_cmp(&(self.screen(rect, 0.0, b.position).y - p.y).abs())
                }) {
                    if let Some(&node) = row.nodes.first() {
                        let seconds =
                            (self.offset[0] + (p.x - rect.left()) as f64 / self.scale[0]).round();
                        self.draft
                            .push((node, seconds.clamp(i32::MIN as f64, i32::MAX as f64) as i32));
                    }
                }
            }
        }
        if response.dragged() && self.drag.is_none() && !self.drafting {
            let delta = ui.input(|i| i.pointer.delta());
            self.offset[0] -= delta.x as f64 / self.scale[0];
            self.offset[1] -= delta.y as f64 / self.scale[1];
            self.follow = false;
        }
        if response.drag_stopped() {
            if let Some(drag) = self.drag.take() {
                if let Some(cmd) = drag_command(&drag) {
                    app.command_queue.push(cmd);
                }
            }
        }
    }
}

fn time_label(seconds: i64) -> String {
    let day = seconds.div_euclid(86400);
    let t = seconds.rem_euclid(86400);
    let clock = format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60);
    if day == 0 {
        clock
    } else {
        format!("{day:+}d {clock}")
    }
}

fn time_editor(ui: &mut Ui, label: &str, mode: &mut TravelMode, estimate: TimetableTime) -> bool {
    let before = *mode;
    ui.push_id(label, |ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt("mode")
            .selected_text(match mode {
                TravelMode::At(_) => "At",
                TravelMode::For(_) => "Duration",
                TravelMode::Flexible => "Flexible",
            })
            .show_ui(ui, |ui| {
                if ui.selectable_label(matches!(mode, TravelMode::At(_)), "At").clicked() {
                    *mode = TravelMode::At(estimate);
                }
                if ui.selectable_label(matches!(mode, TravelMode::For(_)), "Duration").clicked() {
                    *mode = TravelMode::For(TimetableDuration(0));
                }
                if ui.selectable_label(matches!(mode, TravelMode::Flexible), "Flexible").clicked() {
                    *mode = TravelMode::Flexible;
                }
            });
        match mode {
            TravelMode::At(t) => {
                ui.add(time_value(&mut t.0));
            }
            TravelMode::For(d) => {
                ui.add(
                    egui::DragValue::new(&mut d.0)
                        .speed(10)
                        .update_while_editing(false)
                        .custom_formatter(|v, _| TimetableDuration(v as i32).to_string_no_arrow())
                        .custom_parser(|s| TimetableDuration::from_str(s).map(|d| d.0 as f64)),
                );
            }
            TravelMode::Flexible => {}
        }
        ui.small(time_label(estimate.0 as i64));
    });
    before != *mode
}

fn drag_command(drag: &Drag) -> Option<Command> {
    if drag.seconds == 0 {
        return None;
    }
    let shifted = |mode: TravelMode, estimate: TimetableTime| -> Option<TravelMode> {
        Some(match mode {
            TravelMode::At(t) => TravelMode::At(TimetableTime(t.0.checked_add(drag.seconds)?)),
            TravelMode::For(d) => {
                TravelMode::For(TimetableDuration(d.0.checked_add(drag.seconds)?))
            }
            TravelMode::Flexible => {
                TravelMode::At(TimetableTime(estimate.0.checked_add(drag.seconds)?))
            }
        })
    };
    let mut entry = drag.point.entry;
    match &mut entry {
        TEntry::Pinned { arr, dep, .. } => {
            if drag.departure {
                *dep = shifted(*dep, drag.point.time.dep)?;
            } else {
                *arr = shifted(*arr, drag.point.time.arr)?;
            }
        }
        TEntry::PinnedNonStop { pass, .. } => {
            *pass = shifted(*pass, drag.point.time.arr)?;
        }
        TEntry::Derived { node, id } => {
            entry = TEntry::PinnedNonStop {
                node: *node,
                id: *id,
                pass: shifted(TravelMode::Flexible, drag.point.time.arr)?,
                external: false,
            };
        }
    }
    Some(Command::ChangeTripEntry {
        key: drag.trip,
        id: entry.id(),
        new_entry: entry,
    })
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    if d.length_sq() == 0.0 {
        return p.distance(a);
    }
    p.distance(a + d * ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0))
}

fn draft_entries(app: &App, route: RouteKey, draft: &[(NodeKey, i32)]) -> Option<EcoVec<TEntry>> {
    if draft.len() < 2 || draft.windows(2).any(|p| p[0].1 > p[1].1) {
        return None;
    }
    let mut entries = EcoVec::new();
    let paths = paiagram_core::diagram::draft_paths(app.snap(), route, draft)?;
    for (index, &(node, time)) in draft.iter().enumerate() {
        if index > 0 {
            let path = &paths[index - 1];
            for &node in path.iter().skip(1).take(path.len().saturating_sub(2)) {
                entries.push(TEntry::Derived {
                    node,
                    id: TEntryId::new(),
                });
            }
        }
        entries.push(TEntry::Pinned {
            node,
            id: TEntryId::new(),
            arr: TravelMode::At(TimetableTime(time)),
            dep: TravelMode::For(TimetableDuration(0)),
            external: false,
        });
    }
    Some(entries)
}

fn valid_draft(app: &App, route: RouteKey, draft: &[(NodeKey, i32)]) -> bool {
    paiagram_core::diagram::draft_paths(app.snap(), route, draft).is_some()
}

fn time_value(seconds: &mut i32) -> egui::DragValue<'_> {
    egui::DragValue::new(seconds)
        .speed(60)
        .update_while_editing(false)
        .custom_formatter(|v, _| TimetableTime(v as i32).to_string())
        .custom_parser(|s| TimetableTime::from_str(s).map(|t| t.0 as f64))
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;
    use paiagram_core::{
        Interval, LonLat, NodeInfo, RouteInfo, RouteStationRecord, SaveFile, Source, StationInfo,
        StationKey, WorldSnapshot,
    };

    use super::*;

    fn fixture(ctx: &egui::Context) -> (App, DiagramTab, TripKey, TEntryId) {
        let mut app = App::new(ctx);
        let mut world = WorldSnapshot::default();
        let stations = [StationKey::new(), StationKey::new()];
        let nodes = [NodeKey::new(), NodeKey::new()];
        for i in 0..2 {
            world
                .apply_command(Command::AddStation {
                    key: stations[i],
                    info: StationInfo {
                        name: format!("Station {i}").into(),
                        pos: LonLat::ZERO,
                    },
                })
                .unwrap();
            world
                .apply_command(Command::AddNode {
                    key: nodes[i],
                    info: NodeInfo {
                        name: "Platform".into(),
                        pos: LonLat::ZERO,
                        parent: stations[i],
                        is_platform: true,
                    },
                })
                .unwrap();
        }
        world
            .apply_command(Command::AddInterval {
                key: (nodes[0], nodes[1]),
                info: Interval {
                    nodes: eco_vec![LonLat::ZERO, LonLat::ZERO],
                    length: std::num::NonZeroU32::new(1000),
                    trips: EcoVec::new(),
                },
            })
            .unwrap();
        let route = RouteKey::new();
        world
            .apply_command(Command::AddRoute {
                key: route,
                info: RouteInfo {
                    name: "Route".into(),
                    stations: eco_vec![
                        RouteStationRecord::for_station(&world, stations[0], None),
                        RouteStationRecord::for_station(&world, stations[1], Some(stations[0]))
                    ],
                },
            })
            .unwrap();
        let trip = TripKey::new();
        let id = TEntryId::new();
        world
            .apply_command(Command::AddTrip {
                key: trip,
                info: TripInfo {
                    name: "Test train".into(),
                    service_class: None,
                    vehicles: Default::default(),
                    schedule: TripSchedule::new(eco_vec![
                        TEntry::Pinned {
                            node: nodes[0],
                            id,
                            arr: TravelMode::At(TimetableTime(3600)),
                            dep: TravelMode::For(TimetableDuration(120)),
                            external: false
                        },
                        TEntry::PinnedNonStop {
                            node: nodes[1],
                            id: TEntryId::new(),
                            pass: TravelMode::At(TimetableTime(4200)),
                            external: false
                        },
                    ]),
                },
            })
            .unwrap();
        app.source = Source::try_from(SaveFile::from(world)).unwrap();
        (app, DiagramTab::new(route), trip, id)
    }

    fn frame(ctx: &egui::Context, app: &mut App, tab: &mut DiagramTab, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| tab.main_display(app, ui),
        )
        .drop_without_applying_deltas();
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn handle_drag_previews_then_commits_once_and_undo_restores_schedule() {
        let ctx = egui::Context::default();
        let (mut app, mut tab, trip, _) = fixture(&ctx);
        frame(&ctx, &mut app, &mut tab, vec![]);
        let start = tab.screen(tab.viewport.unwrap(), 3720.0, 0.0);
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::PointerMoved(start + vec2(15.0, 0.0))],
        );
        assert!(
            tab.drag.is_some(),
            "Dragging must hit the press origin, even after a large first movement"
        );
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::PointerMoved(start + vec2(30.0, 0.0))],
        );
        assert!(app.command_queue.is_empty());
        assert_eq!(app.source.revision(), 0);
        let delta = (30.0 / tab.scale[0]).round() as i32;
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![button(start + vec2(30.0, 0.0), false)],
        );
        assert_eq!(app.command_queue.len(), 1);
        app.apply_commands();
        let departure = |app: &App| {
            app.trips
                .query(trip, |t| match t.schedule.entries()[0] {
                    TEntry::Pinned {
                        dep: TravelMode::For(d),
                        ..
                    } => d.0,
                    _ => panic!(),
                })
                .unwrap()
        };
        assert_eq!(departure(&app), 120 + delta);
        assert!(app.source.undo());
        assert_eq!(departure(&app), 120);
        assert!(!app.source.undoable());
        assert!(app.source.redo());
        assert_eq!(departure(&app), 120 + delta);
    }

    #[test]
    fn escape_cancels_drag_and_history_refreshes_cached_geometry() {
        let ctx = egui::Context::default();
        let (mut app, mut tab, _, _) = fixture(&ctx);
        frame(&ctx, &mut app, &mut tab, vec![]);
        let start = tab.screen(tab.viewport.unwrap(), 3720.0, 0.0);
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::PointerMoved(start + vec2(15.0, 0.0))],
        );
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(
            &ctx,
            &mut app,
            &mut tab,
            vec![button(start + vec2(15.0, 0.0), false)],
        );
        assert!(app.command_queue.is_empty());
        assert!(tab.drag.is_none());
        assert!(app.source.apply_command(Command::RemoveRoute { key: tab.route }));
        frame(&ctx, &mut app, &mut tab, vec![]);
        assert!(tab.cache.is_none());
        assert!(app.source.undo());
        frame(&ctx, &mut app, &mut tab, vec![]);
        assert_eq!(tab.cache.as_ref().unwrap().1.name, "Route");
    }

    #[test]
    fn dragging_estimated_time_pins_without_changing_the_entry_id() {
        let id = TEntryId::new();
        let drag = Drag {
            trip: TripKey::new(),
            point: DiagramPoint {
                position: 10.0,
                entry: TEntry::Derived {
                    node: NodeKey::new(),
                    id,
                },
                time: paiagram_core::trip::TEstimate {
                    arr: TimetableTime(100),
                    dep: TimetableTime(100),
                },
            },
            departure: false,
            seconds: 30,
            origin_x: 0.0,
            revision: 0,
        };
        let Some(Command::ChangeTripEntry {
            id: new_id,
            new_entry:
                TEntry::PinnedNonStop {
                    pass: TravelMode::At(t),
                    ..
                },
            ..
        }) = drag_command(&drag)
        else {
            panic!()
        };
        assert_eq!(new_id, id);
        assert_eq!(t.0, 130);
    }
}
