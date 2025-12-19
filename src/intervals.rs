use crate::{
    units::speed::Velocity,
    vehicles::{
        AdjustTimetableEntry, AdjustVehicle, TimetableAdjustment, VehicleAdjustment,
        entries::{ActualRouteEntry, TimetableEntry, VehicleScheduleCache},
    },
};
use bevy::prelude::*;
use petgraph::{Undirected, graphmap};

pub type IntervalGraphType = graphmap::GraphMap<Entity, Entity, Undirected>;

/// A graph representing the transportation network
#[derive(Resource, Default, Deref)]
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
    /// WARNING: this method does not automatically clear vehicle entities. Clear before calling
    /// This is for chaining
    pub fn passing_vehicles<'a, F>(&self, buffer: &mut Vec<Entity>, mut get_parent: F)
    where
        F: FnMut(Entity) -> Option<&'a ChildOf>,
    {
        for entity in self.passing_entries.iter().cloned() {
            let Some(vehicle) = get_parent(entity) else {
                continue
            };
            buffer.push(vehicle.0)
        }
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
                continue
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
            let Ok(mut cache) = intervals.get_mut(edge) else {
                continue;
            };
            // now that we have the cache, invalidate the cache first
            if !invalidated.contains(&edge) {
                cache
                    .passing_entries
                    .retain(|e| matches!(e, ActualRouteEntry::Nominal(_)));
                invalidated.push(edge)
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
