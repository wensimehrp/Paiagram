use crate::{
    graph::arrange::GraphLayoutTask, units::speed::Velocity, vehicles::entries::{ActualRouteEntry, TimetableEntry}
};
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use moonshine_core::kind::*;
use petgraph::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod arrange;

pub mod instance_serde {
    use super::*;

    pub fn serialize<S, T: Kind>(instance: &Instance<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        instance.entity().serialize(serializer)
    }

    pub fn deserialize<'de, D, T: Kind>(deserializer: D) -> Result<Instance<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entity = Entity::deserialize(deserializer)?;
        Ok(unsafe { Instance::from_entity_unchecked(entity) })
    }
}

pub mod option_instance_serde {
    use super::*;

    pub fn serialize<S, T: Kind>(
        instance: &Option<Instance<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        instance.map(|i| i.entity()).serialize(serializer)
    }

    pub fn deserialize<'de, D, T: Kind>(deserializer: D) -> Result<Option<Instance<T>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entity = Option::<Entity>::deserialize(deserializer)?;
        Ok(entity.map(|e| unsafe { Instance::from_entity_unchecked(e) }))
    }
}

pub mod vec_instance_f32_serde {
    use super::*;

    pub fn serialize<S, T: Kind>(
        vec: &Option<Vec<(Instance<T>, f32)>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        vec.as_ref()
            .map(|v| v.iter().map(|(i, f)| (i.entity(), *f)).collect::<Vec<_>>())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D, T: Kind>(
        deserializer: D,
    ) -> Result<Option<Vec<(Instance<T>, f32)>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Option::<Vec<(Entity, f32)>>::deserialize(deserializer)?;
        Ok(vec.map(|v| {
            v.into_iter()
                .map(|(e, f)| (unsafe { Instance::from_entity_unchecked(e) }, f))
                .collect()
        }))
    }
}

pub type IntervalGraphType = StableDiGraph<Instance<Station>, Instance<Interval>>;

/// A graph representing the transportation network
#[derive(Resource, Default, Debug)]
pub struct Graph {
    inner: IntervalGraphType,
    indices: EntityHashMap<NodeIndex>,
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
}

/// A depot or yard in the transportation network
/// A depot cannot be a node in the transportation network graph. Use `Station` for that.
#[derive(Component)]
#[require(Station)]
pub struct Depot;

/// A station or in the transportation network
#[derive(Component, Default, Deref, DerefMut, Debug, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Component, opaque, Serialize, Deserialize)]
#[require(Name, StationEntries)]
pub struct Station(pub egui::Pos2);

#[derive(Component, Debug, Default)]
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
#[derive(Component)]
#[require(Name)]
pub struct Interval {
    /// The length of the track segment
    pub length: crate::units::distance::Distance,
    /// The speed limit on the track segment, if any
    pub speed_limit: Option<Velocity>,
}

pub struct GraphPlugin;
impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Graph::default()).add_systems(
            Update,
            arrange::apply_graph_layout.run_if(resource_exists::<GraphLayoutTask>),
        );
    }
}
