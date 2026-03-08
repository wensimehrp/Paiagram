use bevy::prelude::*;
use itertools::Itertools;

use crate::{
    entry::{DerivedEntryBundle, EntryEstimate, EntryMode, EntryStop, IsDerivedEntry, TravelMode},
    graph::Graph,
    interval::IntervalQuery,
    station::ParentStationOrStation,
    trip::{TripClass, TripNominalSchedule, TripQuery, TripSchedule},
    units::{
        distance::Distance,
        time::{Duration, TimetableTime},
    },
};

pub struct RoutingPlugin;

impl Plugin for RoutingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AddEntryToTrip>().add_systems(
            Update,
            (add_entries, recalculate_route, recalculate_estimate).chain(),
        );
    }
}

#[derive(Message, Clone, Copy)]
pub struct AddEntryToTrip {
    pub trip: Entity,
    pub entry: Entity,
}

pub fn add_entries(
    mut msgs: MessageReader<AddEntryToTrip>,
    mut commands: Commands,
    mut trips: Query<&mut TripNominalSchedule>,
) {
    for AddEntryToTrip { trip, entry } in msgs.read().copied() {
        commands.entity(trip).add_child(entry);
        trips.get_mut(trip).unwrap().push(entry);
    }
}

pub fn recalculate_route(
    changed_schedule: Query<
        (Entity, &TripNominalSchedule, &mut TripSchedule),
        Changed<TripNominalSchedule>,
    >,
    entry_q: Query<(Entity, &EntryStop)>,
    derived_q: Query<Entity, With<IsDerivedEntry>>,
    graph: Res<Graph>,
    parent_station_or_station: Query<ParentStationOrStation>,
    mut commands: Commands,
    interval_q: Query<IntervalQuery>,
) {
    for (trip_entity, nominal_schedule, mut actual_schedule) in changed_schedule {
        for derived_entity in derived_q.iter_many(actual_schedule.iter()) {
            commands.entity(derived_entity).despawn();
        }
        recalculate_inner(
            nominal_schedule,
            trip_entity,
            &mut actual_schedule,
            &graph,
            &mut commands,
            &parent_station_or_station,
            &entry_q,
            &interval_q,
        );
    }
}

fn recalculate_inner(
    route: &[Entity],
    trip_entity: Entity,
    buffer: &mut Vec<Entity>,
    graph: &Graph,
    commands: &mut Commands,
    parent_station_or_station: &Query<ParentStationOrStation>,
    entry_q: &Query<(Entity, &EntryStop)>,
    interval_q: &Query<IntervalQuery>,
) {
    buffer.clear();
    let mut route_iter = entry_q.iter_many(route.iter()).map(|(a, c)| {
        (
            a,
            parent_station_or_station.get(c.entity()).unwrap().parent(),
        )
    });
    let Some((prev_entity, mut prev_stop)) = route_iter.next() else {
        return;
    };
    buffer.push(prev_entity);
    for (curr_entity, stops) in route_iter.map(|(curr_entity, curr_stop)| {
        if prev_stop == curr_stop || graph.contains_edge(prev_stop.entity(), curr_stop.entity()) {
            prev_stop = curr_stop;
            return (curr_entity, Vec::new());
        }
        let Some((_, mut station_list)) =
            graph.route_between(prev_stop.entity(), curr_stop.entity(), interval_q)
        else {
            prev_stop = curr_stop;
            return (curr_entity, Vec::new());
        };
        prev_stop = curr_stop;
        station_list.remove(0);
        station_list.pop();
        return (curr_entity, station_list);
    }) {
        for stop in stops {
            let e = commands.spawn(DerivedEntryBundle::new(stop)).id();
            commands.entity(trip_entity).add_child(e);
            buffer.push(e)
        }
        buffer.push(curr_entity);
    }
}

/// Parameters used for unwinding the flexible stack.
enum UnwindParams {
    At(TimetableTime),
    ForAt(Duration, TimetableTime),
    ForFor(Duration, Duration),
}

/// Recalculate the estimates for updated routes.
/// This should always run after [`recalculate_route`].
fn recalculate_estimate(
    changed_trips: Query<Entity, (Changed<TripSchedule>, With<TripClass>)>,
    changed_entries: Query<&ChildOf, Changed<EntryMode>>,
    trip_q: Query<TripQuery>,
    entry_q: Query<(Entity, &EntryMode, &EntryStop)>,
    parent_station_or_station: Query<ParentStationOrStation>,
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
        'iter_entries: for (entry_entity, mode, stop) in entry_q.iter_many(q.schedule.iter()) {
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
            let initial_t = last_t;
            let total_stop_dur: Duration = flexible_stack.iter().map(|(_, _, d)| *d).sum();
            let total_dur = match params {
                UnwindParams::ForAt(d, _t) => d,
                UnwindParams::ForFor(ad, _dd) => ad,
                UnwindParams::At(t) => t - initial_t,
            };
            // stopping time should not be counted while average velocity
            let travel_dur = total_dur - total_stop_dur;
            let mut distance_stack = Vec::with_capacity(flexible_stack.len());

            for (ps, cs) in std::iter::once(last_s)
                .chain(flexible_stack.iter().map(|(_, s, _)| *s))
                .chain(std::iter::once(stop.entity()))
                .map(|e| parent_station_or_station.get(e).unwrap().parent())
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
            let mut fi = flexible_stack.drain(..);
            let mut di = distance_stack.drain(..);
            let average_v = total_dis / travel_dur;
            while let (Some((e, _, dur)), Some(dis)) = (fi.next(), di.next()) {
                last_t += dis / average_v;
                commands.entity(e).insert(EntryEstimate {
                    arr: last_t,
                    dep: last_t + dur,
                });
                last_t += dur;
            }
            match params {
                UnwindParams::At(_) => {}
                UnwindParams::ForAt(d, t) => {
                    commands.entity(entry_entity).insert(EntryEstimate {
                        arr: initial_t + d,
                        dep: t,
                    });
                }
                UnwindParams::ForFor(ad, dd) => {
                    commands.entity(entry_entity).insert(EntryEstimate {
                        arr: initial_t + ad,
                        dep: initial_t + ad + dd,
                    });
                    next_stable = Some((initial_t + ad + dd, stop.entity()))
                }
            }
        }
        for (e, _, _) in flexible_stack {
            commands.entity(e).remove::<EntryEstimate>();
        }
    }
}
