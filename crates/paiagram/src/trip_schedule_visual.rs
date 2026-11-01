use either::Either;
use paiagram_core::NodeKey;
use paiagram_core::time::{TDuration, TimetableTime};
use paiagram_core::trip::{TEntry, TEntryId, TEstimate, TravelMode, TripSchedule};

type EditingOffset = (TEntryId, TEstimate, Either<TDuration, TDuration>);

pub(crate) trait WithVisualMod {
    fn with_entry_shift(self, editing_offset: Option<EditingOffset>) -> Self;
    fn with_pushed_entry(self, focused_pos: Option<(NodeKey, TimetableTime)>) -> Self;
}

impl WithVisualMod for TripSchedule {
    fn with_entry_shift(mut self, editing_offset: Option<EditingOffset>) -> Self {
        let Some((entry_id, _base_estimate, shift)) = editing_offset else {
            return self;
        };
        match shift {
            Either::Left(d) => self.shift_arr_or_pass(entry_id, d),
            Either::Right(d) => self.shift_dep(entry_id, d),
        }
        self
    }
    fn with_pushed_entry(mut self, focused_pos: Option<(NodeKey, TimetableTime)>) -> Self {
        let Some((node, new_time)) = focused_pos else {
            return self;
        };
        if let Some(prev_entry) = self.entries().last()
            && prev_entry.node == node
        {
            let prev_id = prev_entry.id;
            self.set_dep_mode(prev_id, Some(TravelMode::At(new_time)));
        } else {
            self.push(TEntry {
                node,
                arr_or_pass: TravelMode::At(new_time),
                dep: None,
                id: unsafe { TEntryId::dummy() },
            })
        }
        self
    }
}
