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
    let inv = world
        .apply_command(Command::RemoveTrip { key: trip })
        .unwrap();
    assert!(
        world
            .vehicles
            .query(vehicle, |v| v.trips.clone())
            .unwrap()
            .is_empty()
    );

    // Undo restores the trip and re-populates the vehicle cache.
    let _ = world.apply_command(inv).unwrap();
    assert!(world.trips.contains_key(trip));
    assert_eq!(
        world
            .vehicles
            .query(vehicle, |v| v.trips.clone())
            .unwrap()
            .as_slice(),
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
    assert!(
        world
            .vehicles
            .query(vehicle, |v| v.trips.clone())
            .unwrap()
            .is_empty()
    );

    world.rebuild_vehicle_trip_cache();
    assert_eq!(
        world
            .vehicles
            .query(vehicle, |v| v.trips.clone())
            .unwrap()
            .as_slice(),
        &[trip]
    );
}

#[test]
fn interval_maintains_world_graph() {
    let mut world = WorldSnapshot::default();
    let source = NodeKey::new();
    let target = NodeKey::new();
    for node in [source, target] {
        let parent = StationKey::new();
        world
            .apply_command(Command::AddStation {
                key: parent,
                info: StationInfo {
                    name: "S".into(),
                    pos: LonLat::ZERO,
                },
            })
            .unwrap();
        world
            .apply_command(Command::AddNode {
                key: node,
                info: NodeInfo {
                    name: "".into(),
                    parent,
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
        let parent = StationKey::new();
        world
            .apply_command(Command::AddStation {
                key: parent,
                info: StationInfo {
                    name: "S".into(),
                    pos: LonLat::ZERO,
                },
            })
            .unwrap();
        world
            .apply_command(Command::AddNode {
                key: node,
                info: NodeInfo {
                    name: "".into(),
                    parent,
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
    let inv = world
        .apply_command(Command::RemoveInterval { key: ab })
        .unwrap();
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

fn network_fixture() -> (Source, StationKey, StationKey, NodeKey, NodeKey, TripKey) {
    use crate::trip::{TEntry, TravelMode};
    let mut source = Source::new();
    let (s1, s2) = (StationKey::new(), StationKey::new());
    let (a, b) = (NodeKey::new(), NodeKey::new());
    let trip = TripKey::new();
    for (station, node, lon) in [(s1, a, 0.0), (s2, b, 0.01)] {
        let pos = Wgs84LonLat::new(lon, 45.0).into();
        assert!(source.apply_command(Command::AddStation {
            key: station,
            info: StationInfo {
                name: "S".into(),
                pos
            }
        }));
        assert!(source.apply_command(Command::AddNode {
            key: node,
            info: NodeInfo {
                name: "1".into(),
                parent: station,
                pos,
                is_platform: true
            }
        }));
    }
    assert!(source.apply_command(Command::AddInterval {
        key: (a, b),
        info: Interval {
            nodes: eco_vec![],
            length: NonZeroU32::new(1000),
            trips: eco_vec![]
        }
    }));
    let schedule = TripSchedule::new(
        [(a, 86_100), (b, 86_700)]
            .into_iter()
            .map(|(node, t)| TEntry::PinnedNonStop {
                node,
                pass: TravelMode::At(time::TimetableTime(t)),
                external: false,
                id: TEntryId::new(),
            })
            .collect(),
    );
    assert!(source.apply_command(Command::AddTrip {
        key: trip,
        info: TripInfo {
            name: "Overnight".into(),
            schedule,
            service_class: None,
            vehicles: SmallVec::new()
        }
    }));
    (source, s1, s2, a, b, trip)
}

#[test]
fn entry_edits_update_usage_and_are_reversible() {
    let (mut source, _, _, a, b, trip) = network_fixture();
    let entries = source
        .trips
        .query(trip, |v| v.schedule.entries().to_vec())
        .unwrap();
    assert_eq!(
        source.intervals.get((a, b)).unwrap().trips.as_slice(),
        &[trip]
    );
    assert!(source.apply_command(Command::RemoveTripEntry {
        key: trip,
        id: entries[1].id()
    }));
    assert!(source.intervals.get((a, b)).unwrap().trips.is_empty());
    assert!(source.undo());
    assert_eq!(
        source.intervals.get((a, b)).unwrap().trips.as_slice(),
        &[trip]
    );
    assert!(source.redo());
    assert!(source.apply_command(Command::InsertTripEntry {
        key: trip,
        entry: entries[1],
        pos: 1
    }));
    assert!(!source.apply_command(Command::InsertTripEntry {
        key: trip,
        entry: entries[1],
        pos: 1
    }));
    assert!(!source.apply_command(Command::InsertTripEntry {
        key: trip,
        entry: entries[0],
        pos: 99
    }));
    let mut replacement = entries[1];
    if let trip::TEntry::PinnedNonStop { ref mut node, .. } = replacement {
        *node = a;
    }
    assert!(source.apply_command(Command::ChangeTripEntry {
        key: trip,
        id: replacement.id(),
        new_entry: replacement
    }));
    assert!(source.intervals.get((a, b)).unwrap().trips.is_empty());
    assert!(source.undo());
    assert_eq!(
        source
            .trips
            .query(trip, |v| v.schedule.entries().to_vec())
            .unwrap(),
        entries
    );
}

#[test]
fn serialized_world_rebuilds_all_relationships_and_source_geometry() {
    let (source, s1, _, a, b, trip) = network_fixture();
    let bytes = cbor4ii::serde::to_vec(Vec::new(), &SaveFile::from(source.snap().clone())).unwrap();
    let save: SaveFile = cbor4ii::serde::from_slice(&bytes).unwrap();
    let loaded = Source::try_from(save).unwrap();
    assert_eq!(
        loaded
            .stations
            .query(s1, |v| v.nodes.clone())
            .unwrap()
            .as_slice(),
        &[a]
    );
    assert_eq!(
        loaded
            .nodes
            .query(a, |v| v.outgoing.clone())
            .unwrap()
            .as_slice(),
        &[b]
    );
    assert_eq!(
        loaded.intervals.get((a, b)).unwrap().trips.as_slice(),
        &[trip]
    );
    assert_eq!(
        loaded
            .graph_cache()
            .nodes([i32::MIN; 2], [i32::MAX; 2])
            .count(),
        2
    );
    assert!(
        !loaded
            .graph_cache()
            .trips([i32::MIN; 2], [i32::MAX; 2], 0.0, 86400.0)
            .is_empty()
    );
}

#[test]
fn node_move_reparent_and_undo_restore_geometry_and_membership() {
    let (mut source, s1, s2, a, b, _) = network_fixture();
    let old = source.nodes.query(a, |v| *v.pos).unwrap();
    let original_edge = source.intervals.get((a, b)).unwrap().clone();
    let pos = Wgs84LonLat::new(0.005, 45.01).into();
    assert!(source.apply_command(Command::ChangeNode {
        key: a,
        info: NodeInfo {
            name: "2".into(),
            parent: s2,
            pos,
            is_platform: false
        }
    }));
    assert!(source.stations.query(s1, |v| v.nodes.is_empty()).unwrap());
    assert!(source.stations.query(s2, |v| v.nodes.contains(&a)).unwrap());
    assert_eq!(source.intervals.get((a, b)).unwrap().nodes[0], pos);
    let p = [pos.lon, pos.lat];
    assert!(source.graph_cache().nodes(p, p).any(|n| n.key == a));
    assert!(source.undo());
    assert_eq!(source.nodes.query(a, |v| *v.pos).unwrap(), old);
    assert_eq!(source.intervals.get((a, b)).unwrap(), &original_edge);
    assert_eq!(
        source
            .stations
            .query(s1, |v| v.nodes.clone())
            .unwrap()
            .as_slice(),
        &[a]
    );
    assert!(source.redo());
    assert_eq!(source.nodes.query(a, |v| *v.pos).unwrap(), pos);
}

#[test]
fn deleting_vehicle_restores_assignments_including_order() {
    let (mut source, _, _, _, _, trip) = network_fixture();
    let (a, b) = (VehicleKey::new(), VehicleKey::new());
    for key in [a, b] {
        assert!(source.apply_command(Command::AddVehicle {
            key,
            name: "V".into()
        }));
    }
    assert!(source.apply_command(Command::ChangeTripVehicles {
        key: trip,
        vehicles: smallvec::smallvec![a, b]
    }));
    assert!(source.apply_command(Command::RemoveVehicle { key: a }));
    assert!(source.undo());
    assert_eq!(
        source
            .trips
            .query(trip, |v| v.vehicles.clone())
            .unwrap()
            .as_slice(),
        &[a, b]
    );
    assert_eq!(
        source
            .vehicles
            .query(a, |v| v.trips.clone())
            .unwrap()
            .as_slice(),
        &[trip]
    );
    assert!(source.redo());
    assert_eq!(
        source
            .trips
            .query(trip, |v| v.vehicles.clone())
            .unwrap()
            .as_slice(),
        &[b]
    );
}

#[test]
fn failed_macro_preserves_relationships_and_spatial_cache() {
    let (mut source, s1, _, a, _, _) = network_fixture();
    let pos = source.nodes.query(a, |v| *v.pos).unwrap();
    assert!(!source.apply_command(Command::Macro(Box::new([
        Command::ChangeNode {
            key: a,
            info: NodeInfo {
                name: "changed".into(),
                parent: s1,
                pos: LonLat::ZERO,
                is_platform: true
            }
        },
        Command::RemoveStation { key: s1 },
    ]))));
    assert_eq!(source.nodes.query(a, |v| *v.pos).unwrap(), pos);
    let p = [pos.lon, pos.lat];
    assert!(source.graph_cache().nodes(p, p).any(|n| n.key == a));
    assert!(!source.apply_command(Command::AddNode {
        key: NodeKey::new(),
        info: NodeInfo {
            name: "bad".into(),
            parent: StationKey::new(),
            pos,
            is_platform: true
        }
    }));
}

#[test]
fn overnight_spatial_samples_interpolate_and_timing_edits_invalidate() {
    let (mut source, _, _, a, b, trip) = network_fixture();
    let low = [i32::MIN; 2];
    let high = [i32::MAX; 2];
    let graph_cache = source.graph_cache();
    let samples = graph_cache.trips(low, high, 0.0, 86400.0);
    let (s, t) = samples.iter().find(|(s, _)| s.from < s.until).unwrap();
    let p = XyPosF64::from(s.position_and_angle(*t).unwrap().0);
    let pa = source.nodes.query(a, |v| spatial::project(*v.pos)).unwrap();
    let pb = source.nodes.query(b, |v| spatial::project(*v.pos)).unwrap();
    assert!((p.x - (pa[0] + pb[0]) / 2.0).abs() <= 0.01);
    let id = source
        .trips
        .query(trip, |v| v.schedule.entries()[1].id())
        .unwrap();
    assert!(source.apply_command(Command::ShiftTripEntryArrOrPass {
        key: trip,
        id,
        dur: time::TDuration::from_hms(0, 10, 0)
    }));
    assert!(
        !source
            .graph_cache()
            .trips(low, high, 87100.0, 0.0)
            .is_empty()
    );
    assert!(source.undo());
    assert!(
        source
            .graph_cache()
            .trips(low, high, 87100.0, 0.0)
            .is_empty()
    );
    assert!(source.apply_command(Command::UnloadWorld));
    assert_eq!(source.graph_cache().nodes(low, high).count(), 0);
    assert!(source.undo());
    assert_eq!(source.graph_cache().nodes(low, high).count(), 2);
}

#[test]
fn loading_entry_ids_advances_the_allocator() {
    let saved: TEntryId = serde_json::from_str("100000000").unwrap();
    let generated = TEntryId::new();
    assert_ne!(saved, generated);
    let value: u32 = serde_json::from_str(&serde_json::to_string(&generated).unwrap()).unwrap();
    assert!(value > 100000000);
}

#[test]
fn oudia_import_is_an_applicable_reversible_batch() {
    let bytes = include_bytes!("../../paiagram-oudia/test/sample.oud2");
    let command = import::generate_commands(bytes, import::ImportType::OuDiaSecond).unwrap();
    let mut source = Source::new();
    assert!(source.apply_command(command));
    assert!(source.nodes.len() > 0);
    for node in source.nodes.iter() {
        assert!(
            source
                .stations
                .query(*node.parent, |s| s.nodes.contains(&node.key))
                .unwrap()
        );
    }
    for trip in source.trips.iter() {
        assert!(
            trip.service_class
                .is_none_or(|k| source.service_classes.contains_key(k))
        );
    }
    assert!(source.undo());
    assert_eq!(source.nodes.len(), 0);
    assert!(source.redo());
}

#[test]
fn route_records_keep_intermediate_switches_and_reject_invalid_subsets() {
    let (mut source, s1, s2, a, b, _) = network_fixture();
    let middle = NodeKey::new();
    assert!(source.apply_command(Command::AddNode {
        key: middle,
        info: NodeInfo {
            name: "switch".into(),
            parent: s1,
            pos: LonLat::ZERO,
            is_platform: false
        }
    }));
    for edge in [(a, middle), (middle, b)] {
        assert!(source.apply_command(Command::AddInterval {
            key: edge,
            info: Interval {
                nodes: eco_vec![],
                length: NonZeroU32::new(10),
                trips: eco_vec![]
            }
        }));
    }
    let records = eco_vec![
        RouteStationRecord::for_station(source.snap(), s1, None),
        RouteStationRecord::for_station(source.snap(), s2, Some(s1))
    ];
    assert!(records[1].prev_curr_nodes.contains(&middle));
    let route = RouteKey::new();
    assert!(source.apply_command(Command::AddRoute {
        key: route,
        info: RouteInfo {
            name: "R".into(),
            stations: records.clone()
        }
    }));
    let mut edited = records.clone();
    edited.make_mut()[1].milestone = Some(Distance(1234));
    assert!(source.apply_command(Command::ChangeRouteStations {
        key: route,
        stations: edited
    }));
    assert!(source.undo());
    assert_eq!(
        source.routes.query(route, |v| v.stations.clone()).unwrap(),
        records
    );
    let mut invalid = records;
    invalid.make_mut()[0].stn = StationRecord::Some(eco_vec![middle]);
    assert!(!source.apply_command(Command::ChangeRouteStations {
        key: route,
        stations: invalid
    }));
}

#[test]
fn external_entries_do_not_break_internal_interval_usage() {
    let (mut source, _, _, a, b, trip) = network_fixture();
    let external = trip::TEntry::PinnedNonStop {
        node: a,
        pass: trip::TravelMode::Flexible,
        external: true,
        id: TEntryId::new(),
    };
    assert!(source.apply_command(Command::InsertTripEntry {
        key: trip,
        entry: external,
        pos: 1
    }));
    assert_eq!(
        source.intervals.get((a, b)).unwrap().trips.as_slice(),
        &[trip]
    );
    assert!(
        !source
            .graph_cache()
            .trips([i32::MIN; 2], [i32::MAX; 2], 86400.0, 0.0)
            .is_empty()
    );
}

#[test]
fn route_progress_ignores_unreachable_platforms_and_filters_branches() {
    let (mut source, s1, s2, a, b, _) = network_fixture();
    let isolated = NodeKey::new();
    let middle = NodeKey::new();
    for (key, platform) in [(isolated, true), (middle, false)] {
        assert!(source.apply_command(Command::AddNode {
            key,
            info: NodeInfo {
                name: "N".into(),
                parent: s1,
                pos: LonLat::ZERO,
                is_platform: platform
            }
        }));
    }
    for key in [(a, middle), (middle, b)] {
        assert!(source.apply_command(Command::AddInterval {
            key,
            info: Interval {
                nodes: eco_vec![],
                length: NonZeroU32::new(100),
                trips: eco_vec![]
            }
        }));
    }
    let route = RouteInfo {
        name: "R".into(),
        stations: eco_vec![
            RouteStationRecord::for_station(source.snap(), s1, None),
            RouteStationRecord::for_station(source.snap(), s2, Some(s1))
        ],
    };
    let progress = route.gen_progresses(source.snap());
    assert_eq!(progress.len(), 1);
    assert!((progress[0].0[0].unwrap().to_ratio() - 0.5).abs() < 0.001);
    let mut excluded = route;
    excluded.stations.make_mut()[1].prev_curr_nodes = eco_vec![isolated];
    assert_eq!(excluded.gen_progresses(source.snap())[0].0[0], None);
}
