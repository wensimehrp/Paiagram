use super::PageCache;
use crate::colors;
use crate::interface::SelectedElement;
use crate::interface::side_panel::CurrentTab;
use crate::interface::tabs::Tab;
use crate::vehicles::entries::{ActualRouteEntry, VehicleScheduleCache};
use crate::{
    interface::widgets::{buttons, timetable_popup},
    intervals::{Station, StationCache},
    lines::DisplayedLine,
    units::time::{Duration, TimetableTime},
    vehicles::{
        AdjustTimetableEntry,
        entries::{TimetableEntry, TimetableEntryCache, TravelMode, VehicleSchedule},
    },
};
use bevy::prelude::*;
use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Painter, Popup, Pos2, Rect, RichText, Sense,
    Shape, Stroke, Ui, UiBuilder, Vec2, response, vec2,
};
use strum::EnumCount;
use strum_macros::EnumCount;
mod edit_line;

// Time and time-canvas related constants
// const SECONDS_PER_WORLD_UNIT: f64 = 1.0; // world units -> seconds
const TICKS_PER_SECOND: i64 = 100;
// const TICKS_PER_WORLD_UNIT: f64 = SECONDS_PER_WORLD_UNIT * TICKS_PER_SECOND as f64;
const LINE_ANIMATION_TIME: f32 = 0.2; // 0.2 seconds

// TODO: implement multi select and editing
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SelectedEntityType {
    Vehicle(Entity),
    TimetableEntry { entry: Entity, vehicle: Entity },
    Interval((Entity, Entity)),
    Station(Entity),
    Map(Entity),
}

#[derive(Debug, Clone)]
pub struct DiagramPageCache {
    previous_total_drag_delta: Option<f32>,
    stroke: Stroke,
    tick_offset: i64,
    vertical_offset: f32,
    heights: Option<Vec<(Entity, f32)>>,
    zoom: Vec2,
    vehicle_entities: Vec<Entity>,
}

impl DiagramPageCache {
    // linear search is quicker for a small data set
    fn get_visible_stations(&self, range: std::ops::Range<f32>) -> &[(Entity, f32)] {
        let Some(heights) = &self.heights else {
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
            heights: None,
            zoom: vec2(0.0005, 1.0),
            vehicle_entities: Vec::new(),
        }
    }
}

type PointData<'a> = (
    Pos2,
    Option<Pos2>,
    (&'a TimetableEntry, &'a TimetableEntryCache),
    ActualRouteEntry,
);

struct RenderedVehicle<'a> {
    segments: Vec<Vec<PointData<'a>>>,
    stroke: Stroke,
    entity: Entity,
}

#[derive(PartialEq, Debug, Default, Clone, EnumCount)]
enum EditingState {
    #[default]
    None,
    EditingLine,
}

#[derive(Debug, Clone)]
pub struct DiagramTab {
    pub displayed_line_entity: Entity,
    editing: EditingState,
    state: DiagramPageCache,
}

impl DiagramTab {
    pub fn new(displayed_line_entity: Entity) -> Self {
        Self {
            displayed_line_entity,
            editing: EditingState::default(),
            state: DiagramPageCache::default(),
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
        if let Err(e) = world.run_system_cached_with(
            show_diagram,
            (ui, self.displayed_line_entity, &mut self.state),
        ) {
            error!(
                "UI Error while displaying diagram ({}): {}",
                self.displayed_line_entity, e
            )
        }
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        // edit line, edit stations on line, etc.
        let current_tab = world.resource::<SelectedElement>();
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
            world.run_system_cached_with(edit_line::edit_line, (ui, self.displayed_line_entity));
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
                world.run_system_cached_with(interval_stats::show_interval_stats, (ui, i));
            }
            Some(SelectedEntityType::Map(i)) => {}
            Some(SelectedEntityType::Station(s)) => {
                world.run_system_cached_with(station_stats::show_station_stats, (ui, s));
            }
            Some(SelectedEntityType::TimetableEntry { entry, vehicle }) => {}
            Some(SelectedEntityType::Vehicle(v)) => {
                world.run_system_cached_with(vehicle_stats::show_vehicle_stats, (ui, v));
            }
        }
    }
    fn id(&self) -> egui::Id {
        egui::Id::new(self.displayed_line_entity)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

fn show_diagram(
    (InMut(ui), In(displayed_line_entity), InMut(state)): (
        InMut<egui::Ui>,
        In<Entity>,
        InMut<DiagramPageCache>,
    ),
    displayed_lines: Populated<Ref<DisplayedLine>>,
    vehicles_query: Populated<(Entity, &Name, &VehicleSchedule, &VehicleScheduleCache)>,
    entry_parents: Query<&ChildOf, With<TimetableEntry>>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    station_names: Query<&Name, With<Station>>,
    station_updated: Query<&StationCache, Changed<StationCache>>,
    station_caches: Query<&StationCache, With<Station>>,
    mut selected_element: ResMut<SelectedElement>,
    mut timetable_adjustment_writer: MessageWriter<AdjustTimetableEntry>,
    // Buffer used between all calls to avoid repeated allocations
    mut visible_stations_scratch: Local<Vec<(Entity, f32)>>,
) {
    let Ok(displayed_line) = displayed_lines.get(displayed_line_entity) else {
        ui.centered_and_justified(|ui| ui.heading("Diagram not found"));
        return;
    };

    state.stroke.color = ui.visuals().text_color();

    let entries_updated = displayed_line.is_changed()
        || state.vehicle_entities.is_empty()
        || displayed_line
            .stations
            .iter()
            .copied()
            .any(|(s, _)| station_updated.get(s).is_ok());

    if entries_updated {
        info!("Updating vehicle entities for diagram display");
    }

    ui.style_mut().visuals.menu_corner_radius = CornerRadius::ZERO;
    ui.style_mut().visuals.window_stroke.width = 0.0;

    if entries_updated {
        for station in displayed_line
            .stations
            .iter()
            .filter_map(|(s, _)| station_caches.get(*s).ok())
        {
            station.passing_vehicles(&mut state.vehicle_entities, |e| entry_parents.get(e).ok());
        }
        state.vehicle_entities.sort();
        state.vehicle_entities.dedup();
    }

    if displayed_line.is_changed() || state.heights.is_none() {
        ensure_heights(state, &displayed_line);
    }

    Frame::canvas(ui.style())
        .fill(if ui.visuals().dark_mode {
            Color32::BLACK
        } else {
            Color32::WHITE
        })
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            let (response, mut painter) =
                ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

            let (vertical_visible, horizontal_visible, ticks_per_screen_unit) =
                calculate_visible_ranges(state, &response.rect);

            // `get_visible_stations` returns a slice borrowed from `state`, so copy it out
            // (into a reusable buffer) to avoid holding an immutable borrow of `state`
            // across later mutations.
            visible_stations_scratch.clear();
            visible_stations_scratch
                .extend_from_slice(state.get_visible_stations(vertical_visible.clone()));
            let visible_stations: &[(Entity, f32)] = visible_stations_scratch.as_slice();

            draw_station_lines(
                state.vertical_offset,
                state.stroke,
                &mut painter,
                &response.rect,
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
                &response.rect,
                ticks_per_screen_unit,
                &horizontal_visible,
                ui.pixels_per_point(),
            );

            let rendered_vehicles = collect_rendered_vehicles(
                |e| timetable_entries.get(e).ok(),
                |e| vehicles_query.get(e).ok(),
                visible_stations,
                &horizontal_visible,
                ticks_per_screen_unit,
                state,
                &response.rect,
                &state.vehicle_entities,
            );

            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                handle_input_selection(
                    pos,
                    &rendered_vehicles,
                    visible_stations,
                    &response.rect,
                    state.vertical_offset,
                    state.zoom.y,
                    &mut selected_element,
                );
            }

            let background_strength = ui.ctx().animate_bool(
                ui.id().with("background animation"),
                selected_element.is_some(),
            );

            draw_vehicles(&mut painter, &rendered_vehicles, &mut selected_element);

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

            match selected_element.0 {
                None => {}
                Some(SelectedEntityType::Vehicle(v)) => {
                    draw_vehicle_selection_overlay(
                        ui,
                        &mut painter,
                        &rendered_vehicles,
                        state,
                        background_strength,
                        v,
                        &mut timetable_adjustment_writer,
                        &station_names,
                    );
                }
                Some(SelectedEntityType::TimetableEntry {
                    entry: e,
                    vehicle: v,
                }) => {}
                Some(SelectedEntityType::Interval(i)) => {
                    draw_interval_selection_overlay(
                        ui,
                        background_strength,
                        &mut painter,
                        response.rect,
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
                        response.rect,
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

            handle_navigation(ui, &response, state);
        });
}

fn ensure_heights(state: &mut DiagramPageCache, displayed_line: &DisplayedLine) {
    let mut current_height = 0.0;
    let mut heights = Vec::new();
    for (station, distance) in &displayed_line.stations {
        current_height += distance.abs().log2().max(1.0) * 15f32;
        heights.push((*station, current_height))
    }
    state.heights = Some(heights);
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

/// Collects and transforms vehicle schedule data into screen-space segments for rendering.
/// This function handles the mapping of timetable entries to station lines, including
/// cases where a station might appear multiple times in the diagram.
fn collect_rendered_vehicles<'a, F, G>(
    get_timetable_entries: F,
    get_vehicles: G,
    visible_stations: &[(Entity, f32)],
    horizontal_visible: &std::ops::Range<i64>,
    ticks_per_screen_unit: f64,
    state: &DiagramPageCache,
    screen_rect: &Rect,
    vehicles: &[Entity],
) -> Vec<RenderedVehicle<'a>>
where
    F: Fn(Entity) -> Option<(&'a TimetableEntry, &'a TimetableEntryCache)> + Copy + 'a,
    G: Fn(
            Entity,
        ) -> Option<(
            Entity,
            &'a Name,
            &'a VehicleSchedule,
            &'a VehicleScheduleCache,
        )> + 'a,
{
    let mut rendered_vehicles = Vec::new();

    for (vehicle_entity, _name, schedule, schedule_cache) in
        vehicles.iter().copied().filter_map(|e| get_vehicles(e))
    {
        // Get all repetitions of the schedule that fall within the visible time range.
        let Some(visible_sets) = schedule_cache.get_entries_range(
            schedule,
            TimetableTime((horizontal_visible.start / TICKS_PER_SECOND) as i32)
                ..TimetableTime((horizontal_visible.end / TICKS_PER_SECOND) as i32),
            get_timetable_entries,
        ) else {
            continue;
        };

        let mut segments = Vec::new();

        for (initial_offset, set) in visible_sets {
            // local_edges holds WIP segments and the index of the station line they are currently on.
            let mut local_edges: Vec<(Vec<PointData<'a>>, usize)> = Vec::new();
            let mut previous_indices: Vec<usize> = Vec::new();

            // Initialize previous_indices with all occurrences of the first station in the set.
            if let Some((ce_data, _)) = set.first() {
                let (ce, _) = ce_data;
                previous_indices = visible_stations
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (s, _))| if *s == ce.station { Some(i) } else { None })
                    .collect();
            }

            for entry_idx in 0..set.len() {
                let (ce_data, ce_actual) = &set[entry_idx];
                let (ce, ce_cache) = ce_data;
                let ne = set.get(entry_idx + 1);

                // If the current station isn't visible, try to find the next one and flush WIP edges.
                if previous_indices.is_empty() {
                    if let Some((ne_data, _)) = ne {
                        let (ne_entry, _) = ne_data;
                        previous_indices = visible_stations
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (s, _))| {
                                if *s == ne_entry.station {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .collect();
                    }
                    for (segment, _) in local_edges.drain(..) {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }
                    continue;
                }

                let mut next_local_edges = Vec::new();

                // If there's no time estimate, we can't draw this point. Flush WIP edges.
                let Some(estimate) = ce_cache.estimate.as_ref() else {
                    for (segment, _) in local_edges.drain(..) {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }
                    if let Some((ne_data, _)) = ne {
                        let (ne_entry, _) = ne_data;
                        previous_indices = visible_stations
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (s, _))| {
                                if *s == ne_entry.station {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .collect();
                    }
                    continue;
                };

                // Calculate absolute ticks for arrival and departure.
                let arrival_ticks = (initial_offset.0 as i64
                    + (estimate.arrival.0 - schedule.start.0) as i64)
                    * TICKS_PER_SECOND;
                let departure_ticks = (initial_offset.0 as i64
                    + (estimate.departure.0 - schedule.start.0) as i64)
                    * TICKS_PER_SECOND;

                // For each occurrence of the current station in the diagram...
                for &current_line_index in &previous_indices {
                    let height = visible_stations[current_line_index].1;

                    // Try to find a WIP edge that was on an adjacent station line.
                    // Adjacency is defined as being within 1 index in the station list.
                    // This matching logic relies on the "no A-B-A" constraint to ensure that
                    // a vehicle line doesn't have multiple valid "previous" segments to choose from.
                    let matched_idx = local_edges
                        .iter()
                        .position(|(_, idx)| current_line_index.abs_diff(*idx) <= 1);

                    let mut segment = if let Some(idx) = matched_idx {
                        local_edges.swap_remove(idx).0
                    } else {
                        Vec::new()
                    };

                    let arrival_pos = Pos2::new(
                        ticks_to_screen_x(
                            arrival_ticks,
                            screen_rect,
                            ticks_per_screen_unit,
                            state.tick_offset,
                        ),
                        (height - state.vertical_offset) * state.zoom.y + screen_rect.top(),
                    );

                    let departure_pos = if ce.departure.is_some() {
                        Some(Pos2::new(
                            ticks_to_screen_x(
                                departure_ticks,
                                screen_rect,
                                ticks_per_screen_unit,
                                state.tick_offset,
                            ),
                            (height - state.vertical_offset) * state.zoom.y + screen_rect.top(),
                        ))
                    } else {
                        None
                    };

                    segment.push((arrival_pos, departure_pos, (*ce, *ce_cache), *ce_actual));

                    // Check if the next station in the schedule is adjacent in the diagram.
                    // We only check the immediate neighbors (index -1, 0, +1) of the current station line.
                    //
                    // SAFETY: The diagram layout does not contain "A - B - A" arrangements
                    // where a station B has the same station A as both its predecessor and successor.
                    // This ensures that there is at most one valid adjacent station line to connect to,
                    // allowing us to 'break' after the first match without ambiguity.
                    let mut continued = false;
                    if let Some((ne_data, _)) = ne {
                        let (ne_entry, _) = ne_data;
                        for offset in [-1, 0, 1] {
                            let next_idx = (current_line_index as isize + offset) as usize;
                            if let Some((s, _)) = visible_stations.get(next_idx) {
                                if *s == ne_entry.station {
                                    next_local_edges.push((segment.clone(), next_idx));
                                    continued = true;
                                    break;
                                }
                            }
                        }
                    }

                    // If the path doesn't continue to an adjacent station, flush this segment.
                    if !continued {
                        if segment.len() >= 2 {
                            segments.push(segment);
                        }
                    }
                }

                // Flush any WIP edges that weren't matched to the current station.
                for (segment, _) in local_edges.drain(..) {
                    if segment.len() >= 2 {
                        segments.push(segment);
                    }
                }

                local_edges = next_local_edges;
                if let Some((ne_data, _)) = ne {
                    let (ne_entry, _) = ne_data;
                    previous_indices = visible_stations
                        .iter()
                        .enumerate()
                        .filter_map(|(i, (s, _))| {
                            if *s == ne_entry.station {
                                Some(i)
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }

            // Final flush of remaining WIP edges for this repetition.
            for (segment, _) in local_edges {
                if segment.len() >= 2 {
                    segments.push(segment);
                }
            }
        }

        rendered_vehicles.push(RenderedVehicle {
            segments,
            stroke: state.stroke,
            entity: vehicle_entity,
        });
    }
    rendered_vehicles
}

fn handle_input_selection(
    pointer_pos: Pos2,
    rendered_vehicles: &[RenderedVehicle],
    visible_stations: &[(Entity, f32)],
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
    selected_entity: Entity,
    timetable_adjustment_writer: &mut MessageWriter<AdjustTimetableEntry>,
    station_names: &Query<&Name, With<Station>>,
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
            let (arrival_pos, departure_pos, (_, entry_cache), _) = segment[idx];
            let (next_arrival_pos, _, (next_entry, next_entry_cache), _) = segment[idx + 1];
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

        let mut previous_entry: Option<(
            Pos2,
            Option<Pos2>,
            (&TimetableEntry, &TimetableEntryCache),
            ActualRouteEntry,
        )> = None;
        let show_button = state.zoom.x.min(state.zoom.y) > 0.0002;
        let mut button_strength = ui
            .ctx()
            .animate_bool(ui.id().with("all buttons animation"), show_button);
        if button_strength > 0.0 && !show_button {
            button_strength = 0.0;
        }
        if button_strength <= 0.0 {
            continue;
        }
        for fragment in segment
            .iter()
            .cloned()
            .filter(|(_, _, _, a)| matches!(a, ActualRouteEntry::Nominal(_)))
        {
            let (mut arrival_pos, maybe_departure_pos, (entry, entry_cache), entry_entity) =
                fragment;
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
                ui.put(Rect::from_pos(arrival_pos).expand(5.2), |ui: &mut Ui| {
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
                            .get(entry.station)
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
                            previous_entry.map(|e| {
                                let e = e.2;
                                (e.0, e.1)
                            }),
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
                            .get(entry.station)
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
                            previous_entry.map(|e| {
                                let e = e.2;
                                (e.0, e.1)
                            }),
                            timetable_adjustment_writer,
                            ui,
                            false,
                        );
                    });
                previous_entry = Some(fragment);
            }
        }
    }
}

fn draw_station_selection_overlay(
    ui: &mut Ui,
    strength: f32,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    station_entity: Entity,
    visible_stations: &[(Entity, f32)],
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
    ui: &mut Ui,
    strength: f32,
    painter: &mut Painter,
    screen_rect: Rect,
    vertical_offset: f32,
    zoom_y: f32,
    (s1, s2): (Entity, Entity),
    visible_stations: &[(Entity, f32)],
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

fn handle_navigation(ui: &mut Ui, response: &response::Response, state: &mut DiagramPageCache) {
    let mut zoom_delta: Vec2 = Vec2::default();
    let mut translation_delta: Vec2 = Vec2::default();
    if response.contains_pointer() {
        ui.input(|input| {
            zoom_delta = input.zoom_delta_2d();
            translation_delta = input.translation_delta();
        });
    }
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
        .heights
        .as_ref()
        .unwrap()
        .last()
        .map(|(_, h)| *h)
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

fn draw_station_lines<'a, F>(
    vertical_offset: f32,
    stroke: Stroke,
    painter: &mut Painter,
    screen_rect: &Rect,
    zoom: f32,
    to_draw: &[(Entity, f32)],
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
        let Some(station_name) = get_station_name(entity) else {
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
