use super::RevertableActions::ChangeEntryMode as ChangeEntryModeWrapper;
use anyhow::{Result, anyhow};
use bevy::prelude::*;
use paiagram_core::entry::{AdjustEntryMode, EntryMode, EntryModeAdjustment, transform_entry_mode};

pub(super) fn watch_entry_mode_changes(
    msg: On<AdjustEntryMode>,
    entry_q: Query<&EntryMode>,
    time: Res<Time>,
    mut action_history: ResMut<super::ActionHistory>,
) {
    const MERGE_THRESHOLD_MS: std::time::Duration = std::time::Duration::from_millis(300);
    let previous_state = entry_q.get(msg.entity).unwrap();
    let new_state = transform_entry_mode(*previous_state, msg.adj);
    let action_inner = ChangeEntryModeInner {
        entry: msg.entity,
        previous_state: *previous_state,
        new_state,
    };
    let action = match msg.adj {
        EntryModeAdjustment::ShiftArrival(_) => {
            ChangeEntryMode::ShiftArrival((action_inner, time.elapsed()))
        }
        EntryModeAdjustment::ShiftDeparture(_) => {
            ChangeEntryMode::ShiftDeparture((action_inner, time.elapsed()))
        }
        _ => ChangeEntryMode::Other(action_inner),
    };
    match (action, action_history.history.back_mut()) {
        (
            ChangeEntryMode::ShiftArrival((a1, t1)),
            Some(ChangeEntryModeWrapper(ChangeEntryMode::ShiftArrival((a2, t2)))),
        )
        | (
            ChangeEntryMode::ShiftDeparture((a1, t1)),
            Some(ChangeEntryModeWrapper(ChangeEntryMode::ShiftDeparture((a2, t2)))),
        ) if (t1 - *t2) < MERGE_THRESHOLD_MS => {
            a2.new_state = a1.new_state;
            *t2 = t1;
        }
        (action, _) => {
            action_history.add(ChangeEntryModeWrapper(action));
        }
    }
}

#[derive(Reflect)]
pub(super) enum ChangeEntryMode {
    ShiftArrival((ChangeEntryModeInner, std::time::Duration)),
    ShiftDeparture((ChangeEntryModeInner, std::time::Duration)),
    Other(ChangeEntryModeInner),
}

impl super::RevertableAction for ChangeEntryMode {
    fn undo(&self, world: &mut World) -> Result<()> {
        match self {
            Self::ShiftArrival((t, _)) => t.undo(world),
            Self::ShiftDeparture((t, _)) => t.undo(world),
            Self::Other(t) => t.undo(world),
        }
    }
    fn redo(&self, world: &mut World) -> Result<()> {
        match self {
            Self::ShiftArrival((t, _)) => t.redo(world),
            Self::ShiftDeparture((t, _)) => t.redo(world),
            Self::Other(t) => t.redo(world),
        }
    }
}

#[derive(Reflect)]
pub(super) struct ChangeEntryModeInner {
    entry: Entity,
    previous_state: EntryMode,
    new_state: EntryMode,
}

impl super::RevertableAction for ChangeEntryModeInner {
    fn undo(&self, world: &mut World) -> Result<()> {
        let Some(mut mode) = world.get_mut::<EntryMode>(self.entry) else {
            return Err(anyhow!("The entry has been modified or deleted"));
        };
        *mode = self.previous_state;
        Ok(())
    }
    fn redo(&self, world: &mut World) -> Result<()> {
        let Some(mut mode) = world.get_mut::<EntryMode>(self.entry) else {
            return Err(anyhow!("The entry has been modified or deleted"));
        };
        *mode = self.new_state;
        Ok(())
    }
}
