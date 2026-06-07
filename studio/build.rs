//! pinion-forge codegen entrypoint for mnemosyne-studio.
//!
//! Same shape as the pinion hello-* binary build scripts: compile the
//! renderer manifest `app.pinion.xml` into `$OUT_DIR/app.rs` (the
//! `StudioRenderer` vello renderer struct the shell constructs/drives).

use std::path::Path;

fn main() {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("cargo populates CARGO_MANIFEST_DIR for build scripts");
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo populates OUT_DIR");

    let input = Path::new(&manifest_dir).join("app.pinion.xml");

    println!("cargo:rerun-if-changed={}", input.display());
    println!("cargo:rerun-if-changed=build.rs");

    match pinion_forge::compile_file(&input, Path::new(&out_dir)) {
        Ok(_) => {}
        Err(pinion_forge::CompileError::Diagnostics(diags)) => {
            for d in &diags {
                eprintln!("{d}");
            }
            panic!(
                "pinion-forge: {} diagnostic(s) in {}",
                diags.len(),
                input.display(),
            );
        }
        Err(other) => panic!("pinion-forge: {other}"),
    }
}
