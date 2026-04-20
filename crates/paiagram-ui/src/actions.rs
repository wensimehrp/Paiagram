//! A set of actions the user can perform
//! Each action has a reverse action that would be triggered when the user hits the revert shortcut

use bevy::prelude::*;
use eros::bail;
use std::collections::VecDeque;

mod change_entry_mode;

pub(crate) struct ActionsPlugin;
impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionHistory>()
            .add_observer(change_entry_mode::watch_entry_mode_changes);
    }
}

pub(crate) trait RevertableAction {
    fn undo(&self, world: &mut World) -> eros::Result<()>;
    fn redo(&self, world: &mut World) -> eros::Result<()>;
}

macro_rules! for_all_actions {
    ($action:expr, $t:ident, $body:expr) => {
        match $action {
            RevertableActions::ChangeEntryMode($t) => $body,
        }
    };
}

#[derive(Reflect)]
enum RevertableActions {
    ChangeEntryMode(change_entry_mode::ChangeEntryMode),
}

impl RevertableActions {
    fn undo(&self, world: &mut World) -> eros::Result<()> {
        for_all_actions!(self, it, it.undo(world))
    }
    fn redo(&self, world: &mut World) -> eros::Result<()> {
        for_all_actions!(self, it, it.redo(world))
    }
}

#[derive(Reflect, Resource, Default)]
#[reflect(Resource)]
pub(crate) struct ActionHistory {
    history: VecDeque<RevertableActions>,
    ptr: usize,
}

impl ActionHistory {
    // TODO: use size limitation in user settings?
    /// The hard limit of the action history stack
    const SIZE: usize = 1000;
    pub fn can_undo(&self) -> bool {
        self.ptr != 0
    }
    pub fn can_redo(&self) -> bool {
        self.ptr < self.history.len()
    }
    /// Add a variant of [`RevertableActions`] to the action queue
    /// Note that some types of actions, such as [`RevertableActions::ChangeEntryMode`] might require merging
    /// actions instead of appending actions.
    fn add(&mut self, action: RevertableActions) {
        if self.ptr < self.history.len() {
            self.history.truncate(self.ptr);
        }
        self.history.push_back(action);
        if self.history.len() > Self::SIZE {
            self.history.pop_front();
        }
        self.ptr = self.history.len();
    }
    pub(crate) fn try_undo(&mut self, world: &mut World) -> eros::Result<()> {
        if !self.can_undo() {
            bail!("Cannot undo!");
        }
        self.ptr -= 1;
        self.history[self.ptr].undo(world)
    }
    pub(crate) fn try_redo(&mut self, world: &mut World) -> eros::Result<()> {
        if !self.can_redo() {
            bail!("Cannot redo!");
        }
        let result = self.history[self.ptr].redo(world);
        self.ptr += 1;
        result
    }
}
