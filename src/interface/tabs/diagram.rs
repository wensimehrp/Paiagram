use crate::graph::Station;
use crate::interface::SelectedElement;
use crate::interface::tabs::Tab;
use crate::interface::widgets::{buttons, timetable_popup};
use crate::lines::DisplayedLine;
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::AdjustTimetableEntry;
use crate::vehicles::entries::ActualRouteEntry;
use crate::vehicles::entries::{TimetableEntry, TimetableEntryCache, TravelMode};
use crate::vehicles::vehicle_set::VehicleSet;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Popup, Pos2, Rect, RichText, Sense,
    Shape, Stroke, Ui, UiBuilder, Vec2, response, vec2,
};
use egui_i18n::tr;
use moonshine_core::kind::Instance;
use serde::{Deserialize, Serialize};
use strum::EnumCount;
use strum_macros::EnumCount;
mod calculate_lines;
mod edit_line;

// Time and time-canvas related constants
const TICKS_PER_SECOND: i64 = 100;

// TODO: implement multi select and editing
#[derive(PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)]
pub enum SelectedEntityType {
    Vehicle(Entity),
    TimetableEntry { entry: Entity, vehicle: Entity },
    Interval((Instance<Station>, Instance<Station>)),
    Station(Instance<Station>),
    Map(Entity),
}

#[derive(Debug, Clone)]
pub struct DiagramPageCache {
    /// The previous total drag delta, used for dragging time points on the canvas
    previous_total_drag_delta: Option<f32>,
    /// The stroke style used for drawing lines on the diagram
    /// TODO: make this adapt to dark and light mode, and train settings
    stroke: Stroke,
    /// Horizontal tick offset for panning
    tick_offset: i64,
    /// Vertical offset for panning
    vertical_offset: f32,
    zoom: Vec2,
    line_cache: DiagramLineCache,
}

impl MapEntities for DiagramPageCache {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some(heights) = &mut self.line_cache.heights {
            for (station, _) in heights.iter_mut() {
                station.map_entities(entity_mapper);
            }
        }
        for entity in &mut self.line_cache.vehicle_entities {
            entity.map_entities(entity_mapper);
        }
        if let Some(vehicle_set) = &mut self.line_cache.vehicle_set {
            vehicle_set.map_entities(entity_mapper);
        }
    }
}

impl DiagramPageCache {
    // linear search is quicker for a small data set
    fn get_visible_stations(&self, range: std::ops::Range<f32>) -> &[(Instance<Station>, f32)] {
        let Some(heights) = &self.line_cache.heights else {
            return &[];
        };
        let first_visible = heights.iter().position(|(_, h)| *h > range.start);
        let last_visible = heights.iter().rposition(|(_, h)| *h < range.end);
        if let (Some(mut first_visible), Some(mut last_visible)) = (first_visible, last_visible) {
            // saturating sub 2 to add some buffer
            first_visible = first_visible.saturating_sub(2);
            last_visible = (last_visible + 1).min(heights.len() - 1);
            &heights[first_visible..=last_visible]
        } else {
            &[]
        }
    }
}

impl Default for DiagramPageCache {
    fn default() -> Self {
        Self {
            previous_total_drag_delta: None,
            tick_offset: 0,
            vertical_offset: 0.0,
            stroke: Stroke {
                width: 1.0,
                color: Color32::BLACK,
            },
            zoom: vec2(0.0005, 1.0),
            line_cache: DiagramLineCache::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct DiagramLineCache {
    heights: Option<Vec<(Instance<Station>, f32)>>,
    vehicle_entities: Vec<Entity>,
    vehicle_set: Option<Entity>,
    line_missing: bool,
    last_render_context: Option<DiagramRenderContext>,
}

#[derive(Debug, Clone)]
struct DiagramRenderContext {
    screen_rect: Rect,
    vertical_visible: std::ops::Range<f32>,
    horizontal_visible: std::ops::Range<i64>,
    ticks_per_screen_unit: f64,
}

#[derive(Debug, Clone)]
struct DiagramLineParams {
    tick_offset: i64,
    vertical_offset: f32,
    zoom_y: f32,
    stroke: Stroke,
}

type PointData = (Pos2, Option<Pos2>, ActualRouteEntry);

#[derive(Debug, Clone)]
struct RenderedVehicle {
    segments: Vec<Vec<PointData>>,
    stroke: Stroke,
    entity: Entity,
}

#[derive(PartialEq, Debug, Default, Clone, EnumCount, Serialize, Deserialize)]
enum EditingState {
    #[default]
    None,
    EditingLine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramTab {
    pub displayed_line_entity: Entity,
    editing: EditingState,
    #[serde(skip, default)]
    state: DiagramPageCache,
    #[serde(skip)]
    typst_output: String,
}

impl MapEntities for DiagramTab {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.displayed_line_entity.map_entities(entity_mapper);
        self.state.map_entities(entity_mapper);
    }
}

impl DiagramTab {
    pub fn new(displayed_line_entity: Entity) -> Self {
        Self {
            displayed_line_entity,
            editing: EditingState::default(),
            state: DiagramPageCache::default(),
            typst_output: String::new(),
        }
    }
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.displayed_line_entity == other.displayed_line_entity
    }
}

impl Eq for DiagramTab {}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn main_display(&mut self, world: &mut World, ui: &mut Ui) {
        let mut calculated_vehicles: Vec<RenderedVehicle> = Vec::new();
        Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .show(ui, |ui| {
                self.state.stroke.color = ui.visuals().text_color();

                let (response, mut painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

                handle_navigation(ui, &response, &mut self.state);

                let (vertical_visible, horizontal_visible, ticks_per_screen_unit) =
                    calculate_visible_ranges(&self.state, &response.rect);

                self.state.line_cache.last_render_context = Some(DiagramRenderContext {
                    screen_rect: response.rect,
                    vertical_visible: vertical_visible.clone(),
                    horizontal_visible: horizontal_visible.clone(),
                    ticks_per_screen_unit,
                });

                let line_params = DiagramLineParams {
                    tick_offset: self.state.tick_offset,
                    vertical_offset: self.state.vertical_offset,
                    zoom_y: self.state.zoom.y,
                    stroke: self.state.stroke.clone(),
                };

                if let Err(e) = world.run_system_cached_with(
                    calculate_lines::calculate_lines,
                    (
                        self.displayed_line_entity,
                        &mut self.state.line_cache,
                        &mut calculated_vehicles,
                        line_params,
                    ),
                ) {
                    error!("Error calculating lines for diagram: {}", e);
                }

                if let Err(e) = world.run_system_cached_with(
                    show_diagram,
                    (
                        ui,
                        &mut self.state,
                        &calculated_vehicles,
                        &mut painter,
                        &response,
                    ),
                ) {
                    error!(
                        "UI Error while displaying diagram ({}): {}",
                        self.displayed_line_entity, e
                    )
                }
            });
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        ui.group(|ui| {
            world.run_system_cached_with(
                select_vehicle_set,
                (ui, &mut self.state.line_cache.vehicle_set),
            );
            if ui
                .button("Generate intervals from displayed line")
                .clicked()
                && let Err(e) = world.run_system_once_with(
                    crate::lines::create_intervals_from_displayed_line,
                    self.displayed_line_entity,
                )
            {
                error!(
                    "Error while generating intervals from displayed line: {:?}",
                    e
                )
            }
        });
        // edit line, edit stations on line, etc.
        let width = ui.available_width();
        let spacing = ui.spacing().item_spacing.x;
        let element_width = (width - spacing) / EditingState::COUNT as f32;
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [element_width, 30.0],
                    egui::Button::new("None").selected(self.editing == EditingState::None),
                )
                .clicked()
            {
                self.editing = EditingState::None;
            }
            if ui
                .add_sized(
                    [element_width, 30.0],
                    egui::Button::new("Edit Lines")
                        .selected(self.editing == EditingState::EditingLine),
                )
                .clicked()
            {
                self.editing = EditingState::EditingLine;
            }
        });
        // match current_tab.0 {
        //     // There are nothing selected. In this case, provide tools for editing the displayed line itself
        //     // and the vehicles on it.
        //     None => {}
        //     _ => {}
        // }
        if self.editing == EditingState::EditingLine {
            egui::ScrollArea::vertical().show(ui, |ui| {
                world.run_system_cached_with(edit_line::edit_line, (ui, self.displayed_line_entity))
            });
        }
    }
    fn display_display(&mut self, world: &mut World, ui: &mut Ui) {
        let current_tab = world.resource::<SelectedElement>();
        use super::super::side_panel::*;
        match current_tab.0 {
            None => {
                // this is technically oblique, but let's just wait until the next version of egui.
                ui.label(RichText::new("Nothing Selected").italics());
            }
            Some(SelectedEntityType::Interval(i)) => {
                if let Err(e) =
                    world.run_system_cached_with(interval_stats::show_interval_stats, (ui, i))
                {
                    error!("UI Error while displaying interval stats: {}", e);
                }
            }
            Some(SelectedEntityType::Map(_)) => {}
            Some(SelectedEntityType::Station(s)) => {
                if let Err(e) =
                    world.run_system_cached_with(station_stats::show_station_stats, (ui, s))
                {
                    error!("UI Error while displaying station stats: {}", e);
                }
            }
            Some(SelectedEntityType::TimetableEntry {
                entry: _,
                vehicle: _,
            }) => {}
            Some(SelectedEntityType::Vehicle(v)) => {
                if let Err(e) =
                    world.run_system_cached_with(vehicle_stats::show_vehicle_stats, (ui, v))
                {
                    error!("UI Error while displaying vehicle stats: {}", e);
                }
            }
        }
    }
    fn export_display(&mut self, world: &mut World, ui: &mut Ui) {
        ui.group(|ui| {
            ui.strong(tr!("tab-diagram-export-typst"));
            ui.label(tr!("tab-diagram-export-typst-desc"));
            // TODO: make the export range configurable
            if ui.button(tr!("export")).clicked() {
                let mut calculated_vehicles: Vec<RenderedVehicle> = Vec::new();
                let mut line_cache = self.state.line_cache.clone();
                let max_height = line_cache
                    .heights
                    .as_ref()
                    .and_then(|h| h.last().map(|(_, h)| *h))
                    .unwrap_or(0.0);
                let ticks_per_screen_unit = 1.0 / self.state.zoom.x as f64;
                let horizontal_visible = 0..86400 * TICKS_PER_SECOND;
                let vertical_visible = 0.0..max_height;
                let width = (horizontal_visible.end - horizontal_visible.start) as f64
                    / ticks_per_screen_unit;
                let height = max_height * self.state.zoom.y;
                line_cache.last_render_context = Some(DiagramRenderContext {
                    screen_rect: Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height)),
                    vertical_visible,
                    horizontal_visible,
                    ticks_per_screen_unit,
                });
                if let Err(e) = world.run_system_cached_with(
                    calculate_lines::calculate_lines,
                    (
                        self.displayed_line_entity,
                        &mut line_cache,
                        &mut calculated_vehicles,
                        DiagramLineParams {
                            tick_offset: 0,
                            vertical_offset: 0.0,
                            zoom_y: self.state.zoom.y,
                            stroke: self.state.stroke.clone(),
                        },
                    ),
                ) {
                    error!("Error calculating lines for diagram: {}", e);
                }
                if let Err(e) = world.run_system_once_with(
                    make_typst_string,
                    (
                        &mut self.typst_output,
                        &calculated_vehicles,
                        width as f32,
                        line_cache
                            .heights
                            .as_ref()
                            .map(|h| h.iter().map(|(_, height)| *height).collect::<Vec<_>>())
                            .as_deref()
                            .unwrap_or(&[]),
                    ),
                ) {
                    error!("UI Error while exporting diagram to typst: {}", e);
                }
            }
            if ui.button(tr!("copy-to-clipboard")).clicked() {
                ui.ctx().copy_text(self.typst_output.clone());
            }
            ui.label(tr!("tab-diagram-export-typst-output", {
                bytes: self.typst_output.len()
            }));
        });
    }
    fn id(&self) -> egui::Id {
        egui::Id::new(self.displayed_line_entity)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn frame(&self) -> egui::Frame {
        egui::Frame::default().inner_margin(egui::Margin::same(2))
    }
}

fn make_typst_string(
    (InMut(buffer), InRef(calculated_vehicles), In(width), InRef(heights)): (
        InMut<String>,
        InRef<[RenderedVehicle]>,
        In<f32>,
        InRef<[f32]>,
    ),
) {
    buffer.clear();
    buffer.push_str(&format!(
        r#"#set page(width: auto, height: auto)
#let render-diagram(
  segments,
  width: {width}pt,
  heights: ({}),
  horizontal_scale: 1,
  vertical_scale: 1,
) = box({{
  for segment in segments {{
    let (first, ..rest) = segment
    let first = curve.move((first.at(0) * 1pt * horizontal_scale, first.at(1) * 1pt * vertical_scale))
    let a = rest.map(((x, y)) => curve.line((x * 1pt * horizontal_scale, y * 1pt * vertical_scale)))
    place(curve(first, ..a))
  }}
  grid(
    columns: (width * horizontal_scale / 24,) * 24,
    rows: {{
      let heights = heights.map(h => h * vertical_scale)
      let (_, a) = heights.fold((0pt, (0pt,)), ((curr, acc), v) => (v, acc + (v - curr,)))
      a
    }},
    stroke: 1pt,
  )
}})

#let segments = (
"#, heights.iter().map(|h| format!("{}pt", h)).collect::<Vec<_>>().join(", "))
    );
    for calculated_vehicle in calculated_vehicles {
        for segment in calculated_vehicle.segments.iter().map(|s| {
            s.iter()
                .flat_map(|(a_pos, d_pos, _entry)| std::iter::once(a_pos).chain(d_pos.iter()))
        }) {
            buffer.push_str("  (\n");
            for point in segment {
                buffer.push_str(&format!("    ({}, {}),\n", point.x, point.y));
            }
            buffer.push_str("  ),\n");
        }
    }
    buffer.push_str("\n)\n#render-diagram(segments)");
}

fn select_vehicle_set(
    (InMut(ui), InMut(vehicle_set)): (InMut<egui::Ui>, InMut<Option<Entity>>),
    vehicle_sets: Query<(Entity, &Name), With<VehicleSet>>,
) {
    let displayed_text = vehicle_set.map_or("None", |e| {
        vehicle_sets
            .get(e)
            .map(|(_, name)| name.as_str())
            .unwrap_or("Unknown")
    });
    egui::ComboBox::from_id_salt("vehicle set")
        .selected_text(displayed_text)
        .show_ui(ui, |ui| {
            for (entity, name) in vehicle_sets {
                ui.selectable_value(vehicle_set, Some(entity), name.as_str());
            }
        });
}

fn show_diagram(
    (InMut(ui), InMut(state), InRef(rendered_vehicles), InMut(mut painter), InRef(response)): (
        InMut<egui::Ui>,
        InMut<DiagramPageCache>,
        InRef<[RenderedVehicle]>,
        InMut<Painter>,
        InRef<response::Response>,
    ),
    station_names: Query<&Name, With<Station>>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    mut selected_element: ResMut<SelectedElement>,
    mut timetable_adjustment_writer: MessageWriter<AdjustTimetableEntry>,
    // Buffer used between all calls to avoid repeated allocations
    mut visible_stations_scratch: Local<Vec<(Instance<Station>, f32)>>,
) {
    if state.line_cache.line_missing {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };

    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;

    let Some(render_context) = state.line_cache.last_render_context.clone() else {
        return;
    };

    // `get_visible_stations` returns a slice borrowed from `state`, so copy it out
    // (into a reusable buffer) to avoid holding an immutable borrow of `state`
    // across later mutations.
    visible_stations_scratch.clear();
    visible_stations_scratch
        .extend_from_slice(state.get_visible_stations(render_context.vertical_visible.clone()));
    let visible_stations: &[(Instance<Station>, f32)] = visible_stations_scratch.as_slice();

    draw_station_lines(
        state.vertical_offset,
        state.stroke,
        &mut painter,
        &render_context.screen_rect,
        state.zoom.y,
        visible_stations,
        ui.pixels_per_point(),
        ui.visuals().text_color(),
        |e| station_names.get(e).ok().map(|s| s.as_str()),
    );

    draw_time_lines(
        state.tick_offset,
        state.stroke,
        &mut painter,
        &render_context.screen_rect,
        render_context.ticks_per_screen_unit,
        &render_context.horizontal_visible,
        ui.pixels_per_point(),
    );

    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        handle_input_selection(
            pos,
            rendered_vehicles,
            visible_stations,
            &render_context.screen_rect,
            state.vertical_offset,
            state.zoom.y,
            &mut selected_element,
        );
    }

    let background_strength = ui.ctx().animate_bool(
        ui.id().with("background animation"),
        selected_element.is_some(),
    );

    draw_vehicles(&mut painter, rendered_vehicles, &mut selected_element);

    if background_strength > 0.1 {
        painter.rect_filled(painter.clip_rect(), CornerRadius::ZERO, {
            let amt = (background_strength * 180.0) as u8;
            if ui.ctx().theme().default_visuals().dark_mode {
                Color32::from_black_alpha(amt)
            } else {
                Color32::from_white_alpha(amt)
            }
        });
    }

    let show_button = state.zoom.x.min(state.zoom.y) > 0.0002;
    let button_strength = ui
        .ctx()
        .animate_bool(ui.id().with("all buttons animation"), show_button);

    match selected_element.0 {
        None => {}
        Some(SelectedEntityType::Vehicle(v)) => {
            draw_vehicle_selection_overlay(
                ui,
                &mut painter,
                &rendered_vehicles,
                state,
                background_strength,
                button_strength,
                v,
                &mut timetable_adjustment_writer,
                &station_names,
                &timetable_entries,
            );
        }
        Some(SelectedEntityType::TimetableEntry {
            entry: _,
            vehicle: _,
        }) => {}
        Some(SelectedEntityType::Interval(i)) => {
            draw_interval_selection_overlay(
                ui,
                background_strength,
                &mut painter,
                render_context.screen_rect,
                state.vertical_offset,
                state.zoom.y,
                i,
                visible_stations,
            );
        }
        Some(SelectedEntityType::Station(s)) => {
            draw_station_selection_overlay(
                ui,
                background_strength,
                &mut painter,
                render_context.screen_rect,
                state.vertical_offset,
                state.zoom.y,
                s,
                visible_stations,
            );
        }
        Some(SelectedEntityType::Map(_)) => {
            todo!("Do this bro")
        }
    }
}

fn ensure_heights(line_cache: &mut DiagramLineCache, displayed_line: &DisplayedLine) {
    let mut current_height = 0.0;
    let mut heights = Vec::new();
    for (station, distance) in displayed_line.stations() {
        current_height += distance.abs().log2().max(1.0) * 15f32;
        heights.push((*station, current_height))
    }
    line_cache.heights = Some(heights);
}

fn calculate_visible_ranges(
    state: &DiagramPageCache,
    rect: &Rect,
) -> (std::ops::Range<f32>, std::ops::Range<i64>, f64) {
    let vertical_visible =
        state.vertical_offset..rect.height() / state.zoom.y + state.vertical_offset;
    let horizontal_visible =
        state.tick_offset..state.tick_offset + (rect.width() as f64 / state.zoom.x as f64) as i64;
    let ticks_per_screen_unit =
        (horizontal_visible.end - horizontal_visible.start) as f64 / rect.width() as f64;
    (vertical_visible, horizontal_visible, ticks_per_screen_unit)
}

fn handle_navigation(ui: &mut Ui, response: &response::Response, state: &mut DiagramPageCache) {
    let mut zoom_delta: Vec2 = Vec2::default();
    let mut translation_delta: Vec2 = Vec2::default();
    ui.input(|input| {
        zoom_delta = input.zoom_delta_2d();
        translation_delta = input.translation_delta();
    });
    if let Some(pos) = response.hover_pos() {
        let old_zoom = state.zoom;
        let mut new_zoom = state.zoom * zoom_delta;
        new_zoom.x = new_zoom.x.clamp(0.00001, 0.4);
        new_zoom.y = new_zoom.y.clamp(0.025, 2048.0);
        let rel_pos = (pos - response.rect.min) / response.rect.size();
        let world_width_before = response.rect.width() as f64 / old_zoom.x as f64;
        let world_width_after = response.rect.width() as f64 / new_zoom.x as f64;
        let world_pos_before_x = state.tick_offset as f64 + rel_pos.x as f64 * world_width_before;
        let new_tick_offset =
            (world_pos_before_x - rel_pos.x as f64 * world_width_after).round() as i64;
        let world_height_before = response.rect.height() as f64 / old_zoom.y as f64;
        let world_height_after = response.rect.height() as f64 / new_zoom.y as f64;
        let world_pos_before_y =
            state.vertical_offset as f64 + rel_pos.y as f64 * world_height_before;
        let new_vertical_offset =
            (world_pos_before_y - rel_pos.y as f64 * world_height_after) as f32;
        state.zoom = new_zoom;
        state.tick_offset = new_tick_offset;
        state.vertical_offset = new_vertical_offset;
    }

    let ticks_per_screen_unit = 1.0 / state.zoom.x as f64;

    state.tick_offset -=
        (ticks_per_screen_unit * (response.drag_delta().x + translation_delta.x) as f64) as i64;
    state.vertical_offset -= (response.drag_delta().y + translation_delta.y) / state.zoom.y;
    state.tick_offset = state.tick_offset.clamp(
        -366 * 86400 * TICKS_PER_SECOND,
        366 * 86400 * TICKS_PER_SECOND
            - (response.rect.width() as f64 / state.zoom.x as f64) as i64,
    );
    const TOP_BOTTOM_PADDING: f32 = 30.0;
    let max_height = state
        .line_cache
        .heights
        .as_ref()
        .and_then(|h| h.last().map(|(_, h)| *h))
        .unwrap_or(0.0);
    state.vertical_offset = if response.rect.height() / state.zoom.y
        > (max_height + TOP_BOTTOM_PADDING * 2.0 / state.zoom.y)
    {
        (-response.rect.height() / state.zoom.y + max_height) / 2.0
    } else {
        state.vertical_offset.clamp(
            -TOP_BOTTOM_PADDING / state.zoom.y,
            max_height - response.rect.height() / state.zoom.y + TOP_BOTTOM_PADDING / state.zoom.y,
        )
    }
}

fn handle_input_selection(
    pointer_pos: Pos2,
    rendered_vehicles: &[RenderedVehicle],
    visible_stations: &[(Instance<Station>, f32)],
    screen_rect: &Rect,
    vertical_offset: f32,
    zoom_y: f32,
    selected_entity: &mut Option<SelectedEntityType>,
) {
    const VEHICLE_SELECTION_RADIUS: f32 = 7.0;
    const STATION_SELECTION_RADIUS: f32 = VEHICLE_SELECTION_RADIUS;
    if selected_entity.is_some() {
        *selected_entity = None;
        return;
    };
    let mut found: Option<SelectedEntityType> = None;
    'check_selected: for vehicle in rendered_vehicles {
        for segment in &vehicle.segments {
            let mut points = segment
                .iter()
                .flat_map(|(a_pos, d_pos, ..)| std::iter::once(*a_pos).chain(*d_pos));

            if let Some(mut curr) = points.next() {
                for next in points {
                    let a = pointer_pos.x - curr.x;
                    let b = pointer_pos.y - curr.y;
                    let c = next.x - curr.x;
                    let d = next.y - curr.y;
                    let dot = a * c + b * d;
                    let len_sq = c * c + d * d;
                    if len_sq == 0.0 {
                        continue;
                    }
                    let t = (dot / len_sq).clamp(0.0, 1.0);
                    let px = curr.x + t * c;
                    let py = curr.y + t * d;
                    let dx = pointer_pos.x - px;
                    let dy = pointer_pos.y - py;

                    if dx * dx + dy * dy < VEHICLE_SELECTION_RADIUS.powi(2) {
                        found = Some(SelectedEntityType::Vehicle(vehicle.entity));
                        break 'check_selected;
                    }
                    curr = next;
                }
            }
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
    // Handle station lines after vehicle lines,
    for (station_entity, height) in visible_stations {
        let y = (*height - vertical_offset) * zoom_y + screen_rect.top();
        if (y - STATION_SELECTION_RADIUS..y + STATION_SELECTION_RADIUS).contains(&pointer_pos.y) {
            found = Some(SelectedEntityType::Station(*station_entity));
            break;
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
    for w in visible_stations.windows(2) {
        let [(e1, h1), (e2, h2)] = w else {
            continue;
        };
        let y1 = (*h1 - vertical_offset) * zoom_y + screen_rect.top();
        let y2 = (*h2 - vertical_offset) * zoom_y + screen_rect.top();
        let (min_y, max_y) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        if (min_y..max_y).contains(&pointer_pos.y) {
            found = Some(SelectedEntityType::Interval((*e1, *e2)));
            break;
        }
    }
    if found.is_some() {
        *selected_entity = found;
        return;
    }
}

fn draw_vehicles(
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    selected_entity: &Option<SelectedEntityType>,
) {
    let mut selected_vehicle = None;
    for vehicle in rendered_vehicles {
        if selected_vehicle.is_none()
            && let Some(selected_entity) = selected_entity
            && matches!(selected_entity, SelectedEntityType::Vehicle(e) if *e == vehicle.entity)
        {
            selected_vehicle = Some(vehicle);
            continue;
        }

        for segment in &vehicle.segments {
            let points = segment
                .iter()
                .flat_map(|(a, d, ..)| std::iter::once(*a).chain(*d))
                .collect::<Vec<_>>();
            painter.line(points, vehicle.stroke);
        }
    }
}

fn draw_vehicle_selection_overlay(
    ui: &mut Ui,
    painter: &mut Painter,
    rendered_vehicles: &[RenderedVehicle],
    state: &mut DiagramPageCache,
    line_strength: f32,
    button_strength: f32,
    selected_entity: Entity,
    timetable_adjustment_writer: &mut MessageWriter<AdjustTimetableEntry>,
    station_names: &Query<&Name, With<Station>>,
    timetable_entries: &Query<(&TimetableEntry, &TimetableEntryCache)>,
) {
    let Some(vehicle) = rendered_vehicles
        .iter()
        .find(|v| selected_entity == v.entity)
    else {
        return;
    };

    let mut stroke = vehicle.stroke;
    stroke.width = line_strength * 3.0 * stroke.width + stroke.width;

    for (line_index, segment) in vehicle.segments.iter().enumerate() {
        let mut line_vec = Vec::with_capacity(segment.len() * 2);

        for idx in 0..segment.len().saturating_sub(1) {
            let (arrival_pos, departure_pos, entry_entity) = segment[idx];
            let (next_arrival_pos, _, next_entry_entity) = segment[idx + 1];
            let Ok((_, entry_cache)) = timetable_entries.get(entry_entity.inner()) else {
                continue;
            };
            let Ok((next_entry, next_entry_cache)) =
                timetable_entries.get(next_entry_entity.inner())
            else {
                continue;
            };
            let signal_stroke = Stroke {
                width: 1.0 + line_strength,
                color: if matches!(next_entry.arrival, TravelMode::For(_)) {
                    Color32::BLUE
                } else {
                    Color32::ORANGE
                },
            };

            line_vec.push(arrival_pos);
            let mut curr_pos = if let Some(d_pos) = departure_pos {
                line_vec.push(d_pos);
                d_pos
            } else {
                arrival_pos
            };

            let mut next_pos = next_arrival_pos;

            curr_pos.y += 5.0;
            next_pos.y += 5.0;

            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.x);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut curr_pos.y);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.x);
            signal_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut next_pos.y);

            let duration = next_entry_cache.estimate.as_ref().unwrap().arrival
                - entry_cache.estimate.as_ref().unwrap().departure;

            let points = if next_pos.y <= curr_pos.y {
                vec![curr_pos, Pos2::new(next_pos.x, curr_pos.y), next_pos]
            } else {
                vec![curr_pos, Pos2::new(curr_pos.x, next_pos.y), next_pos]
            };
            painter.add(Shape::line(points, signal_stroke));

            if duration != Duration(0) {
                let time = duration.to_hms();
                let text = format!("{}:{:02}", time.0 * 60 + time.1, time.2);
                let duration_text = painter.layout_no_wrap(
                    text,
                    egui::FontId::monospace(15.0),
                    signal_stroke.color,
                );
                painter.galley(
                    Pos2 {
                        x: (curr_pos.x + next_pos.x - duration_text.size().x) / 2.0,
                        y: curr_pos.y.max(next_pos.y) + 1.0,
                    },
                    duration_text,
                    signal_stroke.color,
                );
            }
        }

        if let Some(last_pos) = segment.last() {
            line_vec.push(last_pos.0);
            if let Some(departure) = last_pos.1 {
                line_vec.push(departure)
            }
        }
        painter.line(line_vec, stroke);

        let mut previous_entry: Option<ActualRouteEntry> = None;
        if button_strength <= 0.0 {
            continue;
        }
        for fragment in segment
            .iter()
            .copied()
            .filter(|(_, _, a)| matches!(a, ActualRouteEntry::Nominal(_)))
        {
            let (mut arrival_pos, maybe_departure_pos, entry_entity) = fragment;
            let Ok((entry, entry_cache)) = timetable_entries.get(entry_entity.inner()) else {
                continue;
            };
            const HANDLE_SIZE: f32 = 12.0;
            const CIRCLE_HANDLE_SIZE: f32 = 7.0;
            const TRIANGLE_HANDLE_SIZE: f32 = 10.0;
            const DASH_HANDLE_SIZE: f32 = 9.0;
            let departure_pos: Pos2;
            if let Some(unwrapped_pos) = maybe_departure_pos {
                if (arrival_pos.x - unwrapped_pos.x).abs() < HANDLE_SIZE {
                    let midpoint_x = (arrival_pos.x + unwrapped_pos.x) / 2.0;
                    arrival_pos.x = midpoint_x - HANDLE_SIZE / 2.0;
                    let mut pos = unwrapped_pos;
                    pos.x = midpoint_x + HANDLE_SIZE / 2.0;
                    departure_pos = pos;
                } else {
                    departure_pos = unwrapped_pos;
                }
            } else {
                arrival_pos.x -= HANDLE_SIZE / 2.0;
                let mut pos = arrival_pos;
                pos.x += HANDLE_SIZE;
                departure_pos = pos;
            };
            let arrival_point_response =
                ui.place(Rect::from_pos(arrival_pos).expand(5.2), |ui: &mut Ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                    ui
                        .scope_builder(
                            UiBuilder::new()
                                .sense(resp.sense)
                                .max_rect(rect)
                                .id(entry_entity.inner().to_bits() as u128
                                    | (line_index as u128) << 64),
                            |ui| {
                                ui.set_min_size(ui.available_size());
                                let response = ui.response();
                                let fill = if response.hovered() {
                                    Color32::GRAY
                                } else {
                                    Color32::WHITE
                                }
                                .linear_multiply(button_strength);
                                let handle_stroke = Stroke {
                                    width: 2.5,
                                    color: stroke.color.linear_multiply(button_strength),
                                };
                                match entry.arrival {
                                    TravelMode::At(_) => buttons::circle_button_shape(
                                        painter,
                                        arrival_pos,
                                        CIRCLE_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                    TravelMode::For(_) => buttons::dash_button_shape(
                                        painter,
                                        arrival_pos,
                                        DASH_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                    TravelMode::Flexible => buttons::triangle_button_shape(
                                        painter,
                                        arrival_pos,
                                        TRIANGLE_HANDLE_SIZE,
                                        handle_stroke,
                                        fill,
                                    ),
                                };
                            },
                        )
                        .response
                });

            if arrival_point_response.drag_started() {
                state.previous_total_drag_delta = None;
            }
            if let Some(total_drag_delta) = arrival_point_response.total_drag_delta() {
                let previous_drag_delta = state.previous_total_drag_delta.unwrap_or(0.0);
                let duration = Duration(
                    ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                        / state.zoom.x as f64
                        / TICKS_PER_SECOND as f64) as i32,
                );
                if duration != Duration(0) {
                    timetable_adjustment_writer.write(AdjustTimetableEntry {
                        entity: entry_entity.inner(),
                        adjustment: crate::vehicles::TimetableAdjustment::AdjustArrivalTime(
                            duration,
                        ),
                    });
                    state.previous_total_drag_delta = Some(
                        previous_drag_delta
                            + (duration.0 as f64 * TICKS_PER_SECOND as f64 * state.zoom.x as f64)
                                as f32,
                    );
                }
            }
            if arrival_point_response.drag_stopped() {
                state.previous_total_drag_delta = None;
            }
            if arrival_point_response.dragged() {
                arrival_point_response.show_tooltip_ui(|ui| {
                    ui.label(entry_cache.estimate.as_ref().unwrap().arrival.to_string());
                    ui.label(
                        station_names
                            .get(entry.station.entity())
                            .map_or("??", |s| s.as_str()),
                    );
                });
            } else {
                Popup::menu(&arrival_point_response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            entry_entity.inner(),
                            (entry, entry_cache),
                            previous_entry.and_then(|e| timetable_entries.get(e.inner()).ok()),
                            timetable_adjustment_writer,
                            ui,
                            true,
                        );
                    });
            }

            let departure_point_response =
                ui.put(Rect::from_pos(departure_pos).expand(4.5), |ui: &mut Ui| {
                    let (rect, resp) = ui.allocate_exact_size(
                        ui.available_size(),
                        if matches!(
                            entry.departure.unwrap_or(TravelMode::Flexible),
                            TravelMode::Flexible
                        ) {
                            Sense::click()
                        } else {
                            Sense::click_and_drag()
                        },
                    );
                    ui.scope_builder(
                        UiBuilder::new()
                            .sense(resp.sense)
                            .max_rect(rect)
                            .id((entry_entity.inner().to_bits() as u128
                                | (line_index as u128) << 64)
                                ^ (1 << 127)),
                        |ui| {
                            ui.set_min_size(ui.available_size());
                            let response = ui.response();
                            let fill = if response.hovered() {
                                Color32::GRAY
                            } else {
                                Color32::WHITE
                            }
                            .linear_multiply(button_strength);
                            let handle_stroke = Stroke {
                                width: 2.5,
                                color: stroke.color.linear_multiply(button_strength),
                            };
                            match entry.departure {
                                Some(TravelMode::At(_)) => buttons::circle_button_shape(
                                    painter,
                                    departure_pos,
                                    CIRCLE_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                Some(TravelMode::For(_)) => buttons::dash_button_shape(
                                    painter,
                                    departure_pos,
                                    DASH_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                Some(TravelMode::Flexible) => buttons::triangle_button_shape(
                                    painter,
                                    departure_pos,
                                    TRIANGLE_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                                None => buttons::double_triangle(
                                    painter,
                                    departure_pos,
                                    DASH_HANDLE_SIZE,
                                    handle_stroke,
                                    fill,
                                ),
                            };
                        },
                    )
                    .response
                });

            if departure_point_response.drag_started() {
                state.previous_total_drag_delta = None;
            }
            if let Some(total_drag_delta) = departure_point_response.total_drag_delta() {
                let previous_drag_delta = state.previous_total_drag_delta.unwrap_or(0.0);
                let duration = Duration(
                    ((total_drag_delta.x as f64 - previous_drag_delta as f64)
                        / state.zoom.x as f64
                        / TICKS_PER_SECOND as f64) as i32,
                );
                if duration != Duration(0) {
                    timetable_adjustment_writer.write(AdjustTimetableEntry {
                        entity: entry_entity.inner(),
                        adjustment: crate::vehicles::TimetableAdjustment::AdjustDepartureTime(
                            duration,
                        ),
                    });
                    state.previous_total_drag_delta = Some(
                        previous_drag_delta
                            + (duration.0 as f64 * TICKS_PER_SECOND as f64 * state.zoom.x as f64)
                                as f32,
                    );
                }
            }
            if departure_point_response.drag_stopped() {
                state.previous_total_drag_delta = None;
            }
            if departure_point_response.dragged() {
                departure_point_response.show_tooltip_ui(|ui| {
                    ui.label(entry_cache.estimate.as_ref().unwrap().departure.to_string());
                    ui.label(
                        station_names
                            .get(entry.station.entity())
                            .map_or("??", |s| s.as_str()),
                    );
                });
            } else {
                Popup::menu(&departure_point_response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            entry_entity.inner(),
                            (entry, entry_cache),
                            previous_entry.and_then(|e| timetable_entries.get(e.inner()).ok()),
                            timetable_adjustment_writer,
                            ui,
                            false,
                        );
                    });
                previous_entry = Some(entry_entity);
            }
        }
    }
}

fn draw_station_selection_overlay(
    _ui: &mut Ui,
    strength: f32,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    station_entity: Instance<Station>,
    visible_stations: &[(Instance<Station>, f32)],
) {
    let stations = visible_stations
        .iter()
        .copied()
        .filter_map(|(s, h)| if s == station_entity { Some(h) } else { None });
    for station in stations {
        let station_height = (station - vertical_offset) * zoom_y + screen_rect.top();
        painter.rect(
            Rect::from_two_pos(
                Pos2 {
                    x: screen_rect.left(),
                    y: station_height,
                },
                Pos2 {
                    x: screen_rect.right(),
                    y: station_height,
                },
            )
            .expand2(Vec2 { x: -1.0, y: 7.0 }),
            4,
            Color32::BLUE.linear_multiply(strength * 0.5),
            Stroke::new(1.0, Color32::BLUE.linear_multiply(strength)),
            egui::StrokeKind::Middle,
        );
    }
}

fn draw_interval_selection_overlay(
    _ui: &mut Ui,
    strength: f32,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    (s1, s2): (Instance<Station>, Instance<Station>),
    visible_stations: &[(Instance<Station>, f32)],
) {
    for w in visible_stations.windows(2) {
        let [(e1, h1), (e2, h2)] = w else { continue };
        if !((*e1 == s1 && *e2 == s2) || (*e1 == s2 && *e2 == s1)) {
            continue;
        }
        let station_height_1 = (h1 - vertical_offset) * zoom_y + screen_rect.top();
        let station_height_2 = (h2 - vertical_offset) * zoom_y + screen_rect.top();
        painter.rect(
            Rect::from_two_pos(
                Pos2 {
                    x: screen_rect.left(),
                    y: station_height_1,
                },
                Pos2 {
                    x: screen_rect.right(),
                    y: station_height_2,
                },
            )
            .expand2(Vec2 { x: -1.0, y: 7.0 }),
            4,
            Color32::GREEN.linear_multiply(strength * 0.5),
            Stroke::new(1.0, Color32::GREEN.linear_multiply(strength)),
            egui::StrokeKind::Middle,
        );
    }
}

fn draw_station_lines<'a, F>(
    vertical_offset: f32,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    zoom: f32,
    to_draw: &[(Instance<Station>, f32)],
    pixels_per_point: f32,
    text_color: Color32,
    mut get_station_name: F,
) where
    F: FnMut(Entity) -> Option<&'a str>,
{
    // Guard against invalid zoom
    for (entity, height) in to_draw.iter().copied() {
        let mut draw_height = (height - vertical_offset) * zoom + screen_rect.top();
        stroke.round_center_to_pixel(pixels_per_point, &mut draw_height);
        painter.hline(
            screen_rect.left()..=screen_rect.right(),
            draw_height,
            stroke,
        );
        let Some(station_name) = get_station_name(*entity) else {
            continue;
        };
        let layout = painter.layout_no_wrap(
            station_name.to_string(),
            egui::FontId::proportional(13.0),
            text_color,
        );
        let layout_pos = Pos2 {
            x: screen_rect.left(),
            y: draw_height - layout.size().y,
        };
        painter.galley(layout_pos, layout, text_color);
    }
}

fn ticks_to_screen_x(
    ticks: i64,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    offset_ticks: i64,
) -> f32 {
    let base = (ticks - offset_ticks) as f64 / ticks_per_screen_unit;
    screen_rect.left() + base as f32
}

/// Draw vertical time lines and labels
fn draw_time_lines(
    tick_offset: i64,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    ticks_per_screen_unit: f64,
    visible_ticks: &std::ops::Range<i64>,
    pixels_per_point: f32,
) {
    const MAX_SCREEN_WIDTH: f64 = 64.0;
    const MIN_SCREEN_WIDTH: f64 = 32.0;
    const SIZES: &[i64] = &[
        TICKS_PER_SECOND * 1,            // 1 second
        TICKS_PER_SECOND * 10,           // 10 seconds
        TICKS_PER_SECOND * 30,           // 30 seconds
        TICKS_PER_SECOND * 60,           // 1 minute
        TICKS_PER_SECOND * 60 * 5,       // 5 minutes
        TICKS_PER_SECOND * 60 * 10,      // 10 minutes
        TICKS_PER_SECOND * 60 * 30,      // 30 minutes
        TICKS_PER_SECOND * 60 * 60,      // 1 hour
        TICKS_PER_SECOND * 60 * 60 * 4,  // 4 hours
        TICKS_PER_SECOND * 60 * 60 * 24, // 1 day
    ];
    let mut drawn: Vec<i64> = Vec::with_capacity(30);

    // align the first tick to a spacing boundary that is <= visible start.
    let first_visible_position = SIZES
        .iter()
        .position(|s| *s as f64 / ticks_per_screen_unit * 1.5 > MIN_SCREEN_WIDTH)
        .unwrap_or(0);
    let visible = &SIZES[first_visible_position..];
    for (i, spacing) in visible.iter().enumerate().rev() {
        let first = visible_ticks.start - visible_ticks.start.rem_euclid(*spacing) - spacing;
        let mut tick = first;
        let strength = (((*spacing as f64 / ticks_per_screen_unit * 1.5) - MIN_SCREEN_WIDTH)
            / (MAX_SCREEN_WIDTH - MIN_SCREEN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        let mut current_stroke = stroke;
        if strength.is_finite() {
            // strange bug here
            current_stroke.color = current_stroke.color.gamma_multiply(strength as f32);
        }
        current_stroke.width = 0.5;
        while tick <= visible_ticks.end {
            tick += *spacing;
            if drawn.contains(&tick) {
                continue;
            }
            let mut x = ticks_to_screen_x(tick, screen_rect, ticks_per_screen_unit, tick_offset);
            current_stroke.round_center_to_pixel(pixels_per_point, &mut x);
            painter.vline(x, screen_rect.top()..=screen_rect.bottom(), current_stroke);
            drawn.push(tick);
            let time = TimetableTime((tick / 100) as i32);
            let text = match i + first_visible_position {
                0..=2 => time.to_hmsd().2.to_string(),
                3..=8 => format!("{}:{:02}", time.to_hmsd().0, time.to_hmsd().1),
                _ => time.to_string(),
            };
            let label = painter.layout_no_wrap(text, FontId::monospace(13.0), current_stroke.color);
            painter.galley(
                Pos2 {
                    x: x - label.size().x / 2.0,
                    y: screen_rect.top(),
                },
                label,
                current_stroke.color,
            );
        }
    }
}
