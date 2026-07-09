use std::io::Write;

use paiagram_core::export::ExportObject;

pub(crate) struct TypstModule;

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

pub(crate) struct TypstDiagram<'a> {
    pub(crate) source: &'a paiagram_core::WorldSnapshot,
}

impl<'a> ExportObject for TypstDiagram<'a> {
    fn export_to_buffer(&mut self, _buffer: &mut Vec<u8>) {
        todo!("Implement Typst diagram export with new core types")
    }
    fn extension(&self) -> impl AsRef<str> {
        ".json"
    }
    fn filename(&self) -> impl AsRef<str> {
        "exported_diagram_data"
    }
}
