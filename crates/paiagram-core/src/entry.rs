//! Entries define a [`crate::trip`]'s location at a given time.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

use crate::trip::TripQueryItem;
use crate::units::time::{Duration, TimetableTime};

/// The entry plugin
pub struct EntryPlugin;
impl Plugin for EntryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_entry_mode)
            .add_observer(update_entry_stop);
    }
}

/// Marker component given to derived entries
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct IsDerivedEntry;

/// Travel mode for entries' arrival and departure fields.
#[derive(Reflect, Default, Debug, Clone, Copy)]
pub enum TravelMode {
    /// The event is guaranteed to happen at this point.
    At(TimetableTime),
    /// The event is guaranteed to happen [`Duration`] after the previous event.
    For(Duration),
    /// The event does not have a fixed timepoint
    #[default]
    Flexible,
}

/// The entry's arrival and departure times.
#[derive(Default, Reflect, Component, Clone, Copy)]
#[reflect(Component)]
pub struct EntryMode {
    /// Arrival. Arrival is defined as [`Option<TravelMode>`]. The [`None`] value covers the case
    /// where the trip does not stop at this station.
    pub arr: Option<TravelMode>,
    /// Departure. Every entry must have a departure mode.
    pub dep: TravelMode,
}

impl EntryMode {
    /// Generate a derived entry that doesn't stop at the station and has flexible departure mode.
    /// This must be used with [`IsDerivedEntry`].
    pub fn new_derived() -> Self {
        Self {
            arr: None,
            dep: TravelMode::Flexible,
        }
    }
    /// Shift the arrival mode. The function does nothing if the arrival mode is
    /// [`TravelMode::Flexible`] or [`Option::None`]
    pub fn shift_arr(&mut self, d: Duration) {
        match &mut self.arr {
            Some(TravelMode::At(t)) => *t += d,
            Some(TravelMode::For(t)) => *t += d,
            Some(TravelMode::Flexible) | None => (),
        }
    }
    /// Shift the departure mode. The function does nothing if the departure
    /// mode is [`TravelMode::Flexible`]
    pub fn shift_dep(&mut self, d: Duration) {
        match &mut self.dep {
            TravelMode::At(t) => *t += d,
            TravelMode::For(t) => *t += d,
            TravelMode::Flexible => (),
        }
    }
}

/// Where the vehicle stops. The stop could be a station, or a platform that
/// belongs to the station.
#[derive(Reflect, Component, MapEntities, Deref, DerefMut)]
#[reflect(Component, MapEntities)]
#[relationship(relationship_target = crate::station::PlatformEntries)]
#[require(EntryMode)]
pub struct EntryStop(
    #[relationship]
    #[entities]
    pub Entity,
);

/// The estimated arrival and departure times of the entry. This is not a hard
/// requirement for entries.
#[derive(Reflect, Component, Clone, Copy)]
#[reflect(Component)]
pub struct EntryEstimate {
    /// The arrival time. This must be a determined time
    pub arr: TimetableTime,
    /// The departure time. This must be a determined time
    pub dep: TimetableTime,
}

impl EntryEstimate {
    /// Creates a new [`EntryEstimate`] given the arrival and departure times
    pub fn new(arr: TimetableTime, dep: TimetableTime) -> Self {
        Self { arr, dep }
    }
}

/// Bundle for spawning entries easily.
#[derive(Bundle)]
pub struct EntryBundle {
    /// The time component.
    time: EntryMode,
    /// The stop component.
    stop: EntryStop,
}

impl EntryBundle {
    /// Create a new [`EntryBundle`].
    pub fn new(arr: Option<TravelMode>, dep: TravelMode, stop: Entity) -> Self {
        Self {
            time: EntryMode { arr, dep },
            stop: EntryStop(stop),
        }
    }
}

/// Bundle for easy spawning
#[derive(Bundle)]
pub struct DerivedEntryBundle {
    /// The time component
    mode: EntryMode,
    /// The stop component
    stop: EntryStop,
    /// Marker for derived entry
    derived: IsDerivedEntry,
}

impl DerivedEntryBundle {
    /// Create a new [`DerivedEntryBundle`].
    pub fn new(stop: Entity) -> Self {
        Self {
            mode: EntryMode::new_derived(),
            stop: EntryStop(stop),
            derived: IsDerivedEntry,
        }
    }
}

// TODO: rewrite this in functional style? And only pick the required components?
// I don't actually know if Rust would optimize it so that unused components are not touched at all

/// A set of common components related with the entry.
#[derive(QueryData)]
pub struct EntryQuery {
    pub entity: Entity,
    pub mode: &'static EntryMode,
    pub estimate: Option<&'static EntryEstimate>,
    pub parent_schedule: &'static ChildOf,
    stop: &'static EntryStop,
    is_derived: Option<&'static IsDerivedEntry>,
}

impl<'w, 's> EntryQueryItem<'w, 's> {
    /// Check if the current entry is derived
    pub fn is_derived(&self) -> bool {
        self.is_derived.is_some()
    }
    /// Returns the stop of the entry
    pub fn stop(&self) -> Entity {
        self.stop.entity()
    }
    /// Returns how long the stop duration is. Returns [`None`] if the entry does not have an
    /// estimate.
    pub fn stop_duration(&self) -> Option<Duration> {
        self.estimate.map(|e| e.dep - e.arr)
    }
    /// Returns the travel duration (The previous entry of the current entry to the current entry).
    /// Returns [`None`] if any of the two entries don't have estimates.
    pub fn travel_duration(
        &self,
        parent_it: &TripQueryItem,
        entry_q: &Query<(&EntryMode, Option<&EntryEstimate>)>,
    ) -> Option<Duration> {
        assert_eq!(parent_it.entity, self.parent_schedule.parent());
        let arr = self.estimate?.arr;
        let parent_schedule = parent_it.schedule;
        let idx = parent_schedule
            .iter()
            .copied()
            .position(|e| e == self.entity)?;
        if idx == 0 {
            return Some(arr.as_duration());
        }
        let prev_dep = entry_q
            .iter_many(parent_schedule[0..idx].iter().rev())
            .find(|(mode, _)| match (mode.arr, mode.dep) {
                (Some(TravelMode::For(_)), _) => true,
                (Some(TravelMode::At(_)), _) => true,
                (_, TravelMode::At(_)) => true,
                _ => false,
            })?
            .1?
            .dep;
        Some(arr - prev_dep)
    }
}

/// Changes the entry's stop.
/// This would trigger a route recalculation
#[derive(Debug, EntityEvent)]
pub struct ChangeEntryStop {
    /// The entry's entity
    pub entity: Entity,
    /// The stop's entity
    pub stop: Entity,
}

/// Changes the entry's mode
/// This would trigger a schedule estimate recalculation
#[derive(Reflect, Debug, EntityEvent, Clone, Copy)]
pub struct AdjustEntryMode {
    /// The entry's entity
    pub entity: Entity,
    /// The adjustment to the entry's times
    pub adj: EntryModeAdjustment,
}

/// How to adjust an [`EntryMode`]
#[derive(Reflect, Debug, Clone, Copy)]
pub enum EntryModeAdjustment {
    /// Set the arrival mode to a new value
    SetArrival(Option<TravelMode>),
    /// Set the departure mode to a new value
    SetDeparture(TravelMode),
    /// Shift the arrival mode by [`Duration`].
    /// Has no effect when the mode is [`TravelMode::Flexible`]
    ShiftArrival(Duration),
    /// Shift the departure mode by [`Duration`]
    /// Has no effect when the mode is [`TravelMode::Flexible`]
    ShiftDeparture(Duration),
}

fn update_entry_stop(event: On<ChangeEntryStop>, mut commands: Commands) {
    commands.entity(event.entity).insert(EntryStop(event.stop));
}

fn update_entry_mode(event: On<AdjustEntryMode>, mut entry_modes: Query<&mut EntryMode>) {
    let mut entry_mode = entry_modes
        .get_mut(event.entity)
        .expect("Entity does not carry an EntryMode component");
    *entry_mode = transform_entry_mode(*entry_mode, event.adj);
}

pub fn transform_entry_mode(mut old: EntryMode, adjustment: EntryModeAdjustment) -> EntryMode {
    use EntryModeAdjustment::*;
    match adjustment {
        SetArrival(m) => {
            old.arr = m;
        }
        SetDeparture(m) => {
            old.dep = m;
        }
        ShiftArrival(d) => {
            old.shift_arr(d);
        }
        ShiftDeparture(d) => {
            old.shift_dep(d);
        }
    }
    old
}
