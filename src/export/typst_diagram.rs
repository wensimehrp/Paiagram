use bevy::prelude::*;
pub struct TypstDiagram;

impl super::ExportObject<()> for TypstDiagram {
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        input: &(),
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }
}
