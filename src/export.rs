use bevy::prelude::*;
use std::error::Error;

use crate::rw_data::write::write_file;

pub mod graphviz;
pub mod typst_diagram;
pub mod typst_timetable;

pub trait ExportObject<S = ()> {
    /// Export contents to a Vec<u8> buffer, with optional parameters
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        input: S,
    ) -> Result<(), Box<dyn Error>>;
    /// Export contents and save them on disk, with optional parameters
    fn export_to_file(&mut self, world: &mut World, input: S) -> Result<(), Box<dyn Error>> {
        let mut buffer = Vec::new();
        self.export_to_buffer(world, &mut buffer, input)?;
        let mut filename = String::new();
        filename.push_str(self.filename().as_ref());
        filename.push_str(self.extension().as_ref());
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                if let Err(e) = write_file(buffer, filename).await {
                    error!("Error while writing file: {:?}", e)
                }
            })
            .detach();
        Ok(())
    }
    fn filename(&self) -> impl AsRef<str> {
        "exported_file"
    }
    fn extension(&self) -> impl AsRef<str> {
        ".paiagram"
    }
}
