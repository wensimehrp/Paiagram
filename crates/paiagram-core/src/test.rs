use ecow::eco_vec;

use super::*;
use crate::trip::TEntryId;

fn empty_trip_schedule() -> TripSchedule {
    TripSchedule::new(EcoVec::new())
}

fn trip_info(name: &str) -> TripInfo {
    TripInfo {
        name: name.into(),
        schedule: empty_trip_schedule(),
        service_class: None,
        vehicles: SmallVec::new(),
    }
}

#[test]
fn add_remove_round_trip() {
    let mut world = WorldSnapshot::default();

    // Trip
    let trip_key = TripKey::new();
    let info = trip_info("T1");
    let inv = world
        .apply_command(Command::AddTrip {
            key: trip_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.trips.contains_key(trip_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.trips.contains_key(trip_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.trips.contains_key(trip_key));

    // Vehicle
    let vehicle_key = VehicleKey::new();
    let inv = world
        .apply_command(Command::AddVehicle {
            key: vehicle_key,
            name: "V1".into(),
        })
        .unwrap();
    assert!(world.vehicles.contains_key(vehicle_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.vehicles.contains_key(vehicle_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.vehicles.contains_key(vehicle_key));

    // Station
    let station_key = StationKey::new();
    let info = StationInfo {
        name: "S1".into(),
        pos: LonLat::ZERO,
    };
    let inv = world
        .apply_command(Command::AddStation {
            key: station_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.stations.contains_key(station_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.stations.contains_key(station_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.stations.contains_key(station_key));

    // Node
    let node_key = NodeKey::new();
    let info = NodeInfo {
        name: "N1".into(),
        parent: station_key,
        pos: LonLat::ZERO,
        is_platform: true,
    };
    let inv = world
        .apply_command(Command::AddNode {
            key: node_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.nodes.contains_key(node_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.nodes.contains_key(node_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.nodes.contains_key(node_key));

    // Service class
    let class_key = ServiceClassKey::new();
    let info = ServiceClassInfo {
        name: "C1".into(),
        style: StrokeStyle {
            color: Color32::from_rgb(255, 0, 0),
            width: 1,
        },
    };
    let inv = world
        .apply_command(Command::AddServiceClass {
            key: class_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.service_classes.contains_key(class_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.service_classes.contains_key(class_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.service_classes.contains_key(class_key));

    // Route
    let route_key = RouteKey::new();
    let info = RouteInfo {
        name: "R1".into(),
        stations: EcoVec::new(),
    };
    let inv = world
        .apply_command(Command::AddRoute {
            key: route_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.routes.contains_key(route_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.routes.contains_key(route_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.routes.contains_key(route_key));

    // Interval
    let interval_key = (node_key, node_key);
    let info = Interval {
        nodes: eco_vec![LonLat::ZERO, LonLat::ZERO],
        length: None,
        direction: IntervalDirection::OneWay,
        trips: EcoVec::new(),
    };
    let inv = world
        .apply_command(Command::AddInterval {
            key: interval_key,
            info: info.clone(),
        })
        .unwrap();
    assert!(world.intervals.contains_key(interval_key));
    let inv = world.apply_command(inv).unwrap();
    assert!(!world.intervals.contains_key(interval_key));
    let _inv = world.apply_command(inv).unwrap();
    assert!(world.intervals.contains_key(interval_key));
}

#[test]
fn rename_round_trip() {
    let mut world = WorldSnapshot::default();

    // Trip
    let trip_key = TripKey::new();
    world.apply_command(Command::AddTrip {
        key: trip_key,
        info: trip_info("old"),
    });
    let inv = world
        .apply_command(Command::RenameTrip {
            key: trip_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.trips.query(trip_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.trips.query(trip_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );

    // Vehicle
    let vehicle_key = VehicleKey::new();
    world.apply_command(Command::AddVehicle {
        key: vehicle_key,
        name: "old".into(),
    });
    let inv = world
        .apply_command(Command::RenameVehicle {
            key: vehicle_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.vehicles.query(vehicle_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.vehicles.query(vehicle_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );

    // Station
    let station_key = StationKey::new();
    world.apply_command(Command::AddStation {
        key: station_key,
        info: StationInfo {
            name: "old".into(),
            pos: LonLat::ZERO,
        },
    });
    let inv = world
        .apply_command(Command::RenameStation {
            key: station_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.stations.query(station_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.stations.query(station_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );

    // Node
    let node_key = NodeKey::new();
    world.apply_command(Command::AddNode {
        key: node_key,
        info: NodeInfo {
            name: "old".into(),
            parent: station_key,
            pos: LonLat::ZERO,
            is_platform: true,
        },
    });
    let inv = world
        .apply_command(Command::RenameNode {
            key: node_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.nodes.query(node_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.nodes.query(node_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );

    // Service class
    let class_key = ServiceClassKey::new();
    world.apply_command(Command::AddServiceClass {
        key: class_key,
        info: ServiceClassInfo {
            name: "old".into(),
            style: StrokeStyle {
                color: Color32::from_rgb(0, 0, 255),
                width: 1,
            },
        },
    });
    let inv = world
        .apply_command(Command::RenameServiceClass {
            key: class_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.service_classes.query(class_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.service_classes.query(class_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );

    // Route
    let route_key = RouteKey::new();
    world.apply_command(Command::AddRoute {
        key: route_key,
        info: RouteInfo {
            name: "old".into(),
            stations: EcoVec::new(),
        },
    });
    let inv = world
        .apply_command(Command::RenameRoute {
            key: route_key,
            name: "new".into(),
        })
        .unwrap();
    assert_eq!(
        world.routes.query(route_key, |v| v.name.clone()),
        Some(EcoString::from("new"))
    );
    let _inv = world.apply_command(inv).unwrap();
    assert_eq!(
        world.routes.query(route_key, |v| v.name.clone()),
        Some(EcoString::from("old"))
    );
}

#[test]
fn change_trip_entries_round_trip() {
    let mut world = WorldSnapshot::default();
    let trip_key = TripKey::new();
    world.apply_command(Command::AddTrip {
        key: trip_key,
        info: trip_info("T"),
    });

    let node_a = NodeKey::new();
    let node_b = NodeKey::new();
    let entry_a = TEntry::Derived {
        node: node_a,
        id: TEntryId::new(),
    };
    let entry_b = TEntry::Derived {
        node: node_b,
        id: TEntryId::new(),
    };
    let entries: EcoVec<TEntry> = [entry_a, entry_b].into_iter().collect();

    let inv = world
        .apply_command(Command::ChangeTripEntries {
            key: trip_key,
            entries,
        })
        .unwrap();
    // The inverse carries the old, empty entry list.
    assert!(matches!(&inv, Command::ChangeTripEntries { entries, .. } if entries.is_empty()));
    assert_eq!(
        world.trips.query(trip_key, |v| v.schedule.entries().to_vec()),
        Some(vec![entry_a, entry_b])
    );

    let _inv = world.apply_command(inv).unwrap();
    assert!(world.trips.query(trip_key, |v| v.schedule.entries().to_vec()).unwrap().is_empty());
}

#[test]
fn change_trip_vehicles() {
    let mut world = WorldSnapshot::default();
    let vehicle_a = VehicleKey::new();
    let vehicle_b = VehicleKey::new();
    let trip = TripKey::new();
    world.apply_command(Command::AddVehicle {
        key: vehicle_a,
        name: "VA".into(),
    });
    world.apply_command(Command::AddVehicle {
        key: vehicle_b,
        name: "VB".into(),
    });
    world.apply_command(Command::AddTrip {
        key: trip,
        info: trip_info("T"),
    });

    let vehicles_of =
        |world: &WorldSnapshot| world.trips.query(trip, |v| v.vehicles.clone()).unwrap();
    let trips_of = |world: &WorldSnapshot, vehicle| {
        world.vehicles.query(vehicle, |v| v.trips.clone()).unwrap()
    };

    let inv = world
        .apply_command(Command::ChangeTripVehicles {
            key: trip,
            vehicles: [vehicle_a, vehicle_b].into_iter().collect(),
        })
        .unwrap();
    assert_eq!(vehicles_of(&world).as_slice(), &[vehicle_a, vehicle_b]);
    assert_eq!(trips_of(&world, vehicle_a).as_slice(), &[trip]);
    assert_eq!(trips_of(&world, vehicle_b).as_slice(), &[trip]);

    // Replace with a single vehicle.
    let inv2 = world
        .apply_command(Command::ChangeTripVehicles {
            key: trip,
            vehicles: [vehicle_b].into_iter().collect(),
        })
        .unwrap();
    assert_eq!(vehicles_of(&world).as_slice(), &[vehicle_b]);
    assert!(trips_of(&world, vehicle_a).is_empty());
    assert_eq!(trips_of(&world, vehicle_b).as_slice(), &[trip]);

    // Undo the second change.
    let _ = world.apply_command(inv2).unwrap();
    assert_eq!(vehicles_of(&world).as_slice(), &[vehicle_a, vehicle_b]);
    // Undo the first change.
    let _ = world.apply_command(inv).unwrap();
    assert!(vehicles_of(&world).is_empty());
    assert!(trips_of(&world, vehicle_a).is_empty());
    assert!(trips_of(&world, vehicle_b).is_empty());
}

#[test]
fn removing_trip_cleans_vehicle_cache() {
    let mut world = WorldSnapshot::default();
    let vehicle = VehicleKey::new();
    let trip = TripKey::new();
    world.apply_command(Command::AddVehicle {
        key: vehicle,
        name: "V".into(),
    });
    world.apply_command(Command::AddTrip {
        key: trip,
        info: trip_info("T"),
    });
    world.apply_command(Command::ChangeTripVehicles {
        key: trip,
        vehicles: [vehicle].into_iter().collect(),
    });

    // Removing the trip drops it from the serving vehicle's cache.
    let inv = world.apply_command(Command::RemoveTrip { key: trip }).unwrap();
    assert!(world.vehicles.query(vehicle, |v| v.trips.clone()).unwrap().is_empty());

    // Undo restores the trip and re-populates the vehicle cache.
    let _ = world.apply_command(inv).unwrap();
    assert!(world.trips.contains_key(trip));
    assert_eq!(
        world.vehicles.query(vehicle, |v| v.trips.clone()).unwrap().as_slice(),
        &[trip]
    );
}

#[test]
fn rebuild_vehicle_trip_cache() {
    let mut world = WorldSnapshot::default();
    let vehicle = VehicleKey::new();
    let trip = TripKey::new();
    world.apply_command(Command::AddVehicle {
        key: vehicle,
        name: "V".into(),
    });
    world.apply_command(Command::AddTrip {
        key: trip,
        info: trip_info("T"),
    });
    world.apply_command(Command::ChangeTripVehicles {
        key: trip,
        vehicles: [vehicle].into_iter().collect(),
    });

    // Simulate a fresh load: the derived cache is empty.
    let vehicles: Vec<VehicleKey> = world.vehicles.keys().collect();
    for v in vehicles {
        world.vehicles.update(v, |mut view| {
            view.trips.get_mut().clear();
        });
    }
    assert!(world.vehicles.query(vehicle, |v| v.trips.clone()).unwrap().is_empty());

    world.rebuild_vehicle_trip_cache();
    assert_eq!(
        world.vehicles.query(vehicle, |v| v.trips.clone()).unwrap().as_slice(),
        &[trip]
    );
}

#[test]
fn interval_maintains_world_graph() {
    let mut world = WorldSnapshot::default();
    let source = NodeKey::new();
    let target = NodeKey::new();
    for node in [source, target] {
        world
            .apply_command(Command::AddNode {
                key: node,
                info: NodeInfo {
                    name: "".into(),
                    parent: StationKey::new(),
                    pos: LonLat::ZERO,
                    is_platform: false,
                },
            })
            .unwrap();
    }
    let key = (source, target);
    let info = Interval {
        nodes: eco_vec![LonLat::ZERO, LonLat::ZERO],
        length: Some(NonZeroU32::new(1000).unwrap()),
        direction: IntervalDirection::OneWay,
        trips: EcoVec::new(),
    };
    let inv = world
        .apply_command(Command::AddInterval {
            key,
            info: info.clone(),
        })
        .unwrap();

    let (distance, path) = world.route_between_nodes(source, target).unwrap();
    assert_eq!(distance, Distance(1000));
    assert_eq!(path, vec![source, target]);

    // A second interval between the same node pair is rejected.
    let duplicate = (source, target);
    assert!(
        world
            .apply_command(Command::AddInterval {
                key: duplicate,
                info: info.clone(),
            })
            .is_none()
    );
    assert_eq!(world.intervals.len(), 1);

    let _ = world.apply_command(inv).unwrap();
    assert!(world.route_between_nodes(source, target).is_none());
    assert!(!world.intervals.contains_key(key));
}

#[test]
fn graph_keeps_opposite_edges_distinct() {
    // `(a, b)` and `(b, a)` are distinct directed edges.
    let mut world = WorldSnapshot::default();
    let a = NodeKey::new();
    let b = NodeKey::new();
    for node in [a, b] {
        world
            .apply_command(Command::AddNode {
                key: node,
                info: NodeInfo {
                    name: "".into(),
                    parent: StationKey::new(),
                    pos: LonLat::ZERO,
                    is_platform: false,
                },
            })
            .unwrap();
    }
    let ab = (a, b);
    let ba = (b, a);
    let mk_info = || Interval {
        nodes: eco_vec![LonLat::ZERO, LonLat::ZERO],
        length: Some(NonZeroU32::new(1000).unwrap()),
        direction: IntervalDirection::OneWay,
        trips: EcoVec::new(),
    };
    world.apply_command(Command::AddInterval {
        key: ab,
        info: mk_info(),
    });
    world.apply_command(Command::AddInterval {
        key: ba,
        info: mk_info(),
    });

    assert!(world.route_between_nodes(a, b).is_some());
    assert!(world.route_between_nodes(b, a).is_some());

    // Removing one edge leaves the other intact.
    let inv = world.apply_command(Command::RemoveInterval { key: ab }).unwrap();
    assert!(world.route_between_nodes(a, b).is_none());
    assert!(world.route_between_nodes(b, a).is_some());
    let _ = world.apply_command(inv).unwrap();
    assert!(world.route_between_nodes(a, b).is_some());
}

#[test]
fn macro_rolls_back_on_failure() {
    let mut world = WorldSnapshot::default();
    let key = TripKey::new();
    let info = trip_info("T");
    let commands: Box<[Command]> = Box::new([
        Command::AddTrip {
            key,
            info: info.clone(),
        },
        // Duplicate key: this command fails.
        Command::AddTrip { key, info },
    ]);
    assert!(world.apply_command(Command::Macro(commands)).is_none());
    assert!(!world.trips.contains_key(key));
}
