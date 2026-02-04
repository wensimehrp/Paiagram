use std::ops::AddAssign;

use crate::interval::Interval;
use crate::interval::IntervalQuery;
use crate::station::Station;
use bevy::ecs::entity::EntityHashSet;
use bevy::{ecs::entity::EntityHash, prelude::*};
use moonshine_core::prelude::MapEntities;
use petgraph::prelude::DiGraphMap;
use petgraph::{algo::astar, visit::EdgeRef};
use serde::{Deserialize, Serialize};

pub struct GraphPlugin;
impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Graph>()
            .add_observer(update_graph_on_station_removal)
            .add_observer(update_graph_on_interval_removal);
        #[cfg(debug_assertions)]
        {
            use bevy::time::common_conditions::on_real_timer;
            app.add_systems(
                PostUpdate,
                check_stations_in_graph.run_if(on_real_timer(std::time::Duration::from_secs(10))),
            );
        }
    }
}

#[derive(Reflect, Clone, Resource, Serialize, Deserialize, Default, Deref, DerefMut)]
#[reflect(Resource, opaque, Serialize, Deserialize)]
pub struct Graph {
    pub map: DiGraphMap<Entity, Entity, EntityHash>,
}

impl MapEntities for Graph {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        // construct a new graph instead
        let (nodes, edges) = self.capacity();
        let mut new_graph = DiGraphMap::with_capacity(nodes, edges);
        for (mut source, mut target, weight) in self.all_edges() {
            let mut weight = *weight;
            source.map_entities(entity_mapper);
            target.map_entities(entity_mapper);
            weight.map_entities(entity_mapper);
            new_graph.add_edge(source, target, weight);
        }
        self.map = new_graph;
    }
}

impl Graph {
    pub fn route_between<'w>(
        &self,
        source: Entity,
        target: Entity,
        interval_q: &Query<'w, 'w, IntervalQuery>,
    ) -> Option<(i32, Vec<Entity>)> {
        astar(
            &self.map,
            source,
            |f| f == target,
            |e| {
                let Ok(i) = interval_q.get(*e.weight()) else {
                    return i32::MAX;
                };
                i.distance().0
            },
            |_| 0,
        )
    }
    pub fn into_graph(self) -> petgraph::Graph<Entity, Entity> {
        self.map.into_graph()
    }
}

/// The position of the node
#[derive(Reflect, Clone, Copy)]
pub enum NodePos {
    /// X and Y used for normal mapping
    Xy { x: f64, y: f64 },
    /// Longitude and Latitude. This is for GTFS
    LonLat { lon: f64, lat: f64 },
}

impl Default for NodePos {
    fn default() -> Self {
        Self::new_xy(0.0, 0.0)
    }
}

impl NodePos {
    pub fn new_xy(x: f64, y: f64) -> Self {
        Self::Xy { x, y }
    }
    pub fn new_lon_lat(lon: f64, lat: f64) -> Self {
        Self::LonLat { lon, lat }
    }
    pub fn x(&self) -> f64 {
        match *self {
            Self::Xy { x, y: _ } => x,
            Self::LonLat { lon, lat } => {
                let (_northing, easting, _) = utm::to_utm_wgs84_no_zone(lat, lon);
                easting
            }
        }
    }
    pub fn y(&self) -> f64 {
        match *self {
            Self::Xy { x: _, y } => y,
            Self::LonLat { lon, lat } => {
                let (northing, _easting, _) = utm::to_utm_wgs84_no_zone(lat, lon);
                -northing
            }
        }
    }
    pub fn lon(&self) -> f64 {
        match *self {
            Self::Xy { x, y: _ } => x,
            Self::LonLat { lon, lat: _ } => lon,
        }
    }
    pub fn lat(&self) -> f64 {
        match *self {
            Self::Xy { x: _, y } => y,
            Self::LonLat { lon: _, lat } => lat,
        }
    }
    /// Shift the node on the canvas by x and y
    pub fn shift(&mut self, dx: f64, dy: f64) {
        match self {
            Self::Xy { x, y } => {
                *x += dx;
                *y += dy
            }
            Self::LonLat { lon, lat } => {
                let zone_num = utm::lat_lon_to_zone_number(*lat, *lon);
                let Some(zone_letter) = utm::lat_to_zone_letter(*lat) else {
                    return;
                };
                let (northing, easting, _) = utm::to_utm_wgs84(*lat, *lon, zone_num);
                let shifted_easting = easting + dx;
                let shifted_northing = northing - dy;
                if let Ok((new_lat, new_lon)) = utm::wsg84_utm_to_lat_lon(
                    shifted_easting,
                    shifted_northing,
                    zone_num,
                    zone_letter,
                ) {
                    *lat = new_lat;
                    *lon = new_lon;
                }
            }
        }
    }
}

#[derive(Default, Reflect, Component)]
#[reflect(Component)]
pub struct Node {
    pos: NodePos,
}

fn update_graph_on_station_removal(
    removed_station: On<Remove, Station>,
    mut commands: Commands,
    mut graph: ResMut<Graph>,
) {
    let s = removed_station.entity;
    for e in graph
        .neighbors_directed(s, petgraph::Direction::Incoming)
        .chain(graph.neighbors_directed(s, petgraph::Direction::Outgoing))
    {
        commands.entity(e).despawn();
    }
    graph.remove_node(s);
}

fn update_graph_on_interval_removal(
    removed_interval: On<Remove, Interval>,
    mut graph: ResMut<Graph>,
) {
    let i = removed_interval.entity;
    let mut source = None;
    let mut target = None;
    for (s, t, weight) in graph.all_edges() {
        if i != *weight {
            continue;
        }
        source = Some(s);
        target = Some(t);
        break;
    }
    let (Some(s), Some(t)) = (source, target) else {
        return;
    };
    graph.remove_edge(s, t);
}

#[cfg(debug_assertions)]
fn check_stations_in_graph(
    graph: Res<Graph>,
    stations: Populated<Entity, With<Station>>,
    intervals: Populated<Entity, With<Interval>>,
    names: Query<&Name>,
) {
    let queried_station_set: EntityHashSet = stations.iter().collect();
    let queried_interval_set: EntityHashSet = intervals.iter().collect();
    let mut graphed_station_set = EntityHashSet::new();
    let mut graphed_interval_set = EntityHashSet::new();
    for (_, _, w) in graph.all_edges() {
        graphed_interval_set.insert(*w);
    }
    for node in graph.nodes() {
        graphed_station_set.insert(node);
    }
    if queried_station_set != graphed_station_set {
        debug_graph_set_diff(
            "station",
            &queried_station_set,
            &graphed_station_set,
            &names,
        );
    }
    if queried_interval_set != graphed_interval_set {
        debug_graph_set_diff(
            "interval",
            &queried_interval_set,
            &graphed_interval_set,
            &names,
        );
    }
    assert_eq!(queried_station_set, graphed_station_set);
    assert_eq!(queried_interval_set, graphed_interval_set);
}

#[cfg(debug_assertions)]
fn debug_graph_set_diff(
    label: &str,
    queried: &EntityHashSet,
    graphed: &EntityHashSet,
    names: &Query<&Name>,
) {
    let intersection: EntityHashSet = queried.intersection(graphed).copied().collect();
    let only_queried: EntityHashSet = queried.difference(graphed).copied().collect();
    let only_graphed: EntityHashSet = graphed.difference(queried).copied().collect();

    let list_with_names = |set: &EntityHashSet| -> Vec<String> {
        let mut out: Vec<String> = set
            .iter()
            .map(|e| match names.get(*e) {
                Ok(name) => format!("{} ({})", name.as_str(), e.index()),
                Err(_) => format!("<unnamed> ({})", e.index()),
            })
            .collect();
        out.sort_unstable();
        out
    };

    warn!(
        "Graph {label} set mismatch: intersection={:#?} | only_queried={:#?} | only_graphed={:#?}",
        list_with_names(&intersection),
        list_with_names(&only_queried),
        list_with_names(&only_graphed)
    );
}
