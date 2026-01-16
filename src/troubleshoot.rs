use crate::units::time::Duration;
use crate::vehicles::AdjustTimetableEntry;
use crate::vehicles::entries::{TimetableEntry, TimetableEntryCache, TravelMode, VehicleSchedule};
use bevy::prelude::*;
use thiserror::Error;

pub struct TroubleShootPlugin;

impl Plugin for TroubleShootPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            analyze_entry.run_if(on_message::<AdjustTimetableEntry>),
        );
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ScheduleProblem(pub Vec<ScheduleProblemType>);
#[derive(Debug, PartialEq, Eq, Error)]
pub enum ScheduleProblemType {
    #[error("Schedule is too short")]
    TooShort,
    #[error("Schedule collides with another vehicle")]
    CollidesWithAnotherVehicle {
        own_entry: Entity,
        other_entry: Entity,
        location: Entity,
    },
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct EntryProblem(pub Vec<EntryProblemType>);
#[derive(Debug, PartialEq, Eq, Error)]
pub enum EntryProblemType {
    #[error("No estimation available for this entry")]
    NoEstimation,
    #[error("Travel duration is too short or negative")]
    TravelDurationTooShort,
    #[error("Arrival is flexible but departure is at a fixed time")]
    ReversedFlexibleMode,
    #[error("This entry collides with another entry")]
    CollidesWithAnotherEntry(Entity),
}

pub fn analyze_schedules(
    mut commands: Commands,
    regular_schedules: Query<&VehicleSchedule, Without<ScheduleProblem>>,
    problematic_schedules: Query<&VehicleSchedule, With<ScheduleProblem>>,
) {
}

pub fn analyze_entry(
    mut commands: Commands,
    mut msg_read_entry: MessageReader<AdjustTimetableEntry>,
    mut entries: Query<(
        Option<&mut EntryProblem>,
        &TimetableEntry,
        &TimetableEntryCache,
    )>,
) {
    for entry_entity in msg_read_entry.read().map(|msg| msg.entity) {
        let Ok((mut existing_problem, entry, entry_cache)) = entries.get_mut(entry_entity) else {
            continue;
        };

        let check = |problems: &mut Vec<EntryProblemType>| {
            if entry_cache.estimate.is_none() {
                problems.push(EntryProblemType::NoEstimation)
            }
            if let TravelMode::For(a) = entry.arrival
                && a <= Duration(0)
            {
                problems.push(EntryProblemType::TravelDurationTooShort);
            } else if let Some(TravelMode::For(d)) = entry.departure
                && d <= Duration(0)
            {
                problems.push(EntryProblemType::TravelDurationTooShort);
            }
            if matches!(entry.arrival, TravelMode::Flexible)
                && matches!(entry.departure, Some(TravelMode::At(_)))
            {
                problems.push(EntryProblemType::ReversedFlexibleMode)
            }
            problems.dedup();
        };

        if let Some(ref mut problem) = existing_problem {
            problem.0.clear();
            check(&mut problem.0);
            if problem.0.is_empty() {
                commands.entity(entry_entity).remove::<EntryProblem>();
            }
        } else {
            let mut problems = Vec::new();
            check(&mut problems);
            if !problems.is_empty() {
                commands.entity(entry_entity).insert(EntryProblem(problems));
            }
        }
    }
}
