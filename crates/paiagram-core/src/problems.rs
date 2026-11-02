use crate::{IntervalKey, TripKey};

pub enum Problem {
    TripCollision {
        trips: Vec<TripKey>,
        interval: IntervalKey,
    },
}
