// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

use either::Either;
use paiagram_core::time::{TDuration, TimetableTime};
use paiagram_core::trip::{TEntryId, TEstimate};
use paiagram_core::{IntervalKey, LonLat, NodeKey, RouteKey, ServiceClassKey, StationKey, TripKey};
use serde::Serialize;
use vec1::{Vec1, vec1};

macro_rules! gen_selected_items {
    {
        $(type ($selected_name:ident, $selected_name_plural:ident) = $selected_type:ty;)*
        no_single_selection {
            $(type $no_single_name:ident = $no_single_type:ty;)*
        }
    } => {
        /// Items selected by the user
        #[derive(Clone, Serialize)]
        pub enum SelectedItems {
            $( $selected_name_plural(Vec1<$selected_type>), )*
            $( $no_single_name($no_single_type), )*
            None,
        }
        /// Item selected by the user
        #[derive(Clone, Serialize)]
        pub enum SelectedItem {
            $( $selected_name($selected_type), )*
            None,
        }
        impl From<SelectedItem> for SelectedItems {
            fn from(value: SelectedItem) -> Self {
                match value {
                    $( SelectedItem::$selected_name(val) => SelectedItems::$selected_name_plural(vec1![val]), )*
                    SelectedItem::None => Self::None,
                }
            }
        }
        impl SelectedItems {
            fn toggle(&mut self, selected: SelectedItem) {
                match (selected, &mut *self) {
                    $((
                        SelectedItem::$selected_name(new),
                        Self::$selected_name_plural(arr),
                    ) => {
                        let mut exists: Option<usize> = None;
                        for (idx, item) in arr.iter().enumerate() {
                            if new == *item {
                                exists = Some(idx);
                                break;
                            }
                        }
                        let Some(idx) = exists else {
                            arr.push(new);
                            return;
                        };
                        if arr.len() == 1 {
                            *self = Self::None;
                        } else {
                            arr.remove(idx);
                        }
                    }, )*
                    _ => {}
                }
            }
            fn merge(&mut self, selected: SelectedItem) {
                match (selected, self) {
                    $((
                        SelectedItem::$selected_name(new),
                        Self::$selected_name_plural(arr),
                    ) => arr.push(new),)*
                    _ => {}
                }
            }
        }
    };
}

#[derive(Clone, Serialize)]
pub struct TripSelection {
    pub trip_key: TripKey,
    #[serde(skip)]
    pub editing_offset: Option<(TEntryId, TEstimate, Either<TDuration, TDuration>)>,
}

impl TripSelection {
    pub fn new(trip_key: TripKey) -> Self {
        Self {
            trip_key,
            editing_offset: None,
        }
    }
}

impl From<TripKey> for TripSelection {
    fn from(value: TripKey) -> Self {
        Self::new(value)
    }
}

impl PartialEq for TripSelection {
    fn eq(&self, other: &Self) -> bool {
        self.trip_key == other.trip_key
    }
}

gen_selected_items! {
    type (Station, Stations) = StationKey;                  // Moving stations and creating routes
    type (Node, Nodes) = NodeKey;                           // Moving nodes
    type (Interval, Intervals) = IntervalKey;               // No use yet
    type (Trip, Trips) = TripSelection;                     // Select and shift current trip
    type (TEntry, TEntries) = (TripKey, u64);               // Batch edit times
    type (ServiceClass, ServiceClasses) = ServiceClassKey;  // Batch edit color
    type (Route, Routes) = RouteKey;
    no_single_selection {
        type Coordinate = (LonLat, String);                               // Add new station
        type TripExtension = (TripKey, Option<(NodeKey, TimetableTime)>); // Extend current trip
        type RouteExtension = ();                                         // Extend current route
    }
}

impl SelectedItems {
    /// Take the current selected item and leave [`SelectedItems::None`] in place.
    pub fn take(&mut self) -> Self {
        let mut a = Self::None;
        std::mem::swap(self, &mut a);
        a
    }
    pub fn replace(&mut self, selected: SelectedItem) {
        *self = selected.into();
    }
}
