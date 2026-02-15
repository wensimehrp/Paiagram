use std::collections::HashMap;

pub mod arrange;

use crate::entry::EntryStop;
use crate::interval::Interval;
use crate::interval::IntervalQuery;
use crate::route::Route;
use crate::station::Platforms;
use crate::station::Station;
use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::entity::EntityHashSet;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future::poll_once};
use bevy::{ecs::entity::EntityHash, prelude::*};
use moonshine_core::prelude::MapEntities;
use petgraph::prelude::DiGraphMap;
use petgraph::{algo::astar, visit::EdgeRef};
use rstar::{AABB, PointDistance, RTree, RTreeObject};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub struct GraphPlugin;
impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Graph>()
            .init_resource::<GraphSpatialIndex>()
            .init_resource::<GraphSpatialIndexState>()
            .init_resource::<GraphIntervalSpatialIndex>()
            .init_resource::<GraphIntervalSpatialIndexState>()
            .add_systems(Update, arrange::apply_graph_layout_task)
            .add_systems(
                Update,
                (
                    mark_graph_spatial_index_dirty,
                    start_graph_spatial_index_rebuild,
                    apply_graph_spatial_index_task,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    mark_graph_interval_spatial_index_dirty,
                    start_graph_interval_spatial_index_rebuild,
                    apply_graph_interval_spatial_index_task,
                )
                    .chain(),
            )
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

#[derive(Clone, Copy, Debug)]
struct SpatialIndexedEntity {
    entity: Entity,
    point: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
struct IntervalSpatialIndexedEntity {
    interval: Entity,
    p0: [f64; 2],
    p1: [f64; 2],
}

impl RTreeObject for SpatialIndexedEntity {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.point)
    }
}

impl PointDistance for SpatialIndexedEntity {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let dx = self.point[0] - point[0];
        let dy = self.point[1] - point[1];
        dx * dx + dy * dy
    }
}

impl RTreeObject for IntervalSpatialIndexedEntity {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.p0[0].min(self.p1[0]), self.p0[1].min(self.p1[1])],
            [self.p0[0].max(self.p1[0]), self.p0[1].max(self.p1[1])],
        )
    }
}

#[derive(Resource, Default)]
pub struct GraphSpatialIndex {
    tree: RTree<SpatialIndexedEntity>,
}

#[derive(Clone, Copy, Debug)]
pub struct GraphIntervalSpatialSample {
    pub interval: Entity,
    pub p0: [f64; 2],
    pub p1: [f64; 2],
}

#[derive(Resource, Default)]
pub struct GraphIntervalSpatialIndex {
    tree: RTree<IntervalSpatialIndexedEntity>,
}

impl GraphSpatialIndex {
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn clear(&mut self) {
        self.tree = RTree::new();
    }

    pub fn insert_xy(&mut self, entity: Entity, x: f64, y: f64) {
        self.tree.insert(SpatialIndexedEntity {
            entity,
            point: [x, y],
        });
    }

    pub fn insert_lon_lat(&mut self, entity: Entity, lon: f64, lat: f64) {
        let (x, y) = lon_lat_to_xy(lon, lat);
        self.insert_xy(entity, x, y);
    }

    pub fn entities_in_xy_aabb(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    ) -> Vec<Entity> {
        let envelope = AABB::from_corners(
            [min_x.min(max_x), min_y.min(max_y)],
            [min_x.max(max_x), min_y.max(max_y)],
        );
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|entry| entry.entity)
            .collect()
    }

    pub fn entities_in_lon_lat_aabb(
        &self,
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    ) -> Vec<Entity> {
        let (x0, y0) = lon_lat_to_xy(min_lon, min_lat);
        let (x1, y1) = lon_lat_to_xy(max_lon, max_lat);
        self.entities_in_xy_aabb(x0, y0, x1, y1)
    }

    pub fn nearest_in_xy(&self, x: f64, y: f64) -> Option<Entity> {
        self.tree
            .nearest_neighbor(&[x, y])
            .map(|entry| entry.entity)
    }

    pub fn nearest_in_lon_lat(&self, lon: f64, lat: f64) -> Option<Entity> {
        let (x, y) = lon_lat_to_xy(lon, lat);
        self.nearest_in_xy(x, y)
    }

    fn replace_tree(&mut self, tree: RTree<SpatialIndexedEntity>) {
        self.tree = tree;
    }
}

impl GraphIntervalSpatialIndex {
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn query_xy_aabb(&self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<GraphIntervalSpatialSample> {
        if self.is_empty() {
            return Vec::new();
        }

        let envelope = AABB::from_corners(
            [min_x.min(max_x), min_y.min(max_y)],
            [min_x.max(max_x), min_y.max(max_y)],
        );

        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|item| GraphIntervalSpatialSample {
                interval: item.interval,
                p0: item.p0,
                p1: item.p1,
            })
            .collect()
    }

    fn replace_tree(&mut self, tree: RTree<IntervalSpatialIndexedEntity>) {
        self.tree = tree;
    }
}

#[derive(Resource)]
struct GraphSpatialIndexState {
    dirty: bool,
    task: Option<Task<RTree<SpatialIndexedEntity>>>,
}

#[derive(Resource)]
struct GraphIntervalSpatialIndexState {
    dirty: bool,
    task: Option<Task<RTree<IntervalSpatialIndexedEntity>>>,
}

impl Default for GraphSpatialIndexState {
    fn default() -> Self {
        Self {
            dirty: true,
            task: None,
        }
    }
}

impl Default for GraphIntervalSpatialIndexState {
    fn default() -> Self {
        Self {
            dirty: true,
            task: None,
        }
    }
}

fn lon_lat_to_xy(lon: f64, lat: f64) -> (f64, f64) {
    let (northing, easting, _) = utm::to_utm_wgs84_no_zone(lat, lon);
    (easting, -northing)
}

fn mark_graph_spatial_index_dirty(
    mut state: ResMut<GraphSpatialIndexState>,
    changed_nodes: Query<(), Or<(Added<Node>, Changed<Node>)>>,
    mut removed_nodes: RemovedComponents<Node>,
) {
    if !changed_nodes.is_empty() || removed_nodes.read().next().is_some() {
        state.dirty = true;
    }
}

fn mark_graph_interval_spatial_index_dirty(
    mut state: ResMut<GraphIntervalSpatialIndexState>,
    graph: Res<Graph>,
    changed_nodes: Query<(), Or<(Added<Node>, Changed<Node>)>>,
    changed_intervals: Query<(), Or<(Added<Interval>, Changed<Interval>)>>,
    mut removed_nodes: RemovedComponents<Node>,
    mut removed_intervals: RemovedComponents<Interval>,
) {
    if graph.is_added()
        || graph.is_changed()
        || !changed_nodes.is_empty()
        || !changed_intervals.is_empty()
        || removed_nodes.read().next().is_some()
        || removed_intervals.read().next().is_some()
    {
        state.dirty = true;
    }
}

fn start_graph_spatial_index_rebuild(
    mut state: ResMut<GraphSpatialIndexState>,
    nodes: Query<(Entity, &Node)>,
) {
    if !state.dirty || state.task.is_some() {
        return;
    }
    state.dirty = false;

    let snapshot: Vec<(Entity, [f64; 2])> = nodes
        .iter()
        .map(|(entity, node)| (entity, [node.pos.x(), node.pos.y()]))
        .collect();
    state.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let entries: Vec<SpatialIndexedEntity> = snapshot
            .into_iter()
            .map(|(entity, point)| SpatialIndexedEntity { entity, point })
            .collect();
        RTree::bulk_load(entries)
    }));
}

fn start_graph_interval_spatial_index_rebuild(
    mut state: ResMut<GraphIntervalSpatialIndexState>,
    graph: Res<Graph>,
    nodes: Query<&Node>,
) {
    if !state.dirty || state.task.is_some() {
        return;
    }
    state.dirty = false;

    let mut snapshot = Vec::<IntervalSpatialIndexedEntity>::new();
    for (source, target, interval) in graph.all_edges() {
        let Ok(source_node) = nodes.get(source) else {
            continue;
        };
        let Ok(target_node) = nodes.get(target) else {
            continue;
        };
        snapshot.push(IntervalSpatialIndexedEntity {
            interval: *interval,
            p0: [source_node.pos.x(), source_node.pos.y()],
            p1: [target_node.pos.x(), target_node.pos.y()],
        });
    }

    state.task = Some(AsyncComputeTaskPool::get().spawn(async move { RTree::bulk_load(snapshot) }));
}

fn apply_graph_spatial_index_task(
    mut state: ResMut<GraphSpatialIndexState>,
    mut index: ResMut<GraphSpatialIndex>,
) {
    let Some(task) = state.task.as_mut() else {
        return;
    };
    let Some(tree) = block_on(poll_once(task)) else {
        return;
    };
    index.replace_tree(tree);
    state.task = None;
}

fn apply_graph_interval_spatial_index_task(
    mut state: ResMut<GraphIntervalSpatialIndexState>,
    mut index: ResMut<GraphIntervalSpatialIndex>,
) {
    let Some(task) = state.task.as_mut() else {
        return;
    };
    let Some(tree) = block_on(poll_once(task)) else {
        return;
    };
    index.replace_tree(tree);
    state.task = None;
}

/// The position of the node
#[derive(Reflect, Clone, Copy, Debug)]
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
    /// Linearly interpolates between `self` and `other` by fraction `t`.
    /// `t` is typically between 0.0 and 1.0.
    /// The returned `NodePos` matches the variant of `self`.
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        match *self {
            Self::Xy { x, y } => {
                let end_x = other.x();
                let end_y = other.y();

                Self::Xy {
                    x: x + (end_x - x) * t,
                    y: y + (end_y - y) * t,
                }
            }
            Self::LonLat { lon, lat } => {
                let end_lon = other.lon();
                let end_lat = other.lat();

                Self::LonLat {
                    lon: lon + (end_lon - lon) * t,
                    lat: lat + (end_lat - lat) * t,
                }
            }
        }
    }
}

#[derive(Default, Reflect, Component, Debug)]
#[reflect(Component)]
pub struct Node {
    pub pos: NodePos,
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
    debug_assert_eq!(queried_station_set, graphed_station_set);
    debug_assert_eq!(queried_interval_set, graphed_interval_set);
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

pub fn merge_station_by_name(
    mut commands: Commands,
    mut graph: ResMut<Graph>,
    stations: Query<(Entity, &Name, &Platforms), With<Station>>,
    entry_stops: Query<(Entity, &EntryStop)>,
    mut routes: Query<&mut Route>,
) {
    let mut name_map: HashMap<&str, SmallVec<[Entity; 1]>> = HashMap::new();
    for (entity, name, _) in &stations {
        let v = name_map.entry(name.as_str()).or_default();
        v.push(entity);
    }

    let mut remap: EntityHashMap<Entity> = EntityHashMap::default();

    for (_name, mut entities) in name_map.into_iter().filter(|(_, v)| v.len() > 1) {
        entities.sort_unstable_by_key(|entity| entity.index());
        let keep = entities[0];

        for duplicate in entities.into_iter().skip(1) {
            if let Ok((_, _, platforms)) = stations.get(duplicate) {
                let to_move: SmallVec<[Entity; 8]> = platforms.iter().collect();
                if !to_move.is_empty() {
                    commands.entity(keep).add_children(&to_move);
                }
            }
            remap.insert(duplicate, keep);
        }
    }

    if remap.is_empty() {
        return;
    }

    let (nodes, edges) = graph.capacity();
    let mut new_graph = DiGraphMap::with_capacity(nodes, edges);
    let mut removed_intervals = EntityHashSet::default();
    for (source, target, weight) in graph.all_edges() {
        let source = remap.get(&source).copied().unwrap_or(source);
        let target = remap.get(&target).copied().unwrap_or(target);
        if let Some(existing_weight) = new_graph.edge_weight(source, target) {
            if *existing_weight != *weight {
                removed_intervals.insert(*weight);
            }
            continue;
        }
        new_graph.add_edge(source, target, *weight);
    }
    for node in graph.nodes() {
        let node = remap.get(&node).copied().unwrap_or(node);
        new_graph.add_node(node);
    }
    graph.map = new_graph;

    for (entry, stop) in &entry_stops {
        if let Some(new_stop) = remap.get(&stop.0) {
            commands.entity(entry).insert(EntryStop(*new_stop));
        }
    }

    for mut route in &mut routes {
        for stop in &mut route.stops {
            if let Some(new_stop) = remap.get(stop) {
                *stop = *new_stop;
            }
        }
    }

    for interval in removed_intervals {
        commands.entity(interval).despawn();
    }

    for duplicate in remap.into_keys() {
        commands.entity(duplicate).despawn();
    }
}
