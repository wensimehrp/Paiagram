use bevy::{
    ecs::entity::EntityHashMap,
    prelude::*,
    scene::serde::{SceneDeserializer, SceneSerializer},
};
use cbor4ii::core::utils::SliceReader;
use serde::de::DeserializeSeed;
use logging_timer::time;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, load_scene.run_if(resource_exists::<LoadCandidate>));
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct LoadCandidate(pub SaveData);

pub fn add_load_candidate_ron(commands: &mut Commands, data: Vec<u8>) {
    commands.insert_resource(LoadCandidate(SaveData::Ron(data)));
}

pub fn add_load_candidate_compressed_cbor(commands: &mut Commands, data: Vec<u8>) {
    commands.insert_resource(LoadCandidate(SaveData::CompressedCbor(data)));
}

pub enum SaveData {
    CompressedCbor(Vec<u8>),
    Ron(Vec<u8>),
}

// TODO: make this async
#[time("info")]
pub fn save(world: &mut World) -> Vec<u8> {
    let entities: Vec<_> = world.query::<Entity>().iter(&world).collect();
    let registry = world.resource::<AppTypeRegistry>().read();
    let scene = make_scene(world, entities.into_iter());
    let serializer = SceneSerializer::new(&scene, &registry);
    let serialized = cbor4ii::serde::to_vec(Vec::new(), &serializer).unwrap();
    lz4_flex::compress_prepend_size(&serialized)
}

#[time("info")]
pub fn save_ron(world: &mut World) -> Vec<u8> {
    let entities: Vec<_> = world.query::<Entity>().iter(&world).collect();
    let registry = world.resource::<AppTypeRegistry>().read();
    let scene = make_scene(world, entities.into_iter());
    let data = scene.serialize(&registry).unwrap().into_bytes();
    data
}

#[time("info")]
fn make_scene(world: &World, entities: impl Iterator<Item = Entity>) -> DynamicScene {
    let now = instant::Instant::now();
    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .allow_resource::<crate::graph::Graph>()
        .allow_resource::<crate::ui::MainUiState>()
        .allow_resource::<crate::ui::GlobalTimer>()
        .allow_all_components()
        .extract_entities(entities)
        .extract_resources()
        .build();
    scene
}

#[time("info")]
fn load_scene(world: &mut World) {
    let Some(data) = world.remove_resource::<LoadCandidate>() else {
        error!("Tried to load data but the data does not exist");
        return;
    };
    let registry = world.resource::<AppTypeRegistry>().read();
    let scene_deserializer = SceneDeserializer {
        type_registry: &registry,
    };

    let scene: DynamicScene;
    match data.0 {
        SaveData::CompressedCbor(d) => {
            let decompressed = lz4_flex::decompress_size_prepended(&d).unwrap();
            let mut deserializer =
                cbor4ii::serde::Deserializer::new(SliceReader::new(decompressed.as_slice()));
            scene = scene_deserializer.deserialize(&mut deserializer).unwrap();
        }
        SaveData::Ron(d) => {
            let mut deserializer = ron::Deserializer::from_bytes(d.as_slice()).unwrap();
            scene = scene_deserializer.deserialize(&mut deserializer).unwrap();
        }
    }
    drop(registry);
    let mut entity_map = EntityHashMap::default();
    scene.write_to_world(world, &mut entity_map).unwrap();
}
