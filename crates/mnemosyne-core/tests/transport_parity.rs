//! Round 306 — transport parity smoke tests.
//!
//! Asserts the substrate's promise: every `Transport` variant declared in
//! the public type surface is reachable through `SymbolResolver` calls.
//! `InProcess` returns real answers (verified per-backend in their own
//! crates). `Mcp` / `Cli` placeholders return `ResolverError::NotImplemented`
//! — a callable surface for R307+ to wire without breaking call sites.
//!
//! If a future round wires MCP / CLI transports for real, the
//! `NotImplemented` assertions below flip to expectation matrices keyed
//! on the sample backend's response shape — that is the R307+ trigger.

use std::path::Path;

use mnemosyne_core::{CliResolver, McpResolver, ResolverError, SymbolResolver};

#[test]
fn mcp_transport_surfaces_not_implemented_until_r307() {
    let r = McpResolver {
        command: vec!["python".into(), "-m".into(), "resolver".into()],
    };
    match r.resolve_symbol_at(Path::new("/dev/null"), 1) {
        Err(ResolverError::NotImplemented) => {}
        other => panic!("expected NotImplemented, got {:?}", other),
    }
}

#[test]
fn cli_transport_surfaces_not_implemented_until_r307() {
    let r = CliResolver {
        command: vec!["gopls".into()],
        output_parser: Some("gopls_v0_15".into()),
    };
    match r.resolve_symbol_at(Path::new("/dev/null"), 1) {
        Err(ResolverError::NotImplemented) => {}
        other => panic!("expected NotImplemented, got {:?}", other),
    }
}

#[test]
fn version_surface_present_on_all_transports() {
    let mcp = McpResolver { command: vec![] };
    let cli = CliResolver {
        command: vec![],
        output_parser: None,
    };
    let mvs = mcp.version_surface();
    let cvs = cli.version_surface();
    assert!(mvs.plugin_name.contains("McpResolver"));
    assert!(cvs.plugin_name.contains("CliResolver"));
    assert_eq!(mvs.schema_min, 4);
    assert_eq!(cvs.schema_min, 4);
}
