//! Tree-sitter C++ `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! Answers `(file, line) -> Option<symbol_name>` by parsing the file with
//! `tree-sitter-cpp` and walking the tree for the smallest declarative
//! node whose extent covers the requested line. Best-effort — macro-
//! expanded code, generated files, and code behind preprocessor gates may
//! resolve under their textual name rather than the post-expansion form.
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

use std::fs;
use std::path::Path;

use mnemosyne_core::{PluginRegistry, ResolverError, SymbolResolver, VersionSurface};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub const BACKEND_KEY: &str = "tree-sitter-cpp";

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

    fn resolve_symbol_at(&self, file: &Path, line: u32) -> Result<Option<String>, ResolverError> {
        let source = fs::read_to_string(file)
            .map_err(|e| ResolverError::Internal(format!("read `{}`: {}", file.display(), e)))?;
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .map_err(|e| ResolverError::Internal(format!("set_language: {}", e)))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| ResolverError::Internal("parse returned None".into()))?;
        let root = tree.root_node();

        // tree-sitter rows are 0-indexed; callers pass 1-indexed line numbers
        // per the project convention (editor / grep alignment).
        if line == 0 {
            return Ok(None);
        }
        let row = (line - 1) as usize;

        let lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
        // Captures the declaration node itself, not the name node: C++ names
        // sit several declarator levels deep, so extraction happens in
        // `symbol_name`. `field_declaration` covers in-class member decls
        // (variables and method prototypes); function-body locals are
        // `declaration` nodes, deliberately excluded so a citation inside a
        // body resolves to the enclosing function, not a local variable.
        let query_src = r#"
        (function_definition) @item
        (field_declaration) @item
        (class_specifier) @item
        (struct_specifier) @item
        (union_specifier) @item
        (enum_specifier) @item
        (namespace_definition) @item
        "#;
        let query = Query::new(&lang, query_src)
            .map_err(|e| ResolverError::Internal(format!("query compile: {}", e)))?;

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root, source.as_bytes());
        let mut best: Option<(usize, String)> = None;

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node: Node = cap.node;
                let start = node.start_position().row;
                let end = node.end_position().row;
                if row < start || row > end {
                    continue;
                }
                let Some(name) = symbol_name(node, source.as_bytes()) else {
                    continue;
                };
                let span = end.saturating_sub(start);
                best = match best {
                    Some((cur_span, _)) if span >= cur_span => best,
                    _ => Some((span, name)),
                };
            }
        }
        Ok(best.map(|(_, n)| n))
    }
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn resolve(source: &str, line: u32) -> Option<String> {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(source.as_bytes()).unwrap();
        let resolver = TreesitterCppResolver;
        resolver.resolve_symbol_at(tmp.path(), line).unwrap()
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
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }
}
