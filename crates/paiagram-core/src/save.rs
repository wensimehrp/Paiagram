use std::io;
use std::sync::mpsc;

use paiagram_rw::ExportObject;
use serde::{Deserialize, Serialize};

use crate::spatial_index::SpatialCache;
use crate::{Source, World};

/// The save file format.
#[derive(Serialize, Deserialize, Clone)]
pub enum SaveData {
    V1(World),
}

impl SaveData {
    const MAGIC_BYTES: &'static [u8; 4] = b"PAIA";

    pub fn from_reader<R: io::Read>(mut reader: R) -> Result<Self, &'static str> {
        let mut buf = [0u8; 4];
        let result = reader.read_exact(&mut buf);
        if result.is_err() || &buf != Self::MAGIC_BYTES {
            return Err("Incorrect file type");
        };
        let decoder = zrip::FrameDecoder::new(reader);
        let buffered_reader = io::BufReader::new(decoder);
        cbor4ii::serde::from_reader(buffered_reader)
            .map_err(|_| "Failed to deserialize CBOR save file")
    }

    /// Reads a `.paia` save file from disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: impl AsRef<std::path::Path>) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(file).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    /// Returns the world stored in this save file.
    pub fn into_world(self) -> World {
        match self {
            Self::V1(world) => world,
        }
    }
}

impl ExportObject for SaveData {
    fn extension(&self) -> &str {
        "paia"
    }
    fn default_filename(&self) -> &str {
        "exported"
    }
    fn write_to_vec(&mut self, mut buf: Vec<u8>) -> io::Result<Vec<u8>> {
        buf.extend_from_slice(Self::MAGIC_BYTES);
        let mut encoder = zrip::FrameEncoder::new(buf, 4).map_err(io::Error::other)?;
        cbor4ii::serde::to_writer(&mut encoder, &self).map_err(io::Error::other)?;
        encoder.finish()
    }
}

impl TryFrom<SaveData> for Source {
    type Error = &'static str;
    fn try_from(value: SaveData) -> Result<Self, Self::Error> {
        match value {
            SaveData::V1(world) => {
                let (spatial_tx, spatial_rx) = mpsc::channel();
                let source = Self {
                    prev_frame_world: world.clone(),
                    history: Vec::new(),
                    redo_len: 0,
                    revision: 0,
                    world,
                    spatial: SpatialCache::default(),
                    spatial_tx,
                    spatial_rx,
                };
                source.spawn_spatial_build();
                Ok(source)
            }
        }
    }
}

impl From<World> for SaveData {
    fn from(value: World) -> Self {
        Self::V1(value)
    }
}
