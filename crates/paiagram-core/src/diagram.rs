//! Route-space geometry for a Marey chart. Times remain integer timetable seconds;
//! vertical positions are canvas points, independent of geographic coordinates.
use std::collections::HashMap;

use ecow::{EcoString, EcoVec};
use petgraph::algo::bidirectional_dijkstra;
use petgraph::visit::{EdgeFiltered, EdgeRef};

use crate::trip::{TEntry, TEstimate};
use crate::{NodeKey, RouteInfo, RouteKey, StationRecord, StrokeStyle, TripKey, WorldSnapshot};

#[derive(Clone)]
pub struct DiagramRow {
    pub name: EcoString,
    pub nodes: EcoVec<NodeKey>,
    pub position: f64,
    pub milestone: Option<f64>,
}

#[derive(Clone, Copy)]
pub struct DiagramPoint {
    pub position: f64,
    pub entry: TEntry,
    pub time: TEstimate,
}

#[derive(Clone)]
pub struct DiagramTrip {
    pub key: TripKey,
    pub name: EcoString,
    pub style: Option<StrokeStyle>,
    pub parts: EcoVec<EcoVec<DiagramPoint>>,
}

#[derive(Clone, Default)]
pub struct Diagram {
    pub name: EcoString,
    pub rows: EcoVec<DiagramRow>,
    pub trips: EcoVec<DiagramTrip>,
}

#[derive(Clone, Copy)]
struct Location {
    order: f64,
    position: f64,
    direction: i8,
}

impl Diagram {
    pub fn build(world: &WorldSnapshot, route: RouteKey) -> Option<Self> {
        let route = world.routes.query(route, |r| RouteInfo {
            name: r.name.clone(),
            stations: r.stations.clone(),
        })?;
        let mut result = Self {
            name: route.name.clone(),
            ..Self::default()
        };
        let mut locations: HashMap<NodeKey, Vec<Location>> = HashMap::new();
        let progresses = route.gen_progresses(world);
        let mut milestone = Some(0.0);
        for (index, record) in route.stations.iter().enumerate() {
            let (name, nodes) = match &record.stn {
                StationRecord::All(key) => world
                    .stations
                    .query(*key, |s| {
                        (
                            s.name.clone(),
                            s.nodes
                                .iter()
                                .copied()
                                .filter(|n| {
                                    world.nodes.query(*n, |n| *n.is_platform).unwrap_or(false)
                                })
                                .collect::<EcoVec<_>>(),
                        )
                    })
                    .unwrap_or_default(),
                StationRecord::Some(nodes) => (
                    nodes
                        .first()
                        .and_then(|n| world.nodes.query(*n, |n| *n.parent))
                        .and_then(|s| world.stations.query(s, |s| s.name.clone()))
                        .unwrap_or_default(),
                    nodes.clone(),
                ),
            };
            let distance = result.rows.last().and_then(|prev| {
                let length = |from: &[NodeKey], to: &[NodeKey], via: &[NodeKey]| {
                    let graph = EdgeFiltered::from_fn(world, |edge| {
                        let allowed = |n| from.contains(&n) || to.contains(&n) || via.contains(&n);
                        allowed(edge.source()) && allowed(edge.target())
                    });
                    from.iter()
                        .flat_map(|a| {
                            to.iter().filter_map(|b| {
                                bidirectional_dijkstra(&graph, *a, *b, |e| {
                                    world.intervals.get(e.id()).unwrap().length().0.max(0) as u64
                                })
                            })
                        })
                        .min()
                };
                length(&prev.nodes, &nodes, &record.prev_curr_nodes)
                    .or_else(|| length(&nodes, &prev.nodes, &record.curr_prev_nodes))
                    .map(|d| d as f64)
            });
            if index > 0 {
                milestone = milestone.zip(distance).map(|(m, d)| m + d);
            }
            // Explicit milestones are labels only, never inputs to route geometry.
            let label_milestone = record.milestone.map(|d| d.0 as f64).or(milestone);
            let height = record
                .canvas_length
                .map(|v| v.to_postscript_pts())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or_else(|| 40.0 + distance.unwrap_or(1000.0).ln_1p() * 8.0);
            let position = result.rows.last().map_or(0.0, |r| r.position + height);
            for &node in &nodes {
                locations.entry(node).or_default().push(Location {
                    order: index as f64,
                    position,
                    direction: 0,
                });
            }
            if index > 0 {
                let previous = result.rows[index - 1].position;
                for (direction, nodes, progress) in [
                    (1, &record.prev_curr_nodes, &progresses[index - 1].0),
                    (-1, &record.curr_prev_nodes, &progresses[index - 1].1),
                ] {
                    for (&node, progress) in nodes.iter().zip(progress) {
                        if let Some(progress) = progress {
                            let p = if direction == 1 {
                                progress.to_ratio()
                            } else {
                                1.0 - progress.to_ratio()
                            };
                            locations.entry(node).or_default().push(Location {
                                order: index as f64 - 1.0 + p,
                                position: previous + (position - previous) * p,
                                direction,
                            });
                        }
                    }
                }
            }
            result.rows.push(DiagramRow {
                name,
                nodes,
                position,
                milestone: label_milestone,
            });
        }
        for trip in world.trips.iter() {
            let mut parts = EcoVec::new();
            trip.schedule.estimates(&world.intervals, |estimates| {
                let mut run = Vec::new();
                for &(estimate, entry) in estimates {
                    if entry.is_external() {
                        continue;
                    }
                    let mapped = estimate.zip(locations.get(&entry.node_key()));
                    if let Some((time, candidates)) = mapped {
                        if run.last().is_some_and(
                            |(previous, _, _): &(TEntry, TEstimate, &[Location])| {
                                previous.node_key() != entry.node_key()
                                    && !world
                                        .intervals
                                        .contains_key((previous.node_key(), entry.node_key()))
                            },
                        ) {
                            project_run(&run, &mut parts);
                            run.clear();
                        }
                        run.push((entry, time, candidates.as_slice()));
                    } else {
                        project_run(&run, &mut parts);
                        run.clear();
                    }
                }
                project_run(&run, &mut parts);
            });
            if !parts.is_empty() {
                result.trips.push(DiagramTrip {
                    key: trip.key,
                    name: trip.name.clone(),
                    parts,
                    style: trip
                        .service_class
                        .and_then(|k| world.service_classes.query(k, |s| *s.style)),
                });
            }
        }
        Some(result)
    }
}

// Select route occurrences as a sequence, rather than picking the first match.
// This disambiguates the station repeated at the two ends of a loop route.
fn project_run(run: &[(TEntry, TEstimate, &[Location])], parts: &mut EcoVec<EcoVec<DiagramPoint>>) {
    if run.is_empty() {
        return;
    }
    let mut costs: Vec<Vec<(f64, usize)>> = Vec::new();
    let mut start = 0;
    for (i, &(_, _, locations)) in run.iter().enumerate() {
        let mut row: Vec<_> = locations
            .iter()
            .map(|current| {
                if i == start {
                    return (0.0, 0);
                }
                run[i - 1]
                    .2
                    .iter()
                    .enumerate()
                    .filter_map(|(j, previous)| {
                        let delta = current.order - previous.order;
                        let direction = if delta > 0.0 {
                            1
                        } else if delta < 0.0 {
                            -1
                        } else {
                            0
                        };
                        if direction != 0
                            && ((current.direction != 0 && current.direction != direction)
                                || (previous.direction != 0 && previous.direction != direction))
                        {
                            return None;
                        }
                        Some((costs[i - start - 1][j].0 + delta.abs(), j))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .unwrap_or((f64::INFINITY, 0))
            })
            .collect();
        if row.iter().all(|(cost, _)| !cost.is_finite()) {
            append_part(&run[start..i], &costs, parts);
            costs.clear();
            start = i;
            row.fill((0.0, 0));
        }
        costs.push(row);
    }
    append_part(&run[start..], &costs, parts);
}

fn append_part(
    run: &[(TEntry, TEstimate, &[Location])],
    costs: &[Vec<(f64, usize)>],
    parts: &mut EcoVec<EcoVec<DiagramPoint>>,
) {
    let Some(last) = costs.last() else {
        return;
    };
    let Some((mut index, _)) = last.iter().enumerate().min_by(|a, b| a.1.0.total_cmp(&b.1.0))
    else {
        return;
    };
    let mut points = EcoVec::with_capacity(run.len());
    for i in (0..run.len()).rev() {
        let (entry, time, locations) = run[i];
        points.push(DiagramPoint {
            entry,
            time,
            position: locations[index].position,
        });
        index = costs[i][index].1;
    }
    points.make_mut().reverse();
    parts.push(points);
}

/// Resolve the directed paths between a draft's timed stops. No timetable IDs are
/// allocated until the user commits the draft.
pub fn draft_paths(
    world: &WorldSnapshot,
    route: RouteKey,
    draft: &[(NodeKey, i32)],
) -> Option<EcoVec<EcoVec<NodeKey>>> {
    if draft.len() < 2 {
        return None;
    }
    let allowed = world.routes.query(route, |r| {
        let mut nodes = std::collections::HashSet::new();
        for record in r.stations {
            match &record.stn {
                StationRecord::All(key) => {
                    if let Some(platforms) = world.stations.query(*key, |s| s.nodes.clone()) {
                        nodes.extend(platforms.into_iter().filter(|n| {
                            world.nodes.query(*n, |n| *n.is_platform).unwrap_or(false)
                        }));
                    }
                }
                StationRecord::Some(platforms) => nodes.extend(platforms.iter().copied()),
            }
            nodes.extend(record.prev_curr_nodes.iter().copied());
            nodes.extend(record.curr_prev_nodes.iter().copied());
        }
        nodes
    })?;
    let graph = EdgeFiltered::from_fn(world, |edge| {
        allowed.contains(&edge.source()) && allowed.contains(&edge.target())
    });
    draft
        .windows(2)
        .map(|pair| {
            if pair[0].1 > pair[1].1
                || !allowed.contains(&pair[0].0)
                || !allowed.contains(&pair[1].0)
            {
                return None;
            }
            petgraph::algo::astar(
                &graph,
                pair[0].0,
                |n| n == pair[1].0,
                |e| world.intervals.get(e.id()).unwrap().length().0.max(0) as u64,
                |_| 0,
            )
            .map(|(_, path)| path.into_iter().collect())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ecow::eco_vec;

    use super::*;
    use crate::trip::{TEntryId, TravelMode, TripSchedule};
    use crate::*;

    fn fixture() -> (WorldSnapshot, RouteKey, [NodeKey; 3], [StationKey; 2]) {
        let mut w = WorldSnapshot::default();
        let stations = [StationKey::new(), StationKey::new()];
        for (i, &key) in stations.iter().enumerate() {
            w.apply_command(Command::StationAdd {
                key,
                info: StationInfo {
                    name: format!("Station {i}").into(),
                    pos: LonLat::ZERO,
                },
            })
            .unwrap();
        }
        let nodes = [NodeKey::new(), NodeKey::new(), NodeKey::new()];
        for (i, &key) in nodes.iter().enumerate() {
            w.apply_command(Command::NodeAdd {
                key,
                info: NodeInfo {
                    name: format!("Node {i}").into(),
                    pos: LonLat::ZERO,
                    parent: stations[usize::from(i == 2)],
                    is_platform: i != 1,
                },
            })
            .unwrap();
        }
        for (a, b, length) in [(0, 1, 100), (1, 2, 300), (2, 1, 300), (1, 0, 100)] {
            w.apply_command(Command::IntervalAdd {
                key: (nodes[a], nodes[b]),
                info: Interval {
                    nodes: eco_vec![LonLat::ZERO, LonLat::ZERO, LonLat::ZERO],
                    length: NonZeroU32::new(length),
                    trips: EcoVec::new(),
                },
            })
            .unwrap();
        }
        let key = RouteKey::new();
        let mut second = RouteStationRecord::for_station(&w, stations[1], Some(stations[0]));
        second.canvas_length = Some(CanvasLength::from_postscript_pts(100.0));
        w.apply_command(Command::RouteAdd {
            key,
            info: RouteInfo {
                name: "Line".into(),
                stations: eco_vec![
                    RouteStationRecord::for_station(&w, stations[0], None),
                    second,
                ],
            },
        })
        .unwrap();
        (w, key, nodes, stations)
    }

    fn add_trip(w: &mut WorldSnapshot, entries: EcoVec<TEntry>) -> TripKey {
        let key = TripKey::new();
        w.apply_command(Command::TripAdd {
            key,
            info: TripInfo {
                name: "Train".into(),
                schedule: TripSchedule::new(entries),
                service_class: None,
                vehicles: Default::default(),
            },
        })
        .unwrap();
        key
    }
    fn pass(node: NodeKey, time: i32) -> TEntry {
        TEntry::PinnedNonStop {
            node,
            id: TEntryId::new(),
            pass: TravelMode::At(time::TimetableTime(time)),
            external: false,
        }
    }

    #[test]
    fn projects_switches_in_both_directions_without_changing_integer_times() {
        let (mut w, route, n, _) = fixture();
        add_trip(
            &mut w,
            eco_vec![pass(n[0], 86300), pass(n[1], 86400), pass(n[2], 86700)],
        );
        add_trip(
            &mut w,
            eco_vec![pass(n[2], 100), pass(n[1], 400), pass(n[0], 500)],
        );
        let d = Diagram::build(&w, route).unwrap();
        assert_eq!(d.rows.len(), 2);
        assert_eq!(d.rows[1].milestone, Some(400.0));
        for t in &d.trips {
            assert_eq!(t.parts.len(), 1);
            assert_eq!(t.parts[0].len(), 3);
            assert!((t.parts[0][1].position - 25.0).abs() < 0.01);
        }
        assert_eq!(d.trips[0].parts[0][1].time.arr.0, 86400);
        // Long retained polylines share their backing buffer on clone.
        let clone = d.clone();
        assert_eq!(clone.trips.as_ptr(), d.trips.as_ptr());
    }

    #[test]
    fn excluded_platforms_and_disconnections_break_paths() {
        let (mut w, route, n, _) = fixture();
        let mut external = pass(n[1], 100);
        if let TEntry::PinnedNonStop { external: flag, .. } = &mut external {
            *flag = true;
        }
        add_trip(&mut w, eco_vec![pass(n[0], 0), external, pass(n[2], 200)]);
        let d = Diagram::build(&w, route).unwrap();
        assert_eq!(d.trips[0].parts.len(), 2);
        assert!(d.trips[0].parts.iter().all(|p| p.len() == 1));
        add_trip(&mut w, eco_vec![pass(n[0], 0), pass(n[2], 200)]);
        let d = Diagram::build(&w, route).unwrap();
        assert_eq!(d.trips[1].parts.len(), 2);
        let mut records = w.routes.query(route, |r| r.stations.clone()).unwrap();
        records.make_mut()[0].stn = StationRecord::Some(eco_vec![]);
        // A valid subset is tested using a second platform, since empty records are rejected.
        let other = NodeKey::new();
        let station = w.nodes.query(n[0], |n| *n.parent).unwrap();
        w.apply_command(Command::NodeAdd {
            key: other,
            info: NodeInfo {
                name: "Other".into(),
                pos: LonLat::ZERO,
                parent: station,
                is_platform: true,
            },
        })
        .unwrap();
        records.make_mut()[0].stn = StationRecord::Some(eco_vec![other]);
        w.apply_command(Command::RouteStationsChange {
            key: route,
            stations: records,
        })
        .unwrap();
        assert!(
            Diagram::build(&w, route)
                .unwrap()
                .trips
                .iter()
                .flat_map(|t| t.parts.iter())
                .flatten()
                .all(|p| p.entry.node_key() != n[0])
        );
    }

    #[test]
    fn loop_occurrences_follow_the_whole_trip() {
        let (mut w, route, n, stations) = fixture();
        let mut records = w.routes.query(route, |r| r.stations.clone()).unwrap();
        let mut last = RouteStationRecord::for_station(&w, stations[0], Some(stations[1]));
        last.canvas_length = Some(CanvasLength::from_postscript_pts(100.0));
        records.push(last);
        w.apply_command(Command::RouteStationsChange {
            key: route,
            stations: records,
        })
        .unwrap();
        add_trip(
            &mut w,
            eco_vec![pass(n[2], 0), pass(n[1], 300), pass(n[0], 400)],
        );
        let d = Diagram::build(&w, route).unwrap();
        // Both equivalent directions are valid on a two-station out-and-back loop;
        // the final point must use the same branch as the intermediate switch.
        let part = &d.trips[0].parts[0];
        assert_eq!(part.len(), 3);
        assert!(
            (part[1].position - part[0].position) * (part[2].position - part[1].position) >= 0.0
        );
        let (a, b, c) = (
            Location {
                order: 0.0,
                position: 0.0,
                direction: 0,
            },
            Location {
                order: 2.0,
                position: 200.0,
                direction: 0,
            },
            Location {
                order: 3.0,
                position: 300.0,
                direction: 0,
            },
        );
        let mut parts = EcoVec::new();
        let estimate = TEstimate {
            arr: time::TimetableTime(0),
            dep: time::TimetableTime(0),
        };
        project_run(
            &[
                (pass(n[2], 0), estimate, &[b]),
                (pass(n[0], 1), estimate, &[a, c]),
            ],
            &mut parts,
        );
        assert_eq!(parts[0][1].position, 300.0);
    }

    #[test]
    fn external_annotations_do_not_interrupt_the_internal_trip() {
        let (mut w, route, n, _) = fixture();
        let mut external = pass(n[2], 9999);
        if let TEntry::PinnedNonStop { external: flag, .. } = &mut external {
            *flag = true;
        }
        add_trip(
            &mut w,
            eco_vec![pass(n[0], 0), external, pass(n[1], 100), pass(n[2], 400)],
        );
        let d = Diagram::build(&w, route).unwrap();
        assert_eq!(d.trips[0].parts.len(), 1);
        assert_eq!(d.trips[0].parts[0].len(), 3);
    }

    #[test]
    fn milestones_are_labels_and_canvas_lengths_control_geometry() {
        let (mut w, route, n, _) = fixture();
        add_trip(
            &mut w,
            eco_vec![pass(n[0], 0), pass(n[1], 100), pass(n[2], 400)],
        );
        let mut records = w.routes.query(route, |r| r.stations.clone()).unwrap();
        records.make_mut()[1].milestone = Some(Distance(10000));
        w.apply_command(Command::RouteStationsChange {
            key: route,
            stations: records.clone(),
        })
        .unwrap();
        let d = Diagram::build(&w, route).unwrap();
        assert_eq!(d.rows[1].milestone, Some(10000.0));
        assert!((d.rows[1].position - 100.0).abs() < 0.001);
        records.make_mut()[1].canvas_length = Some(CanvasLength::from_postscript_pts(200.0));
        w.apply_command(Command::RouteStationsChange {
            key: route,
            stations: records,
        })
        .unwrap();
        let d = Diagram::build(&w, route).unwrap();
        assert!((d.trips[0].parts[0][1].position - 50.0).abs() < 0.01);
    }

    #[test]
    fn draft_paths_keep_intermediate_nodes_and_validate_order() {
        let (w, route, n, _) = fixture();
        assert_eq!(
            draft_paths(&w, route, &[(n[0], 0), (n[2], 400)]).unwrap()[0].as_slice(),
            &n
        );
        assert_eq!(
            draft_paths(&w, route, &[(n[2], 0), (n[0], 400)]).unwrap()[0].as_slice(),
            &[n[2], n[1], n[0]]
        );
        assert!(draft_paths(&w, route, &[(n[0], 400), (n[2], 0)]).is_none());
        assert!(draft_paths(&w, route, &[(NodeKey::new(), 0), (n[2], 400)]).is_none());
    }

    #[test]
    fn revision_invalidates_diagrams_for_commands_undo_and_redo() {
        let (w, route, _, _) = fixture();
        let mut source = Source::try_from(SaveFile::from(w)).unwrap();
        let before = source.revision();
        assert!(!source.undo());
        assert_eq!(source.revision(), before);
        assert!(source.apply_command(Command::RouteRename {
            key: route,
            name: "Renamed".into()
        }));
        assert_ne!(source.revision(), before);
        assert_eq!(
            Diagram::build(source.snap(), route).unwrap().name,
            "Renamed"
        );
        let changed = source.revision();
        assert!(source.undo());
        assert_ne!(source.revision(), changed);
        assert_eq!(Diagram::build(source.snap(), route).unwrap().name, "Line");
        let undone = source.revision();
        assert!(source.redo());
        assert_ne!(source.revision(), undone);
        assert_eq!(
            Diagram::build(source.snap(), route).unwrap().name,
            "Renamed"
        );
        assert!(!source.apply_command(Command::RouteRename {
            key: RouteKey::new(),
            name: "Missing".into()
        }));
        assert_eq!(source.revision(), undone + 1);
    }
}
