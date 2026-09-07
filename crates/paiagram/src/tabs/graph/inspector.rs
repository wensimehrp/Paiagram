//! Editing controls queue reversible commands; route and trip builders keep local drafts.
use super::*;

impl GraphTab {
    pub(super) fn inspector(&mut self, app: &mut App, ui: &mut Ui) {
        ui.heading("Network");
        if let Some(error) = app.command_error.clone() {
            ui.colored_label(ui.visuals().error_fg_color, error);
            if ui.button("Dismiss").clicked() {
                app.command_error = None;
            }
        }
        ui.add(&mut self.underlay_tile_type);
        if let Some(pos) = self.coordinate {
            ui.separator();
            ui.label(Wgs84LonLat::from(pos).to_string());
            ui.text_edit_singleline(&mut self.name);
            if ui.button("Create station + platform").clicked() {
                let station = StationKey::new();
                let node = NodeKey::new();
                let name = if self.name.trim().is_empty() {
                    "New station".into()
                } else {
                    self.name.clone().into()
                };
                app.command_queue.push(Command::Macro(Box::new([
                    Command::AddStation {
                        key: station,
                        info: StationInfo { name, pos },
                    },
                    Command::AddNode {
                        key: node,
                        info: NodeInfo {
                            name: "1".into(),
                            parent: station,
                            pos,
                            is_platform: true,
                        },
                    },
                ])));
                self.coordinate = None;
                self.selected = Some(Hit::Station(station));
                self.name.clear();
            }
        }
        match self.selected {
            Some(Hit::Station(key)) => {
                if let Some((mut name, pos, nodes)) =
                    app.stations.query(key, |v| (v.name.to_string(), *v.pos, v.nodes.clone()))
                {
                    ui.separator();
                    ui.label("Station");
                    if ui.text_edit_singleline(&mut name).changed() {
                        app.command_queue.push(Command::RenameStation {
                            key,
                            name: name.into(),
                        });
                    }
                    ui.small(Wgs84LonLat::from(pos).to_string());
                    ui.horizontal(|ui| {
                        for (label, platform) in [("Add platform", true), ("Add switch", false)] {
                            if ui.button(label).clicked() {
                                let point = project(pos);
                                let offset = 15.0 * (nodes.len() + 1) as f64;
                                let pos = Wgs84LonLat::from(XyPosF64::new(
                                    point[0] + offset,
                                    point[1] + 15.0,
                                ))
                                .into();
                                app.command_queue.push(Command::AddNode {
                                    key: NodeKey::new(),
                                    info: NodeInfo {
                                        name: if platform {
                                            format!("{}", nodes.len() + 1).into()
                                        } else {
                                            "Switch".into()
                                        },
                                        parent: key,
                                        pos,
                                        is_platform: platform,
                                    },
                                });
                            }
                        }
                    });
                    for node in nodes {
                        if let Some(label) = app.nodes.query(node, |v| {
                            format!(
                                "{} {}",
                                if *v.is_platform { "Platform" } else { "Switch" },
                                v.name
                            )
                        }) {
                            if ui.selectable_label(false, label).clicked() {
                                self.select(app, Hit::Node(node));
                            }
                        }
                    }
                    if ui.button("Append to route draft").clicked() {
                        self.route_stations.push(key);
                    }
                    let removable = !app.nodes.iter().any(|n| *n.parent == key)
                        && !app.routes.iter().any(|r| {
                            r.stations
                                .iter()
                                .any(|s| matches!(s.stn,StationRecord::All(k) if k==key))
                        });
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete station"))
                        .on_disabled_hover_text("Remove its nodes and route references first.")
                        .clicked()
                    {
                        app.command_queue.push(Command::RemoveStation { key });
                    }
                }
            }
            Some(Hit::Node(key)) => {
                if let Some(mut info) = app.nodes.query(key, |v| NodeInfo {
                    name: v.name.clone(),
                    parent: *v.parent,
                    pos: *v.pos,
                    is_platform: *v.is_platform,
                }) {
                    ui.separator();
                    ui.label("Node");
                    let mut name = info.name.to_string();
                    let mut changed = ui.text_edit_singleline(&mut name).changed();
                    info.name = name.into();
                    changed |= ui.checkbox(&mut info.is_platform, "Platform (can stop)").changed();
                    egui::ComboBox::from_id_salt("node parent")
                        .selected_text(
                            app.stations
                                .query(info.parent, |v| v.name.to_string())
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for station in app.stations.iter() {
                                changed |= ui
                                    .selectable_value(
                                        &mut info.parent,
                                        station.key,
                                        station.name.as_str(),
                                    )
                                    .changed();
                            }
                        });
                    if changed {
                        app.command_queue.push(Command::ChangeNode {
                            key,
                            info: info.clone(),
                        });
                    }
                    if ui.button("Select parent station").clicked() {
                        self.select(app, Hit::Station(info.parent));
                    }
                    ui.small(Wgs84LonLat::from(info.pos).to_string());
                    if ui.button("Append to trip draft").clicked() {
                        self.trip_nodes.push(key);
                    }
                    ui.label("Shift-click another node to connect both directions.");
                    if let Some(edges) = app.nodes.query(key, |v| {
                        v.outgoing
                            .iter()
                            .map(|n| (key, *n))
                            .chain(v.incoming.iter().map(|n| (*n, key)))
                            .collect::<Vec<_>>()
                    }) {
                        for edge in edges {
                            let label = format!(
                                "{} → {}",
                                node_label(app.snap(), edge.0),
                                node_label(app.snap(), edge.1)
                            );
                            if ui.button(label).clicked() {
                                self.select(app, Hit::Interval(edge));
                            }
                        }
                    }
                    let removable = !app.intervals.keys().any(|(a, b)| *a == key || *b == key)
                        && !app
                            .trips
                            .iter()
                            .any(|t| t.schedule.entries().iter().any(|e| e.node_key() == key))
                        && !app.routes.iter().any(|r| {
                            r.stations.iter().any(|s| {
                                s.prev_curr_nodes.contains(&key)
                                    || s.curr_prev_nodes.contains(&key)
                                    || matches!(&s.stn,StationRecord::Some(ns) if ns.contains(&key))
                            })
                        });
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete node"))
                        .on_disabled_hover_text(
                            "Remove interval, trip, and route references first.",
                        )
                        .clicked()
                    {
                        app.command_queue.push(Command::RemoveNode { key });
                    }
                }
            }
            Some(Hit::Interval(key)) => {
                if let Some(mut info) = app.intervals.get(key).cloned() {
                    ui.separator();
                    ui.label("Directed interval");
                    ui.label(format!(
                        "{} → {}",
                        node_label(app.snap(), key.0),
                        node_label(app.snap(), key.1)
                    ));
                    if app.intervals.contains_key((key.1, key.0))
                        && ui.button("Select reverse direction").clicked()
                    {
                        self.select(app, Hit::Interval((key.1, key.0)));
                    }
                    let mut automatic = info.length.is_none();
                    let mut changed =
                        ui.checkbox(&mut automatic, "Calculate length from geometry").changed();
                    let mut metres = info.length().0.max(1) as u32;
                    if !automatic {
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut metres)
                                    .range(1..=i32::MAX as u32)
                                    .suffix(" m"),
                            )
                            .changed();
                    }
                    if changed {
                        info.length = if automatic {
                            None
                        } else {
                            std::num::NonZeroU32::new(metres)
                        };
                        app.command_queue.push(Command::ChangeInterval {
                            key,
                            info: info.clone(),
                        });
                    }
                    ui.label(format!("{} trips", info.trips.len()));
                    if ui.button("Delete interval").clicked() {
                        app.command_queue.push(Command::RemoveInterval { key });
                    }
                }
            }
            Some(Hit::Trip(key)) => {
                if let Some((mut name, mut class, entries)) = app.trips.query(key, |v| {
                    (
                        v.name.to_string(),
                        *v.service_class,
                        v.schedule.entries().to_vec(),
                    )
                }) {
                    ui.separator();
                    ui.label("Trip");
                    if ui.text_edit_singleline(&mut name).changed() {
                        app.command_queue.push(Command::RenameTrip {
                            key,
                            name: name.into(),
                        });
                    }
                    let old = class;
                    egui::ComboBox::from_id_salt("trip class")
                        .selected_text(
                            class
                                .and_then(|k| app.service_classes.query(k, |v| v.name.to_string()))
                                .unwrap_or_else(|| "Default".into()),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut class, None, "Default");
                            for c in app.service_classes.iter() {
                                ui.selectable_value(&mut class, Some(c.key), c.name.as_str());
                            }
                        });
                    if class != old {
                        app.command_queue.push(Command::ChangeTripClass { key, class });
                    }
                    if ui.button("Open timetable").clicked() {
                        app.ui_action_queue
                            .push(UiCommand::OpenOrFocus(MainTab::Trip(TripTab::new(key))));
                    }
                    for entry in entries {
                        ui.horizontal(|ui| {
                            ui.label(
                                app.nodes
                                    .query(entry.node_key(), |v| v.name.to_string())
                                    .unwrap_or_default(),
                            );
                            if ui.small_button("Remove stop").clicked() {
                                app.command_queue.push(Command::RemoveTripEntry {
                                    key,
                                    id: entry.id(),
                                });
                            }
                        });
                    }
                    if ui.button("Delete trip").clicked() {
                        app.command_queue.push(Command::RemoveTrip { key });
                    }
                }
            }
            None => {}
        }
        ui.separator();
        ui.collapsing("Route draft", |ui| {
            ui.small("Select station labels, then append them in route order.");
            self.route_records.truncate(self.route_stations.len());
            while self.route_records.len() < self.route_stations.len() {
                let i = self.route_records.len();
                self.route_records.push(RouteStationRecord::for_station(
                    app.snap(),
                    self.route_stations[i],
                    i.checked_sub(1).map(|p| self.route_stations[p]),
                ));
            }
            for (index, (key, record)) in
                self.route_stations.iter().zip(self.route_records.iter_mut()).enumerate()
            {
                let name = app.stations.query(*key, |v| v.name.to_string()).unwrap_or_default();
                ui.push_id(index, |ui| {
                    ui.collapsing(format!("{}. {name}", index + 1), |ui| {
                        let mut all = matches!(record.stn, StationRecord::All(_));
                        if ui.checkbox(&mut all, "All platforms").changed() {
                            record.stn = if all {
                                StationRecord::All(*key)
                            } else {
                                StationRecord::Some(
                                    app.stations
                                        .query(*key, |s| {
                                            s.nodes
                                                .iter()
                                                .copied()
                                                .filter(|n| {
                                                    app.nodes
                                                        .query(*n, |n| *n.is_platform)
                                                        .unwrap_or(false)
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                )
                            };
                        }
                        if let StationRecord::Some(nodes) = &mut record.stn {
                            for node in
                                app.nodes.iter().filter(|n| *n.parent == *key && *n.is_platform)
                            {
                                let mut selected = nodes.contains(&node.key);
                                if ui.checkbox(&mut selected, node.name.as_str()).changed() {
                                    if selected {
                                        nodes.push(node.key);
                                    } else {
                                        nodes.retain(|n| *n != node.key);
                                    }
                                }
                            }
                        }
                        let mut milestone = record.milestone.is_some();
                        if ui.checkbox(&mut milestone, "Custom milestone").changed() {
                            record.milestone = milestone.then_some(Distance(0));
                        }
                        if let Some(value) = &mut record.milestone {
                            ui.add(egui::DragValue::new(&mut value.0).suffix(" m"));
                        }
                        let mut length = record.canvas_length.is_some();
                        if ui.checkbox(&mut length, "Custom canvas length").changed() {
                            record.canvas_length = length.then_some(CanvasLength(100.0));
                        }
                        if let Some(value) = &mut record.canvas_length {
                            ui.add(egui::DragValue::new(&mut value.0).range(1.0..=100000.0));
                        }
                        if index > 0 {
                            ui.collapsing("Intermediate nodes", |ui| {
                                for (label, nodes) in [
                                    ("From previous station", &mut record.prev_curr_nodes),
                                    ("To previous station", &mut record.curr_prev_nodes),
                                ] {
                                    ui.collapsing(label, |ui| {
                                        for node in app.nodes.iter() {
                                            let mut selected = nodes.contains(&node.key);
                                            let station = app
                                                .stations
                                                .query(*node.parent, |s| s.name.to_string())
                                                .unwrap_or_default();
                                            if ui
                                                .checkbox(
                                                    &mut selected,
                                                    format!("{station} / {}", node.name),
                                                )
                                                .changed()
                                            {
                                                if selected {
                                                    nodes.push(node.key);
                                                } else {
                                                    nodes.retain(|n| *n != node.key);
                                                }
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    });
                });
            }
            ui.horizontal(|ui| {
                if ui.button("Remove last").clicked() {
                    self.route_stations.pop();
                }
                if ui.button("Clear").clicked() {
                    self.route_stations.clear();
                    self.route_records.clear();
                    self.route = None;
                }
            });
            if ui
                .add_enabled(
                    self.route_stations.len() >= 2,
                    egui::Button::new(if self.route.is_some() {
                        "Save route"
                    } else {
                        "Create route"
                    }),
                )
                .clicked()
            {
                let stations = self
                    .route_stations
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        self.route_records
                            .get(i)
                            .filter(|r| match &r.stn {
                                StationRecord::All(station) => station == k,
                                StationRecord::Some(nodes) => {
                                    nodes.first().and_then(|n| app.nodes.query(*n, |v| *v.parent))
                                        == Some(*k)
                                }
                            })
                            .cloned()
                            .unwrap_or_else(|| {
                                RouteStationRecord::for_station(
                                    app.snap(),
                                    *k,
                                    i.checked_sub(1).map(|p| self.route_stations[p]),
                                )
                            })
                    })
                    .collect();
                if let Some(key) = self.route {
                    app.command_queue.push(Command::ChangeRouteStations { key, stations });
                } else {
                    let key = RouteKey::new();
                    app.command_queue.push(Command::AddRoute {
                        key,
                        info: RouteInfo {
                            name: "New route".into(),
                            stations,
                        },
                    });
                    self.route = Some(key);
                }
            }
            let routes: Vec<_> =
                app.routes.iter().map(|v| (v.key, v.name.clone(), v.stations.clone())).collect();
            for (key, name, stations) in routes {
                if ui.selectable_label(self.route == Some(key), name.as_str()).clicked() {
                    self.route = Some(key);
                    self.route_records = stations.to_vec();
                    self.route_stations = stations
                        .iter()
                        .filter_map(|s| match &s.stn {
                            StationRecord::All(k) => Some(*k),
                            StationRecord::Some(ns) => {
                                ns.first().and_then(|k| app.nodes.query(*k, |v| *v.parent))
                            }
                        })
                        .collect();
                }
            }
            if let Some(key) = self.route {
                if let Some(mut name) = app.routes.query(key, |v| v.name.to_string()) {
                    if ui.text_edit_singleline(&mut name).changed() {
                        app.command_queue.push(Command::RenameRoute {
                            key,
                            name: name.into(),
                        });
                    }
                }
                if ui.button("Open diagram").clicked() {
                    app.ui_action_queue.push(UiCommand::OpenOrFocus(MainTab::Diagram(
                        crate::tabs::diagram::DiagramTab::new(key),
                    )));
                }
                if ui.button("Delete route").clicked() {
                    app.command_queue.push(Command::RemoveRoute { key });
                    self.route = None;
                }
            }
        });
        ui.collapsing("Trip draft",|ui|{
            ui.small("Append connected nodes in travel order. New trips start at the current clock time, five minutes per leg.");
            for key in &self.trip_nodes {ui.label(app.nodes.query(*key,|v|v.name.to_string()).unwrap_or_default());}
            if ui.button("Remove last node").clicked(){self.trip_nodes.pop();}
            let connected=self.trip_nodes.windows(2).all(|p|app.intervals.contains_key((p[0],p[1])));
            if ui.add_enabled(
                self.trip_nodes.len() >= 2 && connected,
                egui::Button::new("Create trip")
            ).on_disabled_hover_text("Add at least two nodes with directed intervals between them.").clicked(){
                let start=app.timer.ticks().to_timetable_time();
                let entries = self
                    .trip_nodes
                    .iter()
                    .enumerate()
                    .map(|(i,k)|
                        TEntry::PinnedNonStop {
                            node: *k,
                            pass: TravelMode::At(start + time::TDuration::from_hms(0,0,i as i32*300)),
                            external: false,
                            id: TEntryId::new()
                        }).collect();
                let key=TripKey::new();
                app.command_queue.push(
                    Command::AddTrip{
                        key,
                        info: TripInfo{
                            name: "New trip".into(),
                            schedule: TripSchedule::new(entries),
                            service_class: None,
                            vehicles:Default::default()
                        }
                    });
                self.trip_nodes.clear();
                self.selected = Some(Hit::Trip(key));
            }
        });
        ui.collapsing("All stations",|ui| {
            let stations: Vec<_> = app.stations.iter().map(|v|(v.key,v.name.to_string(),*v.pos)).collect();
            for (key,name,_) in &stations {
                if ui.selectable_label(self.selected==Some(Hit::Station(*key)),name).clicked() { self.select(app,Hit::Station(*key)); }
            }
            if ui.button("Lay out stations at (0°, 0°)").on_hover_text("Place stations with missing coordinates on a grid. This changes their coordinates and can be undone.").clicked() {
                let commands: Vec<_> = stations.iter().filter(|(_,_,p)| *p==LonLat::ZERO).enumerate().map(|(i,(key,_,_))| move_command(app.snap(),Hit::Station(*key),Wgs84LonLat::from(XyPosF64::new((i%8) as f64*1000.0,(i/8) as f64*1000.0)).into())).collect();
                if !commands.is_empty() { app.command_queue.push(Command::Macro(commands.into_boxed_slice())); self.fitted=false; }
            }
        });
        ui.collapsing("All trips", |ui| {
            let trips: Vec<_> = app.trips.iter().map(|v| (v.key, v.name.clone())).collect();
            for (key, name) in trips {
                if ui
                    .selectable_label(self.selected == Some(Hit::Trip(key)), name.as_str())
                    .clicked()
                {
                    self.select(app, Hit::Trip(key));
                }
            }
        });
    }
}
