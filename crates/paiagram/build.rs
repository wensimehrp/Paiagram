// SPDX-License-Identifier: MPL-2.0
//
// Build script for the shaders.

use std::path::{Path, PathBuf};

use wesl::syntax::ModulePath;
use wesl::{CompileOptions, Compiler, ManglerKind};
use wgsl_bindgen::{RustWgslTypeMap, WgslBindgenOptionBuilder, WgslTypeSerializeStrategy};

const WESL_PACKAGE_ROOT: &str = "src/tabs";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    let gpu_trip_wgsl = compile_wesl(
        WESL_PACKAGE_ROOT,
        "package::diagram::gpu_trip",
        &out_dir.join("gpu_trip.wgsl"),
    )?;

    WgslBindgenOptionBuilder::default()
        .workspace_root(out_dir.clone())
        .add_entry_point(gpu_trip_wgsl.to_string_lossy().into_owned())
        .serialization_strategy(WgslTypeSerializeStrategy::Bytemuck)
        .type_map(RustWgslTypeMap) // Use glam for math types
        .output("src/tabs/diagram/gpu_trip.rs")
        .build()?
        .generate()?;

    let gpu_graph_wgsl = compile_wesl(
        WESL_PACKAGE_ROOT,
        "package::graph::gpu_graph",
        &out_dir.join("gpu_graph.wgsl"),
    )?;

    WgslBindgenOptionBuilder::default()
        .workspace_root(out_dir.clone())
        .add_entry_point(gpu_graph_wgsl.to_string_lossy().into_owned())
        .serialization_strategy(WgslTypeSerializeStrategy::Bytemuck)
        .type_map(RustWgslTypeMap) // Use glam for math types
        .output("src/tabs/graph/gpu_graph.rs")
        .build()?
        .generate()?;

    Ok(())
}

fn compile_wesl(
    package_root: &str,
    main_module: &str,
    output: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let compiler = Compiler::new(CompileOptions {
        // don't strip or mangle since we still have to run wgsl bindgen
        strip: false,
        mangler: ManglerKind::None,
        ..Default::default()
    });

    let main_module: ModulePath = main_module.parse()?;
    let result = compiler
        .compile_module(package_root, &main_module)
        .inspect_err(|err| eprintln!("WESL compilation error:\n{err}"))?;

    result.emit_rerun_if_changed();
    result.write_to_file(output)?;

    Ok(output.to_path_buf())
}
