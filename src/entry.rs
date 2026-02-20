use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

use crate::units::time::{Duration, TimetableTime};

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

/// Travel mode for entries
#[derive(Reflect, Default, Debug, Clone, Copy)]
pub enum TravelMode {
    At(TimetableTime),
    For(Duration),
    #[default]
    Flexible,
}

/// The entry's [`TravelMode`]s.
#[derive(Default, Reflect, Component)]
#[reflect(Component)]
pub struct EntryMode {
    pub arr: Option<TravelMode>,
    pub dep: TravelMode,
}

impl EntryMode {
    pub fn new_derived() -> Self {
        Self {
            arr: None,
            dep: TravelMode::Flexible,
        }
    }
    /// Shift the arrival time. The function does nothing if the arrival mode is [`TravelMode::Flexible`]
    /// or [`Option::None`]
    pub fn shift_arr(&mut self, d: Duration) {
        match &mut self.arr {
            Some(TravelMode::At(t)) => *t += d,
            Some(TravelMode::For(t)) => *t += d,
            Some(TravelMode::Flexible) | None => (),
        }
    }
    /// Shift the departure time. The function does nothing if the departure mode is [`TravelMode::Flexible`]
    pub fn shift_dep(&mut self, d: Duration) {
        match &mut self.dep {
            TravelMode::At(t) => *t += d,
            TravelMode::For(t) => *t += d,
            TravelMode::Flexible => (),
        }
    }
}

/// Where the vehicle stops. The stop could be a station, or a platform that belongs to the station.
#[derive(Reflect, Component, MapEntities, Deref, DerefMut)]
#[reflect(Component, MapEntities)]
#[relationship(relationship_target = crate::station::PlatformEntries)]
#[require(EntryMode)]
pub struct EntryStop(
    #[relationship]
    #[entities]
    pub Entity,
);

/// The estimated arrival and departure times of the entry. This is not a hard requirement for entries.
#[derive(Reflect, Component)]
#[reflect(Component)]
pub struct EntryEstimate {
    pub arr: TimetableTime,
    pub dep: TimetableTime,
}

impl EntryEstimate {
    pub fn new(arr: TimetableTime, dep: TimetableTime) -> Self {
        Self { arr, dep }
    }
}

/// Bundle for easy spawning
#[derive(Bundle)]
pub struct EntryBundle {
    time: EntryMode,
    stop: EntryStop,
}

impl EntryBundle {
    pub fn new(arr: Option<TravelMode>, dep: TravelMode, stop: Entity) -> Self {
        Self {
            time: EntryMode { arr, dep },
            stop: EntryStop(stop),
        }
    }
    pub fn new_derived(stop: Entity) -> Self {
        Self {
            time: EntryMode::new_derived(),
            stop: EntryStop(stop),
        }
    }
}

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
    pub fn is_derived(&self) -> bool {
        self.is_derived.is_some()
    }
    pub fn is_not_derived(&self) -> bool {
        self.is_derived.is_none()
    }
    pub fn stop(&self) -> Entity {
        self.stop.entity()
    }
    pub fn stop_duration(&self) -> Option<Duration> {
        self.estimate.map(|e| e.dep - e.arr)
    }
    pub fn travel_duration<'a>(
        &self,
        parent_q: &Query<'a, 'a, &crate::trip::TripSchedule>,
        entry_q: &Query<'a, 'a, (&EntryMode, Option<&EntryEstimate>)>,
    ) -> Option<Duration> {
        let arr = self.estimate?.arr;
        let parent_schedule = parent_q.get(self.parent_schedule.parent()).ok()?;
        let idx = parent_schedule.iter().position(|e| e == self.entity)?;
        if idx == 0 {
            return Some(arr.as_duration());
        }
        let prev_dep = entry_q
            .iter_many(parent_schedule[0..idx].iter().rev())
            .find(|(mode, _)| match (mode.arr, mode.dep) {
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
    pub entity: Entity,
    pub stop: Entity,
}

/// Changes the entry's mode
/// This would trigger a schedule estimate recalculation
#[derive(Debug, EntityEvent)]
pub struct AdjustEntryMode {
    pub entity: Entity,
    pub adj: EntryModeAdjustment,
}

#[derive(Debug)]
pub enum EntryModeAdjustment {
    SetArrival(Option<TravelMode>),
    SetDeparture(TravelMode),
    ShiftArrival(Duration),
    ShiftDeparture(Duration),
}

fn update_entry_stop(event: On<ChangeEntryStop>, mut commands: Commands) {
    commands.entity(event.entity).insert(EntryStop(event.stop));
}

fn update_entry_mode(event: On<AdjustEntryMode>, mut entry_modes: Query<&mut EntryMode>) {
    let mut entry_mode = entry_modes
        .get_mut(event.entity)
        .expect("Entity does not carry an EntryMode component");
    use EntryModeAdjustment::*;
    match event.adj {
        SetArrival(m) => {
            entry_mode.arr = m;
        }
        SetDeparture(m) => {
            entry_mode.dep = m;
        }
        ShiftArrival(d) => {
            entry_mode.shift_arr(d);
        }
        ShiftDeparture(d) => {
            entry_mode.shift_dep(d);
        }
    }
}
