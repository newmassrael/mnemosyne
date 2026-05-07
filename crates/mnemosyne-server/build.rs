// Round 91 — tonic-build wires the proto schema in proto/mnemosyne.proto into
// the generated `mnemosyne.v1` Rust module that `src/grpc.rs` includes via
// `tonic::include_proto!`. The .proto file path is tracked explicitly so a
// schema mutation triggers a rebuild.
//
// Round 96 — emit a FileDescriptorSet binary alongside the generated Rust so
// the gRPC reflection service can serve the proto schema at runtime.

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
