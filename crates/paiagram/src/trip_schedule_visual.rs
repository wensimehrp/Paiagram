use either::Either;
use paiagram_core::NodeKey;
use paiagram_core::time::{TDuration, TimetableTime};
use paiagram_core::trip::{TEntry, TEntryId, TEstimate, TripSchedule};

type EditingOffset = (TEntryId, TEstimate, Either<TDuration, TDuration>);

pub(crate) trait WithVisualMod {
    fn with_entry_shift(self, editing_offset: Option<EditingOffset>) -> Self;
    fn with_pushed_entry(self, focused_pos: Option<(NodeKey, TimetableTime)>) -> Self;
}

impl WithVisualMod for TripSchedule {
    fn with_entry_shift(mut self, editing_offset: Option<EditingOffset>) -> Self {
        if let Some((entry_id, _base_estimate, shift)) = editing_offset {
            match shift {
                Either::Left(d) => self.shift_arr_or_pass(entry_id, d),
                Either::Right(d) => self.shift_dep(entry_id, d),
            }
        }
        self
    }
    fn with_pushed_entry(mut self, focused_pos: Option<(NodeKey, TimetableTime)>) -> Self {
        if let Some((node, new_time)) = focused_pos {
            self.push(TEntry {
                node,
                arr_or_pass: paiagram_core::trip::TravelMode::At(new_time),
                dep: None,
                id: unsafe { TEntryId::dummy() },
            });
        }
        self
    }
}
