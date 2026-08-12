//! Tree-sitter Python `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are Python's.
//!
//! WHY PYTHON, AND WHY NOW. `.py` has routed to the language `python` since
//! Round 855 and this build shipped no resolver for it, so every citation in a
//! Python file took FILE-level binding while `severity_binding = reject` read
//! as symbol-level enforcement. It is the second of the three backends the
//! consumer's spec ledger names, and the second-to-last line of what
//! `describe-symbol-axis-reach` publishes as `languages_without_backend`.
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{
    field_text, DocCommentRule, LanguageSpec, TreesitterResolver,
};
use tree_sitter::{Node, Query};

pub const BACKEND_KEY: &str = "tree-sitter-python";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under.
pub const SYMBOL_AXIS_LANGUAGE: &str = "python";

static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();

/// Python's answer to the doc-comment criterion (`DocCommentRule`).
///
/// 1. ONE SPELLING. This grammar calls a `#` line a `comment`, and Python has
///    no other comment form.
///
/// 2. NO INWARD MARKER, AND THE DOCSTRING IS NOT ONE. A module docstring is the
///    nearest thing Python has to "documents the scope I am in", and it is not
///    a comment at all — it is a string expression, and the citation gate reads
///    COMMENTS. So there is no spelling in this language that a marker would
///    have to separate.
///
/// 3. THE DOCSTRING IS ALSO NOT A REASON TO SKIP THE RULE. It lives INSIDE the
///    definition, so it is not what the rule is about, and it is not where a
///    citation can live either. The consumer met that exactly, in a commit
///    titled "Read a Python docstring where the citation gate reads a comment".
///    What a `§` citation in a Python file actually is, then, is a `#` line, and
///    the place authors put one is directly above the `def` or `class` it is
///    about — the same adjacency Go and C++ take.
///
///    `decorated_definition` IS HERE BECAUSE THE COMMENT'S SIBLING IS THE
///    DECORATOR'S WRAPPER, not the `def` inside it. Listing only the two inner
///    kinds would leave every decorated function — a large share of real Python
///    — resolving to its enclosing scope, which is the shape Round 1153 shipped
///    and caught one language earlier.
const DOC_COMMENTS: DocCommentRule = DocCommentRule {
    comment_kinds: &["comment"],
    inward_markers: &[],
    documented_kinds: &[
        "function_definition",
        "class_definition",
        "decorated_definition",
    ],
};

/// Python's four differences.
///
/// The query captures `decorated_definition` ALONGSIDE the definitions it
/// wraps, and the two answer different lines: a citation on a decorator line is
/// inside the wrapper only, while one in the body is inside both and the
/// smallest-covering rule takes the inner `function_definition`. Both resolve
/// to the same name, so the pair costs nothing and reaches a region the inner
/// node does not span.
pub static SPEC: LanguageSpec = LanguageSpec {
    backend_key: BACKEND_KEY,
    plugin_name: "mnemosyne-plugin-tree-sitter-python",
    plugin_version: env!("CARGO_PKG_VERSION"),
    symbol_axis_language: SYMBOL_AXIS_LANGUAGE,
    language: || tree_sitter_python::LANGUAGE.into(),
    query_source: r"
        (function_definition) @item
        (class_definition) @item
        (decorated_definition) @item
    ",
    name_of: python_name_of,
    doc_comments: DOC_COMMENTS,
    query_cache: &QUERY,
};

/// The declared name of a Python declaration node.
///
/// A METHOD RESOLVES TO ITS BARE NAME. Python's own vocabulary is what an
/// author records as an `Implementation.symbol` — `help()` and every traceback
/// name a method by the `def`'s identifier — so this follows Go rather than the
/// C++ backend's qualified out-of-line form, which is qualified only because
/// the source text itself is.
fn python_name_of(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            field_text(node, "name", &["identifier"], src)
        }
        // The decorators are the wrapper's other children; the thing with a
        // name is the definition it holds.
        "decorated_definition" => node
            .child_by_field_name("definition")
            .and_then(|d| python_name_of(d, src)),
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
            .resolve_symbols_at(Path::new("/no/such/file.py"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    #[test]
    fn def_name_at_definition_line() {
        let src = "def alpha():\n    return 42\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("alpha"));
    }

    #[test]
    fn def_name_inside_body() {
        let src = "def beta():\n    x = 1\n    return x\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("beta"));
    }

    #[test]
    fn class_name() {
        let src = "class Gamma:\n    field = 0\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("Gamma"));
    }

    #[test]
    fn nested_method_takes_inner() {
        let src = "class Delta:\n    def epsilon(self):\n        return 1\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("epsilon"));
        assert_eq!(resolve(src, 1).as_deref(), Some("Delta"));
    }

    /// A `#` citation directly above a `def` binds to that `def` — the shape a
    /// Python author writes, and the only shape the gate can read (a docstring
    /// is a string expression, not a comment).
    #[test]
    fn a_comment_above_a_def_binds_to_that_def() {
        let src = "# Alpha does a thing. §X\ndef alpha():\n    return 1\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("alpha"));
    }

    #[test]
    fn a_comment_above_a_class_binds_to_that_class() {
        let src = "# Widget is a thing. §X\nclass Widget:\n    pass\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("Widget"));
    }

    /// The comment's sibling is the DECORATOR'S wrapper, not the `def` inside
    /// it, so a decorated definition needs the wrapper listed or the rule does
    /// not fire for it at all.
    #[test]
    fn a_comment_above_a_decorated_def_binds_to_that_def() {
        let src = "# Zeta is decorated. §X\n@staticmethod\ndef zeta():\n    return 1\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("zeta"));
    }

    /// And a citation ON the decorator line resolves to the same name, which
    /// the inner `function_definition` alone could not answer — its extent
    /// starts below the decorator.
    #[test]
    fn a_citation_on_the_decorator_line_resolves_to_the_definition() {
        let src = "@staticmethod\ndef zeta():\n    return 1\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("zeta"));
    }

    #[test]
    fn a_blank_line_breaks_the_doc_association() {
        let src = "# a standalone note §X\n\ndef alpha():\n    return 1\n";
        assert_eq!(resolve(src, 1), None);
    }

    /// A docstring is NOT a comment, so the rule does not reach into the body
    /// looking for one — the line resolves by enclosure, as any body line does.
    #[test]
    fn a_docstring_line_resolves_by_enclosure_not_by_the_comment_rule() {
        let src = "def alpha():\n    \"\"\"documents alpha\"\"\"\n    return 1\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("alpha"));
    }

    #[test]
    fn line_outside_any_definition_returns_none() {
        let src = "import os\n\ndef theta():\n    return os\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 3).as_deref(), Some("theta"));
    }

    #[test]
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }

    /// Many lines, one call — and every per-line answer is the one that line
    /// gets on its own. Python has TWO resolution paths (the comment rule and
    /// the smallest covering definition), so the batch must not let one line's
    /// path decide another's.
    #[test]
    fn one_call_answers_every_line_exactly_as_a_single_line_call_would() {
        let src = "# Widget is a thing.\n\
                   class Widget:\n\
                   \x20   # draw draws.\n\
                   \x20   @staticmethod\n\
                   \x20   def draw():\n\
                   \x20       return 1\n";
        let lines = [1u32, 2, 3, 4, 5, 6];
        let batched = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.py"), src, &lines)
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
            batched.get(&1).map(String::as_str),
            Some("Widget"),
            "a comment still binds to what it documents inside a batch"
        );
        assert_eq!(
            batched.get(&3).map(String::as_str),
            Some("draw"),
            "and the decorated method's comment still binds to the method"
        );
        assert_eq!(
            batched.get(&6).map(String::as_str),
            Some("draw"),
            "the body line still takes the smallest covering definition"
        );
    }

    /// Line 0 has no row. It is dropped rather than shifted onto line 1.
    #[test]
    fn line_zero_is_dropped_and_does_not_become_line_one() {
        let out = resolver()
            .resolve_symbols_at(
                Path::new("/no/such/file.py"),
                "def alpha():\n    return 1\n",
                &[0, 1],
            )
            .unwrap();
        assert_eq!(out.get(&1).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0), "no answer for a line that cannot be");
    }

    /// The spec files this backend under the language it answers in, and the
    /// resolver reports the crate it came from.
    #[test]
    fn the_spec_names_this_crate_and_its_language() {
        let surface = resolver().version_surface();
        assert_eq!(surface.plugin_name, "mnemosyne-plugin-tree-sitter-python");
        assert_eq!(SPEC.symbol_axis_language, "python");
        assert_eq!(SPEC.backend_key, BACKEND_KEY);
    }
}
