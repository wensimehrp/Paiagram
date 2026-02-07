use crate::trip::class::DisplayedStroke;
use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

pub struct StationPlugin;
impl Plugin for StationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_station_position);
    }
}

/// Entries that visit this station.
/// This does not contain all entries that visit this station, since a station
/// may contain extra platforms, and each platform would also record a set of
/// entries that visit the platform, not the station.
/// See [`PlatformEntries`] for further details.
type StationEntries = PlatformEntries;
/// A type alias to make things clearer.
type Platforms = Children;

/// Marker component for stations that are not managed by the current diagram.
/// Note that being "external" doesn't mean that it can be excluded from
/// [`crate::graph::Graph`].
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(Name, Station)]
pub struct IsExternalStation;

/// Marker component for depots.
#[derive(Reflect, Component)]
#[reflect(Component)]
#[require(Name, Station)]
pub struct IsDepot;

/// A station in the network. A station would also host a default platform.
#[derive(Reflect, Component, Default)]
#[reflect(Component)]
#[require(Name, Platform, PlatformEntries, Platforms, DisplayedStroke)]
pub struct Station;

#[derive(Bundle)]
pub struct StationBundle {
    station: Station,
    platforms: Platforms,
}

/// Entries that passes this platform.
#[derive(Default, Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[relationship_target(relationship = crate::entry::EntryStop)]
pub struct PlatformEntries(#[entities] Vec<Entity>);

#[derive(Default, Reflect, Component)]
#[reflect(Component)]
#[require(Name, PlatformEntries, crate::graph::Node)]
pub struct Platform;

#[derive(QueryData)]
pub struct StationQuery {
    entity: Entity,
    station: &'static Station,
    platforms: &'static Platforms,
    entries: &'static StationEntries,
    name: &'static Name,
    is_external_station: Option<&'static IsExternalStation>,
    position: &'static crate::graph::Node,
    stroke: &'static DisplayedStroke,
}

impl<'w, 's> StationQueryItem<'w, 's> {
    /// Whether if the station is an external station
    pub fn is_external_station(&self) -> bool {
        self.is_external_station.is_some()
    }
    /// Whether if the station is movable by the user
    pub fn is_movable_by_user(&self) -> bool {
        !self.is_external_station() && self.platforms.is_empty()
    }
    /// Returns all entries passing the station.
    /// The order is not guaranteed.
    pub fn passing_entries<'a>(
        &self,
        platform_entries: &Query<'a, 'a, &PlatformEntries>,
    ) -> impl Iterator<Item = Entity> {
        let platform_entries = platform_entries
            .iter_many(self.platforms)
            .flat_map(|p| p.iter());
        self.entries.iter().chain(platform_entries)
    }
    /// Returns all entries passing the station.
    pub fn passing_entries_by_platform<'a>(
        &self,
        platform_entries: &'a Query<'a, 'a, &PlatformEntries>,
    ) -> impl Iterator<Item = (Entity, impl Iterator<Item = Entity> + 'a)> + 'a
    where
        'w: 'a,
    {
        let platform_entries = self.platforms.iter().filter_map(|pe| {
            let Ok(entries) = platform_entries.get(pe) else {
                return None;
            };
            Some((pe, entries.iter()))
        });
        std::iter::once((self.entity, self.entries.iter())).chain(platform_entries)
    }
}

#[derive(QueryData)]
pub struct PlatformQuery {
    entity: Entity,
    name: &'static Name,
    station: AnyOf<(&'static Station, &'static ChildOf)>,
    entries: &'static PlatformEntries,
}

fn update_station_position(
    changed_positions: Populated<
        &ChildOf,
        (
            With<Platform>,
            Without<Station>,
            Changed<crate::graph::Node>,
        ),
    >,
    positions: Query<&crate::graph::Node, (Without<Station>, With<Platform>)>,
    mut stations: Query<(&mut crate::graph::Node, &Platforms), With<Station>>,
) {
    for parent in changed_positions {
        // TODO: calculate the station position based on the child's position
    }
}
