use super::*;
use crate::time::TDuration;
use crate::trip::{TEntry, TEntryId};

#[derive(Clone, Debug)]
pub enum Command {
    // trips
    AddTrip {
        key: TripKey,
        info: TripInfo,
    },
    RenameTrip {
        key: TripKey,
        name: EcoString,
    },
    ChangeTripClass {
        key: TripKey,
        class: Option<ServiceClassKey>,
    },
    RemoveTrip {
        key: TripKey,
    },
    // trip entry
    ShiftTripEntryArrOrPass {
        key: TripKey,
        id: TEntryId,
        dur: TDuration,
    },
    ShiftTripEntryDep {
        key: TripKey,
        id: TEntryId,
        dur: TDuration,
    },
    ChangeTripEntry {
        key: TripKey,
        id: TEntryId,
        new_entry: TEntry,
    },
    RemoveTripEntry {
        key: TripKey,
        id: TEntryId,
    },
    InsertTripEntry {
        key: TripKey,
        entry: TEntry,
        pos: usize,
    },
    // vehicles
    AddVehicle {
        key: VehicleKey,
        name: EcoString,
    },
    RenameVehicle {
        key: VehicleKey,
        name: EcoString,
    },
    RemoveVehicle {
        key: VehicleKey,
    },
    // Stations
    AddStation {
        key: StationKey,
        info: StationInfo,
    },
    RenameStation {
        key: StationKey,
        name: EcoString,
    },
    RemoveStation {
        key: StationKey,
    },
    // nodes
    AddNode {
        key: NodeKey,
        info: NodeInfo,
    },
    RenameNode {
        key: NodeKey,
        name: EcoString,
    },
    RemoveNode {
        key: NodeKey,
    },
    // classes
    AddServiceClass {
        key: ServiceClassKey,
        info: ServiceClassInfo,
    },
    RenameServiceClass {
        key: ServiceClassKey,
        name: EcoString,
    },
    RemoveServiceClass {
        key: ServiceClassKey,
    },
    // route
    AddRoute {
        key: RouteKey,
        info: RouteInfo,
    },
    RenameRoute {
        key: RouteKey,
        name: EcoString,
    },
    RemoveRoute {
        key: RouteKey,
    },
    // interval
    AddInterval {
        key: IntervalKey,
        info: Interval,
    },
    RemoveInterval {
        key: IntervalKey,
    },
    /// Change the vehicles that serve a trip.
    ChangeTripVehicles {
        key: TripKey,
        vehicles: SmallVec<[VehicleKey; 1]>,
    },
    ChangeStation {
        key: StationKey,
        info: StationInfo,
    },
    ChangeNode {
        key: NodeKey,
        info: NodeInfo,
    },
    ChangeRouteStations {
        key: RouteKey,
        stations: EcoVec<RouteStationRecord>,
    },
    ChangeInterval {
        key: IntervalKey,
        info: Interval,
    },
    ChangeServiceClassStyle {
        key: ServiceClassKey,
        style: StrokeStyle,
    },
    // World related stuff
    UnloadWorld,
    LoadWorld {
        snapshot: Box<WorldSnapshot>,
    },
    /// A user-defined macro.
    Macro(Box<[Command]>),
}

impl Command {
    pub fn new_empty() -> Self {
        Self::Macro(Box::new([]))
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Macro(inner) if inner.is_empty() => true,
            _ => false,
        }
    }
}

impl WorldSnapshot {
    fn valid_trip(&self, info: &TripInfo) -> bool {
        info.service_class
            .is_none_or(|k| self.service_classes.contains_key(k))
            && info.vehicles.iter().all(|k| self.vehicles.contains_key(*k))
            && info.schedule.entries().iter().enumerate().all(|(i, e)| {
                self.nodes.contains_key(e.node_key())
                    && !info.schedule.entries()[..i]
                        .iter()
                        .any(|prev| prev.id() == e.id())
            })
    }

    fn valid_route_stations(&self, records: &[RouteStationRecord]) -> bool {
        let mut seen = Vec::new();
        records.iter().enumerate().all(|(index, r)| {
            let station = match &r.stn {
                StationRecord::All(k) => Some(*k),
                StationRecord::Some(nodes) => nodes
                    .first()
                    .and_then(|k| self.nodes.query(*k, |v| *v.parent)),
            };
            let Some(station) = station else {
                return false;
            };
            if seen.contains(&station)
                && !(index + 1 == records.len() && seen.first() == Some(&station) && index > 1)
            {
                return false;
            }
            seen.push(station);
            if r.canvas_length
                .is_some_and(|n| !n.0.is_finite() || n.0 <= 0.0)
            {
                return false;
            }

            let valid = match &r.stn {
                StationRecord::All(k) => self.stations.contains_key(*k),
                StationRecord::Some(nodes) => {
                    !nodes.is_empty()
                        && nodes.iter().all(|k| {
                            self.nodes
                                .query(*k, |v| {
                                    *v.is_platform
                                        && self
                                            .nodes
                                            .query(nodes[0], |first| first.parent == v.parent)
                                            .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                }
            };
            valid
                && r.prev_curr_nodes
                    .iter()
                    .chain(r.curr_prev_nodes.iter())
                    .all(|k| self.nodes.contains_key(*k))
        })
    }

    /// Applies a command and returns its inverse. Could modify the world and return the inverse if
    /// the application succeeds; doesn't modify the world and returns None if the application
    /// fails.
    pub fn apply_command(&mut self, cmd: Command) -> Option<Command> {
        let inverse = self.apply_command_inner(cmd)?;
        self.rebuild_caches();
        Some(inverse)
    }

    fn apply_command_inner(&mut self, cmd: Command) -> Option<Command> {
        match cmd {
            Command::AddTrip { key, info } => {
                (!self.trips.contains_key(key) && self.valid_trip(&info)).then(|| {
                    self.cache_trip(key, &info.vehicles);
                    self.trips.insert(key, info.into());
                    Command::RemoveTrip { key }
                })
            }
            Command::RenameTrip {
                key,
                name: mut new_name,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameTrip {
                    key,
                    name: new_name,
                }
            }),
            Command::ChangeTripClass {
                key,
                class: mut new_class,
            } => {
                if new_class.is_some_and(|k| !self.service_classes.contains_key(k)) {
                    return None;
                }
                self.trips.update(key, |mut view| {
                    std::mem::swap(view.service_class.get_mut(), &mut new_class);
                    Command::ChangeTripClass {
                        key,
                        class: new_class,
                    }
                })
            }
            Command::RemoveTrip { key } => self.trips.remove(key).map(|view| {
                // Drop the trip from the serving vehicles' caches.
                self.uncache_trip(key, &view.vehicles);
                Command::AddTrip {
                    key,
                    info: view.into(),
                }
            }),
            Command::ShiftTripEntryArrOrPass { key, id, dur } => self
                .trips
                .update(key, |mut v| {
                    let sch = v.schedule.get_mut().entries_mut();
                    let entry = sch.make_mut().iter_mut().find(|ent| ent.id() == id)?;
                    let mode = entry.arr_or_pass_mut()?;
                    *mode = mode.shifted_by(dur);
                    Some(Command::ShiftTripEntryArrOrPass { key, id, dur: -dur })
                })
                .flatten(),
            Command::ShiftTripEntryDep { key, id, dur } => self
                .trips
                .update(key, |mut v| {
                    let sch = v.schedule.get_mut().entries_mut();
                    let entry = sch.make_mut().iter_mut().find(|ent| ent.id() == id)?;
                    let mode = entry.dep_mut()?;
                    *mode = mode.shifted_by(dur);
                    Some(Command::ShiftTripEntryDep { key, id, dur: -dur })
                })
                .flatten(),
            Command::ChangeTripEntry { key, id, new_entry } => {
                if new_entry.id() != id || !self.nodes.contains_key(new_entry.node_key()) {
                    return None;
                }
                self.trips
                    .update(key, |mut v| {
                        let entries = v.schedule.get_mut().entries_mut();
                        let pos = entries.iter().position(|e| e.id() == id)?;
                        let old = std::mem::replace(&mut entries.make_mut()[pos], new_entry);
                        Some(Command::ChangeTripEntry {
                            key,
                            id,
                            new_entry: old,
                        })
                    })
                    .flatten()
            }
            Command::RemoveTripEntry { key, id } => self
                .trips
                .update(key, |mut v| {
                    let entries = v.schedule.get_mut().entries_mut();
                    let pos = entries.iter().position(|e| e.id() == id)?;
                    let entry = entries.remove(pos);
                    Some(Command::InsertTripEntry { key, entry, pos })
                })
                .flatten(),
            Command::InsertTripEntry { key, entry, pos } => {
                if !self.nodes.contains_key(entry.node_key()) {
                    return None;
                }
                self.trips
                    .update(key, |mut v| {
                        let entries = v.schedule.get_mut().entries_mut();
                        if pos > entries.len() || entries.iter().any(|e| e.id() == entry.id()) {
                            return None;
                        }
                        entries.insert(pos, entry);
                        Some(Command::RemoveTripEntry {
                            key,
                            id: entry.id(),
                        })
                    })
                    .flatten()
            }
            Command::AddVehicle { key, name } => (!self.vehicles.contains_key(key)).then(|| {
                self.vehicles.insert(
                    key,
                    VehicleView {
                        name,
                        trips: EcoVec::new(),
                    },
                );
                Command::RemoveVehicle { key }
            }),
            Command::RenameVehicle {
                key,
                name: mut new_name,
            } => self.vehicles.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameVehicle {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveVehicle { key } => {
                let mut restore = Vec::new();
                let vehicle = self.vehicles.remove(key)?;
                restore.push(Command::AddVehicle {
                    key,
                    name: vehicle.name,
                });
                let trips: Vec<_> = self
                    .trips
                    .iter()
                    .filter(|v| v.vehicles.contains(&key))
                    .map(|v| v.key)
                    .collect();
                for trip in trips {
                    self.trips.update(trip, |mut v| {
                        restore.push(Command::ChangeTripVehicles {
                            key: trip,
                            vehicles: v.vehicles.get().clone(),
                        });
                        v.vehicles.get_mut().retain(|v| *v != key);
                    });
                }
                Some(Command::Macro(restore.into_boxed_slice()))
            }
            Command::AddStation { key, info } => (!self.stations.contains_key(key)).then(|| {
                self.stations.insert(key, info.into());
                Command::RemoveStation { key }
            }),
            Command::RenameStation {
                key,
                name: mut new_name,
            } => self.stations.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameStation {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveStation { key } => {
                if self.nodes.iter().any(|v| *v.parent == key)
                    || self.routes.iter().any(|v| {
                        v.stations
                            .iter()
                            .any(|r| matches!(r.stn, StationRecord::All(k) if k == key))
                    })
                {
                    return None;
                }
                self.stations.remove(key).map(|view| Command::AddStation {
                    key,
                    info: view.into(),
                })
            }
            Command::AddNode { key, info } => (!self.nodes.contains_key(key)
                && self.stations.contains_key(info.parent))
            .then(|| {
                self.nodes.insert(key, info.into());
                Command::RemoveNode { key }
            }),
            Command::RenameNode {
                key,
                name: mut new_name,
            } => self.nodes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameNode {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveNode { key } => {
                // Edges cannot exist without their endpoint nodes, so a node that
                // still has incident intervals cannot be removed.
                if self
                    .intervals
                    .keys()
                    .any(|&(source, target)| source == key || target == key)
                {
                    return None;
                }
                if self.trips.iter().any(|v| v.schedule.entries().iter().any(|e| e.node_key() == key))
                    || self.routes.iter().any(|v| v.stations.iter().any(|r| r.prev_curr_nodes.contains(&key) || r.curr_prev_nodes.contains(&key) || matches!(&r.stn, StationRecord::Some(nodes) if nodes.contains(&key)))) { return None; }
                self.nodes.remove(key).map(|view| Command::AddNode {
                    key,
                    info: view.into(),
                })
            }
            Command::AddServiceClass { key, info } => (!self.service_classes.contains_key(key))
                .then(|| {
                    self.service_classes.insert(key, info.into());
                    Command::RemoveServiceClass { key }
                }),
            Command::RenameServiceClass {
                key,
                name: mut new_name,
            } => self.service_classes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameServiceClass {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveServiceClass { key } => {
                if self.trips.iter().any(|v| *v.service_class == Some(key)) {
                    return None;
                }
                self.service_classes
                    .remove(key)
                    .map(|view| Command::AddServiceClass {
                        key,
                        info: view.into(),
                    })
            }
            Command::AddRoute { key, info } => (!self.routes.contains_key(key)
                && self.valid_route_stations(&info.stations))
            .then(|| {
                self.routes.insert(key, info.into());
                Command::RemoveRoute { key }
            }),
            Command::RenameRoute {
                key,
                name: mut new_name,
            } => self.routes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RenameRoute {
                    key,
                    name: new_name,
                }
            }),
            Command::RemoveRoute { key } => self.routes.remove(key).map(|view| Command::AddRoute {
                key,
                info: view.into(),
            }),
            Command::AddInterval { key, info } => {
                let (source, target) = key;
                // An interval is an edge, so both endpoints must exist and the
                // ordered pair must not already have an interval (no parallel edges).
                if !self.nodes.contains_key(source)
                    || !self.nodes.contains_key(target)
                    || self.intervals.contains_key(key)
                {
                    return None;
                }
                self.intervals.insert(key, info);
                // Maintain the node adjacency cache used by `IntoNeighbors`/`IntoEdges`.
                self.nodes.update(source, |mut view| {
                    view.outgoing.get_mut().push(target);
                });
                self.nodes.update(target, |mut view| {
                    view.incoming.get_mut().push(source);
                });
                Some(Command::RemoveInterval { key })
            }
            Command::RemoveInterval { key } => {
                let Some(Interval {
                    nodes,
                    length,
                    trips,
                }) = self.intervals.remove(key)
                else {
                    return None;
                };
                let (source, target) = key;
                self.nodes.update(source, |mut view| {
                    view.outgoing.get_mut().retain(|n| *n != target);
                });
                self.nodes.update(target, |mut view| {
                    view.incoming.get_mut().retain(|n| *n != source);
                });
                Some(Command::AddInterval {
                    key,
                    info: Interval {
                        nodes,
                        length,
                        trips,
                    },
                })
            }
            Command::ChangeTripVehicles { key, mut vehicles } => {
                if !self.trips.contains_key(key)
                    || vehicles.iter().any(|k| !self.vehicles.contains_key(*k))
                {
                    return None;
                }
                self.trips.update(key, |mut view| {
                    std::mem::swap(view.vehicles.get_mut(), &mut vehicles);
                });
                self.uncache_trip(key, &vehicles);
                let new_vehicles = self
                    .trips
                    .query(key, |view| view.vehicles.clone())
                    .unwrap_or_default();
                self.cache_trip(key, &new_vehicles);
                Some(Command::ChangeTripVehicles { key, vehicles })
            }
            Command::ChangeStation { key, info } => self.stations.update(key, |mut v| {
                let old = StationInfo {
                    name: v.name.get().clone(),
                    pos: *v.pos.get(),
                };
                *v.name.get_mut() = info.name;
                *v.pos.get_mut() = info.pos;
                Command::ChangeStation { key, info: old }
            }),
            Command::ChangeNode { key, info } => {
                if !self.stations.contains_key(info.parent) {
                    return None;
                }
                if self.routes.iter().any(|r| {
                    r.stations.iter().any(|r| match &r.stn {
                        StationRecord::Some(nodes) if nodes.contains(&key) => {
                            !info.is_platform
                                || nodes.iter().any(|n| {
                                    *n != key
                                        && self
                                            .nodes
                                            .query(*n, |v| *v.parent != info.parent)
                                            .unwrap_or(true)
                                })
                        }
                        _ => false,
                    })
                }) {
                    return None;
                }
                let mut restore = Vec::new();
                let old_pos = self.nodes.query(key, |v| *v.pos)?;
                if old_pos != info.pos {
                    let incident: Vec<_> = self
                        .intervals
                        .keys()
                        .copied()
                        .filter(|(a, b)| *a == key || *b == key)
                        .collect();
                    for edge in incident {
                        let old = self.intervals.get(edge)?.clone();
                        let mut new = old.clone();
                        if new.nodes.len() < 2 {
                            new.nodes = [
                                self.nodes.query(edge.0, |v| *v.pos)?,
                                self.nodes.query(edge.1, |v| *v.pos)?,
                            ]
                            .into_iter()
                            .collect();
                        }
                        if edge.0 == key {
                            new.nodes.make_mut()[0] = info.pos;
                        }
                        if edge.1 == key {
                            let last = new.nodes.len() - 1;
                            new.nodes.make_mut()[last] = info.pos;
                        }
                        self.intervals.insert(edge, new);
                        restore.push(Command::ChangeInterval {
                            key: edge,
                            info: old,
                        });
                    }
                }
                let inverse = self.nodes.update(key, |mut v| {
                    let old = NodeInfo {
                        name: v.name.get().clone(),
                        parent: *v.parent.get(),
                        pos: *v.pos.get(),
                        is_platform: *v.is_platform.get(),
                    };
                    *v.name.get_mut() = info.name;
                    *v.parent.get_mut() = info.parent;
                    *v.pos.get_mut() = info.pos;
                    *v.is_platform.get_mut() = info.is_platform;
                    Command::ChangeNode { key, info: old }
                })?;
                restore.insert(0, inverse);
                Some(Command::Macro(restore.into_boxed_slice()))
            }
            Command::ChangeRouteStations { key, mut stations } => {
                if !self.valid_route_stations(&stations) {
                    return None;
                }
                self.routes.update(key, |mut v| {
                    std::mem::swap(v.stations.get_mut(), &mut stations);
                    Command::ChangeRouteStations { key, stations }
                })
            }
            Command::ChangeInterval { key, mut info } => {
                let old = self.intervals.get(key)?.clone();
                info.trips = old.trips.clone();
                self.intervals.insert(key, info);
                Some(Command::ChangeInterval { key, info: old })
            }
            Command::ChangeServiceClassStyle { key, mut style } => {
                self.service_classes.update(key, |mut v| {
                    std::mem::swap(v.style.get_mut(), &mut style);
                    Command::ChangeServiceClassStyle { key, style }
                })
            }
            Command::Macro(commands) => {
                let backup = self.clone();
                let mut inverses = Vec::with_capacity(commands.len());

                for cmd in commands.into_vec() {
                    match self.apply_command_inner(cmd) {
                        Some(inverse) => inverses.push(inverse),
                        None => {
                            *self = backup;
                            return None;
                        }
                    }
                }

                inverses.reverse();
                Some(Command::Macro(inverses.into_boxed_slice()))
            }
            Command::UnloadWorld => {
                let old = std::mem::take(self);
                Some(Command::LoadWorld {
                    snapshot: Box::new(old),
                })
            }
            Command::LoadWorld { snapshot: mut new } => {
                std::mem::swap(self, &mut *new);
                Some(Command::LoadWorld { snapshot: new })
            }
        }
    }
}
