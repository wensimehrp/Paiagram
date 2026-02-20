use bevy::prelude::*;
use itertools::Itertools;

use crate::{
    entry::{
        EntryBundle, EntryEstimate, EntryMode, EntryQuery, EntryStop, IsDerivedEntry, TravelMode,
    },
    graph::Graph,
    interval::IntervalQuery,
    trip::{Trip, TripClass, TripQuery, TripSchedule},
    units::{
        distance::Distance,
        time::{Duration, TimetableTime},
    },
};

pub struct RoutingPlugin;

#[derive(Default, Resource)]
struct RecalculateCandidates(Vec<Entity>);

impl Plugin for RoutingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AddEntryToTrip>()
            .init_resource::<RecalculateCandidates>()
            .add_observer(remove_entries)
            .add_systems(PreUpdate, edit_entries)
            .add_systems(Update, clear_route)
            .add_systems(PostUpdate, recalculate_route)
            .add_systems(Last, recalculate_estimate);
    }
}

#[derive(Message, Clone, Copy)]
pub struct AddEntryToTrip {
    pub trip: Entity,
    pub entry: Entity,
}

fn remove_entries(
    trigger: On<Remove, EntryMode>,
    filter: Query<&ChildOf, Without<IsDerivedEntry>>,
    mut recalculate_candidates: ResMut<RecalculateCandidates>,
) {
    if let Ok(p) = filter.get(trigger.entity) {
        recalculate_candidates.0.push(p.parent())
    }
}

fn edit_entries(
    mut added_entries: MessageReader<AddEntryToTrip>,
    mut commands: Commands,
    mut candidates: ResMut<RecalculateCandidates>,
) {
    for AddEntryToTrip { trip, entry } in added_entries.read().copied() {
        commands.entity(trip).add_child(entry);
        candidates.0.push(trip);
    }
}

fn clear_route(
    added_trips: Query<Entity, (Added<TripSchedule>, With<Trip>)>,
    changed_entries: Query<&ChildOf, (Changed<EntryStop>, Without<IsDerivedEntry>)>,
    trips_q: Query<&TripSchedule, With<Trip>>,
    derived_q: Query<Entity, With<IsDerivedEntry>>,
    mut commands: Commands,
    mut recalculate_candidates: ResMut<RecalculateCandidates>,
) {
    let recalculate_candidate = added_trips
        .iter()
        .chain(changed_entries.iter().map(|it| it.parent()));
    recalculate_candidates.0.extend(recalculate_candidate);
    recalculate_candidates.0.sort_unstable();
    recalculate_candidates.0.dedup();
    for trip_entity in recalculate_candidates.0.iter().copied() {
        let schedule = trips_q.get(trip_entity).unwrap();
        for e in derived_q.iter_many(schedule) {
            commands.entity(e).despawn();
        }
    }
}

fn recalculate_route(
    trips_q: Query<&TripSchedule, With<Trip>>,
    entry_q: Query<EntryQuery>,
    interval_q: Query<IntervalQuery>,
    graph: Res<Graph>,
    mut commands: Commands,
    mut recalculate_candidates: ResMut<RecalculateCandidates>,
) {
    for trip_entity in recalculate_candidates.0.drain(..) {
        let schedule = trips_q.get(trip_entity).unwrap();
        let original = schedule.iter().collect::<Vec<_>>();
        let mut inserted = 0usize;
        for (idx, (source_entry, target_entry)) in original.iter().tuple_windows().enumerate() {
            let source = entry_q.get(*source_entry).unwrap();
            let target = entry_q.get(*target_entry).unwrap();
            debug_assert!(!(source.is_derived() || target.is_derived()));
            let Some((_, mut route)) =
                graph.route_between(source.stop(), target.stop(), &interval_q)
            else {
                continue;
            };
            if route.first().copied() == Some(source.stop()) {
                route.remove(0);
            }
            if route.last().copied() == Some(target.stop()) {
                route.pop();
            }
            if route.is_empty() {
                continue;
            }
            let mut collected = Vec::with_capacity(route.len());
            for node in route {
                let entry = commands
                    .spawn((EntryBundle::new_derived(node), IsDerivedEntry))
                    .id();
                collected.push(entry);
            }
            commands
                .entity(trip_entity)
                .insert_children(idx + 1 + inserted, &collected);
            inserted += collected.len();
        }
    }
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
            match (mode.arr.unwrap_or(TravelMode::Flexible), mode.dep) {
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
