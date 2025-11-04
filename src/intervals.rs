use crate::basic::*;
use bevy::prelude::*;
use petgraph::{Undirected, graphmap};

pub type IntervalGraphType = graphmap::GraphMap<Entity, Entity, Undirected>;

/// A graph representing the transportation network
#[derive(Resource, Default)]
pub struct Graph(pub IntervalGraphType);

/// A station or node in the transportation network
#[derive(Component)]
#[require(Name)]
pub struct Station;

/// A depot or yard in the transportation network
/// A depot cannot be a node in the transportation network graph. Use `Station` for that.
#[derive(Component)]
#[require(Name)]
pub struct Depot;

/// A track segment between two stations or nodes
#[derive(Component, Reflect)]
#[require(Name)]
pub struct Interval {
    /// The length of the track segment
    pub length: TrackDistance,
    /// The speed limit on the track segment, if any
    pub speed_limit: Option<Speed>,
}

pub struct IntervalsPlugin;
impl Plugin for IntervalsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Graph::default())
            .init_resource::<IntervalsResource>();
    }
}

#[derive(Resource)]
pub struct IntervalsResource {
    pub default_depot: Entity,
}

impl FromWorld for IntervalsResource {
    fn from_world(world: &mut World) -> Self {
        // create a depot once and stash the entity so callers can rely on it existing
        let default_depot = world
            .spawn((Name::new("Default Depot"), Depot))
            .id();
        Self { default_depot }
    }
}
