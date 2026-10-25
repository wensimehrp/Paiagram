// SPDX-License-Identifier: MPL-2.0

use ecow::eco_vec;
use smallvec::smallvec;

use super::*;
use crate::graph::IntervalDirection;
use crate::import::{ImportMode, ImportType, make_snapshot};
use crate::time::TimetableTime;
use crate::trip::{TEntry, TEntryId, TravelMode, TripSchedule};
use crate::{
    Interval, LonLat, Node, NodeKey, Source, Station, StationKey, TripKey, Vehicle, VehicleKey,
};

fn populated_world() -> (
    WorldSnapshot,
    StationKey,
    NodeKey,
    NodeKey,
    VehicleKey,
    TripKey,
) {
    let station = StationKey::new();
    let a = NodeKey::new();
    let b = NodeKey::new();
    let vehicle = VehicleKey::new();
    let trip = TripKey::new();

    let mut world = WorldSnapshot::default();
    world.stations.insert(
        station,
        Wfc::new(Station {
            name: "station".into(),
            pos: LonLat::ZERO,
        }),
    );
    for node in [a, b] {
        world.graph.insert_node(
            node,
            Node {
                name: "node".into(),
                parent: station,
                pos: LonLat::ZERO,
            },
        );
    }
    world.graph.insert_interval_entry(
        IntervalKey::new(a, b),
        Wfc::new(Interval {
            nodes: eco_vec![LonLat::ZERO, LonLat::ZERO],
            length: None,
            direction: IntervalDirection::Both,
        }),
    );
    world.vehicles.insert(
        vehicle,
        Wfc::new(Vehicle {
            name: "vehicle".into(),
        }),
    );
    world.trips.insert(
        trip,
        Wfc::new(Trip {
            name: "trip".into(),
            schedule: TripSchedule::new(eco_vec![
                TEntry {
                    node: a,
                    arr_or_pass: TravelMode::At(TimetableTime(0)),
                    dep: Some(TravelMode::At(TimetableTime(0))),
                    id: TEntryId::new(),
                },
                TEntry {
                    node: b,
                    arr_or_pass: TravelMode::At(TimetableTime(10)),
                    dep: Some(TravelMode::At(TimetableTime(10))),
                    id: TEntryId::new(),
                },
            ]),
            service_class: None,
            vehicles: smallvec![vehicle],
        }),
    );

    (world, station, a, b, vehicle, trip)
}

fn assert_same_world(a: &WorldSnapshot, b: &WorldSnapshot) {
    // don't use assert_eq since they print A LOT when not eq
    assert!(a.trips == b.trips);
    assert!(a.vehicles == b.vehicles);
    assert!(a.stations == b.stations);
    assert!(a.graph == b.graph);
    assert!(a.service_classes == b.service_classes);
    assert!(a.routes == b.routes);
}

/// Remove every top-level entity introduced by `snapshot` from `target`.
fn remove_snapshot(target: &mut WorldSnapshot, snapshot: &WorldSnapshot) {
    for key in snapshot.graph.intervals.keys() {
        target.graph.remove_interval_entry(key);
    }
    for key in snapshot.graph.nodes.keys() {
        target.graph.nodes.remove(key);
    }
    for key in snapshot.stations.keys() {
        target.stations.remove(key);
    }
    for key in snapshot.vehicles.keys() {
        target.vehicles.remove(key);
    }
    for key in snapshot.trips.keys() {
        target.trips.remove(key);
    }
    for key in snapshot.service_classes.keys() {
        target.service_classes.remove(key);
    }
    for key in snapshot.routes.keys() {
        target.routes.remove(key);
    }
}

#[test]
fn building_a_world_populates_derived_caches() {
    let (new, station, a, b, vehicle, trip) = populated_world();
    let world = WorldSnapshot::default().update_diff(new);

    let interval = IntervalKey::new(a, b);
    assert_eq!(
        world.graph.intervals.get(&interval).unwrap().cache.trips.as_slice(),
        &[trip]
    );
    assert_eq!(
        world.vehicles.get(&vehicle).unwrap().cache.trips.as_slice(),
        &[trip]
    );
    assert_eq!(
        world.stations.get(&station).unwrap().cache.nodes.as_slice(),
        &[a, b]
    );
    assert_eq!(world.graph.neighbor_intervals(a).count(), 1);
    assert!(world.spatial_dirty);
}

#[test]
fn removing_a_trip_removes_it_from_derived_caches() {
    let (new, _station, a, b, vehicle, trip) = populated_world();
    let world = WorldSnapshot::default().update_diff(new);

    let mut changed = world.clone();
    changed.trips.remove(&trip);
    let updated = world.update_diff(changed);

    let interval = IntervalKey::new(a, b);
    assert!(updated.graph.intervals.get(&interval).unwrap().cache.trips.is_empty());
    assert!(updated.vehicles.get(&vehicle).unwrap().cache.trips.is_empty());
    assert!(!updated.trips.contains_key(&trip));
}

#[test]
fn removing_an_interval_updates_adjacency() {
    let (new, _station, a, b, _vehicle, _trip) = populated_world();
    let world = WorldSnapshot::default().update_diff(new);

    let mut changed = world.clone();
    changed.graph.remove_interval_entry(&IntervalKey::new(a, b));
    let updated = world.update_diff(changed);

    assert_eq!(updated.graph.neighbor_intervals(a).count(), 0);
}

#[test]
fn moving_a_node_updates_station_caches() {
    let (new, station, a, b, _vehicle, _trip) = populated_world();
    let world = WorldSnapshot::default().update_diff(new);

    let new_station = StationKey::new();
    let mut changed = world.clone();
    changed.stations.insert(
        new_station,
        Wfc::new(Station {
            name: "new station".into(),
            pos: LonLat::ZERO,
        }),
    );
    let mut node = changed.graph.nodes.get(&b).unwrap().clone();
    node.data.parent = new_station;
    changed.graph.nodes.insert(b, node);

    let updated = world.update_diff(changed);
    assert_eq!(
        updated.stations.get(&station).unwrap().cache.nodes.as_slice(),
        &[a]
    );
    assert_eq!(
        updated.stations.get(&new_station).unwrap().cache.nodes.as_slice(),
        &[b]
    );
}

#[test]
fn updating_a_trips_vehicles_moves_its_cache_entry() {
    let (new, _station, _a, _b, old_vehicle, trip) = populated_world();
    let world = WorldSnapshot::default().update_diff(new);

    let new_vehicle = VehicleKey::new();
    let mut changed = world.clone();
    changed.vehicles.insert(
        new_vehicle,
        Wfc::new(Vehicle {
            name: "new vehicle".into(),
        }),
    );
    let mut changed_trip = changed.trips.get(&trip).unwrap().clone();
    changed_trip.data.vehicles = smallvec![new_vehicle];
    changed.trips.insert(trip, changed_trip);

    let updated = world.update_diff(changed);
    assert!(updated.vehicles.get(&old_vehicle).unwrap().cache.trips.is_empty());
    assert_eq!(
        updated.vehicles.get(&new_vehicle).unwrap().cache.trips.as_slice(),
        &[trip]
    );
}

#[test]
fn undoing_imports_matches_removing_their_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    use ImportMode::{Merge, Replace};

    let bytes = include_bytes!("../../../paiagram-oudia/test/sample.oud2");
    let imported = [
        make_snapshot(bytes, ImportType::OuDiaSecond)?,
        make_snapshot(bytes, ImportType::OuDiaSecond)?,
        make_snapshot(bytes, ImportType::OuDiaSecond)?,
    ];

    for modes in [
        [Replace, Replace, Replace],
        [Replace, Replace, Merge],
        [Replace, Merge, Replace],
        [Replace, Merge, Merge],
        [Merge, Replace, Replace],
        [Merge, Replace, Merge],
        [Merge, Merge, Replace],
        [Merge, Merge, Merge],
    ] {
        let mut source = Source::new();
        let mut after = Vec::with_capacity(imported.len());

        for (world, mode) in imported.iter().zip(modes) {
            let before = source.snap.clone();
            source.import(world.clone(), mode).unwrap();

            match mode {
                // A replace discards everything that came before it.
                Replace => {
                    let expected = WorldSnapshot::default().update_diff(world.clone());
                    assert_same_world(&source.snap, &expected);
                }
                // A merge only adds `world` to the previous snapshot.
                Merge => {
                    let mut target = source.snap.clone();
                    remove_snapshot(&mut target, world);
                    let mut removed = source.snap.clone();
                    removed.update_snapshot(|_| target);
                    assert_same_world(&removed, &before);
                }
            }

            after.push(source.snap.clone());
        }

        // Undo restores each intermediate snapshot, then the empty world.
        for expected in after.iter().rev().skip(1) {
            source.undo().unwrap();
            assert_same_world(&source.snap, expected);
        }
        source.undo().unwrap();
        assert_same_world(&source.snap, &WorldSnapshot::default());
    }

    Ok(())
}
