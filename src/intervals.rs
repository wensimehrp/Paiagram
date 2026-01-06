use crate::{
    units::speed::Velocity,
    vehicles::{
        AdjustTimetableEntry, AdjustVehicle, TimetableAdjustment, VehicleAdjustment,
        entries::{ActualRouteEntry, TimetableEntry, VehicleScheduleCache},
    },
};
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use moonshine_core::kind::*;
use petgraph::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
#[require(Name, StationCache)]
pub struct Station(pub egui::Pos2);

#[derive(Component, Debug, Default)]
pub struct StationCache {
    pub passing_entries: Vec<Entity>,
}

impl StationCache {
    /// WARNING: this method does not automatically clear vehicle entities. Clear before calling
    /// This is for chaining
    pub fn passing_vehicles<'a, F>(&self, buffer: &mut Vec<Entity>, mut get_parent: F)
    where
        F: FnMut(Entity) -> Option<&'a ChildOf>,
    {
        for entity in self.passing_entries.iter().cloned() {
            let Some(vehicle) = get_parent(entity) else {
                continue;
            };
            buffer.push(vehicle.0)
        }
    }
}

/// A track segment between two stations or nodes
#[derive(Component)]
#[require(Name, IntervalCache)]
pub struct Interval {
    /// The length of the track segment
    pub length: crate::units::distance::Distance,
    /// The speed limit on the track segment, if any
    pub speed_limit: Option<Velocity>,
}

#[derive(Component, Debug, Default)]
pub struct IntervalCache {
    // start of the interval.
    pub passing_entries: Vec<ActualRouteEntry>,
}

impl IntervalCache {
    /// WARNING: this method does not automatically clear vehicle entities. Clear before calling
    /// This is for chaining
    pub fn passing_vehicles<'a, F>(&self, buffer: &mut Vec<Entity>, mut get_parent: F)
    where
        F: FnMut(Entity) -> Option<&'a ChildOf>,
    {
        for entity in self.passing_entries.iter().cloned() {
            let Some(vehicle) = get_parent(entity.inner()) else {
                continue;
            };
            buffer.push(vehicle.0)
        }
    }
}

pub struct IntervalsPlugin;
impl Plugin for IntervalsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Graph::default())
            .init_resource::<IntervalsResource>()
            .add_systems(
                FixedPostUpdate,
                (
                    update_station_cache.run_if(on_message::<AdjustTimetableEntry>),
                    update_interval_cache,
                ),
            );
    }
}

#[derive(Resource)]
pub struct IntervalsResource {
    pub default_depot: Entity,
}

impl FromWorld for IntervalsResource {
    fn from_world(world: &mut World) -> Self {
        // create a depot once and stash the entity so callers can rely on it existing
        let default_depot = world.spawn((Name::new("Default Depot"), Depot)).id();
        Self { default_depot }
    }
}

fn update_station_cache(
    mut msg_entry_change: MessageReader<AdjustTimetableEntry>,
    mut msg_schedule_change: MessageReader<AdjustVehicle>,
    timetable_entries: Query<&TimetableEntry>,
    mut station_caches: Query<&mut StationCache>,
) {
    for msg in msg_entry_change.read() {
        let Ok(entry) = timetable_entries.get(msg.entity) else {
            continue;
        };
        let Ok(mut current_station_cache) = station_caches.get_mut(entry.station.entity()) else {
            continue;
        };
        let index = current_station_cache
            .passing_entries
            .binary_search(&msg.entity);
        match (&msg.adjustment, index) {
            (&TimetableAdjustment::SetStation(_new_station), Ok(index)) => {
                current_station_cache.passing_entries.remove(index);
            }
            (_, Err(index)) => {
                current_station_cache
                    .passing_entries
                    .insert(index, msg.entity);
            }
            _ => {}
        }
    }
    for entity in msg_schedule_change.read().filter_map(|msg| {
        if let VehicleAdjustment::RemoveEntry(entity) = msg.adjustment {
            Some(entity)
        } else {
            None
        }
    }) {
        if let Ok(entry) = timetable_entries.get(entity)
            && let Ok(mut station_cache) = station_caches.get_mut(entry.station.entity())
            && let Ok(index) = station_cache.passing_entries.binary_search(&entity)
        {
            station_cache.passing_entries.remove(index);
        };
    }
}

pub fn update_interval_cache(
    changed_schedules: Populated<&VehicleScheduleCache, Changed<VehicleScheduleCache>>,
    mut intervals: Query<&mut IntervalCache>,
    timetable_entries: Query<&TimetableEntry>,
    graph: Res<Graph>,
    mut invalidated: Local<Vec<Entity>>,
) {
    invalidated.clear();
    for schedule in changed_schedules {
        let Some(actual_route) = &schedule.actual_route else {
            continue;
        };
        for w in actual_route.windows(2) {
            let [beg, end] = w else { continue };
            let Ok(beg_entry) = timetable_entries.get(beg.inner()) else {
                continue;
            };
            let Ok(end_entry) = timetable_entries.get(end.inner()) else {
                continue;
            };
            let Some(&edge) = graph.edge_weight(beg_entry.station, end_entry.station) else {
                continue;
            };
            let Ok(mut cache) = intervals.get_mut(edge.entity()) else {
                continue;
            };
            // now that we have the cache, invalidate the cache first
            if !invalidated.contains(&edge.entity()) {
                cache
                    .passing_entries
                    .retain(|e| matches!(e, ActualRouteEntry::Nominal(_)));
                invalidated.push(edge.entity())
            }
            cache.passing_entries.push(*end);
        }
    }
    for invalidated in invalidated.iter().copied() {
        let Ok(mut cache) = intervals.get_mut(invalidated) else {
            continue;
        };
        cache.passing_entries.sort_unstable_by_key(|e| e.inner());
        cache.passing_entries.dedup_by_key(|e| e.inner());
    }
}
