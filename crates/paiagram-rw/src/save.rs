use bevy::{
    prelude::*,
    scene::serde::{SceneDeserializer, SceneSerializer},
};
use cbor4ii::core::utils::IoReader;
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
    super::write::write_file(filename, move |writer| {
        let reg = registry.read();
        let serializer = SceneSerializer::new(&scene, &reg);
        let mut encoder = lz4_flex::frame::FrameEncoder::new(writer);
        cbor4ii::serde::to_writer(&mut encoder, &serializer)
            .map_err(std::io::Error::other)
            .and_then(|_| encoder.finish().map(|_| ()).map_err(std::io::Error::other))
    });
}

pub fn save_ron(scene: DynamicScene, registry: AppTypeRegistry, filename: String) {
    super::write::write_file(filename, move |writer| {
        let reg = registry.read();
        scene
            .serialize(&reg)
            .map_err(std::io::Error::other)
            .and_then(|s| {
                writer
                    .write(s.as_bytes())
                    .map(|_| ())
                    .map_err(std::io::Error::other)
            })
    });
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
            let mut decoder = lz4_flex::frame::FrameDecoder::new(d.as_slice());
            let mut deserializer = cbor4ii::serde::Deserializer::new(IoReader::new(
                std::io::BufReader::new(&mut decoder),
            ));
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
