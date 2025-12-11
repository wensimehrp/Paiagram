use crate::{
    units::speed::Velocity,
    vehicles::{
        AdjustTimetableEntry, AdjustVehicle, TimetableAdjustment, VehicleAdjustment,
        entries::TimetableEntry,
    },
};
use bevy::prelude::*;
use petgraph::{Undirected, graphmap};

pub type IntervalGraphType = graphmap::GraphMap<Entity, Entity, Undirected>;

/// A graph representing the transportation network
#[derive(Resource, Default)]
pub struct Graph(pub IntervalGraphType);

#[derive(Message)]
pub enum GraphAdjustment {
    AddEdge(GraphAdjustmentEdgeAddition),
    RemoveEdge(Entity),
    AddNode(Entity),
}

pub struct GraphAdjustmentEdgeAddition {
    from: Entity,
    to: Entity,
    weight: Entity,
}

/// A station or node in the transportation network
#[derive(Component)]
#[require(Name, StationCache)]
pub struct Station;

#[derive(Component, Debug, Default)]
pub struct StationCache {
    pub passing_entries: Vec<Entity>,
}

impl StationCache {
    pub fn passing_vehicles<'a, F>(&self, mut get_parent: F) -> Vec<Entity>
    where
        F: FnMut(Entity) -> Option<&'a ChildOf> + 'a,
    {
        let mut iterated = Vec::new();
        for entry_entity in self.passing_entries.iter().copied() {
            if let Some(parent_entity) = get_parent(entry_entity)
                && !iterated.contains(&parent_entity.0)
            {
                iterated.push(parent_entity.0)
            }
        }
        iterated
    }
}

/// A depot or yard in the transportation network
/// A depot cannot be a node in the transportation network graph. Use `Station` for that.
#[derive(Component)]
#[require(Name)]
pub struct Depot;

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
    passing_entries: Vec<Entity>,
}

pub struct IntervalsPlugin;
impl Plugin for IntervalsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Graph::default())
            .init_resource::<IntervalsResource>()
            .add_systems(
                FixedPostUpdate,
                update_station_cache.run_if(on_message::<AdjustTimetableEntry>),
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

pub fn update_station_cache(
    mut msg_entry_change: MessageReader<AdjustTimetableEntry>,
    mut msg_schedule_change: MessageReader<AdjustVehicle>,
    timetable_entries: Query<&TimetableEntry>,
    mut station_caches: Query<&mut StationCache>,
) {
    for msg in msg_entry_change.read() {
        let Ok(entry) = timetable_entries.get(msg.entity) else {
            continue;
        };
        let Ok(mut current_station_cache) = station_caches.get_mut(entry.station) else {
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
            && let Ok(mut station_cache) = station_caches.get_mut(entry.station)
            && let Ok(index) = station_cache.passing_entries.binary_search(&entity)
        {
            station_cache.passing_entries.remove(index);
        };
    }
}
