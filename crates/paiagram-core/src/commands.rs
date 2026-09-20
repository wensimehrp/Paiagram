use super::*;
use crate::time::TDuration;
use crate::trip::{TEntry, TEntryId};

/// Errors that might occur when applying a command
pub struct CommandError {}

/// Describes how to execute a command
trait Execute {
    /// Applies the command. Returns the modified world and the inverse of the command if the
    /// application succeeds. Returns a [`CommandError`] if it fails.
    fn apply(self, snap: WorldSnapshot) -> Result<(WorldSnapshot, Command), CommandError>;
}

#[derive(Clone, Debug)]
pub enum Command {
    // trips
    TripAdd {
        key: TripKey,
        info: TripInfo,
    },
    TripRename {
        key: TripKey,
        name: EcoString,
    },
    TripClassChange {
        key: TripKey,
        class: Option<ServiceClassKey>,
    },
    TripRemove {
        key: TripKey,
    },
    // trip entry
    TripEntryShiftArrOrPass {
        key: TripKey,
        id: TEntryId,
        dur: TDuration,
    },
    TripEntryShiftDep {
        key: TripKey,
        id: TEntryId,
        dur: TDuration,
    },
    TripEntryChange {
        key: TripKey,
        id: TEntryId,
        new_entry: TEntry,
    },
    TripEntryRemove {
        key: TripKey,
        id: TEntryId,
    },
    TripEntryInsert {
        key: TripKey,
        entry: TEntry,
        pos: usize,
    },
    // vehicles
    VehicleAdd {
        key: VehicleKey,
        name: EcoString,
    },
    VehicleRename {
        key: VehicleKey,
        name: EcoString,
    },
    VehicleRemove {
        key: VehicleKey,
    },
    // Stations
    StationAdd {
        key: StationKey,
        info: StationInfo,
    },
    StationRename {
        key: StationKey,
        name: EcoString,
    },
    StationRemove {
        key: StationKey,
    },
    // nodes
    NodeAdd {
        key: NodeKey,
        info: NodeInfo,
    },
    NodeRename {
        key: NodeKey,
        name: EcoString,
    },
    NodeRemove {
        key: NodeKey,
    },
    // classes
    ServiceClassAdd {
        key: ServiceClassKey,
        info: ServiceClassInfo,
    },
    ServiceClassRename {
        key: ServiceClassKey,
        name: EcoString,
    },
    ServiceClassRemove {
        key: ServiceClassKey,
    },
    // route
    RouteAdd {
        key: RouteKey,
        info: RouteInfo,
    },
    RouteRename {
        key: RouteKey,
        name: EcoString,
    },
    RouteRemove {
        key: RouteKey,
    },
    // interval
    IntervalAdd {
        key: IntervalKey,
        info: Interval,
    },
    IntervalRemove {
        key: IntervalKey,
    },
    /// Change the vehicles that serve a trip.
    TripVehiclesChange {
        key: TripKey,
        vehicles: SmallVec<[VehicleKey; 1]>,
    },
    StationChange {
        key: StationKey,
        info: StationInfo,
    },
    NodeChange {
        key: NodeKey,
        info: NodeInfo,
    },
    RouteStationsChange {
        key: RouteKey,
        stations: EcoVec<RouteStationRecord>,
    },
    IntervalChange {
        key: IntervalKey,
        info: Interval,
    },
    ServiceClassStyleChange {
        key: ServiceClassKey,
        style: StrokeStyle,
    },
    // World related stuff
    WorldUnload,
    WorldLoad {
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
        info.service_class.is_none_or(|k| self.service_classes.contains_key(k))
            && info.vehicles.iter().all(|k| self.vehicles.contains_key(*k))
            && info.schedule.entries().iter().enumerate().all(|(i, e)| {
                self.nodes.contains_key(e.node_key())
                    && !info.schedule.entries()[..i].iter().any(|prev| prev.id() == e.id())
            })
    }

    fn valid_route_stations(&self, records: &[RouteStationRecord]) -> bool {
        let mut seen = Vec::new();
        records.iter().enumerate().all(|(index, r)| {
            let station = match &r.stn {
                StationRecord::All(k) => Some(*k),
                StationRecord::Some(nodes) => {
                    nodes.first().and_then(|k| self.nodes.query(*k, |v| *v.parent))
                }
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
            if r.canvas_length.is_some_and(|n| !n.0.is_finite() || n.0 <= 0.0) {
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
            Command::TripAdd { key, info } => {
                (!self.trips.contains_key(key) && self.valid_trip(&info)).then(|| {
                    self.cache_trip(key, &info.vehicles);
                    self.trips.insert(key, info.into());
                    Command::TripRemove { key }
                })
            }
            Command::TripRename {
                key,
                name: mut new_name,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::TripRename {
                    key,
                    name: new_name,
                }
            }),
            Command::TripClassChange {
                key,
                class: mut new_class,
            } => {
                if new_class.is_some_and(|k| !self.service_classes.contains_key(k)) {
                    return None;
                }
                self.trips.update(key, |mut view| {
                    std::mem::swap(view.service_class.get_mut(), &mut new_class);
                    Command::TripClassChange {
                        key,
                        class: new_class,
                    }
                })
            }
            Command::TripRemove { key } => self.trips.remove(key).map(|view| {
                // Drop the trip from the serving vehicles' caches.
                self.uncache_trip(key, &view.vehicles);
                Command::TripAdd {
                    key,
                    info: view.into(),
                }
            }),
            Command::TripEntryShiftArrOrPass { key, id, dur } => self
                .trips
                .update(key, |mut v| {
                    let sch = v.schedule.get_mut().entries_mut();
                    let entry = sch.make_mut().iter_mut().find(|ent| ent.id() == id)?;
                    let mode = entry.arr_or_pass_mut()?;
                    *mode = mode.shifted_by(dur);
                    Some(Command::TripEntryShiftArrOrPass { key, id, dur: -dur })
                })
                .flatten(),
            Command::TripEntryShiftDep { key, id, dur } => self
                .trips
                .update(key, |mut v| {
                    let sch = v.schedule.get_mut().entries_mut();
                    let entry = sch.make_mut().iter_mut().find(|ent| ent.id() == id)?;
                    let mode = entry.dep_mut()?;
                    *mode = mode.shifted_by(dur);
                    Some(Command::TripEntryShiftDep { key, id, dur: -dur })
                })
                .flatten(),
            Command::TripEntryChange { key, id, new_entry } => {
                if new_entry.id() != id || !self.nodes.contains_key(new_entry.node_key()) {
                    return None;
                }
                self.trips
                    .update(key, |mut v| {
                        let entries = v.schedule.get_mut().entries_mut();
                        let pos = entries.iter().position(|e| e.id() == id)?;
                        let old = std::mem::replace(&mut entries.make_mut()[pos], new_entry);
                        Some(Command::TripEntryChange {
                            key,
                            id,
                            new_entry: old,
                        })
                    })
                    .flatten()
            }
            Command::TripEntryRemove { key, id } => self
                .trips
                .update(key, |mut v| {
                    let entries = v.schedule.get_mut().entries_mut();
                    let pos = entries.iter().position(|e| e.id() == id)?;
                    let entry = entries.remove(pos);
                    Some(Command::TripEntryInsert { key, entry, pos })
                })
                .flatten(),
            Command::TripEntryInsert { key, entry, pos } => {
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
                        Some(Command::TripEntryRemove {
                            key,
                            id: entry.id(),
                        })
                    })
                    .flatten()
            }
            Command::VehicleAdd { key, name } => (!self.vehicles.contains_key(key)).then(|| {
                self.vehicles.insert(
                    key,
                    VehicleView {
                        name,
                        trips: EcoVec::new(),
                    },
                );
                Command::VehicleRemove { key }
            }),
            Command::VehicleRename {
                key,
                name: mut new_name,
            } => self.vehicles.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::VehicleRename {
                    key,
                    name: new_name,
                }
            }),
            Command::VehicleRemove { key } => {
                let mut restore = Vec::new();
                let vehicle = self.vehicles.remove(key)?;
                restore.push(Command::VehicleAdd {
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
                        restore.push(Command::TripVehiclesChange {
                            key: trip,
                            vehicles: v.vehicles.get().clone(),
                        });
                        v.vehicles.get_mut().retain(|v| *v != key);
                    });
                }
                Some(Command::Macro(restore.into_boxed_slice()))
            }
            Command::StationAdd { key, info } => (!self.stations.contains_key(key)).then(|| {
                self.stations.insert(key, info.into());
                Command::StationRemove { key }
            }),
            Command::StationRename {
                key,
                name: mut new_name,
            } => self.stations.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::StationRename {
                    key,
                    name: new_name,
                }
            }),
            Command::StationRemove { key } => {
                if self.nodes.iter().any(|v| *v.parent == key)
                    || self.routes.iter().any(|v| {
                        v.stations
                            .iter()
                            .any(|r| matches!(r.stn, StationRecord::All(k) if k == key))
                    })
                {
                    return None;
                }
                self.stations.remove(key).map(|view| Command::StationAdd {
                    key,
                    info: view.into(),
                })
            }
            Command::NodeAdd { key, info } => (!self.nodes.contains_key(key)
                && self.stations.contains_key(info.parent))
            .then(|| {
                self.nodes.insert(key, info.into());
                Command::NodeRemove { key }
            }),
            Command::NodeRename {
                key,
                name: mut new_name,
            } => self.nodes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::NodeRename {
                    key,
                    name: new_name,
                }
            }),
            Command::NodeRemove { key } => {
                // Edges cannot exist without their endpoint nodes, so a node that
                // still has incident intervals cannot be removed.
                if self.intervals.keys().any(|&(source, target)| source == key || target == key) {
                    return None;
                }
                if self.trips.iter().any(|v| v.schedule.entries().iter().any(|e| e.node_key() == key))
                    || self.routes.iter().any(|v| v.stations.iter().any(|r| r.prev_curr_nodes.contains(&key) || r.curr_prev_nodes.contains(&key) || matches!(&r.stn, StationRecord::Some(nodes) if nodes.contains(&key)))) { return None; }
                self.nodes.remove(key).map(|view| Command::NodeAdd {
                    key,
                    info: view.into(),
                })
            }
            Command::ServiceClassAdd { key, info } => (!self.service_classes.contains_key(key))
                .then(|| {
                    self.service_classes.insert(key, info.into());
                    Command::ServiceClassRemove { key }
                }),
            Command::ServiceClassRename {
                key,
                name: mut new_name,
            } => self.service_classes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::ServiceClassRename {
                    key,
                    name: new_name,
                }
            }),
            Command::ServiceClassRemove { key } => {
                if self.trips.iter().any(|v| *v.service_class == Some(key)) {
                    return None;
                }
                self.service_classes.remove(key).map(|view| Command::ServiceClassAdd {
                    key,
                    info: view.into(),
                })
            }
            Command::RouteAdd { key, info } => (!self.routes.contains_key(key)
                && self.valid_route_stations(&info.stations))
            .then(|| {
                self.routes.insert(key, info.into());
                Command::RouteRemove { key }
            }),
            Command::RouteRename {
                key,
                name: mut new_name,
            } => self.routes.update(key, |mut view| {
                std::mem::swap(view.name.get_mut(), &mut new_name);
                Command::RouteRename {
                    key,
                    name: new_name,
                }
            }),
            Command::RouteRemove { key } => self.routes.remove(key).map(|view| Command::RouteAdd {
                key,
                info: view.into(),
            }),
            Command::IntervalAdd { key, info } => {
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
                Some(Command::IntervalRemove { key })
            }
            Command::IntervalRemove { key } => {
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
                Some(Command::IntervalAdd {
                    key,
                    info: Interval {
                        nodes,
                        length,
                        trips,
                    },
                })
            }
            Command::TripVehiclesChange { key, mut vehicles } => {
                if !self.trips.contains_key(key)
                    || vehicles.iter().any(|k| !self.vehicles.contains_key(*k))
                {
                    return None;
                }
                self.trips.update(key, |mut view| {
                    std::mem::swap(view.vehicles.get_mut(), &mut vehicles);
                });
                self.uncache_trip(key, &vehicles);
                let new_vehicles =
                    self.trips.query(key, |view| view.vehicles.clone()).unwrap_or_default();
                self.cache_trip(key, &new_vehicles);
                Some(Command::TripVehiclesChange { key, vehicles })
            }
            Command::StationChange { key, info } => self.stations.update(key, |mut v| {
                let old = StationInfo {
                    name: v.name.get().clone(),
                    pos: *v.pos.get(),
                };
                *v.name.get_mut() = info.name;
                *v.pos.get_mut() = info.pos;
                Command::StationChange { key, info: old }
            }),
            Command::NodeChange { key, info } => {
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
                        restore.push(Command::IntervalChange {
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
                    Command::NodeChange { key, info: old }
                })?;
                restore.insert(0, inverse);
                Some(Command::Macro(restore.into_boxed_slice()))
            }
            Command::RouteStationsChange { key, mut stations } => {
                if !self.valid_route_stations(&stations) {
                    return None;
                }
                self.routes.update(key, |mut v| {
                    std::mem::swap(v.stations.get_mut(), &mut stations);
                    Command::RouteStationsChange { key, stations }
                })
            }
            Command::IntervalChange { key, mut info } => {
                let old = self.intervals.get(key)?.clone();
                info.trips = old.trips.clone();
                self.intervals.insert(key, info);
                Some(Command::IntervalChange { key, info: old })
            }
            Command::ServiceClassStyleChange { key, mut style } => {
                self.service_classes.update(key, |mut v| {
                    std::mem::swap(v.style.get_mut(), &mut style);
                    Command::ServiceClassStyleChange { key, style }
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
            Command::WorldUnload => {
                let old = std::mem::take(self);
                Some(Command::WorldLoad {
                    snapshot: Box::new(old),
                })
            }
            Command::WorldLoad { snapshot: mut new } => {
                std::mem::swap(self, &mut *new);
                Some(Command::WorldLoad { snapshot: new })
            }
        }
    }
}
