// SPDX-License-Identifier: MPL-2.0
//! Module for managing user selection
use paiagram_core::{PlatformKey, StationKey, TripKey};
use vec1::Vec1;

/// Items that are selected by the user
pub(crate) enum SelectedItems {
    Stations(Vec1<StationKey>),
    Platforms(Vec1<PlatformKey>),
    Trips(Vec1<TripKey>),
}

/// Items that are currently focused by the user. For example, the user is opening a panel
pub(crate) enum FocusedItems {}
