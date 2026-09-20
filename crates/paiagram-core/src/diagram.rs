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
