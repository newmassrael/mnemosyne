//! Tree-sitter C++ `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! The walk lives once, in `mnemosyne-plugin-tree-sitter-core`; this crate is
//! the four things that are C++'s — the grammar, the query naming its
//! declaration nodes, how a name comes out of one, and the fact that C++ DOES
//! take the doc-comment rule (see [`SPEC`]).
//!
//! C++ declarators nest (a function name lives under `function_definition >
//! declarator > [pointer_declarator >] * function_declarator > declarator`), so
//! the query captures the DECLARATION node and [`cpp_name_of`] descends for the
//! name. Out-of-line definitions resolve to the source-text qualified form
//! (`Foo::bar`); inline members resolve to the bare member name (`bar`) — each
//! matches what the citation author records as the `Implementation.symbol` at
//! that location.
//!
//! Registered into the CLI's backend table (`mnemosyne_cli::backends`), which
//! the config wire and `describe-symbol-axis-reach` both read.

use std::sync::OnceLock;

use mnemosyne_core::PluginRegistry;
use mnemosyne_plugin_tree_sitter_core::{field_text, LanguageSpec, TreesitterResolver};
use tree_sitter::{Node, Query};

pub const BACKEND_KEY: &str = "tree-sitter-cpp";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under. `.c` and the header
/// extensions map to this same language, which is why the id is `cpp` and not
/// one extension's name.
///
/// Declared here rather than at the wiring site because it is a property of
/// what this crate parses: `tree-sitter-cpp` answers in C++'s vocabulary and in
/// no other, so pairing it with a different language key produces enforcement
/// against names a grammar that never saw the language invented.
pub const SYMBOL_AXIS_LANGUAGE: &str = "cpp";

static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();

/// Declaration node kinds a doc comment may document. The enclosing query set
/// MINUS `namespace_definition` — a comment is never "documenting" the
/// namespace it sits in — and `declaration` is absent from both, so a comment
/// above a function-body local binds to the enclosing function rather than to
/// the local.
const DOCUMENTED_KINDS: &[&str] = &[
    "function_definition",
    "field_declaration",
    "class_specifier",
    "struct_specifier",
    "union_specifier",
    "enum_specifier",
];

/// C++'s four differences.
///
/// `field_declaration` covers in-class member declarations (variables and
/// method prototypes); function-body locals are `declaration` nodes,
/// deliberately excluded so a citation inside a body resolves to the enclosing
/// function rather than to a local variable.
pub static SPEC: LanguageSpec = LanguageSpec {
    backend_key: BACKEND_KEY,
    plugin_name: "mnemosyne-plugin-tree-sitter-cpp",
    plugin_version: env!("CARGO_PKG_VERSION"),
    symbol_axis_language: SYMBOL_AXIS_LANGUAGE,
    language: || tree_sitter_cpp::LANGUAGE.into(),
    query_source: r"
        (function_definition) @item
        (field_declaration) @item
        (class_specifier) @item
        (struct_specifier) @item
        (union_specifier) @item
        (enum_specifier) @item
        (namespace_definition) @item
    ",
    name_of: cpp_name_of,
    documented_kinds: DOCUMENTED_KINDS,
    comment_kind: "comment",
    query_cache: &QUERY,
};

/// Name nodes that terminate a declarator descent. `qualified_identifier`
/// returns its full source text (e.g. `Foo::bar`) so out-of-line definitions
/// resolve to the qualified form an author records.
const NAME_KINDS: &[&str] = &[
    "identifier",
    "field_identifier",
    "qualified_identifier",
    "destructor_name",
    "operator_name",
    "operator_cast",
    "type_identifier",
];

/// The declared name of a C++ declaration node. Type-like and namespace nodes
/// read the `name` field directly; function and field declarations descend
/// their declarator. `None` for anonymous declarations (anonymous struct /
/// union / namespace).
fn cpp_name_of(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "class_specifier"
        | "struct_specifier"
        | "union_specifier"
        | "enum_specifier"
        | "namespace_definition" => field_text(node, "name", &[], src),
        "function_definition" | "field_declaration" => {
            declarator_name(node.child_by_field_name("declarator")?, src)
        }
        _ => None,
    }
}

/// Descend through wrapper declarators (`pointer_declarator`,
/// `reference_declarator`, `function_declarator`, `array_declarator`,
/// `parenthesized_declarator`, `init_declarator`) to the innermost name
/// node and return its source text. Follows the `declarator` field when
/// present; falls back to the first declarator-or-name child otherwise
/// (e.g. `reference_declarator`, where the inner declarator is positional).
fn declarator_name(start: Node, src: &[u8]) -> Option<String> {
    let mut cur = start;
    // Bounded by tree depth; the explicit cap guards against any grammar
    // shape that would otherwise fail to make progress.
    for _ in 0..64 {
        let kind = cur.kind();
        if NAME_KINDS.contains(&kind) {
            return cur.utf8_text(src).ok().map(str::to_string);
        }
        if let Some(next) = cur.child_by_field_name("declarator") {
            cur = next;
            continue;
        }
        let mut next = None;
        for i in 0..cur.named_child_count() as u32 {
            let child = cur.named_child(i)?;
            if child.kind().ends_with("declarator") || NAME_KINDS.contains(&child.kind()) {
                next = Some(child);
                break;
            }
        }
        cur = next?;
    }
    None
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
            .resolve_symbols_at(Path::new("/no/such/file.hpp"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    #[test]
    fn free_function_name_at_definition_line() {
        let src = "int alpha() { return 42; }\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("alpha"));
    }

    #[test]
    fn function_name_inside_body() {
        let src = "int beta() {\n    int x = 1;\n    return x;\n}\n";
        // Line 2 declares a local `int x` (a `declaration`, deliberately
        // not captured) inside `beta` — the enclosing function wins.
        assert_eq!(resolve(src, 2).as_deref(), Some("beta"));
    }

    #[test]
    fn class_name() {
        let src = "class Gamma {\n    int field;\n};\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("Gamma"));
    }

    #[test]
    fn nested_inline_method_takes_inner() {
        let src = "class Delta {\n    void epsilon() {}\n};\n";
        // Line 2 is inside both `class Delta` and the inline method — inner
        // wins because we pick the smallest covering declaration.
        assert_eq!(resolve(src, 2).as_deref(), Some("epsilon"));
    }

    #[test]
    fn out_of_line_definition_resolves_qualified() {
        let src = "void Foo::bar() {\n    return;\n}\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("Foo::bar"));
        assert_eq!(resolve(src, 2).as_deref(), Some("Foo::bar"));
    }

    #[test]
    fn pointer_return_function_resolves_name() {
        let src = "int* zeta() {\n    return nullptr;\n}\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("zeta"));
    }

    #[test]
    fn member_function_declaration_resolves() {
        let src = "class Eta {\n    void theta();\n};\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("theta"));
    }

    #[test]
    fn namespace_name() {
        let src = "namespace iota {\nint k;\n}\n";
        // Top-level `int k;` (line 2) is a `declaration`, not captured;
        // the enclosing namespace covers the line.
        assert_eq!(resolve(src, 2).as_deref(), Some("iota"));
    }

    #[test]
    fn line_outside_any_item_returns_none() {
        let src = "// just a comment\n\nint theta() { return 0; }\n";
        assert_eq!(resolve(src, 1), None);
        assert_eq!(resolve(src, 2), None);
        assert_eq!(resolve(src, 3).as_deref(), Some("theta"));
    }

    #[test]
    fn doc_comment_above_class_binds_to_class() {
        // The cite sits in a doc comment above the class it documents; the
        // enclosing scope is `ns`, but documented-symbol semantics bind it to
        // `Widget`.
        let src = "namespace ns {\n// documents Widget §X\nclass Widget {};\n}\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("Widget"));
    }

    #[test]
    fn doc_comment_above_method_binds_to_method() {
        let src = "class C {\n    // documents m §X\n    void m();\n};\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("m"));
    }

    #[test]
    fn multiline_doc_block_binds_every_line_to_following_decl() {
        let src = "// line one §X\n// line two\nclass Zeta {};\n";
        assert_eq!(resolve(src, 1).as_deref(), Some("Zeta"));
        assert_eq!(resolve(src, 2).as_deref(), Some("Zeta"));
    }

    #[test]
    fn blank_line_breaks_doc_association() {
        // A standalone comment separated by a blank line does NOT document the
        // class; it falls through to enclosing scope (none here).
        let src = "// standalone §X\n\nclass Q {};\n";
        assert_eq!(resolve(src, 1), None);
    }

    #[test]
    fn comment_in_body_binds_to_enclosing_function_not_local() {
        // The cite is a comment inside a function body, immediately above a
        // local declaration. The local is not a documentable decl, so it binds
        // to the enclosing function.
        let src = "void f() {\n    // note §X\n    int x = 1;\n}\n";
        assert_eq!(resolve(src, 2).as_deref(), Some("f"));
    }

    #[test]
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }

    /// Many lines, one call — and every per-line answer is the one that line
    /// gets on its own. C++ has TWO resolution paths (the documented-symbol
    /// rule for a comment above a declaration, and the smallest-covering
    /// declaration for everything else), so the batch must not let one line's
    /// path decide another's.
    #[test]
    fn one_call_answers_every_line_exactly_as_a_single_line_call_would() {
        let src = "namespace ns {\n\
                   // documents the class below\n\
                   class Widget {\n\
                     // documents the member below\n\
                     void draw();\n\
                   };\n\
                   }\n";
        let lines = [1u32, 2, 3, 4, 5, 6];
        let batched = resolver()
            .resolve_symbols_at(Path::new("/no/such/file.hpp"), src, &lines)
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
            batched.get(&2).map(String::as_str),
            Some("Widget"),
            "a doc comment still binds to what it documents inside a batch"
        );
        assert_eq!(
            batched.get(&4).map(String::as_str),
            Some("draw"),
            "and the member's doc comment still binds to the member"
        );
    }

    /// Line 0 has no row. It is dropped rather than shifted onto line 1.
    #[test]
    fn line_zero_is_dropped_and_does_not_become_line_one() {
        let out = resolver()
            .resolve_symbols_at(
                Path::new("/no/such/file.hpp"),
                "int alpha() { return 0; }\n",
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
        assert_eq!(surface.plugin_name, "mnemosyne-plugin-tree-sitter-cpp");
        assert_eq!(SPEC.symbol_axis_language, "cpp");
        assert_eq!(SPEC.backend_key, BACKEND_KEY);
    }
}
