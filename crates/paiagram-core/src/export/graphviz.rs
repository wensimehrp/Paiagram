use petgraph::dot;

use crate::WorldSnapshot;

pub struct Graphviz<'a> {
    world: &'a WorldSnapshot,
}

impl<'a> super::ExportObject for Graphviz<'a> {
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        todo!();
    }
    fn extension(&self) -> impl AsRef<str> {
        ".dot"
    }
    fn filename(&self) -> impl AsRef<str> {
        "diagram"
    }
}
