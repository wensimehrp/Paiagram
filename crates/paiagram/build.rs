use wgsl_bindgen::{RustWgslTypeMap, WgslBindgenOptionBuilder, WgslTypeSerializeStrategy};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    WgslBindgenOptionBuilder::default()
        .workspace_root("src/tabs/diagram")
        .add_entry_point("src/tabs/diagram/gpu_trip.wgsl")
        .serialization_strategy(WgslTypeSerializeStrategy::Bytemuck)
        .type_map(RustWgslTypeMap) // Use glam for math types
        .output("src/tabs/diagram/gpu_trip.rs")
        .build()?
        .generate()?;
    Ok(())
}
