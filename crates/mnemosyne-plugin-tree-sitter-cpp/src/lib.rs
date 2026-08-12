//! Tree-sitter C++ `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! Answers `(source, lines) -> {line: symbol_name}` by parsing the caller's
//! text ONCE with `tree-sitter-cpp` and walking the tree for the smallest
//! declarative node whose extent covers each requested line. Best-effort —
//! macro-expanded code, generated files, and code behind preprocessor gates may
//! resolve under their textual name rather than the post-expansion form.
//!
//! Nothing here reads the filesystem: the bytes come from the caller, which is
//! what keeps the symbol answer about the same file revision the citation was
//! extracted from.
//!
//! Unlike the Rust backend, C++ declarators nest (a function name lives
//! under `function_definition > declarator > [pointer_declarator >] *
//! function_declarator > declarator`), so the query captures the
//! *declaration node* directly and a declarator descent extracts the
//! name. Out-of-line definitions resolve to the source-text qualified
//! form (`Foo::bar`); inline members resolve to the bare member name
//! (`bar`) — each matches what the citation author records as the
//! `Implementation.symbol` at that location.
//!
//! Registered into a `PluginRegistry` via [`register`] from the binary's
//! startup path (mnemosyne-cli).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use mnemosyne_core::{PluginRegistry, ResolverError, SymbolResolver, VersionSurface};
use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};

pub const BACKEND_KEY: &str = "tree-sitter-cpp";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under. `.c` and the
/// header extensions map to this same language, which is why the id is `cpp`
/// and not one extension's name.
///
/// Declared here rather than at the wiring site because it is a property of
/// what this crate parses: `tree-sitter-cpp` answers in C++'s vocabulary and in
/// no other, so pairing it with a different language key produces enforcement
/// against names a grammar that never saw the language invented.
pub const SYMBOL_AXIS_LANGUAGE: &str = "cpp";

pub struct TreesitterCppResolver;

impl SymbolResolver for TreesitterCppResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: "mnemosyne-plugin-tree-sitter-cpp".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbols_at(
        &self,
        _file: &Path,
        source: &str,
        lines: &[u32],
    ) -> Result<BTreeMap<u32, String>, ResolverError> {
        let mut out = BTreeMap::new();
        if lines.is_empty() {
            return Ok(out);
        }
        // ONE parse and ONE query traversal for the whole file, over the bytes
        // the caller read the citations from — see
        // `SymbolResolver::resolve_symbols_at` for why the source is a
        // parameter rather than a path this reads for itself.
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .map_err(|e| ResolverError::Internal(format!("set_language: {}", e)))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ResolverError::Internal("parse returned None".into()))?;
        let root = tree.root_node();

        // tree-sitter rows are 0-indexed; callers pass 1-indexed line numbers
        // per the project convention (editor / grep alignment). Line 0 has no
        // row and is dropped rather than shifted onto line 1.
        let rows: Vec<(u32, usize)> = lines
            .iter()
            .filter(|l| **l > 0)
            .map(|l| (*l, (*l - 1) as usize))
            .collect();
        if rows.is_empty() {
            return Ok(out);
        }

        // Documented-symbol semantics first: a `§<id>` citation in source is a
        // doc comment, conventionally placed *above* the declaration it
        // documents. In C++ that comment is lexically a preceding sibling of
        // the declaration, so the smallest *enclosing* node is the outer scope
        // (namespace / class), not the documented symbol. If the cited line is
        // a comment that — with no intervening blank line — immediately
        // precedes a declaration, bind to that declaration. A file-header /
        // taxonomy comment not adjacent to a declaration falls through to the
        // enclosing scope (correctly coarse).
        let mut pending: Vec<(u32, usize)> = Vec::new();
        for (line, row) in rows {
            match documented_symbol(root, source, row) {
                Some(name) => {
                    out.insert(line, name);
                }
                None => pending.push((line, row)),
            }
        }
        if pending.is_empty() {
            return Ok(out);
        }

        let query = query()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root, source.as_bytes());
        // row -> (span of the smallest covering declaration, its name)
        let mut best: BTreeMap<usize, (usize, String)> = BTreeMap::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node: Node = cap.node;
                let start = node.start_position().row;
                let end = node.end_position().row;
                let covered: Vec<usize> = pending
                    .iter()
                    .map(|(_, row)| *row)
                    .filter(|row| *row >= start && *row <= end)
                    .collect();
                if covered.is_empty() {
                    continue;
                }
                let Some(name) = symbol_name(node, source.as_bytes()) else {
                    continue;
                };
                let span = end.saturating_sub(start);
                for row in covered {
                    match best.get(&row) {
                        Some((cur_span, _)) if span >= *cur_span => {}
                        _ => {
                            best.insert(row, (span, name.clone()));
                        }
                    }
                }
            }
        }
        for (line, row) in pending {
            if let Some((_, name)) = best.get(&row) {
                out.insert(line, name.clone());
            }
        }
        Ok(out)
    }
}

/// The declaration query, compiled ONCE for the process.
///
/// Captures the declaration node itself, not the name node: C++ names sit
/// several declarator levels deep, so extraction happens in [`symbol_name`].
/// `field_declaration` covers in-class member decls (variables and method
/// prototypes); function-body locals are `declaration` nodes, deliberately
/// excluded so a citation inside a body resolves to the enclosing function
/// rather than to a local variable.
///
/// It used to be compiled inside `resolve_symbol_at`, so a query compile
/// happened per citation alongside the per-citation parse — the half of the
/// cost the consumer's report did not name.
fn query() -> Result<&'static Query, ResolverError> {
    static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();
    QUERY
        .get_or_init(|| {
            let lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
            Query::new(
                &lang,
                r#"
        (function_definition) @item
        (field_declaration) @item
        (class_specifier) @item
        (struct_specifier) @item
        (union_specifier) @item
        (enum_specifier) @item
        (namespace_definition) @item
        "#,
            )
            .map_err(|e| format!("query compile: {e}"))
        })
        .as_ref()
        .map_err(|e| ResolverError::Internal(e.clone()))
}

/// Name nodes that terminate a declarator descent. `qualified_identifier`
/// returns its full source text (e.g. `Foo::bar`) so out-of-line
/// definitions resolve to the qualified form an author records.
const NAME_KINDS: &[&str] = &[
    "identifier",
    "field_identifier",
    "qualified_identifier",
    "destructor_name",
    "operator_name",
    "operator_cast",
    "type_identifier",
];

/// Declaration node kinds a doc comment may document. Mirrors the enclosing
/// query set minus `namespace_definition` — a comment is never "documenting"
/// the namespace it sits in, and `declaration` is excluded so a comment above
/// a function-body local binds to the enclosing function, not the local.
const DECL_KINDS: &[&str] = &[
    "function_definition",
    "field_declaration",
    "class_specifier",
    "struct_specifier",
    "union_specifier",
    "enum_specifier",
];

/// If `row` (0-indexed) falls on a comment that — through a contiguous run of
/// comment lines with no intervening blank line — immediately precedes a
/// declaration in [`DECL_KINDS`], return that declaration's symbol name. This
/// is the doc-comment convention: `// doc` above `class Foo {}` documents
/// `Foo`, even though `Foo` is not the comment's enclosing scope. A blank line
/// (or a non-declaration sibling) breaks the association and yields `None`,
/// so file-header comments fall through to enclosing-scope resolution.
fn documented_symbol(root: Node, source: &str, row: usize) -> Option<String> {
    let line = source.split('\n').nth(row)?;
    // Byte column of the first non-whitespace char; comments are ASCII-led
    // (`//` or `/*`), so byte == char offset here.
    let col = line.len() - line.trim_start().len();
    let pt = Point::new(row, col);
    let node = root.descendant_for_point_range(pt, pt)?;
    if node.kind() != "comment" {
        return None;
    }
    let mut last = node;
    let mut sib = node.next_named_sibling();
    while let Some(s) = sib {
        // Adjacency: the next sibling must begin on the line immediately after
        // the previous comment ends — a blank line breaks the doc association.
        if s.start_position().row != last.end_position().row + 1 {
            return None;
        }
        if s.kind() == "comment" {
            last = s;
            sib = s.next_named_sibling();
            continue;
        }
        if DECL_KINDS.contains(&s.kind()) {
            return symbol_name(s, source.as_bytes());
        }
        return None;
    }
    None
}

/// Extract the declared symbol name from a captured declaration node.
/// Type-like and namespace nodes read the `name` field directly; function
/// and field declarations descend their declarator. Returns `None` for
/// anonymous declarations (anonymous struct/union/namespace).
fn symbol_name(node: Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "class_specifier"
        | "struct_specifier"
        | "union_specifier"
        | "enum_specifier"
        | "namespace_definition" => node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
            .map(str::to_string),
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

/// Register this backend into the given `PluginRegistry`. The binary's
/// startup path (mnemosyne-cli) calls this once after instantiating the
/// registry; the substrate stays decoupled from any specific transport or
/// language.
pub fn register(registry: &mut PluginRegistry) {
    registry.register_symbol_resolver(BACKEND_KEY, Box::new(TreesitterCppResolver));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolve one line — through a path that DOES NOT EXIST, which is the
    /// oracle for "the answer came from the caller's bytes". Every test below
    /// therefore also asserts the resolver never reads the filesystem.
    fn resolve(source: &str, line: u32) -> Option<String> {
        let resolver = TreesitterCppResolver;
        resolver
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
        let batched = TreesitterCppResolver
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
        let out = TreesitterCppResolver
            .resolve_symbols_at(
                Path::new("/no/such/file.hpp"),
                "int alpha() { return 0; }\n",
                &[0, 1],
            )
            .unwrap();
        assert_eq!(out.get(&1).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0), "no answer for a line that cannot be");
    }
}
