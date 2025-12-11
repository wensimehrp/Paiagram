use crate::{
    intervals::{Graph, Interval},
    units::time::{Duration, TimetableTime},
};
use bevy::prelude::*;
use either::Either;
use smallvec::SmallVec;

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
    /// The station the vehicle stops at or passes.
    pub station: Entity,
    /// The service the entry belongs to.
    pub service: Option<Entity>,
    /// The track/platform/dock/berth etc. at the station.
    pub track: Option<Entity>,
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
    /// Service entities indices. This piece of data is calculated during runtime.
    /// This should always be sorted by Entity
    pub service_entities: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)>,
}

#[derive(Debug)]
pub enum ActualRouteEntry {
    Nominal(Entity),
    Derived(Entity),
}

#[derive(Debug, Default, Component)]
pub struct VehicleScheduleCache {
    actual_route: Vec<ActualRouteEntry>,
    service_entities: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)>,
}

pub fn calculate_actual_route(
    mut vehicles: Query<&mut VehicleScheduleCache, With<VehicleSchedule>>,
    graph: Res<Graph>,
    intervals: Query<&Interval, Changed<Interval>>,
) {
    // cases where the actual route would be recalculated:
    // 1. new route
    // 2. graph connection info updated. This needs to be communicated via messages since
    //    bevy don't store a previous state
    // 3. Interval info changed. In this case just use the old interval cache.
    // the interval cache would be refreshed after this is run
}

impl Default for VehicleSchedule {
    fn default() -> Self {
        Self {
            start: TimetableTime(0),
            repeat: Some(Duration(86400)),
            departures: vec![Duration(0)],
            entities: Vec::new(),
            service_entities: Vec::new(),
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
    pub fn get_service_entries(&self, service: Entity) -> Option<Vec<&[Entity]>> {
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        let (_, entries) = &self.service_entities[i];
        let mut ret = Vec::with_capacity(entries.len());
        for entry in entries {
            ret.push(&self.entities[entry.clone()]);
        }
        Some(ret)
    }
    pub fn get_service_first_entry(&self, service: Entity) -> Option<Entity> {
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        return self.service_entities[i]
            .1
            .first()
            .and_then(|e| Some(self.entities[e.start]));
    }
    pub fn get_service_last_entry(&self, service: Entity) -> Option<Entity> {
        let i = self
            .service_entities
            .binary_search_by_key(&service, |(e, _)| *e);
        let Ok(i) = i else { return None };
        return self.service_entities[i]
            .1
            .last()
            .and_then(|e| Some(self.entities[e.end.saturating_sub(1)]));
    }
    pub fn get_entries_range<'a, F>(
        &self,
        range: std::ops::Range<TimetableTime>,
        mut lookup: F,
    ) -> Option<
        Vec<(
            TimetableTime,
            Vec<((&'a TimetableEntry, &'a TimetableEntryCache), Entity)>,
        )>,
    >
    where
        F: FnMut(Entity) -> Option<(&'a TimetableEntry, &'a TimetableEntryCache)> + 'a,
    {
        let timetable_entries = self
            .entities
            .iter()
            .filter_map(move |e| lookup(*e).map(|t| (t, *e)))
            .collect::<Vec<_>>();
        let mut ret = Vec::with_capacity(timetable_entries.len());
        let repeats_iter = match self.repeat {
            None => Either::Left(std::iter::once(self.start)),
            Some(duration) => {
                let start = self.start
                    + duration * {
                        let main = (range.start - self.start).0.div_euclid(duration.0);
                        let last_dep = *self.departures.last()?
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
            for departure in self.departures.iter().copied() {
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
                let v = &timetable_entries[si..=ei];
                ret.push((departure + base_time, v.to_vec()))
            }
        }
        Some(ret)
    }
}
