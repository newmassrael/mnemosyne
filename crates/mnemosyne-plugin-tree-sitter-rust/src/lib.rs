//! Tree-sitter Rust `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! Answers `(file, line) -> Option<symbol_name>` by parsing the file with
//! `tree-sitter-rust` and walking the tree for the smallest declarative
//! node whose extent covers the requested line. Best-effort — macro-
//! expanded code, generated files, and items inside `cfg_attr` gates may
//! resolve under their textual name rather than the post-expansion form.
//!
//! Registered into a `PluginRegistry` via [`register`] from the binary's
//! startup path (mnemosyne-cli / mnemosyne-mcp).

use std::fs;
use std::path::Path;

use mnemosyne_plugin::{
    PluginRegistry, ResolverError, SymbolResolver, VersionSurface,
};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub const BACKEND_KEY: &str = "tree-sitter-rust";

pub struct TreesitterRustResolver;

impl SymbolResolver for TreesitterRustResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: "mnemosyne-plugin-tree-sitter-rust".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbol_at(
        &self,
        file: &Path,
        line: u32,
    ) -> Result<Option<String>, ResolverError> {
        let source = fs::read_to_string(file).map_err(|e| {
            ResolverError::Internal(format!("read `{}`: {}", file.display(), e))
        })?;
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
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

        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let query_src = r#"
        (function_item name: (identifier) @sym)
        (struct_item name: (type_identifier) @sym)
        (enum_item name: (type_identifier) @sym)
        (trait_item name: (type_identifier) @sym)
        (impl_item type: (type_identifier) @sym)
        (mod_item name: (identifier) @sym)
        (const_item name: (identifier) @sym)
        (static_item name: (identifier) @sym)
        (type_item name: (type_identifier) @sym)
        (union_item name: (type_identifier) @sym)
        (macro_definition name: (identifier) @sym)
        "#;
        let query = Query::new(&lang, query_src).map_err(|e| {
            ResolverError::Internal(format!("query compile: {}", e))
        })?;

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root, source.as_bytes());
        let mut best: Option<(usize, String)> = None;

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let item_node: Node = item_node_for_capture(cap.node);
                let start = item_node.start_position().row;
                let end = item_node.end_position().row;
                if row < start || row > end {
                    continue;
                }
                let span = end.saturating_sub(start);
                let name = cap
                    .node
                    .utf8_text(source.as_bytes())
                    .map_err(|e| ResolverError::Internal(format!("utf8: {}", e)))?
                    .to_string();
                best = match best {
                    Some((cur_span, _)) if span >= cur_span => best,
                    _ => Some((span, name)),
                };
            }
        }
        Ok(best.map(|(_, n)| n))
    }
}

/// Walks up from the captured name node to the enclosing item node so the
/// extent reflects the declaration span (used to pick the *smallest*
/// covering declaration when items nest — e.g., a `fn` inside an `impl`).
fn item_node_for_capture(name_node: Node) -> Node {
    let mut cur = name_node;
    while let Some(parent) = cur.parent() {
        let kind = parent.kind();
        if matches!(
            kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "mod_item"
                | "const_item"
                | "static_item"
                | "type_item"
                | "union_item"
                | "macro_definition"
        ) {
            return parent;
        }
        cur = parent;
    }
    name_node
}

/// Register this backend into the given `PluginRegistry`. The binary's
/// startup path (mnemosyne-cli / mnemosyne-mcp) calls this once after
/// instantiating the registry; the substrate stays decoupled from any
/// specific transport or language.
pub fn register(registry: &mut PluginRegistry) {
    registry.register_symbol_resolver(BACKEND_KEY, Box::new(TreesitterRustResolver));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn resolve(source: &str, line: u32) -> Option<String> {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(source.as_bytes()).unwrap();
        let resolver = TreesitterRustResolver;
        resolver.resolve_symbol_at(tmp.path(), line).unwrap()
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

    #[test]
    fn register_round_trip() {
        let mut reg = PluginRegistry::new();
        register(&mut reg);
        assert!(reg.symbol_resolver(BACKEND_KEY).is_some());
    }
}
