use egui::*;
use paiagram_core::trip::TravelMode;
use paiagram_core::{Key, ServiceClassKey};
use serde::{Deserialize, Serialize};

use super::Tile;
use crate::tiles::trip::TripTile;
use crate::{App, UiTile, UiCommand};

#[derive(Default, Clone, Serialize, Deserialize)]
pub(crate) struct ServiceClassesTile {
    #[serde(skip)]
    selected_class: Option<ServiceClassKey>,
}

impl PartialEq for ServiceClassesTile {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Tile for ServiceClassesTile {
    const NAME: &'static str = "Service Classes";
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        puffin::profile_function!();
        Panel::left("service classes left panel").frame(Frame::NONE).show(ui, |ui| {
            puffin::profile_scope!("service_class_list");
            let spacing = ui.spacing().interact_size.y;
            let rows = app.service_classes.len();
            let scroll_area = ScrollArea::vertical().auto_shrink(false);
            scroll_area.show_rows(ui, spacing, rows, |ui, row_range| {
                let entries =
                    (&app.service_classes).iter().skip(row_range.start).take(row_range.count());
                let show_entries = |ui: &mut Ui, selected| {
                    entries.fold(selected, |selected, (&sc_key, service_class)| {
                        let service_class_str = service_class.name.as_str();
                        let stroke_atom = Atom::paint([35.0, 15.0], |ui, args| {
                            let rect = args.rect;
                            let stroke = Stroke::new(
                                service_class.style.width as f32 * 1.0,
                                service_class.style.color,
                            );
                            ui.painter()
                                .line_segment([rect.left_center(), rect.right_center()], stroke);
                            let stroke = Stroke::new(2.0, service_class.style.color);
                            ui.painter()
                                .line_segment([rect.left_top(), rect.left_bottom()], stroke);
                        })
                        .atom_grow(false)
                        .atom_shrink(false)
                        .atom_align(Align2::LEFT_CENTER);
                        let class_name_atom = Atom::from(service_class_str)
                            .atom_grow(true)
                            .atom_shrink(true)
                            .atom_align(Align2::LEFT_CENTER);
                        egui::Button::selectable(
                            selected == Some(sc_key),
                            (stroke_atom, class_name_atom),
                        )
                        .truncate()
                        .atom_ui(ui)
                        .clicked()
                        .then_some(sc_key)
                        .or(selected)
                    })
                };
                Frame::new().inner_margin(6.0).show(ui, |ui| {
                    self.selected_class = ui
                        .with_layout(Layout::top_down_justified(egui::Align::Min), |ui| {
                            show_entries(ui, self.selected_class)
                        })
                        .inner;
                });
            });
        });
        CentralPanel::default_margins().show(ui, |ui| {
            puffin::profile_scope!("service_class_details");
            let Some(selected_class_key) = self.selected_class else {
                ui.centered_and_justified(|ui| ui.heading("Nothing focused"));
                return;
            };
            let Some(selected_class) = app.service_classes.get(&selected_class_key) else {
                ui.centered_and_justified(|ui| ui.heading("Service class does not exist"));
                return;
            };
            ui.heading(selected_class.name.as_str());
            Grid::new(selected_class.name.as_str()).num_columns(2).show(ui, |ui| {
                ui.label("Creation time");
                let time_string = jiff::Zoned::try_from(selected_class_key.creation_time())
                    .map_or("Unknown".into(), |zoned| {
                        format!("{}", zoned.strftime("%F %T %:z"))
                    });
                ui.label(time_string);
                ui.end_row();
            });
            let mut class_trips: Vec<_> = app
                .trips
                .iter()
                .filter(|(_, trip)| trip.service_class == Some(selected_class_key))
                .filter_map(|(&trip_key, trip)| {
                    if let Some(entry) = trip.schedule.entries().first()
                        && let Some(t) = match (entry.arr_or_pass, entry.dep) {
                            (TravelMode::At(t), _) => Some(t),
                            (_, Some(TravelMode::At(t))) => Some(t),
                            _ => None,
                        }
                    {
                        Some((trip_key, trip.name.as_str(), t))
                    } else {
                        None
                    }
                })
                .collect();
            class_trips.sort_unstable_by_key(|(_, _, t)| *t);

            // ugly solution, but interesting
            let scroll_area = ScrollArea::vertical().show(ui, |ui| {
                let chunks = class_trips.chunk_by(|(_, _, a), (_, _, b)| a.hour() == b.hour());
                chunks.fold(None, |acc, chunk| {
                    let Some(first) = chunk.first() else {
                        return acc;
                    };
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:00", first.2.hour()));
                        ui.add(Separator::default().horizontal());
                    });
                    ui.horizontal_wrapped(|ui| {
                        chunk.iter().fold(acc, |acc, &(trip_key, name, time)| {
                            let resp = ui.button(name).clicked();
                            acc.or(resp.then(|| {
                                UiCommand::OpenOrFocus(UiTile::Trip(TripTile::new(trip_key)))
                            }))
                        })
                    })
                    .inner
                })
            });
            if let Some(action) = scroll_area.inner {
                app.ui_action_tx.send(action);
            }
        });
    }
}
