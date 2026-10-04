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

    generate_bindings(
        &out_dir,
        "package::diagram::gpu_trip",
        "gpu_trip.wgsl",
        "src/tabs/diagram/gpu_trip.rs",
    )?;
    generate_bindings(
        &out_dir,
        "package::graph::graph_intervals",
        "graph_intervals.wgsl",
        "src/tabs/graph/graph_intervals.rs",
    )?;
    generate_bindings(
        &out_dir,
        "package::graph::graph_nodes",
        "graph_nodes.wgsl",
        "src/tabs/graph/graph_nodes.rs",
    )?;
    generate_bindings(
        &out_dir,
        "package::graph::trip_icons",
        "trip_icons.wgsl",
        "src/tabs/graph/trip_icons.rs",
    )?;

    Ok(())
}

/// Compiles a WESL module and runs `wgsl_bindgen` over the result, writing the Rust bindings to
/// `rs_output`.
fn generate_bindings(
    out_dir: &Path,
    module: &str,
    wgsl_name: &str,
    rs_output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let wgsl = compile_wesl(WESL_PACKAGE_ROOT, module, &out_dir.join(wgsl_name))?;

    WgslBindgenOptionBuilder::default()
        .workspace_root(out_dir.to_path_buf())
        .add_entry_point(wgsl.to_string_lossy().into_owned())
        .serialization_strategy(WgslTypeSerializeStrategy::Bytemuck)
        .type_map(RustWgslTypeMap) // Use glam for math types
        .output(rs_output)
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
