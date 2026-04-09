use bevy::prelude::*;
use paiagram_core::export::ExportObject;
use std::io::Write;

pub struct TypstModule;

impl ExportObject for TypstModule {
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        buffer
            .write_all(include_bytes!("./typst_diagram.typ"))
            .unwrap();
    }
    fn extension(&self) -> impl AsRef<str> {
        ".typ"
    }
}

pub struct TypstDiagram<'a> {
    pub route_entity: Entity,
    pub world: &'a mut World,
}

impl<'a> ExportObject for TypstDiagram<'a> {
    fn export_to_buffer(&mut self, _buffer: &mut Vec<u8>) {
        todo!("Implement this thing before I die")
    }
    fn extension(&self) -> impl AsRef<str> {
        ".json"
    }
    fn filename(&self) -> impl AsRef<str> {
        "exported_diagram_data"
    }
}
