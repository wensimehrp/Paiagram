use bevy::prelude::*;

pub struct TypstTimetable;

impl super::ExportObject<()> for TypstTimetable {
    fn export_to_buffer(
        &mut self,
        world: &mut World,
        buffer: &mut Vec<u8>,
        input: &(),
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }
}
