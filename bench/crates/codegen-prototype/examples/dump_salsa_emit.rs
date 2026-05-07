//! Round 49 OPTION A — golden snapshot writer.
//!
//! Salsa wire emit result — driven by tests/fixtures/salsa_wire_emit.rs.
//! salsa_compile.rs unifies the test path: the golden file is inlined with `include!`.
//!
//! execute: `cargo run -p codegen-prototype --example dump_salsa_emit`
//! or drift occurs on update of an identical command.

use codegen_prototype::cf_wrapper::default_layout;
use codegen_prototype::entity_indexer::design_doc_schema_fixture;
use codegen_prototype::salsa_wire::{design_doc_cascade_fixture, emit_salsa_wire};

fn main() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);

 let path = concat!(
 env!("CARGO_MANIFEST_DIR"),
 "/tests/fixtures/salsa_wire_emit.rs"
 );
 let parent = std::path::Path::new(path).parent().expect("parent dir");
 std::fs::create_dir_all(parent).expect("create fixtures dir");
 std::fs::write(path, &emitted.source).expect("write golden");
 println!(
 "wrote {} bytes to {}",
 emitted.source.len(),
 path
 );
}
