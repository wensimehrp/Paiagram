use egui::{Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, TimetableTime};
use paiagram_core::{Command, TripKey};
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TripTab {
    trip_key: TripKey,
}

impl TripTab {
    pub(crate) fn new(trip_key: TripKey) -> Self {
        Self { trip_key }
    }
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn title(&self) -> WidgetText {
        tr!("tab-trip").into()
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        let view = app.source.trips.get_view(self.trip_key);
        let Some(ref view) = view else {
            ui.label("Trip not found");
            return;
        };

        ui.heading(view.name.as_str());
        let entry_count = view.schedule.entries().len();
        ui.label(entry_count.to_string());

        let mut entries: Vec<(usize, &TEntry)> = view.schedule.entries().iter().enumerate().collect();

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new(ui.id().with("trip_grid"))
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(tr!("trip-table-station"));
                    ui.label(tr!("trip-table-arrival"));
                    ui.label(tr!("trip-table-departure"));
                    ui.end_row();
                    for (idx, entry) in entries.iter() {
                        let stn_name = get_station_name(app, entry);
                        ui.label(stn_name);

                        let (arr, dep) = get_entry_times(entry);

                        // Arrival
                        if let Some(arr_time) = arr {
                            let mut t = arr_time;
                            let res = ui.add_sized(vec2(70.0, 18.0), crate::widgets::TimeDragValue(&mut t));
                            if res.changed() && t != arr_time {
                                modify_entry_time(app, self.trip_key, *idx, true, t - arr_time);
                            }
                        } else {
                            ui.label("↓");
                        }

                        // Departure
                        let dep_time = dep;
                        let mut t = dep_time;
                        let res = ui.add_sized(vec2(70.0, 18.0), crate::widgets::TimeDragValue(&mut t));
                        if res.changed() && t != dep_time {
                            modify_entry_time(app, self.trip_key, *idx, false, t - dep_time);
                        }

                        ui.end_row();
                    }
                });
        });
    }
}

fn get_station_name(app: &AppState, entry: &TEntry) -> String {
    let sk = match entry {
        TEntry::Derived(s) => *s,
        TEntry::Pinned { stn: s, .. } => *s,
        TEntry::PinnedNonStop { stn: s, .. } => *s,
        TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
        TEntry::PinnedExternal { .. } => return "External".into(),
    };
    app.source.stations.query(sk, |b| b.name.clone())
        .unwrap_or_default()
        .to_string()
}

fn get_entry_times(entry: &TEntry) -> (Option<TimetableTime>, TimetableTime) {
    match entry {
        TEntry::Pinned { arr: TravelMode::At(a), dep, .. } => {
            let dep_t = match dep {
                TravelMode::At(t) => *t,
                TravelMode::For(d) => *a + *d,
                TravelMode::Flexible => *a,
            };
            (Some(*a), dep_t)
        }
        TEntry::Pinned { dep: TravelMode::At(t), .. } => (None, *t),
        TEntry::Pinned { dep: TravelMode::For(d), arr: TravelMode::At(a), .. } => {
            (None, *a + *d)
        }
        TEntry::Pinned { dep: TravelMode::For(d), .. } => {
            (None, *d + TimetableTime(0))
        }
        TEntry::PinnedNonStop { pass: TravelMode::At(t), .. } => (None, *t),
        TEntry::PinnedNonStop { pass: TravelMode::For(d), .. } => (None, *d + TimetableTime(0)),
        _ => (None, TimetableTime(0)),
    }
}

fn modify_entry_time(app: &mut AppState, trip_key: TripKey, entry_idx: usize, is_arrival: bool, delta: Duration) {
    let view = app.source.trips.get_view(trip_key);
    let Some(view) = view else { return; };

    let mut entries: Vec<TEntry> = view.schedule.entries().to_vec();
    if entry_idx >= entries.len() { return; }

    let modified = match &entries[entry_idx] {
        TEntry::Pinned { stn, trk, arr, dep, id } => {
            let new_arr = if is_arrival {
                match arr {
                    TravelMode::At(t) => Some(TravelMode::At(*t + delta)),
                    TravelMode::For(d) => Some(TravelMode::For(*d + delta)),
                    TravelMode::Flexible => Some(TravelMode::Flexible),
                }
            } else {
                Some(*arr)
            };
            let new_dep = if !is_arrival {
                match dep {
                    TravelMode::At(t) => TravelMode::At(*t + delta),
                    TravelMode::For(d) => TravelMode::For(*d + delta),
                    TravelMode::Flexible => TravelMode::Flexible,
                }
            } else {
                *dep
            };
            TEntry::Pinned { stn: *stn, trk: *trk, arr: new_arr.unwrap_or(*arr), dep: new_dep, id: *id }
        }
        _ => entries[entry_idx].clone(),
    };
    entries[entry_idx] = modified;

    app.source.apply_command(Command::ChangeTripEntries {
        key: trip_key,
        entries: entries.into(),
    });
}
