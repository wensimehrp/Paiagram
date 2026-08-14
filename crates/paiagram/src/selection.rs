// SPDX-License-Identifier: MPL-2.0
//! Module for managing user selection
use paiagram_core::{PlatformKey, StationKey, TripKey};
use vec1::{Vec1, vec1};

/// Items that are selected by the user
pub(crate) enum SelectedItems {
    Stations(Vec1<StationKey>),
    Platforms(Vec1<PlatformKey>),
    Trips(Vec1<TripKey>),
    TEntries(Vec1<(TripKey, u64)>),
    None,
}

pub(crate) enum SelectedItem {
    Station(StationKey),
    Platform(PlatformKey),
    Trip(TripKey),
    TEntry((TripKey, u64)),
    None,
}

impl From<SelectedItem> for SelectedItems {
    fn from(value: SelectedItem) -> Self {
        match value {
            SelectedItem::Station(val) => Self::Stations(vec1![val]),
            SelectedItem::Platform(val) => Self::Platforms(vec1![val]),
            SelectedItem::Trip(val) => Self::Trips(vec1![val]),
            SelectedItem::TEntry(val) => Self::TEntries(vec1![val]),
            SelectedItem::None => Self::None,
        }
    }
}

impl SelectedItems {
    fn discard(&mut self) -> Self {
        let mut a = Self::None;
        std::mem::swap(self, &mut a);
        a
    }
    fn replace(&mut self, selected: SelectedItem) {
        *self = selected.into();
    }
    fn merge(&mut self, selected: SelectedItem) {
        use SelectedItem as Si;
        match (self, selected) {
            (Self::Stations(arr), Si::Station(new)) => arr.push(new),
            (Self::Platforms(arr), Si::Platform(new)) => arr.push(new),
            (Self::Trips(arr), Si::Trip(new)) => arr.push(new),
            (Self::TEntries(arr), Si::TEntry(new)) => arr.push(new),
            _ => {}
        }
    }
}

/// Items that are currently focused by the user. For example, the user is opening a panel
pub(crate) enum FocusedItems {}
