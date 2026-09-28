use crate::{IntervalKey, WorldSnapshot};

impl WorldSnapshot {
    /// Mutates the world and updates the cache.
    /// Doesn't mutate if it fails
    pub fn mutate(&mut self, f: impl FnOnce(WorldSnapshot) -> WorldSnapshot) {
        let new_world = f(self.clone());
        if let Some(updated) = self.update_diff(new_world) {
            *self = updated
        }
    }
    /// Given the new world snapshot, update the cache. Returns the new world snapshot
    // TODO: handle rtrees
    #[inline(never)]
    fn update_diff(&self, new: WorldSnapshot) -> Option<WorldSnapshot> {
        use imbl::ordmap::DiffItem::{Add, Remove, Update};
        let mut ret = new.clone();
        for diff in self.trips.diff(&new.trips) {
            match diff {
                Add(k, v) => v.schedule.estimates(&new.graph, |estimates| {
                    for [(_, curr), (_, next)] in estimates.array_windows() {
                        if curr.node_key() == next.node_key() {
                            continue;
                        }
                        let interval_key = IntervalKey::new(curr.node_key(), next.node_key());
                        let interval = ret.graph.intervals.get_mut(&interval_key).unwrap();
                        match interval.cache.trips.binary_search(k) {
                            Ok(_) => {}
                            Err(idx) => interval.cache.trips.insert(idx, *k),
                        }
                    }
                }),
                Remove(k, v) => {}
                Update {
                    old: (k, old_v),
                    new: (_, new_v),
                } => {}
            }
        }
        for diff in self.vehicles.diff(&new.vehicles) {}
        for diff in self.stations.diff(&new.stations) {}
        for diff in self.graph.nodes.diff(&new.graph.nodes) {
            // Keep each station's cached node list in sync, since route station records
            // (`StationRecord::All`) resolve their nodes through it.
            match diff {
                Add(node_key, node) => {
                    let node_key = *node_key;
                    if let Some(station) = ret.stations.get_mut(&node.parent) {
                        let nodes = &mut station.cache.nodes;
                        if let Err(idx) = nodes.binary_search(&node_key) {
                            nodes.insert(idx, node_key);
                        }
                    }
                }
                Remove(node_key, node) => {
                    let node_key = *node_key;
                    if let Some(station) = ret.stations.get_mut(&node.parent)
                        && let Ok(idx) = station.cache.nodes.binary_search(&node_key)
                    {
                        station.cache.nodes.remove(idx);
                    }
                }
                Update {
                    old: (node_key, old_node),
                    new: (_, new_node),
                } => {
                    let node_key = *node_key;
                    if old_node.parent != new_node.parent {
                        if let Some(station) = ret.stations.get_mut(&old_node.parent)
                            && let Ok(idx) = station.cache.nodes.binary_search(&node_key)
                        {
                            station.cache.nodes.remove(idx);
                        }
                        if let Some(station) = ret.stations.get_mut(&new_node.parent) {
                            let nodes = &mut station.cache.nodes;
                            if let Err(idx) = nodes.binary_search(&node_key) {
                                nodes.insert(idx, node_key);
                            }
                        }
                    }
                }
            }
        }
        for diff in self.graph.intervals.diff(&new.graph.intervals) {
            match diff {
                Add(k, v) => {}
                Remove(k, v) => {}
                Update {
                    old: (k, old_v),
                    new: (_, new_v),
                } => {}
            }
        }
        for diff in self.service_classes.diff(&new.service_classes) {}
        for diff in self.routes.diff(&new.routes) {}
        Some(ret)
    }
}
