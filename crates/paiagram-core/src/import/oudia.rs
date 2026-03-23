use crate::{
    colors::DisplayColor,
    entry::{EntryBundle, TravelMode},
    graph::Graph,
    import::OuDiaContentType,
    route::Route,
    station::Station as StationComponent,
    trip::{
        TripBundle, TripClass,
        class::{Class as ClassComponent, ClassBundle, DisplayedStroke},
    },
    units::{distance::Distance, time::TimetableTime},
};
use bevy::{platform::collections::HashMap, prelude::*};
use itertools::Itertools;
use moonshine_core::kind::*;
use paiagram_oudia::{Direction, ServiceMode, TimetableEntry as OuDiaTimetableEntry, parse_to_ir, Time as OuDiaTime};

#[derive(Debug, Clone, Copy)]
struct TimetableEntry {
    service_mode: ServiceMode,
    arrival_time: Option<TimetableTime>,
    departure_time: Option<TimetableTime>,
}

impl From<OuDiaTime> for TimetableTime {
    fn from(value: OuDiaTime) -> Self {
        Self(value.seconds())
    }
}

pub fn load_oud(
    msg: On<super::LoadOuDia>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
) {
    let str = match &msg.content {
        OuDiaContentType::OuDiaSecond(s) => std::borrow::Cow::Borrowed(s.as_str()),
        OuDiaContentType::OuDia(d) => {
            use encoding_rs::SHIFT_JIS;
            let (s, _, _) = SHIFT_JIS.decode(d);
            s
        }
    };
    info!("Loading OUD/OUD2 data...");
    let root = parse_to_ir(&str).expect("Failed to parse OUD/OUD2 file");
    let route = root.route;
    let mut station_map: HashMap<String, Instance<StationComponent>> = HashMap::new();
    let mut stations: Vec<Option<Instance<StationComponent>>> = vec![None; route.stations.len()];
    // let mut break_flags: Vec<bool> = Vec::with_capacity(route.stations.len());
    for (i, station) in route.stations.iter().enumerate() {
        // a bit slower but standardized
        let station_entity =
            super::make_station(&station.name, &mut station_map, &mut graph, &mut commands);
        stations[i] = Some(station_entity);
        // TODO: restore interval breaking mechanism
        // break_flags.push(station.break_interval);
    }

    let station_instances: Vec<Instance<StationComponent>> =
        stations.into_iter().map(|e| e.unwrap()).collect();
    let class_instances: Vec<Entity> = route
        .classes
        .into_iter()
        .map(|it| {
            let [_, r, g, b] = it.diagram_line_color.0;
            commands
                .spawn(ClassBundle {
                    class: ClassComponent::default(),
                    name: Name::new(it.name),
                    stroke: DisplayedStroke {
                        color: DisplayColor::Custom(egui::Color32::from_rgb(r, g, b)),
                        width: 1.0,
                    },
                })
                .id()
        })
        .collect();

    commands.spawn((
        Name::new(route.name),
        Route {
            stops: station_instances.iter().map(|e| e.entity()).collect(),
            lengths: vec![10.0; station_instances.len()],
        },
    ));

    for i in 0..station_instances.len().saturating_sub(1) {
        // if break_flags[i] {
        //     continue;
        // }
        super::add_interval_pair(
            &mut graph,
            &mut commands,
            station_instances[i].entity(),
            station_instances[i + 1].entity(),
            Distance::from_m(1000),
        );
    }

    // TODO: find a method to support multiple diagrams
    for diagram in route.diagrams.into_iter().take(1) {
        for trip in diagram.trips {
            let times: Vec<TimetableEntry> = trip
                .times
                .into_iter()
                .map(convert_timetable_entry)
                .collect();

            let trip_class = class_instances[trip.class_index];

            let mut times_chunked: Vec<_> = times
                .into_iter()
                .enumerate()
                .filter_map(|(i, time)| {
                    if matches!(time.service_mode, ServiceMode::NoOperation) {
                        return None;
                    }
                    let station_index = match trip.direction {
                        Direction::Down => i,
                        Direction::Up => station_instances.len() - 1 - i,
                    };
                    let stop = station_instances[station_index];
                    Some((stop, time))
                })
                .chunk_by(|(s, _t)| *s)
                .into_iter()
                .map(|(s, mut g)| {
                    let (_, first_time) = g.next().unwrap();
                    let mut group = [None; 2];
                    group[0] = first_time.arrival_time;
                    group[1] = first_time.departure_time;
                    if let Some((_, last_time)) = g.last() {
                        group[1] = last_time.departure_time;
                    }
                    (s, group, first_time.service_mode)
                })
                .collect();

            super::normalize_times(times_chunked.iter_mut().flat_map(|(_, g, _)| g).flatten());

            let nominal_entries: Vec<_> = times_chunked
                .into_iter()
                .map(|(stop, [arrival_time, departure_time], passing_mode)| {
                    // in this case, this would consume the iterator.
                    let arrival_mode = if matches!(passing_mode, ServiceMode::Pass) {
                        None
                    } else {
                        Some(arrival_time.map_or(TravelMode::Flexible, |t| TravelMode::At(t)))
                    };
                    let departure_mode =
                        departure_time.map_or(TravelMode::Flexible, |t| TravelMode::At(t));
                    commands
                        .spawn(EntryBundle::new(
                            arrival_mode,
                            departure_mode,
                            stop.entity(),
                        ))
                        .id()
                })
                .collect();

            commands
                .spawn_empty()
                .add_children(&nominal_entries)
                .insert(TripBundle::new(
                    &trip.name.unwrap_or("<unnamed>".to_string()),
                    TripClass(trip_class.entity()),
                    nominal_entries,
                ));
        }
    }
}

fn convert_timetable_entry(entry: OuDiaTimetableEntry) -> TimetableEntry {
    TimetableEntry {
        service_mode: entry.service_mode,
        arrival_time: entry.arrival_time.map(TimetableTime::from),
        departure_time: entry.arrival_time.map(TimetableTime::from),
    }
}
