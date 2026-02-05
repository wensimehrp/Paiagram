use bevy::prelude::*;
use itertools::Itertools;

use crate::{
    entry::{EntryBundle, EntryEstimate, EntryMode, EntryQuery, EntryStop, IsDerivedEntry, TravelMode},
    graph::Graph,
    interval::IntervalQuery,
    trip::{Trip, TripClass, TripQuery, TripSchedule},
    units::{
        distance::Distance,
        time::{Duration, TimetableTime},
    },
};

pub struct RoutingPlugin;

impl Plugin for RoutingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (recalculate_route_on_new_trip, recalculate_estimate).chain(),
        );
    }
}

/// Recalculate the route when a new trip is created.
pub fn recalculate_route_on_new_trip(
    new_trips: Query<Entity, (Added<Trip>, With<TripClass>)>,
    graph: Res<Graph>,
    entry_q: Query<EntryQuery>,
    interval_q: Query<IntervalQuery>,
    trip_q: Query<&TripSchedule, With<TripClass>>,
    mut commands: Commands,
) {
    for trip_entity in new_trips.iter() {
        let schedule = trip_q
            .get(trip_entity)
            .expect("New trip entity must have a schedule");
        let original = schedule.iter().collect::<Vec<_>>();
        let mut inserted = 0usize;
        for (idx, (source_entry, target_entry)) in original.iter().tuple_windows().enumerate() {
            let source = entry_q
                .get(*source_entry)
                .expect("Trip schedule entry must exist");
            let target = entry_q
                .get(*target_entry)
                .expect("Trip schedule entry must exist");
            if source.is_derived() || target.is_derived() {
                continue;
            }
            let added = insert(
                source.stop(),
                target.stop(),
                &graph,
                trip_entity,
                idx + 1 + inserted,
                &[],
                &mut commands,
                &interval_q,
            );
            inserted += added;
        }
    }
}

fn insert(
    source: Entity,
    target: Entity,
    graph: &Graph,
    schedule: Entity,
    insertion_index: usize,
    to_despawn: &[Entity],
    commands: &mut Commands,
    interval_q: &Query<IntervalQuery>,
) -> usize {
    let Some((_, mut route)) = graph.route_between(source, target, interval_q) else {
        return 0;
    };
    if route.first().copied() == Some(source) {
        route.remove(0);
    }
    if route.last().copied() == Some(target) {
        route.pop();
    }
    let mut collected = Vec::with_capacity(route.len());
    for n in route {
        let e = commands
            .spawn((
                EntryBundle {
                    time: EntryMode::new_derived(),
                    stop: EntryStop(n),
                },
                IsDerivedEntry,
            ))
            .id();
        collected.push(e)
    }
    if !collected.is_empty() {
        commands
            .entity(schedule)
            .insert_children(insertion_index, &collected);
    }
    for e in to_despawn.iter().copied() {
        commands.entity(e).despawn();
    }
    collected.len()
}


/// Parameters used for unwinding the flexible stack.
enum UnwindParams {
    At(TimetableTime),
    ForAt(Duration, TimetableTime),
    ForFor(Duration, Duration),
}

/// Recalculate the estimates for updated routes.
/// This should always run after [`recalculate_route_on_new_trip`].
fn recalculate_estimate(
    changed_trips: Query<Entity, (Changed<TripSchedule>, With<TripClass>)>,
    changed_entries: Query<&ChildOf, Changed<EntryMode>>,
    trip_q: Query<TripQuery>,
    entry_q: Query<(Entity, &EntryMode, &EntryStop)>,
    interval_q: Query<IntervalQuery>,
    mut commands: Commands,
    graph: Res<Graph>,
) {
    let mut to_recalculate = changed_entries
        .iter()
        .map(|c| c.parent())
        .chain(changed_trips.iter())
        .collect::<Vec<_>>();
    to_recalculate.sort_unstable();
    to_recalculate.dedup();
    for q in trip_q.iter_many(to_recalculate) {
        let mut flexible_stack: Vec<(Entity, Entity, Duration)> = Vec::new();
        let mut last_stable: Option<(TimetableTime, Entity)> = None;
        let mut next_stable: Option<(TimetableTime, Entity)> = None;
        let mut unwind_params: Option<UnwindParams> = None;
        'iter_entries: for (entry_entity, mode, stop) in entry_q.iter_many(q.schedule) {
            if let Some(v) = next_stable.take() {
                last_stable = Some(v);
            }
            match (mode.arr, mode.dep.unwrap_or(TravelMode::Flexible)) {
                (TravelMode::At(at), TravelMode::At(dt)) => {
                    commands
                        .entity(entry_entity)
                        .insert(EntryEstimate::new(at, dt));
                    next_stable = Some((dt, stop.entity()));
                    unwind_params = Some(UnwindParams::At(at));
                }
                (TravelMode::At(at), TravelMode::For(dd)) => {
                    commands
                        .entity(entry_entity)
                        .insert(EntryEstimate::new(at, at + dd));
                    next_stable = Some((at + dd, stop.entity()));
                    unwind_params = Some(UnwindParams::At(at));
                }
                (TravelMode::At(at), TravelMode::Flexible) => {
                    commands
                        .entity(entry_entity)
                        .insert(EntryEstimate::new(at, at));
                    next_stable = Some((at, stop.entity()));
                    unwind_params = Some(UnwindParams::At(at));
                }
                (TravelMode::For(ad), TravelMode::At(dt)) => {
                    // estimates are inserted afterwards
                    next_stable = Some((dt, stop.entity()));
                    unwind_params = Some(UnwindParams::ForAt(ad, dt));
                }
                (TravelMode::For(ad), TravelMode::For(dd)) => {
                    // estimates are inserted afterwards
                    unwind_params = Some(UnwindParams::ForFor(ad, dd));
                }
                (TravelMode::For(ad), TravelMode::Flexible) => {
                    // estimates are inserted afterwards
                    unwind_params = Some(UnwindParams::ForFor(ad, Duration::ZERO));
                }
                (TravelMode::Flexible, TravelMode::At(dt)) => {
                    commands
                        .entity(entry_entity)
                        .insert(EntryEstimate::new(dt, dt));
                    next_stable = Some((dt, stop.entity()));
                    unwind_params = Some(UnwindParams::At(dt))
                }
                (TravelMode::Flexible, TravelMode::For(dd)) => {
                    flexible_stack.push((entry_entity, stop.entity(), dd))
                }
                (TravelMode::Flexible, TravelMode::Flexible) => {
                    flexible_stack.push((entry_entity, stop.entity(), Duration::ZERO))
                }
            }
            let Some(params) = unwind_params.take() else {
                continue;
            };
            let Some((mut last_t, last_s)) = last_stable else {
                for (e, _, _) in flexible_stack.drain(..) {
                    commands.entity(e).remove::<EntryEstimate>();
                }
                continue;
            };
            // stopping time should not be counted while average velocity
            let total_dur = match params {
                UnwindParams::ForAt(d, _t) => d,
                UnwindParams::ForFor(ad, _dd) => ad,
                UnwindParams::At(t) => {
                    t - last_t - flexible_stack.iter().map(|(_, _, d)| d).copied().sum()
                }
            };
            // if flexible_stack.is_empty() {
            //     continue;
            // }
            let mut distance_stack = Vec::with_capacity(flexible_stack.len());

            for (ps, cs) in std::iter::once(last_s)
                .chain(flexible_stack.iter().map(|(_, s, _)| *s))
                .chain(std::iter::once(stop.entity()))
                .tuple_windows()
            {
                let Some(weight) = graph
                    .edge_weight(ps, cs)
                    .copied()
                    .map(|w| interval_q.get(w).ok())
                    .flatten()
                else {
                    for (e, _, _) in flexible_stack.drain(..) {
                        commands.entity(e).remove::<EntryEstimate>();
                    }
                    continue 'iter_entries;
                };
                distance_stack.push(weight.distance())
            }
            debug_assert_eq!(distance_stack.len(), flexible_stack.len() + 1);
            let total_dis = distance_stack.iter().cloned().sum::<Distance>();
            // if total_dur.0 <= 0 || total_dis.0 <= 0 {
            //     for (e, _, _) in flexible_stack.drain(..) {
            //         commands.entity(e).remove::<EntryEstimate>();
            //     }
            //     continue;
            // }
            let mut fi = flexible_stack.drain(..);
            let mut di = distance_stack.drain(..);
            match params {
                UnwindParams::At(_) => {
                    let average_v = total_dis / total_dur;
                    while let (Some((e, _, dur)), Some(dis)) = (fi.next(), di.next()) {
                        last_t += dis / average_v;
                        commands.entity(e).insert(EntryEstimate {
                            arr: last_t,
                            dep: last_t + dur,
                        });
                    }
                }
                UnwindParams::ForAt(_, _) | UnwindParams::ForFor(_, _) => {
                    while let (Some((e, _, dur)), Some(dis)) = (fi.next(), di.next()) {
                        let frac = dis.0 as f32 / total_dis.0 as f32;
                        last_t += Duration((frac * total_dis.0 as f32) as i32);
                        commands.entity(e).insert(EntryEstimate {
                            arr: last_t,
                            dep: last_t + dur,
                        });
                    }
                }
            }
            match params {
                UnwindParams::At(_) => {}
                UnwindParams::ForAt(_, t) => {
                    commands.entity(entry_entity).insert(EntryEstimate {
                        arr: last_t,
                        dep: t,
                    });
                }
                UnwindParams::ForFor(_, d) => {
                    commands.entity(entry_entity).insert(EntryEstimate {
                        arr: last_t,
                        dep: last_t + d,
                    });
                    next_stable = Some((last_t + d, stop.entity()))
                }
            }
        }
        for (e, _, _) in flexible_stack {
            commands.entity(e).remove::<EntryEstimate>();
        }
    }
}
