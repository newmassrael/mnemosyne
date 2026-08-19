//! The law: the descriptor set this crate serves is one a CRATE compiled, not
//! one a program did.
//!
//! # What is actually being asserted
//!
//! `build.rs` compiles `proto/mnemosyne.proto` with `protox`, in the build
//! script's own process, and embeds the result as
//! [`MNEMOSYNE_FILE_DESCRIPTOR_SET`]. This test compiles the same file the same
//! way and compares the bytes. Three things fail together if it stops holding:
//!
//! 1. **The compiler went back to being a program.** `tonic_build`'s
//!    `compile_protos` shells out to a `protoc` binary. Its descriptor and
//!    `protox`'s agree byte for byte with `source_code_info` set aside, and
//!    carry the same 264 source locations as a set — but in a DIFFERENT ORDER
//!    (measured on this schema, 2026-08-19, with the arguments prost-build
//!    passes). So the embedded bytes say which compiler produced them, and a
//!    return to the protoc route fails here, on any machine, rather than on the
//!    next machine that happens not to have the binary.
//! 2. **The build script's arguments drifted.** `include_source_info` is what
//!    carries the schema's comments into the generated Rust as doc comments;
//!    turning it off on either side breaks this comparison.
//! 3. **The embedded descriptor went stale against the tracked schema.** The
//!    bytes are compiled from `proto/mnemosyne.proto` here and now.
//!
//! # Why this and not an emptied `PATH`
//!
//! Because emptying `PATH` would prove a property of `protox` — that it spawns
//! nothing — and this crate's property is which API its build script calls. The
//! environment is process-wide and these tests share a process (see `all.rs`),
//! so a test that mutated it would be unsound as well as beside the point.
//!
//! # Why it matters enough to be a test
//!
//! Round 1252: four consecutive runs of `main` died installing `protobuf-compiler`
//! from the Ubuntu archive, six jobs a run, the last of them after R1251 had
//! already given apt retries. Removing the installs makes a regression loud in
//! CI — but only there, and only as a red `main`, which is the thing being paid
//! off. This makes it loud before the push.

use prost::Message;

use mnemosyne_server::MNEMOSYNE_FILE_DESCRIPTOR_SET;

/// The include path and the file, split the way `build.rs` splits them — the
/// include-relative name is what the descriptor records as the file's name.
const INCLUDE: &str = "proto";
const FILE: &str = "mnemosyne.proto";

#[test]
fn the_embedded_descriptor_is_the_one_an_in_process_compile_produces() {
    let include = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(INCLUDE);

    let mut compiler = protox::Compiler::new([&include]).expect("open the include path");
    compiler.include_source_info(true);
    compiler.include_imports(true);
    compiler.open_file(FILE).expect("compile the schema");
    let here = compiler.file_descriptor_set().encode_to_vec();

    assert_eq!(
        here.len(),
        MNEMOSYNE_FILE_DESCRIPTOR_SET.len(),
        "the embedded descriptor is a different size from one compiled here: \
         the build script either used a different compiler or different arguments"
    );
    assert!(
        here == MNEMOSYNE_FILE_DESCRIPTOR_SET,
        "the embedded descriptor differs byte for byte from one compiled here \
         from {}/{FILE}: the build script either used a different compiler, or \
         different arguments, or the embedded copy is stale",
        include.display()
    );
}

#[test]
fn the_schema_the_test_compiles_is_the_one_the_crate_serves() {
    // NOT A TAUTOLOGY WITH THE TEST ABOVE. That one would also pass if both
    // sides compiled an empty file: two equal descriptors say nothing about
    // what is in them. This says the bytes carry this repository's service, so
    // a comparison of them is a comparison of something.
    let set = prost_types::FileDescriptorSet::decode(MNEMOSYNE_FILE_DESCRIPTOR_SET)
        .expect("the embedded bytes are a FileDescriptorSet");
    let names: Vec<_> = set
        .file
        .iter()
        .flat_map(|f| f.service.iter())
        .filter_map(|s| s.name.clone())
        .collect();
    assert!(
        names.iter().any(|n| n == "Mnemosyne"),
        "the embedded descriptor must carry this crate's service; it names {names:?}"
    );
    assert_eq!(
        set.file
            .iter()
            .filter_map(|f| f.name.clone())
            .collect::<Vec<_>>(),
        vec![FILE.to_owned()],
        "the file's name inside the descriptor is the include-relative one, \
         which is what every reflection client asks for it by"
    );
}
