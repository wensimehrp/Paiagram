use ecow::EcoVec;

use crate::graph::Graph;
use crate::trip::TripSchedule;
use crate::{IntervalKey, Trip, Wfc, WorldSnapshot};

#[cfg(test)]
mod tests;

impl WorldSnapshot {
    /// Mutates the world and updates its derived caches. Returns whether the spatial index needs
    /// to be rebuilt.
    pub(crate) fn update_snapshot(
        &mut self,
        f: impl FnOnce(WorldSnapshot) -> WorldSnapshot,
    ) -> bool {
        puffin::profile_function!();
        let new_world = f(self.clone());
        let (updated, spatial_dirty) = self.update_diff(new_world);
        *self = updated;
        spatial_dirty
    }

    /// Given the new world snapshot, update the cache. Returns the updated world snapshot and
    /// whether its spatial data changed.
    /// The new snapshot is applied as a diff against the old one.
    fn update_diff(&self, new: WorldSnapshot) -> (WorldSnapshot, bool) {
        puffin::profile_function!();
        // The new world is only a source of changed values. Caches are derived from the old
        // snapshot and patched incrementally; anything only present in the new snapshot's cache is
        // discarded.
        use imbl::ordmap::DiffItem::{Add, Remove, Update};
        let mut ret = self.clone();
        let mut spatial_dirty = false;

        for diff in self.vehicles.diff(&new.vehicles) {
            match diff {
                Add(key, vehicle) => {
                    ret.vehicles.insert(*key, fresh(vehicle));
                }
                Remove(key, _) => {
                    ret.vehicles.remove(key);
                }
                Update {
                    old: (key, old),
                    new: (_, new_vehicle),
                } => {
                    ret.vehicles.insert(*key, updated(old, new_vehicle));
                }
            }
        }

        for diff in self.stations.diff(&new.stations) {
            match diff {
                Add(key, station) => {
                    ret.stations.insert(*key, fresh(station));
                }
                Remove(key, _) => {
                    ret.stations.remove(key);
                }
                Update {
                    old: (key, old),
                    new: (_, new_station),
                } => {
                    ret.stations.insert(*key, updated(old, new_station));
                }
            }
        }

        for diff in self.graph.nodes.diff(&new.graph.nodes) {
            spatial_dirty = true;
            match diff {
                Add(node_key, node) => {
                    let node_key = *node_key;
                    if let Some(station) = ret.stations.get_mut(&node.parent) {
                        insert_sorted(&mut station.cache.nodes, node_key);
                    }
                    ret.graph.nodes.insert(node_key, fresh(node));
                }
                Remove(node_key, node) => {
                    let node_key = *node_key;
                    if let Some(station) = ret.stations.get_mut(&node.parent) {
                        remove_sorted(&mut station.cache.nodes, &node_key);
                    }
                    ret.graph.nodes.remove(&node_key);
                }
                Update {
                    old: (node_key, old_node),
                    new: (_, new_node),
                } => {
                    let node_key = *node_key;
                    if old_node.parent != new_node.parent {
                        if let Some(station) = ret.stations.get_mut(&old_node.parent) {
                            remove_sorted(&mut station.cache.nodes, &node_key);
                        }
                        if let Some(station) = ret.stations.get_mut(&new_node.parent) {
                            insert_sorted(&mut station.cache.nodes, node_key);
                        }
                    }
                    ret.graph.nodes.insert(node_key, updated(old_node, new_node));
                }
            }
        }

        for diff in self.graph.intervals.diff(&new.graph.intervals) {
            spatial_dirty = true;
            match diff {
                Add(key, interval) => {
                    ret.graph.insert_interval_entry(*key, fresh(interval));
                }
                Remove(key, _) => {
                    ret.graph.remove_interval_entry(key);
                }
                Update {
                    old: (key, old),
                    new: (_, new_interval),
                } => {
                    ret.graph.insert_interval_entry(*key, updated(old, new_interval));
                }
            }
        }

        for diff in self.trips.diff(&new.trips) {
            spatial_dirty = true;
            match diff {
                Add(key, trip) => {
                    add_trip_to_caches(&mut ret, *key, trip, &new.graph);
                    ret.trips.insert(*key, fresh(trip));
                }
                Remove(key, trip) => {
                    remove_trip_from_caches(&mut ret, *key, trip, &self.graph);
                    ret.trips.remove(key);
                }
                Update {
                    old: (key, old_trip),
                    new: (_, new_trip),
                } => {
                    remove_trip_from_caches(&mut ret, *key, old_trip, &self.graph);
                    add_trip_to_caches(&mut ret, *key, new_trip, &new.graph);
                    ret.trips.insert(*key, updated(old_trip, new_trip));
                }
            }
        }

        for diff in self.service_classes.diff(&new.service_classes) {
            match diff {
                Add(key, class) => {
                    ret.service_classes.insert(*key, fresh(class));
                }
                Remove(key, _) => {
                    ret.service_classes.remove(key);
                }
                Update {
                    old: (key, old),
                    new: (_, new_class),
                } => {
                    ret.service_classes.insert(*key, updated(old, new_class));
                }
            }
        }

        for diff in self.routes.diff(&new.routes) {
            match diff {
                Add(key, route) => {
                    ret.routes.insert(*key, fresh(route));
                }
                Remove(key, _) => {
                    ret.routes.remove(key);
                }
                Update {
                    old: (key, old),
                    new: (_, new_route),
                } => {
                    ret.routes.insert(*key, updated(old, new_route));
                }
            }
        }

        (ret, spatial_dirty)
    }
}

/// Clone a new entry but discard its cache; `update_diff` rebuilds it after insertion.
fn fresh<D: Clone, C: Default>(value: &Wfc<D, C>) -> Wfc<D, C> {
    Wfc {
        data: value.data.clone(),
        cache: C::default(),
    }
}

/// Replace an entry's data while preserving the cache maintained by the old snapshot.
fn updated<D: Clone, C: Clone>(old: &Wfc<D, C>, new: &Wfc<D, C>) -> Wfc<D, C> {
    Wfc {
        data: new.data.clone(),
        cache: old.cache.clone(),
    }
}

fn add_trip_to_caches(
    ret: &mut WorldSnapshot,
    trip_key: crate::TripKey,
    trip: &Trip,
    graph: &Graph,
) {
    for_each_trip_interval(&trip.schedule, graph, |interval_key| {
        if let Some(interval) = ret.graph.intervals.get_mut(&interval_key) {
            insert_sorted(&mut interval.cache.trips, trip_key);
        }
    });
    for vehicle_key in &trip.vehicles {
        if let Some(vehicle) = ret.vehicles.get_mut(vehicle_key) {
            insert_sorted(&mut vehicle.cache.trips, trip_key);
        }
    }
}

fn remove_trip_from_caches(
    ret: &mut WorldSnapshot,
    trip_key: crate::TripKey,
    trip: &Trip,
    graph: &Graph,
) {
    for_each_trip_interval(&trip.schedule, graph, |interval_key| {
        if let Some(interval) = ret.graph.intervals.get_mut(&interval_key) {
            remove_sorted(&mut interval.cache.trips, &trip_key);
        }
    });
    for vehicle_key in &trip.vehicles {
        if let Some(vehicle) = ret.vehicles.get_mut(vehicle_key) {
            remove_sorted(&mut vehicle.cache.trips, &trip_key);
        }
    }
}

fn for_each_trip_interval(schedule: &TripSchedule, graph: &Graph, mut f: impl FnMut(IntervalKey)) {
    schedule.estimates(graph, |estimates| {
        for [(_, curr), (_, next)] in estimates.array_windows() {
            if curr.node_key() == next.node_key() {
                continue;
            }
            f(IntervalKey::new(curr.node_key(), next.node_key()));
        }
    });
}

fn insert_sorted<T: Ord + Clone>(values: &mut EcoVec<T>, value: T) {
    if let Err(index) = values.binary_search(&value) {
        values.insert(index, value);
    }
}

fn remove_sorted<T: Ord + Clone>(values: &mut EcoVec<T>, value: &T) {
    if let Ok(index) = values.binary_search(value) {
        values.remove(index);
    }
}
