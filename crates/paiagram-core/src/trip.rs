// SPDX-License-Identifier: MPL-2.0

use std::cell::RefCell;

use ecow::EcoVec;
use serde::{Deserialize, Serialize};

use crate::time::{Duration, TimetableTime};
use crate::{IntervalCollection, StationKey, WorldGraph};

#[derive(Clone, Serialize, Deserialize, Copy, Debug, PartialEq)]
pub enum TravelMode {
    At(TimetableTime),
    For(Duration),
    Flexible,
}

#[derive(Clone, Serialize, Deserialize, Copy, Debug, PartialEq)]
pub enum TEntry {
    /// A derived state. this is calculated by the system
    Derived(StationKey),
    /// A pinned station. The trip must visit this station.
    /// This requires runtime checks to make sure that the start and end are valid
    Pinned {
        stn: StationKey,
        trk: u16,
        arr: TravelMode,
        dep: TravelMode,
        id: u32,
    },
    /// A pinned station. The trip must visit this station,
    /// but the vehicle does not stop at the station.
    PinnedNonStop {
        stn: StationKey,
        trk: u16,
        pass: TravelMode,
        id: u32,
    },
    /// Going to an external station
    PinnedExternalNonStop {
        stn: StationKey,
        trk: u16,
        pass: TravelMode,
        id: u32,
    },
    /// Exit the route
    PinnedExternal { id: u32 },
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct TripSchedule {
    entries: EcoVec<TEntry>,
}

impl TripSchedule {
    pub fn new(entries: EcoVec<TEntry>) -> Self {
        Self { entries }
    }
}

#[derive(Clone, Copy)]
struct TEstimate {
    arr: TimetableTime,
    dep: TimetableTime,
}

impl TEstimate {
    fn duration(self) -> Duration {
        self.dep - self.arr
    }
}

struct EstimateSketch {
    stn: StationKey,
    slot: usize,
    dur: Duration,
}

thread_local! {
    static ESTIMATE_BUFFER: RefCell<Vec<EstimateSketch>> = RefCell::new(Vec::with_capacity(100));
    static OUTPUT_BUFFER: RefCell<Vec<(Option<TEstimate>, TEntry)>> = RefCell::new(Vec::with_capacity(100));
}

enum StackElem {
    Ignored,
    In(StationKey, Duration),
    AtAt(StationKey, TimetableTime, TimetableTime),
    ForAt(StationKey, Duration, TimetableTime),
    ForFor(StationKey, Duration, Duration),
}

impl TripSchedule {
    fn estimates<F, R>(&self, intervals: &IntervalCollection, map: &WorldGraph, mut f: F) -> R
    where
        F: FnMut(&[(Option<TEstimate>, TEntry)]) -> R,
    {
        ESTIMATE_BUFFER.with(|r| {
            let mut estimate_buf = r.borrow_mut();
            estimate_buf.clear();
            OUTPUT_BUFFER.with(|r| {
                let mut output_buf = r.borrow_mut();
                output_buf.clear();
                for entry in &self.entries {
                    let se = make_se(*entry);
                    match se {
                        StackElem::Ignored => {
                            output_buf.push((None, *entry));
                        }
                        StackElem::In(stn, dur) => {
                            let es = EstimateSketch {
                                stn,
                                slot: estimate_buf.len(),
                                dur,
                            };
                            estimate_buf.push(es);
                        }
                        // unwind
                        StackElem::AtAt(stn, at, dt) => {}
                        StackElem::ForAt(stn, ad, dt) => {}
                        StackElem::ForFor(stn, ad, dd) => {}
                    };
                }
                // finalize
                f(output_buf.as_slice())
            })
        })
    }
}

fn make_se(entry: TEntry) -> StackElem {
    use StackElem as Se;
    use TravelMode as Tm;
    match entry {
        TEntry::Derived(stn) => Se::In(stn, Duration::ZERO),
        TEntry::Pinned { stn, arr, dep, .. } => match (arr, dep) {
            (Tm::At(at), Tm::At(dt)) => Se::AtAt(stn, at, dt),
            (Tm::At(at), Tm::For(dd)) => Se::AtAt(stn, at, at + dd),
            (Tm::At(at), Tm::Flexible) => Se::AtAt(stn, at, at),
            (Tm::For(ad), Tm::At(dt)) => Se::ForAt(stn, ad, dt),
            (Tm::For(ad), Tm::For(dd)) => Se::ForFor(stn, ad, dd),
            (Tm::For(ad), Tm::Flexible) => Se::ForFor(stn, ad, Duration::ZERO),
            (Tm::Flexible, Tm::At(dt)) => Se::AtAt(stn, dt, dt),
            (Tm::Flexible, Tm::For(dd)) => Se::In(stn, dd),
            (Tm::Flexible, Tm::Flexible) => Se::In(stn, Duration::ZERO),
        },
        TEntry::PinnedNonStop { stn, .. } => Se::In(stn, Duration::ZERO),
        TEntry::PinnedExternalNonStop { .. } => Se::Ignored,
        TEntry::PinnedExternal { .. } => Se::Ignored,
    }
}
