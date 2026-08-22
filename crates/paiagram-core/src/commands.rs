use super::*;

#[derive(Clone, Debug)]
pub enum Command {
    /// Add a new trip to the world
    AddTrip {
        key: TripKey,
        info: TripInfo,
    },
    /// Rename a trip
    RenameTrip {
        key: TripKey,
        name: EcoString,
    },
    ChangeTripEntries {
        key: TripKey,
        entries: EcoVec<TEntry>,
    },
    /// Change the trip's class to another class
    ChangeTripClass {
        key: TripKey,
        class: Option<ServiceClassKey>,
    },
    /// Remove a trip from the collection
    RemoveTrip {
        key: TripKey,
    },
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
    /// Applies a command and returns its inverse. Could modify the world and return the inverse if
    /// the application succeeds; doesn't modify the world and returns None if the application
    /// fails.
    pub fn apply_command(&mut self, cmd: Command) -> Option<Command> {
        match cmd {
            Command::AddTrip { key, info } => (!self.trips.contains_key(key)).then(|| {
                self.cache_trip(key, &info.vehicles);
                self.trips.insert(key, info.into());
                Command::RemoveTrip { key }
            }),
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
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.service_class.get_mut(), &mut new_class);
                Command::ChangeTripClass {
                    key,
                    class: new_class,
                }
            }),
            Command::RemoveTrip { key } => self.trips.remove(key).map(|view| {
                // Drop the trip from the serving vehicles' caches.
                self.uncache_trip(key, &view.vehicles);
                Command::AddTrip {
                    key,
                    info: view.into(),
                }
            }),
            Command::ChangeTripEntries {
                key,
                entries: mut new_entries,
            } => self.trips.update(key, |mut view| {
                std::mem::swap(view.schedule.get_mut().entries_mut(), &mut new_entries);
                Command::ChangeTripEntries {
                    key,
                    entries: new_entries,
                }
            }),
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
                self.vehicles.remove(key).map(|VehicleView { name, trips }| {
                    // Drop this vehicle from the authoritative vehicle lists of
                    // the trips it served.
                    for trip in &trips {
                        self.trips.update(*trip, |mut view| {
                            view.vehicles.get_mut().retain(|v| *v != key);
                        });
                    }
                    Command::AddVehicle { key, name }
                })
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
                self.stations.remove(key).map(|view| Command::AddStation {
                    key,
                    info: view.into(),
                })
            }
            Command::AddNode { key, info } => (!self.nodes.contains_key(key)).then(|| {
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
                if self.intervals.keys().any(|&(source, target)| source == key || target == key) {
                    return None;
                }
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
                self.service_classes.remove(key).map(|view| Command::AddServiceClass {
                    key,
                    info: view.into(),
                })
            }
            Command::AddRoute { key, info } => (!self.routes.contains_key(key)).then(|| {
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
                    direction,
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
                        direction,
                        trips,
                    },
                })
            }
            Command::ChangeTripVehicles { key, mut vehicles } => {
                if !self.trips.contains_key(key) {
                    return None;
                }
                self.trips.update(key, |mut view| {
                    std::mem::swap(view.vehicles.get_mut(), &mut vehicles);
                });
                self.uncache_trip(key, &vehicles);
                let new_vehicles =
                    self.trips.query(key, |view| view.vehicles.clone()).unwrap_or_default();
                self.cache_trip(key, &new_vehicles);
                Some(Command::ChangeTripVehicles { key, vehicles })
            }
            Command::Macro(commands) => {
                let backup = self.clone();
                let mut inverses = Vec::with_capacity(commands.len());

                for cmd in commands.into_vec() {
                    match self.apply_command(cmd) {
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
