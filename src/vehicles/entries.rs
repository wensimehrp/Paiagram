use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;
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
pub struct TimetableEntry {
    /// How would the vehicle arrive at a station.
    pub arrival: TravelMode,
    /// How would the vehicle depart from a station. A `None` value means that the vehicle does not stop at the station.
    pub departure: Option<TravelMode>,
    /// Estimate of the arrival time. This would be filled in during runtime. An estimate of `None` means that the
    /// arrival time cannot be determined.
    pub arrival_estimate: Option<TimetableTime>,
    /// Estimate of the departure time. This would be filled in during runtime. An estimate of `None` means that the
    /// arrival time cannot be determined.
    pub departure_estimate: Option<TimetableTime>,
    /// The station the vehicle stops at or passes.
    pub station: Entity,
    /// The service the entry belongs to.
    pub service: Option<Entity>,
    /// The track/platform/dock/berth etc. at the station.
    pub track: Option<Entity>,
}

/// A vehicle's schedule and departure pattern
#[derive(Debug, Component)]
pub struct VehicleSchedule {
    /// When would the schedule start.
    pub start: TimetableTime,
    /// How frequent would the schedule repeat. A value of `None` indicates that the schedule would not repeat.
    pub repeat: Option<Duration>,
    /// When would the vehicle depart. The departure times are relative to the start of the schedule.
    /// This should always be sorted
    pub departures: Vec<Duration>,
    /// The timetable entities the schedule holds.
    pub entities: Vec<Entity>,
    /// Service entities indices. This piece of data is calculated during runtime.
    /// This should always be sorted by Entity
    pub service_entities: Vec<(Entity, SmallVec<[std::ops::Range<usize>; 1]>)>,
}

impl Default for VehicleSchedule {
    fn default() -> Self {
        Self {
            start: TimetableTime(0),
            repeat: None,
            departures: vec![Duration(0)],
            entities: Vec::new(),
            service_entities: Vec::new()
        }
    }
}

impl VehicleSchedule {
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
    pub fn get_entries_range<'a>(
        &self,
        range: std::ops::Range<TimetableTime>,
        query: &'a Query<&TimetableEntry>,
    ) -> Option<Vec<(TimetableTime, Vec<(&'a TimetableEntry, Entity)>)>> {
        let timetable_entries = self
            .entities
            .iter()
            .filter_map(|e| query.get(*e).ok().map(|t| (t, *e)))
            .collect::<Vec<_>>();
        let schedule_start = timetable_entries
            .iter()
            .find_map(|(et, _)| et.arrival_estimate)?;
        let schedule_end = timetable_entries
            .iter()
            .rev()
            .find_map(|(et, _)| et.departure_estimate)?;
        let mut ret = Vec::new();
        let Some(repeat) = self.repeat else {
            // does not repeat
            for departure in self.departures.iter().copied() {
                let first_index = timetable_entries.iter().position(|(et, _)| {
                    let Some(ae) = et.arrival_estimate else {
                        return false;
                    };
                    let real_time = self.start + departure + (ae - TimetableTime(0));
                    real_time > range.start
                });
                let last_index = timetable_entries.iter().rposition(|(et, _)| {
                    let Some(de) = et.departure_estimate else {
                        return false;
                    };
                    let real_time = self.start + departure + (de - TimetableTime(0));
                    real_time < range.end
                });
                let (Some(mut first_index), Some(mut last_index)) = (first_index, last_index)
                else {
                    continue;
                };
                first_index = first_index.saturating_sub(1);
                last_index = (last_index + 1).min(timetable_entries.len() - 1);
                let v = &timetable_entries[first_index..=last_index];
                ret.push((self.start + departure, v.to_vec()))
            }
            return Some(ret);
        };
        return None;
    }
}
