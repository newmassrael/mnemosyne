//! Tree-sitter Go `SymbolResolver` backend for the Mnemosyne plugin substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are Go's.
//!
//! WHY GO, AND WHY NOW. The extension table has routed `.go` to the language
//! `go` since Round 855, and this build shipped no resolver for it, so every
//! citation in a Go file took FILE-level binding while `severity_binding =
//! reject` read as symbol-level enforcement. Round 1151 made that gap something
//! the binary says out loud; this closes it. The consumer that reported it
//! enrols five backend runtimes implementing the same clauses, and their own
//! test names this one first.
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{field_text, LanguageSpec, TreesitterResolver};
use tree_sitter::{Node, Query};

pub const BACKEND_KEY: &str = "tree-sitter-go";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under.
pub const SYMBOL_AXIS_LANGUAGE: &str = "go";

static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();

/// Declaration kinds a comment immediately above may be documenting.
///
/// GO TAKES THE RULE, and its own convention is the reason: a doc comment sits
/// directly above the declaration it documents, with no blank line, and `go
/// doc` reads exactly that adjacency.
///
/// BOTH SPELLINGS OF A TYPE / CONST / VAR ARE HERE, and the pair is what a
/// batch test caught. `type Widget struct{}` at top level gives the comment a
/// `type_declaration` sibling, while `type ( … )` gives each member its own
/// `type_spec` and the comment inside the group sits beside THAT. Listing only
/// the spec kinds left the ordinary, ungrouped form — by far the common one —
/// resolving to nothing, which is what a rule that silently never fires looks
/// like from outside.
const DOCUMENTED_KINDS: &[&str] = &[
    "function_declaration",
    "method_declaration",
    "type_declaration",
    "const_declaration",
    "var_declaration",
    "type_spec",
    "const_spec",
    "var_spec",
];

/// Go's four differences.
///
/// The query captures the `*_spec` nodes and NOT the `*_declaration` that wraps
/// them, which is the opposite of what `DOCUMENTED_KINDS` needs and for a
/// different question. Pass 2 asks which declaration COVERS a line, and in a
/// grouped `type ( … )` the spec is the member whose extent contains it; the
/// wrapper's extent is the whole group and would answer with the group's name
/// for every member. Pass 1 asks which declaration a comment SITS ABOVE, and
/// there the wrapper is the sibling.
pub static SPEC: LanguageSpec = LanguageSpec {
    backend_key: BACKEND_KEY,
    plugin_name: "mnemosyne-plugin-tree-sitter-go",
    plugin_version: env!("CARGO_PKG_VERSION"),
    symbol_axis_language: SYMBOL_AXIS_LANGUAGE,
    language: || tree_sitter_go::LANGUAGE.into(),
    query_source: r"
        (function_declaration) @item
        (method_declaration) @item
        (type_spec) @item
        (const_spec) @item
        (var_spec) @item
    ",
    name_of: go_name_of,
    documented_kinds: DOCUMENTED_KINDS,
    comment_kinds: &["comment"],
    query_cache: &QUERY,
};

/// The declared name of a Go declaration node.
///
/// A METHOD RESOLVES TO ITS BARE NAME, not to `Receiver.Method`. Go's own
/// vocabulary is what an author records as an `Implementation.symbol`, and
/// `godoc` names a method `Method` under the type it hangs from rather than as
/// one qualified token — the opposite of the C++ backend, where an out-of-line
/// definition is written `Foo::bar` in the source itself and so resolves that
/// way.
fn go_name_of(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "function_declaration" | "const_spec" | "var_spec" => {
            field_text(node, "name", &["identifier"], src)
        }
        "method_declaration" => field_text(node, "name", &["field_identifier"], src),
        "type_spec" => field_text(node, "name", &["type_identifier"], src),
        // An UNGROUPED declaration wraps exactly one spec, and a comment above
        // it documents that one thing. A GROUPED one wraps several, and a
        // comment above the group documents the group — no single member — so
        // it answers nothing rather than picking the first.
        "type_declaration" => sole_spec(node, "type_spec").and_then(|s| go_name_of(s, src)),
        "const_declaration" => sole_spec(node, "const_spec").and_then(|s| go_name_of(s, src)),
        "var_declaration" => sole_spec(node, "var_spec").and_then(|s| go_name_of(s, src)),
        _ => None,
    }
}

/// The one child of `kind` under `node`, or `None` when there is not exactly
/// one.
fn sole_spec<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut found = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind {
            if found.is_some() {
                return None;
            }
            found = Some(child);
        }
    }
    found
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
            .resolve_symbols_at(Path::new("/no/such/file.go"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    #[test]
    fn func_name_at_definition_line() {
        let src = "package p\n\nfunc alpha() int { return 42 }\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("alpha"));
    }

    #[test]
    fn func_name_inside_body() {
        let src = "package p\n\nfunc beta() int {\n\tx := 1\n\treturn x\n}\n";
        assert_eq!(resolve(src, 4).as_deref(), Some("beta"));
    }

    #[test]
    fn method_resolves_to_its_bare_name() {
        let src = "package p\n\ntype Holder struct{}\n\nfunc (h Holder) alpha() {\n\t_ = 1\n}\n";
        assert_eq!(resolve(src, 6).as_deref(), Some("alpha"));
    }

    #[test]
    fn type_name() {
        let src = "package p\n\ntype Gamma struct {\n\tField int\n}\n";
        assert_eq!(resolve(src, 4).as_deref(), Some("Gamma"));
    }

    /// A grouped declaration gives each member its own spec, so a line inside
    /// the group resolves to THAT member and not to the group.
    #[test]
    fn a_grouped_type_resolves_to_the_member_not_the_group() {
        let src = "package p\n\ntype (\n\tFirst struct {\n\t\tA int\n\t}\n\tSecond int\n)\n";
        assert_eq!(resolve(src, 5).as_deref(), Some("First"));
        assert_eq!(resolve(src, 7).as_deref(), Some("Second"));
    }

    /// Go's doc-comment convention: the comment directly above a declaration
    /// documents it, so a `§` citation written there binds to the declaration
    /// rather than to the file.
    #[test]
    fn a_doc_comment_above_a_func_binds_to_that_func() {
        let src = "package p\n\n// Alpha does a thing. §X\nfunc Alpha() {}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("Alpha"));
    }

    #[test]
    fn a_blank_line_breaks_the_doc_association() {
        let src = "package p\n\n// a standalone note §X\n\nfunc Alpha() {}\n";
        assert_eq!(resolve(src, 3), None);
    }

    /// The ungrouped spelling — the common one — reaches the type through the
    /// `type_declaration` that wraps it.
    #[test]
    fn a_doc_comment_above_an_ungrouped_type_binds_to_that_type() {
        let src = "package p\n\n// Widget is a thing. §X\ntype Widget struct {\n\tA int\n}\n";
        assert_eq!(resolve(src, 3).as_deref(), Some("Widget"));
    }

    /// A comment above a GROUPED declaration documents the group, and the group
    /// has no single name — so it answers nothing rather than picking a member.
    #[test]
    fn a_doc_comment_above_a_group_names_no_member() {
        let src =
            "package p\n\n// these belong together §X\ntype (\n\tFirst int\n\tSecond int\n)\n";
        assert_eq!(resolve(src, 3), None);
    }

    #[test]
    fn line_outside_any_declaration_returns_none() {
        let src = "package p\n\nimport \"fmt\"\n\nfunc theta() { fmt.Println() }\n";
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
    /// gets on its own. Go has TWO resolution paths (the doc-comment rule and
    /// the smallest covering declaration), so the batch must not let one line's
    /// path decide another's.
    #[test]
    fn one_call_answers_every_line_exactly_as_a_single_line_call_would() {
        let src = "package p\n\n\
                   // Widget is a thing.\n\
                   type Widget struct {\n\tA int\n}\n\n\
                   // Draw draws.\n\
                   func (w Widget) Draw() {\n\t_ = 1\n}\n";
        let lines = [1u32, 3, 4, 5, 8, 9, 10];
        let batched = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.go"), src, &lines)
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
            "a doc comment still binds to what it documents inside a batch"
        );
        assert_eq!(
            batched.get(&8).map(String::as_str),
            Some("Draw"),
            "and the method's doc comment still binds to the method"
        );
    }

    /// Line 0 has no row. It is dropped rather than shifted onto line 1.
    #[test]
    fn line_zero_is_dropped_and_does_not_become_line_one() {
        let out = resolver()
            .resolve_symbols_at(
                Path::new("/no/such/file.go"),
                "package p\n\nfunc alpha() {}\n",
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
        assert_eq!(surface.plugin_name, "mnemosyne-plugin-tree-sitter-go");
        assert_eq!(SPEC.symbol_axis_language, "go");
        assert_eq!(SPEC.backend_key, BACKEND_KEY);
    }
}
