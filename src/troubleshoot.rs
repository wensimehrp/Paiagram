use bevy::prelude::*;

use crate::{
    units::time::Duration,
    vehicles::{
        AdjustTimetableEntry,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
    },
};

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
pub struct ScheduleProblem(pub Vec<ScheduleProblemType>);
pub enum ScheduleProblemType {
    TooShort,
    CollidesWithAnotherVehicle {
        own_entry: Entity,
        other_entry: Entity,
        location: Entity,
    },
}

#[derive(Component)]
pub struct EntryProblem(pub Vec<EntryProblemType>);
#[derive(PartialEq, Eq)]
pub enum EntryProblemType {
    NoEstimation,
    TravelDurationTooShort,
    ReversedFlexibleMode,
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
    mut entries: Query<(Option<&mut EntryProblem>, &TimetableEntry)>,
) {
    for entry_entity in msg_read_entry.read().map(|msg| msg.entity) {
        let Ok((mut existing_problem, entry)) = entries.get_mut(entry_entity) else {
            continue;
        };

        let check = |problems: &mut Vec<EntryProblemType>| {
            if entry.arrival_estimate.is_none() || entry.departure_estimate.is_none() {
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
            if matches!(entry.arrival, TravelMode::Flexible) && matches!(entry.departure, Some(TravelMode::At(_))) {
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
