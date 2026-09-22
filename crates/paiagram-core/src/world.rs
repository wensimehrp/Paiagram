use crate::WorldSnapshot;

impl WorldSnapshot {
    // call at the start of each frame
    /// Given the new world snapshot, update the cache. Returns the new world snapshot
    pub fn update_cache(&mut self, new: WorldSnapshot) -> Result<WorldSnapshot, String> {
        for diff in self.trips.diff(&new.trips) {}
        for diff in self.vehicles.diff(&new.vehicles) {}
        for diff in self.stations.diff(&new.stations) {}
        for diff in self.intervals.diff(&new.intervals) {}
        for diff in self.service_classes.diff(&new.service_classes) {}
        for diff in self.routes.diff(&new.routes) {}
        for diff in self.nodes.diff(&new.nodes) {}
        todo!()
    }
}

fn merge(old: WorldSnapshot, diff: WorldSnapshot) -> Result<WorldSnapshot, String> {
    todo!()
}

// app loop
fn main_loop() {
    let mut snap = WorldSnapshot::default();
    loop {
        let mut write_snap = snap.clone();

        snap = merge(snap, write_snap).unwrap();
    }
}
