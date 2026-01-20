use crate::graph::arrange::GraphLayoutTask;
use crate::units::speed::Velocity;
use crate::vehicles::entries::TimetableEntry;
use bevy::ecs::entity::{EntityHashMap, EntityMapper, MapEntities};
use bevy::prelude::*;
use either::Either;
use moonshine_core::kind::prelude::*;
use moonshine_core::save::prelude::*;
use petgraph::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod arrange;

/// The graph type used for the transportation network
pub type IntervalGraphType = StableDiGraph<Instance<Station>, Instance<Interval>>;
/// A raw graph type used for serialization/deserialization
#[derive(Serialize, Deserialize, MapEntities)]
pub struct RawIntervalGraphType(StableDiGraph<Entity, Entity>);

/// A graph representing the transportation network
#[derive(Reflect, Clone, Resource, Default, Debug)]
#[reflect(Resource, opaque, Serialize, Deserialize, MapEntities)]
pub struct Graph {
    /// The inner graph structure
    inner: IntervalGraphType,
    /// Mapping from station entities to their node indices in the graph
    /// This is skipped during serialization/deserialization and rebuilt as needed
    #[reflect(ignore)]
    indices: EntityHashMap<NodeIndex>,
}

impl From<RawIntervalGraphType> for IntervalGraphType {
    fn from(value: RawIntervalGraphType) -> Self {
        value.0.map(
            |_, &node| unsafe { Instance::from_entity_unchecked(node) },
            |_, &edge| unsafe { Instance::from_entity_unchecked(edge) },
        )
    }
}

impl From<IntervalGraphType> for RawIntervalGraphType {
    fn from(value: IntervalGraphType) -> Self {
        Self(value.map(|_, node| node.entity(), |_, edge| edge.entity()))
    }
}

impl Serialize for Graph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw: RawIntervalGraphType = self.inner.clone().into();
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Graph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawIntervalGraphType::deserialize(deserializer)?;
        let inner: IntervalGraphType = raw.into();
        Ok(Graph {
            inner,
            indices: EntityHashMap::default(),
        })
    }
}

pub struct EdgeReference {
    pub weight: Instance<Interval>,
    pub source: Instance<Station>,
    pub target: Instance<Station>,
}

impl MapEntities for Graph {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for node in self.inner.node_weights_mut() {
            node.map_entities(entity_mapper);
        }
        for edge in self.inner.edge_weights_mut() {
            edge.map_entities(entity_mapper);
        }
        let mut new_indices = EntityHashMap::default();
        // construct the indices from the graph instead.
        for index in self.inner.node_indices() {
            let station = &self.inner[index];
            new_indices.insert(station.entity(), index);
        }
        self.indices = new_indices;
    }
}

impl Graph {
    pub fn inner(&self) -> &IntervalGraphType {
        &self.inner
    }
    pub fn clear(&mut self) {
        self.inner.clear();
        self.indices.clear();
    }
    pub fn edge_weight(
        &self,
        a: Instance<Station>,
        b: Instance<Station>,
    ) -> Option<&Instance<Interval>> {
        let &a_index = self.indices.get(&a.entity())?;
        let &b_index = self.indices.get(&b.entity())?;
        self.inner
            .edge_weight(self.inner.find_edge(a_index, b_index)?)
    }
    pub fn contains_edge(&self, a: Instance<Station>, b: Instance<Station>) -> bool {
        let Some(&a_index) = self.indices.get(&a.entity()) else {
            return false;
        };
        let Some(&b_index) = self.indices.get(&b.entity()) else {
            return false;
        };
        self.inner.find_edge(a_index, b_index).is_some()
    }
    pub fn contains_node(&self, a: Instance<Station>) -> bool {
        self.indices.contains_key(&a.entity())
    }
    pub fn node_index(&self, a: Instance<Station>) -> Option<NodeIndex> {
        self.indices.get(&a.entity()).cloned()
    }
    pub fn entity(&self, index: NodeIndex) -> Option<Instance<Station>> {
        self.inner.node_weight(index).cloned()
    }
    pub fn add_edge(
        &mut self,
        a: Instance<Station>,
        b: Instance<Station>,
        edge: Instance<Interval>,
    ) {
        let a_index = if let Some(&index) = self.indices.get(&a.entity()) {
            index
        } else {
            let index = self.inner.add_node(a);
            self.indices.insert(a.entity(), index);
            index
        };
        let b_index = if let Some(&index) = self.indices.get(&b.entity()) {
            index
        } else {
            let index = self.inner.add_node(b);
            self.indices.insert(b.entity(), index);
            index
        };
        self.inner.add_edge(a_index, b_index, edge);
    }
    pub fn add_node(&mut self, a: Instance<Station>) {
        if self.indices.contains_key(&a.entity()) {
            return;
        }
        let index = self.inner.add_node(a);
        self.indices.insert(a.entity(), index);
    }
    pub fn edges_connecting(
        &self,
        a: Instance<Station>,
        b: Instance<Station>,
    ) -> impl Iterator<Item = EdgeReference> {
        let a_idx = match self.indices.get(&a.entity()) {
            None => return Either::Left(std::iter::empty()),
            Some(i) => i.clone(),
        };
        let b_idx = match self.indices.get(&b.entity()) {
            None => return Either::Left(std::iter::empty()),
            Some(i) => i.clone(),
        };
        let edge = self
            .inner
            .edges_connecting(a_idx, b_idx)
            .map(|e| EdgeReference {
                weight: *e.weight(),
                source: self.inner[e.source()],
                target: self.inner[e.target()],
            });
        Either::Right(edge)
    }
}

/// A station or in the transportation network
#[derive(Component, Default, Deref, DerefMut, Debug, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Component, opaque, Serialize, Deserialize)]
#[require(Name, StationEntries, Save)]
pub struct Station(pub egui::Pos2);

#[derive(Reflect, Component, Debug, Default, MapEntities)]
#[reflect(Component, MapEntities)]
#[relationship_target(relationship = TimetableEntry)]
pub struct StationEntries(Vec<Entity>);

impl StationEntries {
    pub fn entries(&self) -> &[Entity] {
        &self.0
    }
    /// WARNING: this method does not automatically clear vehicle entities. Clear before calling
    /// This is for chaining
    pub fn passing_vehicles<'a, F>(&self, buffer: &mut Vec<Entity>, mut get_parent: F)
    where
        F: FnMut(Entity) -> Option<&'a ChildOf>,
    {
        for entity in self.0.iter().cloned() {
            let Some(vehicle) = get_parent(entity) else {
                continue;
            };
            buffer.push(vehicle.0)
        }
    }
}

/// A track segment between two stations or nodes
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(Name, Save)]
pub struct Interval {
    /// The length of the track segment
    pub length: crate::units::distance::Distance,
    /// The speed limit on the track segment, if any
    pub speed_limit: Option<Velocity>,
}

pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Graph>().add_systems(
            Update,
            arrange::apply_graph_layout.run_if(resource_exists::<GraphLayoutTask>),
        );
    }
}
