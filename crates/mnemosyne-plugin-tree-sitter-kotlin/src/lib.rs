//! Tree-sitter Kotlin `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are Kotlin's.
//!
//! KOTLIN IS THE ONE OF THE THREE THAT IS NOT MERELY A MISSING PLUGIN. `.go`
//! and `.py` were already on the symbol-axis extension table with nothing
//! behind them; `.kt` was on NO row, so the table made no claim about a Kotlin
//! file at all and a citation in one took file-level binding with the census
//! naming it as an extension that maps to no language. This round adds the rows
//! AND the backend, which is not a choice: since Round 1154 the reach contract
//! requires `languages_without_backend` to be empty, so an extension row landing
//! without a resolver fails at the moment it is added.
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{
    field_text, DocCommentRule, LanguageSpec, TreesitterResolver,
};
use tree_sitter::{Node, Query};

pub const BACKEND_KEY: &str = "tree-sitter-kotlin";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under. `.kt` and `.kts`
/// both map here, which is why the id is the language's name and not one
/// extension's.
pub const SYMBOL_AXIS_LANGUAGE: &str = "kotlin";

static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();

/// Kotlin's answer to the doc-comment criterion (`DocCommentRule`).
///
/// 1. TWO SPELLINGS, AND KOTLIN IS THE LANGUAGE THAT MADE THIS FIELD A LIST.
///    Every other grammar here calls both forms `comment`, so one string
///    carried the whole answer and nothing said otherwise; this one calls `//`
///    a `line_comment` and KDoc a `block_comment`, and a citation may sit in
///    either.
///
/// 2. NO INWARD MARKER, AND NONE IS NEEDED. Kotlin has no spelling for
///    "documents the scope I am in" — KDoc is written above its subject, and
///    the language has no counterpart to Rust's `//!`.
///
/// 3. KDOC SITS DIRECTLY ABOVE THE DECLARATION IT DOCUMENTS, exactly as Go's
///    and C++'s conventions do, and a `//` line in the same position is the
///    other half of the same habit.
///
///    `property_declaration` is here, and it is the interesting one: see
///    [`kotlin_name_of`] for why listing it does not put a function-body local
///    in front of an enclosing function.
const DOC_COMMENTS: DocCommentRule = DocCommentRule {
    comment_kinds: &["line_comment", "block_comment"],
    inward_markers: &[],
    documented_kinds: &[
        "class_declaration",
        "function_declaration",
        "object_declaration",
        "property_declaration",
    ],
};

/// Kotlin's four differences.
pub static SPEC: LanguageSpec = LanguageSpec {
    backend_key: BACKEND_KEY,
    plugin_name: "mnemosyne-plugin-tree-sitter-kotlin",
    plugin_version: env!("CARGO_PKG_VERSION"),
    symbol_axis_language: SYMBOL_AXIS_LANGUAGE,
    language: || tree_sitter_kotlin_ng::LANGUAGE.into(),
    query_source: r"
        (class_declaration) @item
        (function_declaration) @item
        (object_declaration) @item
        (property_declaration) @item
    ",
    name_of: kotlin_name_of,
    doc_comments: DOC_COMMENTS,
    query_cache: &QUERY,
};

/// The declared name of a Kotlin declaration node.
///
/// A PROPERTY IS NAMED ONLY WHERE A PROPERTY CAN BE AN IMPLEMENTATION. Kotlin
/// spells a class member and a function-body local with the SAME node kind —
/// `property_declaration` — so the kind alone cannot separate the two the way
/// C++'s `field_declaration` and `declaration` do. The parent can: a member
/// hangs off a `class_body` and a top-level property off the `source_file`,
/// while a local hangs off a `block`. Answering `None` for the local is what
/// keeps a citation inside a function body resolving to the FUNCTION, which is
/// the decision the C++ backend made by excluding locals from its query — and
/// because the engine skips a captured node whose name is `None`, one guard
/// here covers both passes.
///
/// A local variable is never recorded as an `Implementation.symbol`, so binding
/// to one would not be a coarser answer but a WRONG one, and it would produce a
/// `symbol_mismatch` against a name no author wrote.
fn kotlin_name_of(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "class_declaration" | "function_declaration" | "object_declaration" => {
            field_text(node, "name", &["identifier", "type_identifier"], src)
        }
        "property_declaration" => {
            let parent = node.parent()?;
            if !matches!(parent.kind(), "class_body" | "source_file") {
                return None;
            }
            // `val k = 2` puts the name under a `variable_declaration` rather
            // than in a `name` field.
            let mut cursor = node.walk();
            let decl = node
                .named_children(&mut cursor)
                .find(|c| c.kind() == "variable_declaration")?;
            let mut inner = decl.walk();
            let name = decl
                .named_children(&mut inner)
                .find(|c| c.kind() == "identifier")
                .and_then(|n| n.utf8_text(src).ok())
                .map(str::to_string);
            name
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
            .resolve_symbols_at(Path::new("/no/such/file.kt"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    #[test]
    fn fun_name_at_definition_line() {
        let src = "package p\n\nfun alpha(): Int {\n    return 42\n}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("alpha"));
    }

    #[test]
    fn fun_name_inside_body() {
        let src = "package p\n\nfun beta(): Int {\n    val x = 1\n    return x\n}\n";
        assert_eq!(resolve(src, 5).as_deref(), Some("beta"));
    }

    /// A function-body local is a `property_declaration` too, and it must NOT
    /// take the line from the function that contains it.
    #[test]
    fn a_local_property_does_not_take_the_line_from_its_function() {
        let src = "package p\n\nfun beta(): Int {\n    val x = 1\n    return x\n}\n";
        assert_eq!(resolve(src, 4).as_deref(), Some("beta"));
    }

    #[test]
    fn class_name() {
        let src = "package p\n\nclass Gamma {\n    val field = 0\n}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("Gamma"));
    }

    /// A CLASS MEMBER property is named — the same node kind as the local
    /// above, separated by what it hangs off.
    #[test]
    fn a_class_member_property_is_named() {
        let src = "package p\n\nclass Gamma {\n    val field = 0\n}\n";
        assert_eq!(resolve(src, 4).as_deref(), Some("field"));
    }

    #[test]
    fn nested_method_takes_inner() {
        let src = "package p\n\nclass Delta {\n    fun epsilon() {\n        val y = 1\n    }\n}\n";
        assert_eq!(resolve(src, 5).as_deref(), Some("epsilon"));
        assert_eq!(resolve(src, 3).as_deref(), Some("Delta"));
    }

    #[test]
    fn object_name() {
        let src = "package p\n\nobject Single {\n    val k = 2\n}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("Single"));
    }

    #[test]
    fn a_line_comment_above_a_fun_binds_to_that_fun() {
        let src = "package p\n\n// Alpha does a thing. §X\nfun alpha() {}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("alpha"));
    }

    /// KDOC IS THE OTHER COMMENT SPELLING, and the one a Kotlin author actually
    /// writes documentation in. A spec that named only `line_comment` would
    /// leave every KDoc citation falling through to the enclosing scope, which
    /// reads exactly like a language that chose not to have the rule.
    #[test]
    fn a_kdoc_block_above_a_class_binds_to_that_class() {
        let src =
            "package p\n\n/**\n * Widget is a thing. §X\n */\nclass Widget {\n    val a = 1\n}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("Widget"));
        assert_eq!(resolve(src, 4).as_deref(), Some("Widget"));
        assert_eq!(resolve(src, 5).as_deref(), Some("Widget"));
    }

    #[test]
    fn a_blank_line_breaks_the_doc_association() {
        let src = "package p\n\n// a standalone note §X\n\nfun alpha() {}\n";
        assert_eq!(resolve(src, 3), None);
    }

    /// A comment above a function-body local documents nothing nameable, so it
    /// falls through to the enclosing function rather than to the local.
    #[test]
    fn a_comment_above_a_local_binds_to_the_enclosing_function() {
        let src =
            "package p\n\nfun beta(): Int {\n    // note §X\n    val x = 1\n    return x\n}\n";
        assert_eq!(resolve(src, 4).as_deref(), Some("beta"));
    }

    #[test]
    fn line_outside_any_declaration_returns_none() {
        let src = "package p\n\nimport kotlin.math.abs\n\nfun theta() = abs(1)\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 3), None);
        assert_eq!(resolve(src, 5).as_deref(), Some("theta"));
    }

    #[test]
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }

    /// Many lines, one call — and every per-line answer is the one that line
    /// gets on its own. Kotlin has TWO resolution paths (the comment rule over
    /// TWO comment spellings, and the smallest covering declaration), so the
    /// batch must not let one line's path decide another's.
    #[test]
    fn one_call_answers_every_line_exactly_as_a_single_line_call_would() {
        let src = "package p\n\n\
                   /** Widget is a thing. */\n\
                   class Widget {\n\
                   \x20   // draw draws.\n\
                   \x20   fun draw(): Int {\n\
                   \x20       val x = 1\n\
                   \x20       return x\n\
                   \x20   }\n\
                   }\n";
        let lines = [3u32, 4, 5, 6, 7, 8];
        let batched = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.kt"), src, &lines)
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
            batched.get(&3).map(String::as_str),
            Some("Widget"),
            "a KDoc block still binds to what it documents inside a batch"
        );
        assert_eq!(
            batched.get(&5).map(String::as_str),
            Some("draw"),
            "and the member's line comment still binds to the member"
        );
        assert_eq!(
            batched.get(&7).map(String::as_str),
            Some("draw"),
            "the local does not take the line from the function around it"
        );
    }

    /// Line 0 has no row. It is dropped rather than shifted onto line 1.
    #[test]
    fn line_zero_is_dropped_and_does_not_become_line_one() {
        let out = resolver()
            .resolve_symbols_at(
                Path::new("/no/such/file.kt"),
                "package p\n\nfun alpha() {}\n",
                &[0, 3],
            )
            .unwrap();
        assert_eq!(out.get(&3).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0), "no answer for a line that cannot be");
    }

    /// The spec files this backend under the language it answers in, and the
    /// resolver reports the crate it came from.
    #[test]
    fn the_spec_names_this_crate_and_its_language() {
        let surface = resolver().version_surface();
        assert_eq!(surface.plugin_name, "mnemosyne-plugin-tree-sitter-kotlin");
        assert_eq!(SPEC.symbol_axis_language, "kotlin");
        assert_eq!(SPEC.backend_key, BACKEND_KEY);
    }
}
