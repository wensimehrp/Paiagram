pub mod coordinates;
pub mod distance;
pub mod speed;
pub mod time;

pub use coordinates::*;
pub use distance::*;
use serde::{Deserialize, Serialize};

use crate::StationKey;
use crate::time::{Duration, TimetableTime};

/// CanvasLength in centimetres
pub struct CanvasLength(f64);

#[derive(Clone, Serialize, Deserialize, Copy, Debug, PartialEq)]
pub enum TravelMode {
    At(TimetableTime),
    For(Duration),
    Flexible,
}

/// The timetable entry
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
    PinnedExternal {
        stn: StationKey,
        trk: u16,
        pass: TravelMode,
        id: u32,
    },
    /// Exit the route
    PinnedExit { id: u32 },
}
