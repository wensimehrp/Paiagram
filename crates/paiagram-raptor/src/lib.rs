//! # RAPTOR Module
//! This crate bridges between Paiagram types and the raptor-rs crate

// TODO: in case if the crate author updates, switch to iterators instead of
// vectors

use std::borrow::Cow;

use paiagram_core::time::TimetableTime;
use paiagram_core::{StationKey, TripKey};
pub use raptor::Journey;
use raptor::Timetable;

pub fn make_query_data(
    from: StationKey,
    to: StationKey,
    at: TimetableTime,
    info: RaptorTimetable,
) -> Vec<raptor::Journey<TripKey, StationKey>> {
    // TODO: add a more reasonable limitation for transfers
    todo!()
}

pub struct RaptorTimetable;

// impl raptor::Timetable for RaptorTimetable<'_, '_> {
//     type Route = Entity; // in this case it is the same as trips
//     type Stop = Entity;
//     type Trip = Entity;
// }
