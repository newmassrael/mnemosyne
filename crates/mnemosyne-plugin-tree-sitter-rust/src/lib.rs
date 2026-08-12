//! Tree-sitter Rust `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are Rust's — the grammar, the query naming its
//! declaration nodes, how a name comes out of one, and where a citation written
//! in a comment binds (see [`DOC_COMMENTS`]).
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{
    field_text, DocCommentRule, LanguageSpec, TreesitterResolver,
};
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

/// Rust's answer to the doc-comment criterion (`DocCommentRule`).
///
/// THIS FIELD WAS EMPTY UNTIL ROUND 1162, and empty was the one value that
/// meant "no rule at all". A `§` citation in a `///` line above an item bound
/// to the ENCLOSING item — or to nothing, at the top level — which is the
/// opposite of what the C++ backend did with the same shape, and the crate's
/// own doc said which one is right is a question with an answer and left it.
///
/// THE ANSWER IS THE MARKER. The reason to hesitate was real: Rust spells
/// "documents the module I am in" as `//!` and "documents the item below" as
/// `///`, and BOTH are a `line_comment` — so a rule keyed on the comment's kind
/// would have bound a module-level citation to whatever declaration happened to
/// follow it. The grammar does not leave it there: it records
/// `inner_doc_comment_marker` under the first and `outer_doc_comment_marker`
/// under the second. Naming the inward marker is what lets the rule serve this
/// language, and `/*! */` needs it for exactly the same reason `//!` does.
///
/// EVERY QUERY KIND IS DOCUMENTABLE HERE. There is no container among them a
/// comment merely sits inside — `mod_item` is documented from OUTSIDE by `///`
/// and from INSIDE by `//!`, and the marker is what separates those two — and
/// Rust's function-body locals are `let_declaration`, which the query does not
/// capture, so a citation inside a body still binds to the function.
pub const DOC_COMMENTS: DocCommentRule = DocCommentRule {
    comment_kinds: &["line_comment", "block_comment"],
    inward_markers: &["inner_doc_comment_marker"],
    documented_kinds: &[
        "function_item",
        "struct_item",
        "enum_item",
        "trait_item",
        "impl_item",
        "mod_item",
        "const_item",
        "static_item",
        "type_item",
        "union_item",
        "macro_definition",
    ],
};

/// Rust's four differences.
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
    doc_comments: DOC_COMMENTS,
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

    /// A comment separated from what follows by a blank line documents nothing
    /// — a file header does not become the name of the first item under it.
    #[test]
    fn line_outside_any_item_returns_none() {
        let src = "// just a comment\n\nfn theta() {}\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2), None);
        assert_eq!(resolve(src, 3).as_deref(), Some("theta"));
    }

    /// AN OUTER DOC COMMENT BINDS TO THE ITEM IT DOCUMENTS. Until Round 1162
    /// this backend answered `None` here — the citation bound to the enclosing
    /// item, or to nothing at the top level — which is the opposite of what the
    /// C++ backend did with the same shape.
    #[test]
    fn an_outer_doc_comment_binds_to_the_item_below_it() {
        let src = "/// documents alpha\nfn alpha() {}\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("alpha"));
        assert_eq!(resolve(src, 2).as_deref(), Some("alpha"));
    }

    /// AN INNER DOC COMMENT DOES NOT, because it is about the module it is in.
    /// Both spellings are a `line_comment`, so nothing but the grammar's marker
    /// separates this case from the one above — and the module IS what falls
    /// through to pass 2 here: nothing at the top of a file, and the `mod` when
    /// the comment is inside one.
    #[test]
    fn an_inner_doc_comment_binds_to_the_module_and_not_to_the_item_below() {
        let src = "//! documents this module\npub struct Alpha;\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2).as_deref(), Some("Alpha"));

        let nested = "mod inner {\n    //! documents inner\n    pub struct Beta;\n}\n";
        assert_eq!(resolve(nested, 2).as_deref(), Some("inner"));
        assert_eq!(resolve(nested, 3).as_deref(), Some("Beta"));
    }

    /// The block spellings are the same two cases: `/** */` documents what
    /// follows and `/*! */` documents the module, and the grammar marks them
    /// the same way it marks the line forms.
    #[test]
    fn the_block_doc_spellings_split_the_same_way() {
        let outer = "/** documents gamma */\nfn gamma() {}\n";
        assert_eq!(resolve(outer, 1).as_deref(), Some("gamma"));
        let inner = "/*! documents this module */\nfn gamma() {}\n";
        assert_eq!(resolve(inner, 1), None);
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
                   /// documents Delta\n\
                   impl Delta {\n    fn epsilon(&self) {}\n}\n\
                   \n\
                   // a header, detached\n\
                   \n\
                   struct Gamma;\n";
        let lines = [1u32, 2, 3, 4, 7, 9];
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
        assert_eq!(
            batched.get(&2).map(String::as_str),
            Some("Delta"),
            "the doc-comment pass runs inside a batch too"
        );
        assert!(
            !batched.contains_key(&7),
            "a comment a blank line away from everything is absent, not \
             guessed: {batched:?}"
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
