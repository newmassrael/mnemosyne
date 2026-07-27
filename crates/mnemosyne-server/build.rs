// tonic-build wires the proto schema in proto/mnemosyne.proto into the
// generated `mnemosyne.v1` Rust module that `src/grpc.rs` includes via
// `tonic::include_proto!`. The .proto file path is tracked explicitly so a
// schema mutation triggers a rebuild.
//
// It also emits a FileDescriptorSet binary alongside the generated Rust, so the
// gRPC reflection service can serve the proto schema at runtime.
//
// This file carried two pre-252 round anchors until Round 783 brought build
// scripts inside the citation gate. Both were off-main, so neither could be
// verified against the store — and CLAUDE.md is explicit that a citation to such
// a round must not be written as though it could. They were not registered as
// known-stale either: an allow-list for unverifiable citations would
// institutionalise exactly what the gate exists to prevent, and unlike a frozen
// ledger entry a comment can simply be corrected. What those rounds decided is
// what the code below does, which is why deleting the anchors costs nothing.
//
// Naming them here would have re-created the citations verbatim — the gate said
// so on the first run of this round, which is the rule working.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = "proto/mnemosyne.proto";
    println!("cargo:rerun-if-changed={proto}");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("mnemosyne_descriptor.bin");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&[proto], &["proto"])?;
    Ok(())
}
