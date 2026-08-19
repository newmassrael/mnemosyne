// The proto schema in proto/mnemosyne.proto becomes the generated `mnemosyne.v1`
// Rust module that `src/grpc.rs` includes via `tonic::include_proto!`, plus a
// FileDescriptorSet binary the gRPC reflection service serves at runtime. The
// .proto file path is tracked explicitly so a schema mutation triggers a rebuild.
//
// THE SCHEMA COMPILER IS A CRATE AND NOT A PROGRAM (Round 1252). `tonic_build`'s
// `compile_protos` shells out to a `protoc` binary, which the hosted runner does
// not ship, so six jobs of this repository's workflow installed one from the
// Ubuntu archive on every run. On 2026-08-19 that archive stalled and four runs
// of `main` in a row died at it — the last of them after R1251 had already given
// apt retries and a shorter timeout, which is the measurement that a bound turns
// a hang into a red build sooner and never into a green one. `protox` compiles
// the file HERE, in this process, so the compiler is pinned by `Cargo.lock` like
// every other dependency and nothing is fetched to build this crate.
//
// The two compilers were compared before the switch, on this file, with the
// arguments prost-build passes (`--include_imports --include_source_info`): the
// descriptor sets agree byte for byte once `source_code_info` is set aside, they
// carry the same 264 source locations as a set, and the Rust `tonic-build` emits
// from either is byte-identical at 28566 bytes. What differs is the ORDER of the
// source locations, and `tests/all.rs` leans on exactly that: the descriptor this
// script writes is compared against a fresh in-process compile, so a return to
// the `protoc` path fails a test here rather than a build on a machine that
// happens to lack the binary.
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

use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // THE INCLUDE PATH AND THE FILE ARE SEPARATE, and the split is what names
    // the file inside the descriptor set: `protoc -I proto proto/mnemosyne.proto`
    // recorded it as `mnemosyne.proto`, so the include-relative name is what is
    // opened here. Opening it by its path from the crate root would rename the
    // file in every descriptor this repository serves.
    let include = "proto";
    let file = "mnemosyne.proto";
    println!("cargo:rerun-if-changed={include}/{file}");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("mnemosyne_descriptor.bin");

    // `include_source_info` is what carries the schema's comments into the
    // generated Rust as doc comments, and it is what `protoc` was being passed;
    // `include_imports` matches the other half of that invocation. This file
    // imports nothing today, and the argument is about what the compiler is
    // asked for rather than about what this schema happens to contain.
    let mut compiler = protox::Compiler::new([include])?;
    compiler.include_source_info(true);
    compiler.include_imports(true);
    compiler.open_file(file)?;
    let descriptor_set = compiler.file_descriptor_set();

    // `compile_fds` does NOT write the descriptor set, even with
    // `file_descriptor_set_path` configured — that path is read by the protoc
    // route, which is the one no longer taken. The reflection service needs the
    // file, so this script writes it.
    std::fs::write(&descriptor_path, descriptor_set.encode_to_vec())?;

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_fds(descriptor_set)?;
    Ok(())
}
