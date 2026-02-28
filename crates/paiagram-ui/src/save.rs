use bevy::{ecs::entity::EntityHashMap, prelude::*};

pub fn save(world: &mut World, filename: String) {
    let entities: Vec<_> = world.query::<Entity>().iter(world).collect();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let scene = make_scene(world, entities.into_iter());
    paiagram_rw::save::save(scene, registry, filename);
}

pub fn save_ron(world: &mut World, filename: String) {
    let entities: Vec<_> = world.query::<Entity>().iter(world).collect();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let scene = make_scene(world, entities.into_iter());
    paiagram_rw::save::save_ron(scene, registry, filename);
}

pub fn apply_loaded_scene(world: &mut World) {
    let Some(loaded) = world.remove_resource::<paiagram_rw::save::LoadedScene>() else {
        return;
    };
    let mut entity_map = EntityHashMap::default();
    loaded.0.write_to_world(world, &mut entity_map).unwrap();
}

fn make_scene(world: &World, entities: impl Iterator<Item = Entity>) -> DynamicScene {
    DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .allow_resource::<crate::MainUiState>()
        .allow_resource::<crate::GlobalTimer>()
        .allow_resource::<paiagram_core::graph::Graph>()
        .allow_all_components()
        .extract_entities(entities)
        .extract_resources()
        .build()
}
