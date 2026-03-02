use crate::{
    colors::DisplayColor,
    entry::{EntryBundle, TravelMode},
    graph::Graph,
    import::OuDiaContentType,
    route::Route,
    station::Station as StationComponent,
    trip::{
        TripBundle, TripClass,
        class::{Class as ClassComponent, ClassBundle, ClassResource, DisplayedStroke},
    },
    units::{distance::Distance, time::TimetableTime},
};
use bevy::{platform::collections::HashMap, prelude::*};
use itertools::Itertools;
use moonshine_core::kind::*;
use paiagram_oudia::{Direction, PassingMode, TimetableEntry as OuDiaTimetableEntry, parse_oud2};

#[derive(Debug, Clone, Copy)]
struct TimetableEntry {
    passing_mode: PassingMode,
    arrival: Option<TimetableTime>,
    departure: Option<TimetableTime>,
}

pub fn load_oud(
    msg: On<super::LoadOuDia>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
    class_resource: Res<ClassResource>,
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
    let root = parse_oud2(&str).expect("Failed to parse OUD/OUD2 file");
    let mut station_map: HashMap<String, Instance<StationComponent>> = HashMap::new();
    for line in root.lines {
        let mut stations: Vec<Option<Instance<StationComponent>>> = vec![None; line.stations.len()];
        let mut break_flags: Vec<bool> = Vec::with_capacity(line.stations.len());
        for (i, station) in line.stations.iter().enumerate() {
            let referenced = station
                .branch_index
                .or(station.loop_index)
                .and_then(|idx| stations.get(idx).and_then(|e| *e));
            // a bit slower but standardized
            let station_entity = referenced.unwrap_or_else(|| {
                super::make_station(&station.name, &mut station_map, &mut graph, &mut commands)
            });
            stations[i] = Some(station_entity);
            break_flags.push(station.break_interval);
        }

        let station_instances: Vec<Instance<StationComponent>> =
            stations.into_iter().map(|e| e.unwrap()).collect();
        let class_instances: Vec<Entity> = line
            .classes
            .into_iter()
            .map(|it| {
                let [r, g, b] = it.color;
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
            Name::new(line.name),
            Route {
                stops: station_instances.iter().map(|e| e.entity()).collect(),
                lengths: vec![10.0; station_instances.len()],
            },
        ));

        for i in 0..station_instances.len().saturating_sub(1) {
            if break_flags[i] {
                continue;
            }
            super::add_interval_pair(
                &mut graph,
                &mut commands,
                station_instances[i].entity(),
                station_instances[i + 1].entity(),
                Distance::from_m(1000),
            );
        }

        // TODO: find a method to support multiple diagrams
        for diagram in line.diagrams.into_iter().take(1) {
            for train in diagram.trains {
                let mut times: Vec<Option<TimetableEntry>> = train
                    .times
                    .into_iter()
                    .map(|entry| entry.map(convert_timetable_entry))
                    .collect();
                let time_iter = times.iter_mut().flat_map(|t| {
                    std::iter::once(t).flatten().flat_map(|t| {
                        std::iter::once(&mut t.arrival)
                            .flatten()
                            .chain(std::iter::once(&mut t.departure).flatten())
                    })
                });
                super::normalize_times(time_iter);

                let trip_class = train
                    .class_index
                    .map_or(class_resource.default_class, |idx| class_instances[idx]);
                commands
                    .spawn(TripBundle::new(&train.name, TripClass(trip_class.entity())))
                    .with_children(|bundle| {
                        for (stop, mut times) in &times
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, time)| {
                                let time = time?;
                                if matches!(time.passing_mode, PassingMode::NoOperation) {
                                    return None;
                                }
                                let station_index = match train.direction {
                                    Direction::Down => i,
                                    Direction::Up => station_instances.len() - 1 - i,
                                };
                                let stop = station_instances[station_index];
                                Some((stop, time))
                            })
                            .chunk_by(|(s, _t)| *s)
                        {
                            let (_, first_time) = times.next().unwrap();
                            let last_time = times.last().map(|(_, t)| t).unwrap_or(first_time);
                            let arrival = if matches!(first_time.passing_mode, PassingMode::Pass) {
                                None
                            } else {
                                Some(
                                    first_time
                                        .arrival
                                        .map_or(TravelMode::Flexible, |t| TravelMode::At(t)),
                                )
                            };
                            let departure = last_time
                                .departure
                                .map_or(TravelMode::Flexible, |t| TravelMode::At(t));
                            bundle.spawn(EntryBundle::new(arrival, departure, stop.entity()));
                        }
                    });
            }
        }
    }
}

fn convert_timetable_entry(entry: OuDiaTimetableEntry) -> TimetableEntry {
    TimetableEntry {
        passing_mode: entry.passing_mode,
        arrival: entry.arrival.map(TimetableTime),
        departure: entry.departure.map(TimetableTime),
    }
}
