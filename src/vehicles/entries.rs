use crate::{
    intervals::{Graph, Interval},
    units::time::{Duration, TimetableTime},
    vehicles::AdjustTimetableEntry,
};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use either::Either;
use petgraph::algo::astar;
use smallvec::{SmallVec, smallvec};

/// How the vehicle travels from/to the station.
#[derive(Debug, Clone, Copy)]
pub enum TravelMode {
    /// The vehicle travels to or stops at the station at a determined time.
    At(TimetableTime),
    /// The vehicle travels to or stops at the station relative to the previous running/stopping time.
    For(Duration),
    /// The time is flexible and calculated.
    /// This could be e.g. for flyover stops or less important intermediate stations.
    Flexible,
}

impl TravelMode {
    pub fn adjust_time(&mut self, adjustment: Duration) {
        match self {
            TravelMode::At(t) => {
                t.0 += adjustment.0;
            }
            TravelMode::For(dur) => {
                dur.0 += adjustment.0;
            }
            TravelMode::Flexible => (),
        }
    }
}

impl std::fmt::Display for TravelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::At(t) => write!(f, "{}", t),
            Self::For(dur) => write!(f, "{}", dur),
            Self::Flexible => write!(f, ">>"),
        }
    }
}

/// An entry in the timetable
#[derive(Debug, Component)]
#[require(TimetableEntryCache)]
pub struct TimetableEntry {
    /// How would the vehicle arrive at a station.
    pub arrival: TravelMode,
    /// How would the vehicle depart from a station. A `None` value means that the vehicle does not stop at the station.
    pub departure: Option<TravelMode>,
    /// The node the vehicle stops at or passes.
    pub station: Entity,
    /// The service the entry belongs to.
    pub service: Option<Entity>,
    /// The track/platform/dock/berth etc. at the station.
    pub track: Option<Entity>,
}

impl TimetableEntry {
    fn new_derived(station: Entity, service: Option<Entity>) -> Self {
        Self {
            arrival: TravelMode::Flexible,
            departure: None,
            station,
            service,
            track: None,
        }
    }
}

#[derive(Debug, Component, Default)]
pub struct TimetableEntryCache {
    pub estimate: Option<TimeEstimate>,
}

#[derive(Debug)]
pub struct TimeEstimate {
    /// Estimate of the arrival time. This would be filled in during runtime. An estimate of `None` means that the
    /// arrival time cannot be determined.
    pub arrival: TimetableTime,
    /// Estimate of the departure time. This would be filled in during runtime. An estimate of `None` means that the
    /// departure time cannot be determined.
    pub departure: TimetableTime,
}

/// A vehicle's schedule and departure pattern
#[derive(Debug, Component)]
#[require(VehicleScheduleCache)]
pub struct VehicleSchedule {
    /// When would the schedule start.
    pub start: TimetableTime,
    /// How frequent would the schedule repeat. A value of `None` indicates that the schedule would not repeat.
    pub repeat: Option<Duration>,
    /// When would the vehicle depart. The departure times are relative to the start of the schedule.
    /// This should always be sorted
    /// In this case, this stores the departure time relative to the starting time.
    pub departures: Vec<Duration>,
    /// The timetable entities the schedule holds.
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Copy)]
pub enum ActualRouteEntry {
    Nominal(Entity),
    Derived(Entity),
}

impl ActualRouteEntry {
    pub fn inner(self) -> Entity {
        match self {
            Self::Nominal(e) => e,
            Self::Derived(e) => e,
        }
    }
}

#[derive(Debug, Default, Component)]
pub struct VehicleScheduleCache {
    pub actual_route: Option<Vec<ActualRouteEntry>>,
    /// Service entities indices. This piece of data is calculated during runtime.
    /// This should always be sorted by Entity
    pub service_entities: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)>,
}

// TODO: implement start times
impl VehicleScheduleCache {
    pub fn position<'a>(
        &self,
        time: f32,
        get_info: impl Fn(Entity) -> Option<(&'a TimetableEntry, &'a TimetableEntryCache)>,
    ) -> Option<Either<(Entity, Entity, f32), Entity>> {
        let actual_route = self.actual_route.as_ref()?;
        let i = actual_route.iter().rposition(|e| {
            let Some((_, cache)) = get_info(e.inner()) else {
                return false;
            };
            let Some(entry_time) = cache.estimate.as_ref().map(|est| est.arrival) else {
                return false;
            };
            entry_time.0 <= time as i32
        })?;
        let (this_entry, this_cache) = get_info(actual_route[i].inner())?;
        let this_times = this_cache.estimate.as_ref()?;
        if this_times.arrival.0 <= (time as i32) && (time as i32) <= this_times.departure.0 {
            return Some(Either::Right(this_entry.station));
        }
        let (next_entry, next_cache) = get_info(actual_route.get(i + 1)?.inner())?;
        let next_times = next_cache.estimate.as_ref()?;
        let duration = next_times.arrival - this_times.departure;
        let elapsed = time - this_times.departure.0 as f32;
        if duration.0 <= 0 {
            return Some(Either::Right(next_entry.station));
        }
        let factor = elapsed / (duration.0 as f32);
        Some(Either::Left((
            this_entry.station,
            next_entry.station,
            factor,
        )))
    }
}

pub fn calculate_actual_route(
    mut vehicles: Query<(&mut VehicleScheduleCache, &VehicleSchedule)>,
    mut msg_vehicle_changes: MessageReader<AdjustTimetableEntry>,
    mut commands: Commands,
    timetable_entries: Query<(&ChildOf, &TimetableEntry)>,
    names: Query<&Name>,
    graph: Res<Graph>,
    intervals: Query<&Interval>,
    mut actual_route_list: Local<Vec<ActualRouteEntry>>,
    mut warned_pairs: Local<HashSet<(Entity, Entity)>>,
    mut processed: Local<Vec<Entity>>,
) {
    processed.clear();
    for vehicle_entity in msg_vehicle_changes
        .read()
        .filter_map(|msg| timetable_entries.get(msg.entity).ok().map(|(c, _)| c.0))
    {
        if processed.contains(&vehicle_entity) {
            continue;
        }
        processed.push(vehicle_entity);
        let Ok((mut cache, schedule)) = vehicles.get_mut(vehicle_entity) else {
            continue;
        };
        let mut previous_route = cache.actual_route.take().unwrap_or_default();

        // Derived entries are a cache: on any schedule change, just rebuild them.
        for entry in previous_route.iter().copied() {
            if let ActualRouteEntry::Derived(id) = entry {
                commands.entity(id).despawn();
            }
        }

        let mut prev_entry: Option<&TimetableEntry> = None;

        actual_route_list.clear();
        warned_pairs.clear();
        for entity in schedule
            .entities
            .iter()
            .copied()
            .map(ActualRouteEntry::Nominal)
        {
            let Ok((_, entry)) = timetable_entries.get(entity.inner()) else {
                continue;
            };
            let Some(prev) = prev_entry.replace(entry) else {
                actual_route_list.push(entity);
                continue;
            };
            if prev.station == entry.station {
                actual_route_list.push(entity);
                continue;
            }
            if graph.contains_edge(prev.station, entry.station) {
                actual_route_list.push(entity);
                continue;
            }

            // If either station isn't in the graph at all (e.g. schedule-only stations), routing
            // is expected to fail. Don't spam warnings or run astar in this case.
            let prev_in_graph = graph.contains_node(prev.station);
            let next_in_graph = graph.contains_node(entry.station);
            let service_name = prev
                .service
                .map(|e| names.get(e).ok())
                .flatten()
                .map(Name::as_str)
                .unwrap_or("<unnammed>");
            if !(prev_in_graph && next_in_graph) {
                let pair = (prev.station, entry.station);
                if warned_pairs.insert(pair) {
                    let prev_name = names
                        .get(prev.station)
                        .ok()
                        .map(Name::as_str)
                        .unwrap_or("<unnamed>");
                    let next_name = names
                        .get(entry.station)
                        .ok()
                        .map(Name::as_str)
                        .unwrap_or("<unnamed>");
                    warn!(
                        "Skipping routing for timetable stations of {service_name} not in graph: {prev_name}({:?}) -> {next_name}({:?}); nodes_in_graph=({prev_in_graph},{next_in_graph})",
                        prev.station, entry.station
                    );
                }
                actual_route_list.push(entity);
                continue;
            }
            // compare the stuff between the last and current entries
            let Some((_, path)) = astar(
                &graph.inner(),
                graph.node_index(prev.station).unwrap(),
                |finish| finish == graph.node_index(entry.station).unwrap(),
                |edge| {
                    if let Ok(interval) = intervals.get(*edge.weight()) {
                        interval.length.0
                    } else {
                        i32::MAX
                    }
                },
                |_| 0,
            ) else {
                // This can happen if the graph is disconnected or if station names don't match
                // between timetable and line data. Log a single warning per pair per rebuild.
                let pair = (prev.station, entry.station);
                if warned_pairs.insert(pair) {
                    let prev_name = names
                        .get(prev.station)
                        .ok()
                        .map(Name::as_str)
                        .unwrap_or("<unnamed>");
                    let next_name = names
                        .get(entry.station)
                        .ok()
                        .map(Name::as_str)
                        .unwrap_or("<unnamed>");
                    warn!(
                        "No route in graph between consecutive timetable stations: {prev_name}({:?}) -> {next_name}({:?})",
                        prev.station, entry.station
                    );
                }
                actual_route_list.push(entity);
                continue;
            };
            if path.len() >= 2 {
                for &node_index in &path[1..path.len() - 1] {
                    let station_entity = graph.entity(node_index).unwrap();
                    let station_name = names
                        .get(station_entity)
                        .ok()
                        .map(Name::as_str)
                        .unwrap_or("<unnamed>");
                    info!(?station_name, ?service_name);
                    let derived_entity = commands
                        .spawn(TimetableEntry::new_derived(station_entity, prev.service))
                        .id();
                    commands.entity(vehicle_entity).add_child(derived_entity);
                    actual_route_list.push(ActualRouteEntry::Derived(derived_entity));
                }
            }
            actual_route_list.push(entity);
        }

        previous_route.clear();
        previous_route.extend(actual_route_list.iter());
        cache.actual_route = Some(previous_route);
    }
}

pub fn populate_services(
    mut msg_reader: MessageReader<super::AdjustTimetableEntry>,
    mut schedules: Populated<(&mut VehicleScheduleCache, &VehicleSchedule)>,
    entries: Populated<(&TimetableEntry, &ChildOf)>,
) {
    for msg in msg_reader.read() {
        let super::AdjustTimetableEntry { entity, .. } = msg;
        let Ok((_, parent)) = entries.get(*entity) else {
            continue;
        };
        let Ok((mut schedule_cache, schedule)) = schedules.get_mut(parent.0) else {
            continue;
        };
        let mut pool: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)> = Vec::new();
        let mut start: usize = 0;
        let mut previous_service: Option<Entity> = None;
        for (idx, entry_entity) in schedule.entities.iter().enumerate() {
            let Ok((entry, _)) = entries.get(*entry_entity) else {
                start = idx;
                continue;
            };
            let Some(current_service) = entry.service else {
                start = idx;
                continue;
            };

            if let Some(prev) = previous_service {
                if prev != current_service {
                    match pool.binary_search_by_key(&prev, |(e, _)| *e) {
                        Ok(j) => {
                            pool[j].1.push(start..idx);
                        }
                        Err(j) => {
                            pool.insert(j, (prev, smallvec![start..idx]));
                        }
                    }
                    start = idx;
                    previous_service = Some(current_service);
                }
            } else {
                previous_service = Some(current_service);
                start = idx;
            }
        }
        if let Some(prev) = previous_service {
            let idx = schedule.entities.len();
            match pool.binary_search_by_key(&prev, |(e, _)| *e) {
                Ok(j) => {
                    pool[j].1.push(start..idx);
                }
                Err(j) => {
                    pool.insert(j, (prev, smallvec![start..idx]));
                }
            }
        }
        schedule_cache.service_entities = pool;
    }
}

impl Default for VehicleSchedule {
    fn default() -> Self {
        Self {
            start: TimetableTime(0),
            repeat: Some(Duration(86400)),
            departures: vec![Duration(0)],
            entities: Vec::new(),
        }
    }
}

impl VehicleSchedule {
    pub fn into_entries<'a, F>(
        &self,
        mut lookup: F,
    ) -> impl Iterator<Item = (&'a TimetableEntry, Entity)>
    where
        F: FnMut(Entity) -> Option<&'a TimetableEntry> + 'a,
    {
        self.entities
            .iter()
            .filter_map(move |e| lookup(*e).map(|r| (r, *e)))
    }
}

impl VehicleScheduleCache {
    pub fn get_service_entries(&self, service: Entity) -> Option<Vec<&[ActualRouteEntry]>> {
        let Some(actual_route) = &self.actual_route else {
            return None;
        };
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        let (_, entries) = &self.service_entities[i];
        let mut ret = Vec::with_capacity(entries.len());
        for entry in entries {
            ret.push(&actual_route[entry.clone()]);
        }
        Some(ret)
    }
    pub fn get_service_first_entry(&self, service: Entity) -> Option<ActualRouteEntry> {
        let Some(actual_route) = &self.actual_route else {
            return None;
        };
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        return self.service_entities[i]
            .1
            .first()
            .and_then(|e| Some(actual_route[e.start]));
    }
    pub fn get_service_last_entry(&self, service: Entity) -> Option<ActualRouteEntry> {
        let Some(actual_route) = &self.actual_route else {
            return None;
        };
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        return self.service_entities[i]
            .1
            .last()
            .and_then(|e| Some(actual_route[e.end.saturating_sub(1)]));
    }

    pub fn get_entries_range<'a, F>(
        &self,
        parent: &'a VehicleSchedule,
        range: std::ops::Range<TimetableTime>,
        mut lookup: F,
    ) -> Option<
        Vec<(
            TimetableTime,
            Vec<(
                (&'a TimetableEntry, &'a TimetableEntryCache),
                ActualRouteEntry,
            )>,
        )>,
    >
    where
        F: FnMut(Entity) -> Option<(&'a TimetableEntry, &'a TimetableEntryCache)> + 'a,
    {
        let Some(actual_route) = &self.actual_route else {
            return None;
        };
        let timetable_entries = actual_route
            .iter()
            .filter_map(move |e| lookup(e.inner()).map(|t| (t, *e)))
            .collect::<Vec<_>>();
        let mut ret = Vec::with_capacity(timetable_entries.len());
        let repeats_iter = match parent.repeat {
            None => Either::Left(std::iter::once(parent.start)),
            Some(duration) => {
                let start = parent.start
                    + duration * {
                        let main = (range.start - parent.start).0.div_euclid(duration.0);
                        let last_dep = *parent.departures.last()?
                            + timetable_entries
                                .last()
                                .map(|((_, tec), _)| tec.estimate.as_ref())??
                                .departure
                                .as_duration();
                        let sub = last_dep.0.div_euclid(duration.0);
                        main - sub
                    };
                Either::Right(std::iter::successors(Some(start), move |t| {
                    let time = *t + duration;
                    if time > range.end {
                        return None;
                    }
                    Some(time)
                }))
            }
        };
        for base_time in repeats_iter {
            for departure in parent.departures.iter().copied() {
                let start_index = timetable_entries.iter().position(|((_, tec), _)| {
                    tec.estimate.as_ref().map_or(false, |e| {
                        departure + base_time + e.arrival.as_duration() > range.start
                    })
                });
                let end_index = timetable_entries.iter().rposition(|((_, tec), _)| {
                    tec.estimate.as_ref().map_or(false, |e| {
                        departure + base_time + e.departure.as_duration() < range.end
                    })
                });
                let (Some(mut si), Some(mut ei)) = (start_index, end_index) else {
                    continue;
                };
                si = si.saturating_sub(1);
                ei = (ei + 1).min(timetable_entries.len() - 1);
                // this should not happen
                if si > ei {
                    continue;
                }
                let v = &timetable_entries[si..=ei];
                ret.push((departure + base_time, v.to_vec()))
            }
        }
        Some(ret)
    }
}
