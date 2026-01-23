use bevy::{ecs::system::RunSystemOnce, prelude::*};
pub struct TypstDiagram;

impl super::ExportObject<Entity> for TypstDiagram {
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        input: Entity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }
}
