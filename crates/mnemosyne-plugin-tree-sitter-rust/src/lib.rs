//! Tree-sitter Rust `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are Rust's — the grammar, the query naming its
//! declaration nodes, how a name comes out of one, and the fact that Rust takes
//! NO doc-comment rule (see [`SPEC`]).
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{field_text, LanguageSpec, TreesitterResolver};
use tree_sitter::{Node, Query};

pub const BACKEND_KEY: &str = "tree-sitter-rust";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under.
///
/// Declared here rather than at the wiring site because it is a property of
/// what this crate parses: `tree-sitter-rust` answers in Rust's vocabulary and
/// in no other, so pairing it with a different language key produces
/// enforcement against names a grammar that never saw the language invented.
pub const SYMBOL_AXIS_LANGUAGE: &str = "rust";

static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();

/// Rust's four differences.
///
/// `documented_kinds` IS EMPTY, AND THAT IS A DECISION WITH A CONSEQUENCE. A
/// `§` citation written in a `///` line above an item resolves to the enclosing
/// item — or to nothing, at the top level — rather than to the item below it,
/// which is the opposite of what the C++ backend does with the same shape. The
/// two behaviours predate this crate and the port preserves both byte for byte;
/// which one is right is a question with an answer and neither is written down
/// as a law yet.
pub static SPEC: LanguageSpec = LanguageSpec {
    backend_key: BACKEND_KEY,
    plugin_name: "mnemosyne-plugin-tree-sitter-rust",
    plugin_version: env!("CARGO_PKG_VERSION"),
    symbol_axis_language: SYMBOL_AXIS_LANGUAGE,
    language: || tree_sitter_rust::LANGUAGE.into(),
    query_source: r"
        (function_item) @item
        (struct_item) @item
        (enum_item) @item
        (trait_item) @item
        (impl_item) @item
        (mod_item) @item
        (const_item) @item
        (static_item) @item
        (type_item) @item
        (union_item) @item
        (macro_definition) @item
    ",
    name_of: rust_name_of,
    documented_kinds: &[],
    comment_kind: "line_comment",
    query_cache: &QUERY,
};

/// The declared name of a Rust declaration node.
///
/// The KIND FILTER on each field is what keeps a form this language has no
/// spelling for out of the answer. `impl Foo<T>` has a `generic_type` where
/// `impl Foo` has a `type_identifier`, and no author records an
/// `Implementation.symbol` of `Foo<T>`, so the generic form resolves to the
/// enclosing scope instead of to text nobody would write in the store.
fn rust_name_of(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "impl_item" => field_text(node, "type", &["type_identifier"], src),
        "function_item" | "mod_item" | "const_item" | "static_item" | "macro_definition" => {
            field_text(node, "name", &["identifier"], src)
        }
        "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" => {
            field_text(node, "name", &["type_identifier"], src)
        }
        _ => None,
    }
}

/// This backend, ready to register.
#[must_use]
pub fn resolver() -> TreesitterResolver {
    TreesitterResolver::new(&SPEC)
}

/// Register this backend into the given `PluginRegistry`.
pub fn register(registry: &mut PluginRegistry) {
    registry.register_symbol_resolver(BACKEND_KEY, Box::new(resolver()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_core::SymbolResolver;
    use std::collections::BTreeMap;
    use std::path::Path;

    /// Resolve one line — through a path that DOES NOT EXIST, which is the
    /// oracle for "the answer came from the caller's bytes". Every test below
    /// therefore also asserts the resolver never reads the filesystem.
    fn resolve(source: &str, line: u32) -> Option<String> {
        resolver()
            .resolve_symbols_at(Path::new("/no/such/file.rs"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    #[test]
    fn fn_name_at_definition_line() {
        let src = "fn alpha() -> u32 { 42 }\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("alpha"));
    }

    #[test]
    fn fn_name_inside_body() {
        let src = "fn beta() -> u32 {\n    let x = 1;\n    x\n}\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("beta"));
    }

    #[test]
    fn struct_name() {
        let src = "pub struct Gamma {\n    field: u32,\n}\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("Gamma"));
    }

    #[test]
    fn nested_fn_inside_impl_takes_inner() {
        let src = "impl Delta {\n    fn epsilon(&self) {}\n}\n";
        // line 2 is inside both `impl Delta` and `fn epsilon` — inner
        // wins because we pick the smallest covering declaration.
        assert_eq!(resolve(src, 2).as_deref(), Some("epsilon"));
    }

    #[test]
    fn line_outside_any_item_returns_none() {
        let src = "// just a comment\n\nfn theta() {}\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2), None);
        assert_eq!(resolve(src, 3).as_deref(), Some("theta"));
    }

    /// A doc comment above an item binds to the ENCLOSING item, not to the item
    /// below — Rust's spec takes no `documented_kinds`. Pinned because the C++
    /// backend does the opposite with the same shape, and the port that put
    /// both on one engine is exactly when the two could have been quietly
    /// unified.
    #[test]
    fn a_doc_comment_above_an_item_does_not_bind_to_that_item() {
        let src = "/// documents alpha\nfn alpha() {}\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2).as_deref(), Some("alpha"));
    }

    /// A generic `impl` has no name this language records, so the line falls
    /// through to whatever encloses it rather than resolving to `Foo<T>`.
    #[test]
    fn a_generic_impl_is_not_named_after_its_type_expression() {
        let src = "impl Foo<T> {\n    fn m(&self) {}\n}\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2).as_deref(), Some("m"));
    }

    #[test]
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }

    /// Many lines, one call — and the per-line answers are the ones each line
    /// gets on its own. A batched resolver that returned the first match for
    /// every line, or that lost the smallest-covering-declaration rule when
    /// several lines share a parse, would pass a call-count test and fail here.
    #[test]
    fn one_call_answers_every_line_exactly_as_a_single_line_call_would() {
        let src = "fn alpha() {}\n\
                   // a comment between items\n\
                   impl Delta {\n    fn epsilon(&self) {}\n}\n\
                   struct Gamma;\n";
        let lines = [1u32, 2, 3, 4, 6];
        let batched = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.rs"), src, &lines)
            .unwrap();
        let one_at_a_time: BTreeMap<u32, String> = lines
            .iter()
            .filter_map(|l| resolve(src, *l).map(|s| (*l, s)))
            .collect();
        assert_eq!(
            batched, one_at_a_time,
            "batching must not change a single answer"
        );
        assert_eq!(
            batched.get(&4).map(String::as_str),
            Some("epsilon"),
            "the smallest covering declaration still wins inside a batch"
        );
        assert_eq!(
            batched.get(&3).map(String::as_str),
            Some("Delta"),
            "and the outer one still wins on its own line"
        );
        assert!(
            !batched.contains_key(&2),
            "a line inside no item is absent, not guessed: {batched:?}"
        );
    }

    /// Line 0 has no row. It is dropped rather than shifted onto line 1, which
    /// is what the single-line form did by returning `None` for it.
    #[test]
    fn line_zero_is_dropped_and_does_not_become_line_one() {
        let out = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.rs"), "fn alpha() {}\n", &[0, 1])
            .unwrap();
        assert_eq!(out.get(&1).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0), "no answer for a line that cannot be");
    }

    /// The spec files this backend under the language it answers in, and the
    /// resolver reports the crate it came from.
    #[test]
    fn the_spec_names_this_crate_and_its_language() {
        let surface = resolver().version_surface();
        assert_eq!(surface.plugin_name, "mnemosyne-plugin-tree-sitter-rust");
        assert_eq!(SPEC.symbol_axis_language, "rust");
        assert_eq!(SPEC.backend_key, BACKEND_KEY);
    }
}
