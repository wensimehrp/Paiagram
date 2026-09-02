// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]

use paiagram_core::WorldSnapshot;

#[derive(Clone, Copy)]
enum GraphyFormat {
    Pdf,
    Svg,
    Png,
}

#[derive(Clone)]
pub struct ExportGraphy {
    world: WorldSnapshot,
    format: GraphyFormat,
}

impl ExportGraphy {
    fn new_pdf(world: WorldSnapshot) -> Self {
        Self {
            world,
            format: GraphyFormat::Pdf,
        }
    }
    fn new_svg(world: WorldSnapshot) -> Self {
        Self {
            world,
            format: GraphyFormat::Svg,
        }
    }
    fn new_png(world: WorldSnapshot) -> Self {
        Self {
            world,
            format: GraphyFormat::Png,
        }
    }
}

impl paiagram_rw::ExportObject for ExportGraphy {
    fn write_content<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        todo!()
    }
    fn extension(&self) -> impl AsRef<str> {
        match self.format {
            GraphyFormat::Pdf => ".pdf",
            GraphyFormat::Svg => ".svg",
            GraphyFormat::Png => ".png",
        }
    }
}
