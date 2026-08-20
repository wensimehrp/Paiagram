// SPDX-License-Identifier: MPL-2.0
//! Module for managing user selection
use paiagram_core::{IntervalKey, LonLat, NodeKey, RouteKey, ServiceClassKey, StationKey, TripKey};
use vec1::{Vec1, vec1};

macro_rules! gen_selected_items {
    {
        $(type ($selected_name:ident, $selected_name_plural:ident) = $selected_type:ty;)*
        no_single_selection {
            $(type $no_single_name:ident = $no_single_type:ty;)*
        }
    } => {
        /// Items selected by the user
        pub(crate) enum SelectedItems {
            $( $selected_name_plural(Vec1<$selected_type>), )*
            $( $no_single_name($no_single_type), )*
            None,
        }
        /// Item selected by the user
        pub(crate) enum SelectedItem {
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

gen_selected_items! {
    type (Station, Stations) = StationKey;                  // Moving stations and creating routes
    type (Node, Nodes) = NodeKey;                           // Moving nodes
    type (Interval, Intervals) = IntervalKey;               // No use yet
    type (Trip, Trips) = TripKey;                           // No use yet
    type (TEntry, TEntries) = (TripKey, u64);               // Batch edit times
    type (ServiceClass, ServiceClasses) = ServiceClassKey;  // Batch edit color
    type (Route, Routes) = RouteKey;
    no_single_selection {
        type Coordinate = (LonLat, String);                 // Add new station
        type TripExtension = ();                            // Extend current trip
        type RouteExtension = ();                           // Extend current route
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
}
