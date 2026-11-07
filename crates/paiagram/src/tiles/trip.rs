use egui::*;
use either::Either;
use paiagram_core::time::{TDuration, TTime, TimetableTime};
use paiagram_core::trip::{EstimateEntry, TEntryId, TEstimate, TravelMode, TripSchedule};
use paiagram_core::{NodeKey, TripKey, World};
use paiagram_selection::SelectedItems;
use serde::{Deserialize, Serialize};

mod widgets;
use self::widgets::SeparateDisplayDragValue;
use super::Tile;
use crate::App;
use crate::font::TIMETABLTE_TEXT_STYLE;
use crate::tiles::service_classes::service_class_button_atoms;
use crate::trip_schedule_visual::WithVisualMod;

#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct TripTile {
    trip_key: Option<TripKey>,
    show_derived: bool,
    #[serde(skip)]
    name_edit_buf: Option<String>,
    #[serde(skip)]
    editing_offset: Option<(TEntryId, Either<TDuration, TDuration>)>,
}

impl PartialEq for TripTile {
    fn eq(&self, other: &Self) -> bool {
        self.trip_key == other.trip_key
    }
}

impl Tile for TripTile {
    const NAME: &'static str = "Trip";
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        puffin::profile_function!();
        Frame::new().inner_margin(6.0).show(ui, |ui| show_trip(self, app, ui));
    }
}

impl TripTile {
    pub(crate) fn new(trip_key: TripKey) -> Self {
        Self {
            trip_key: Some(trip_key),
            show_derived: false,
            name_edit_buf: None,
            editing_offset: None,
        }
    }
}

fn show_trip(tile: &mut TripTile, app: &mut App, ui: &mut Ui) {
    puffin::profile_function!();

    let get_trip_sch = |key| app.trips.get(&key).map(|wfc| (key, wfc, wfc.schedule.clone()));

    let Some((trip_key, trip, base_schedule)) =
        tile.trip_key.and_then(get_trip_sch).or_else(|| match &app.selected_items {
            SelectedItems::Trips(t) => get_trip_sch(t.first().trip_key)
                .map(|(key, wfc, sch)| (key, wfc, sch.with_entry_shift(t.first().editing_offset))),
            SelectedItems::TripExtension((key, focused_pos)) => get_trip_sch(*key)
                .map(|(key, wfc, sch)| (key, wfc, sch.with_pushed_entry(*focused_pos))),
            _ => None,
        })
    else {
        ui.heading("Select a trip to display the schedule");
        select_a_trip(ui, &mut tile.trip_key, &app);
        return;
    };

    // apply tile local editing offset to schedule
    let visual_schedule = base_schedule.with_entry_shift(tile.editing_offset.map(|(a, b)| {
        (
            a,
            TEstimate {
                arr: TTime::ZERO,
                dep: TTime::ZERO,
            },
            b,
        )
    }));

    let trip_name = trip.name.clone();
    let current_service_class = trip.service_class;

    ui.horizontal(|ui| {
        if let Some(buf) = &mut tile.name_edit_buf {
            ui.text_edit_singleline(buf);
            if ui.button("Finish").clicked() {
                let _ = app.mutate(|mut world| {
                    let trip = world.trips.get_mut(&trip_key).ok_or("wtf!")?;
                    trip.name.clear();
                    trip.name.push_str(buf);
                    Ok(world)
                });
                tile.name_edit_buf = None;
            }
        } else {
            ui.heading(trip_name.as_str());
            if ui.button("Edit").clicked() {
                tile.name_edit_buf = Some(trip_name.to_string());
            }
        }
    });

    ui.horizontal_wrapped(|ui| {
        select_a_trip(ui, &mut tile.trip_key, &app);
        let selected_text_atoms = current_service_class
            .and_then(|key| app.service_classes.get(&key))
            .map_or("Unknown".into_atoms(), |wfc| {
                service_class_button_atoms(wfc).into_atoms()
            });
        let mut new_service_class = current_service_class;
        ComboBox::from_id_salt("change current trip service class")
            .selected_text(selected_text_atoms)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_service_class, None, "Unknown");
                let spacing = ui.spacing().interact_size.y;
                let total_rows = app.service_classes.len();
                ScrollArea::vertical().show_rows(ui, spacing, total_rows, |ui, range| {
                    let iter = app.service_classes.iter().skip(range.start).take(range.count());
                    for (key, service_class) in iter {
                        let atoms = service_class_button_atoms(service_class);
                        ui.selectable_value(&mut new_service_class, Some(*key), atoms);
                    }
                });
            });
        if new_service_class != current_service_class {
            let _ = app.mutate(|mut world| {
                let trip = world.trips.get_mut(&trip_key).ok_or("wtf??")?;
                trip.service_class = new_service_class;
                Ok(world)
            });
        }
    });

    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        Grid::new("trip ui").num_columns(3).striped(true).show(ui, |ui| {
            ui.label("Station");
            ui.label("Arrival/Pass");
            ui.label("Departure");
            ui.end_row();
            // Remove button background
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            let new_editing_offset = visual_schedule.estimates(&app.graph, |estimates| {
                show_cursor_to_insert_at_idx(ui, 0);
                let iter = estimates.into_iter().copied().enumerate();
                iter.fold(None, |acc, (idx, (estimate, entry))| {
                    let ret = row_ui(
                        &visual_schedule,
                        estimates,
                        estimate,
                        tile.editing_offset,
                        entry,
                        &app,
                        ui,
                    );
                    ui.end_row();
                    show_cursor_to_insert_at_idx(ui, idx + 1);
                    acc.or(ret)
                })
            });
            if new_editing_offset.is_none()
                && let Some((id, offset)) = tile.editing_offset
            {
                let _ = app.mutate(|mut world| {
                    let trip = world.trips.get_mut(&trip_key).ok_or("??")?;
                    match offset {
                        Either::Left(d) => trip.schedule.shift_arr_or_pass(id, d),
                        Either::Right(d) => trip.schedule.shift_dep(id, d),
                    }
                    Ok(world)
                });
            };
            tile.editing_offset = new_editing_offset;
            if matches!(app.selected_items, SelectedItems::TripExtension(_))
                && !ui.ui_contains_pointer()
            {
                ui.label("Extending...")
                    .scroll_to_me_animation(Some(Align::Max), style::ScrollAnimation::none());
            }
        });
    });
}

fn select_a_trip(ui: &mut Ui, existing_key: &mut Option<TripKey>, world: &World) {
    let selected_text = existing_key.map_or("Current selected trip", |key| {
        world.trips.get(&key).map_or("Unknown", |wfc| &wfc.name)
    });
    let combo_box = ComboBox::from_id_salt("Selected existing trip").selected_text(selected_text);
    combo_box.show_ui(ui, |ui| {
        ui.selectable_value(existing_key, None, "Current selected trip");
        ScrollArea::vertical().show_rows(
            ui,
            ui.spacing().interact_size.y,
            world.trips.len(),
            |ui, range| {
                for (key, trip) in world.trips.iter().skip(range.start).take(range.count()) {
                    ui.selectable_value(existing_key, Some(*key), trip.name.as_str());
                }
            },
        );
    });
}

fn row_ui(
    schedule: &TripSchedule,
    estimates: &[(Option<TEstimate>, EstimateEntry)],
    estimate: Option<TEstimate>,
    edit_offset: Option<(TEntryId, Either<TDuration, TDuration>)>,
    entry: EstimateEntry,
    world: &World,
    ui: &mut Ui,
) -> Option<(TEntryId, Either<TDuration, TDuration>)> {
    // TODO: change this to station name instead
    let Some(node_name) = world.graph.nodes().get(&entry.node_key()).map(|wfc| wfc.name.as_str())
    else {
        ui.label("Unknown Station");
        return None;
    };
    ui.label(node_name);
    let Some(estimate) = estimate else {
        ui.label("No estimate");
        return None;
    };
    fn fmt_time_label(time: TimetableTime) -> WidgetText {
        WidgetText::from(time.to_string()).text_style(TIMETABLTE_TEXT_STYLE.clone()).weak()
    }
    let EstimateEntry::Pinned(entry) = entry else {
        ui.label(fmt_time_label(estimate.arr));
        ui.label(RichText::from("⇂").weak());
        return None;
    };
    let (mut arr_or_pass_offset, mut dep_offset) =
        match edit_offset.and_then(|(id, offset)| (id == entry.id).then_some(offset)) {
            Some(Either::Left(inner)) => (inner, TDuration::ZERO),
            Some(Either::Right(inner)) => (TDuration::ZERO, inner),
            None => (TDuration::ZERO, TDuration::ZERO),
        };
    let arr_or_pass_resp = match entry.arr_or_pass {
        TravelMode::At(t) => ui.add(SeparateDisplayDragValue {
            modified: &mut arr_or_pass_offset,
            displayed: t,
        }),
        TravelMode::For(d) => ui.add(SeparateDisplayDragValue {
            modified: &mut arr_or_pass_offset,
            displayed: d,
        }),
        TravelMode::Flexible => ui.label(fmt_time_label(estimate.arr)),
    };
    let dep_resp = match entry.dep {
        Some(TravelMode::At(t)) => ui.add(SeparateDisplayDragValue {
            modified: &mut dep_offset,
            displayed: t,
        }),
        Some(TravelMode::For(d)) => ui.add(SeparateDisplayDragValue {
            modified: &mut dep_offset,
            displayed: d,
        }),
        Some(TravelMode::Flexible) => ui.label(fmt_time_label(estimate.arr)),
        None => ui.label(RichText::from("⇂").weak()),
    };
    arr_or_pass_resp
        .dragged()
        .then_some(Either::Left(arr_or_pass_offset))
        .or(dep_resp.dragged().then_some(Either::Right(dep_offset)))
        .map(|offset| (entry.id, offset))
}

fn show_cursor_to_insert_at_idx(ui: &mut Ui, idx: usize) -> Option<NodeKey> {
    let left = ui.min_rect().left_bottom();
    let right = ui.min_rect().right_bottom() + vec2(10.0, 0.0);
    let rect = Rect::from_pos(right).expand(8.0);
    let resp = ui
        .interact(
            rect,
            ui.scope_id().with(("insert entry", idx)),
            Sense::click(),
        )
        .on_hover_text("Insert entry to timetable at this position");
    let mut stroke = Stroke::new(2.0, Color32::TRANSPARENT);
    if resp.contains_pointer() {
        stroke.color = Color32::RED;
    }
    if resp.is_pointer_button_down_on() {
        stroke.color = Color32::DARK_RED;
    }
    {
        let stroke = if stroke.color == Color32::TRANSPARENT {
            Stroke::new(1.0, ui.visuals().strong_text_color())
        } else {
            stroke
        };
        let rect = rect.shrink(4.0);
        ui.painter().line_segment([rect.center_top(), rect.center_bottom()], stroke);
        ui.painter().line_segment([rect.right_center(), rect.left_center()], stroke);
    }
    ui.painter().line_segment([left, rect.left_center()], stroke);
    None
}
