use bevy::prelude::*;
use egui::Pos2;

use crate::graph::{Station, StationEntries};
use crate::lines::DisplayedLine;
use crate::units::time::TimetableTime;
use crate::vehicles::entries::{TimetableEntry, TimetableEntryCache, VehicleSchedule, VehicleScheduleCache};
use crate::vehicles::vehicle_set::VehicleSet;

use super::{ensure_heights, DiagramLineCache, DiagramLineParams, PointData, RenderedVehicle, TICKS_PER_SECOND, ticks_to_screen_x};

pub fn calculate_lines(
    (In(displayed_line_entity), InMut(mut line_cache), InMut(buffer), In(line_params)): (
        In<Entity>,
        InMut<DiagramLineCache>,
        InMut<Vec<RenderedVehicle>>,
        In<DiagramLineParams>,
    ),
    displayed_lines: Populated<Ref<DisplayedLine>>,
    vehicles_query: Populated<(Entity, &Name, &VehicleSchedule, &VehicleScheduleCache)>,
    entry_parents: Query<&ChildOf, With<TimetableEntry>>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
    station_updated: Query<&StationEntries, Changed<StationEntries>>,
    station_caches: Query<&StationEntries, With<Station>>,
    vehicle_sets: Query<&Children, With<VehicleSet>>,
    mut previous_vehicle_set: Local<Option<Entity>>,
) {
    let Ok(displayed_line) = displayed_lines.get(displayed_line_entity) else {
        line_cache.line_missing = true;
        buffer.clear();
        return;
    };
    line_cache.line_missing = false;

    let entries_updated = displayed_line.is_changed()
        || line_cache.vehicle_entities.is_empty()
        || displayed_line
            .stations()
            .iter()
            .copied()
            .any(|(s, _)| station_updated.get(s.entity()).is_ok())
        || previous_vehicle_set.as_ref() != line_cache.vehicle_set.as_ref();

    if entries_updated {
        info!("Updating vehicle entities for diagram display");

        for station in displayed_line
            .stations()
            .iter()
            .filter_map(|(s, _)| station_caches.get(s.entity()).ok())
        {
            station.passing_vehicles(&mut line_cache.vehicle_entities, |e| {
                entry_parents.get(e).ok()
            });
        }
        line_cache.vehicle_entities.sort();
        // filter out those not in the vehicle set, if any
        if let Some(vehicle_set_entity) = line_cache.vehicle_set {
            if let Ok(vehicle_set) = vehicle_sets.get(vehicle_set_entity) {
                line_cache.vehicle_entities.retain(|e| vehicle_set.contains(e));
            }
        }
        line_cache.vehicle_entities.dedup();
        *previous_vehicle_set = line_cache.vehicle_set;
    }

    if displayed_line.is_changed() || line_cache.heights.is_none() {
        ensure_heights(&mut line_cache, &displayed_line);
    }

    let Some(render_context) = line_cache.last_render_context.clone() else {
        buffer.clear();
        return;
    };

    let visible_stations = match line_cache.heights.as_ref() {
        Some(heights) => {
            let first_visible = heights.iter().position(|(_, h)| *h > render_context.vertical_visible.start);
            let last_visible = heights.iter().rposition(|(_, h)| *h < render_context.vertical_visible.end);
            if let (Some(mut first_visible), Some(mut last_visible)) = (first_visible, last_visible) {
                first_visible = first_visible.saturating_sub(2);
                last_visible = (last_visible + 1).min(heights.len() - 1);
                &heights[first_visible..=last_visible]
            } else {
                &[]
            }
        }
        None => &[],
    };

    let mut rendered_vehicles = Vec::new();

    for (vehicle_entity, _name, schedule, schedule_cache) in line_cache
        .vehicle_entities
        .iter()
        .copied()
        .filter_map(|e| vehicles_query.get(e).ok())
    {
        // Get all repetitions of the schedule that fall within the visible time range.
        let Some(visible_sets) = schedule_cache.get_entries_range(
            schedule,
            TimetableTime((render_context.horizontal_visible.start / TICKS_PER_SECOND) as i32)
                ..TimetableTime((render_context.horizontal_visible.end / TICKS_PER_SECOND) as i32),
            |e| timetable_entries.get(e).ok(),
        ) else {
            continue;
        };

        let mut segments = Vec::new();

        for (initial_offset, set) in visible_sets {
            // local_edges holds WIP segments and the index of the station line they are currently on.
            let mut local_edges: Vec<(Vec<PointData>, usize)> = Vec::new();
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
                            &render_context.screen_rect,
                            render_context.ticks_per_screen_unit,
                            line_params.tick_offset,
                        ),
                        (height - line_params.vertical_offset) * line_params.zoom_y
                            + render_context.screen_rect.top(),
                    );

                    let departure_pos = if ce.departure.is_some() {
                        Some(Pos2::new(
                            ticks_to_screen_x(
                                departure_ticks,
                                &render_context.screen_rect,
                                render_context.ticks_per_screen_unit,
                                line_params.tick_offset,
                            ),
                            (height - line_params.vertical_offset) * line_params.zoom_y
                                + render_context.screen_rect.top(),
                        ))
                    } else {
                        None
                    };

                    segment.push((arrival_pos, departure_pos, *ce_actual));

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
            stroke: line_params.stroke.clone(),
            entity: vehicle_entity,
        });
    }

    buffer.clear();
    buffer.extend(rendered_vehicles);
}
