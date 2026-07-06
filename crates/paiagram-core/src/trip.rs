// SPDX-License-Identifier: MPL-2.0

pub mod routing;

use ecow::EcoVec;
use serde::{Deserialize, Serialize};

use crate::units::time::{Duration, TimetableTime};
use crate::StationKey;

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

    pub fn entries(&self) -> &[TEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut EcoVec<TEntry> {
        &mut self.entries
    }
}
