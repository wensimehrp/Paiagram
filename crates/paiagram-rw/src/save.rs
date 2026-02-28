use bevy::{
    prelude::*,
    scene::serde::{SceneDeserializer, SceneSerializer},
    tasks::AsyncComputeTaskPool,
};
use cbor4ii::core::utils::SliceReader;
use serde::de::DeserializeSeed;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            deserialize_load_candidate.run_if(resource_exists::<LoadCandidate>),
        );
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

#[derive(Resource, Deref, DerefMut)]
pub struct LoadedScene(pub DynamicScene);

pub fn save(scene: DynamicScene, registry: AppTypeRegistry, filename: String) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let reg = registry.read();
            let serializer = SceneSerializer::new(&scene, &reg);
            let serialized = cbor4ii::serde::to_vec(Vec::new(), &serializer).unwrap();
            let compressed = lz4_flex::compress_prepend_size(&serialized);
            super::write::write_file(compressed, filename);
        })
        .detach();
}

pub fn save_ron(scene: DynamicScene, registry: AppTypeRegistry, filename: String) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let reg = registry.read();
            let data = scene.serialize(&reg).unwrap().into_bytes();
            super::write::write_file(data, filename);
        })
        .detach();
}

pub fn write_compressed_cbor(serialized_scene: Vec<u8>, filename: String) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            let compressed = lz4_flex::compress_prepend_size(&serialized_scene);
            super::write::write_file(compressed, filename);
        })
        .detach();
}

pub fn write_ron(ron_scene: Vec<u8>, filename: String) {
    AsyncComputeTaskPool::get()
        .spawn(async move {
            super::write::write_file(ron_scene, filename);
        })
        .detach();
}

fn deserialize_load_candidate(world: &mut World) {
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
    world.insert_resource(LoadedScene(scene));
}
