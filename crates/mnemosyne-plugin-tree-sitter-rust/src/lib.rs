//! Tree-sitter Rust `SymbolResolver` backend for the Mnemosyne plugin
//! substrate.
//!
//! Answers `(source, lines) -> {line: symbol_name}` by parsing the caller's
//! text ONCE with `tree-sitter-rust` and walking the tree for the smallest
//! declarative node whose extent covers each requested line. Best-effort —
//! macro-expanded code, generated files, and items inside `cfg_attr` gates may
//! resolve under their textual name rather than the post-expansion form.
//!
//! Nothing here reads the filesystem: the bytes come from the caller, which is
//! what keeps the symbol answer about the same file revision the citation was
//! extracted from.
//!
//! Registered into a `PluginRegistry` via [`register`] from the binary's
//! startup path (mnemosyne-cli / mnemosyne-mcp).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use mnemosyne_core::{PluginRegistry, ResolverError, SymbolResolver, VersionSurface};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

pub const BACKEND_KEY: &str = "tree-sitter-rust";

/// The symbol-axis language this backend resolves — the
/// `[plugins.symbol_resolver.<lang>]` key it belongs under.
///
/// Declared here rather than at the wiring site because it is a property of
/// what this crate parses: `tree-sitter-rust` answers in Rust's vocabulary and
/// in no other, so pairing it with a different language key produces
/// enforcement against names a grammar that never saw the language invented.
pub const SYMBOL_AXIS_LANGUAGE: &str = "rust";

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
        // ONE parse and ONE query traversal for the whole file. The caller
        // hands over the bytes it read the citations from, so nothing here
        // touches the disk — see `SymbolResolver::resolve_symbols_at` for why
        // that is a correctness property and not only a saving.
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
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

        let query = query()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root, source.as_bytes());
        // row -> (span of the smallest covering declaration, its name)
        let mut best: BTreeMap<usize, (usize, String)> = BTreeMap::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let item_node: Node = item_node_for_capture(cap.node);
                let start = item_node.start_position().row;
                let end = item_node.end_position().row;
                let covered: Vec<usize> = rows
                    .iter()
                    .map(|(_, row)| *row)
                    .filter(|row| *row >= start && *row <= end)
                    .collect();
                if covered.is_empty() {
                    continue;
                }
                let span = end.saturating_sub(start);
                let name = cap
                    .node
                    .utf8_text(source.as_bytes())
                    .map_err(|e| ResolverError::Internal(format!("utf8: {}", e)))?
                    .to_string();
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
        for (line, row) in rows {
            if let Some((_, name)) = best.get(&row) {
                out.insert(line, name.clone());
            }
        }
        Ok(out)
    }
}

/// The declaration query, compiled ONCE for the process.
///
/// It used to be compiled inside `resolve_symbol_at`, so a tree-sitter query
/// compile happened per citation alongside the per-citation parse — the half of
/// the cost the consumer's report did not name. The source is a constant, so
/// there is nothing per-call about it.
fn query() -> Result<&'static Query, ResolverError> {
    static QUERY: OnceLock<Result<Query, String>> = OnceLock::new();
    QUERY
        .get_or_init(|| {
            let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            Query::new(
                &lang,
                r#"
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
        "#,
            )
            .map_err(|e| format!("query compile: {e}"))
        })
        .as_ref()
        .map_err(|e| ResolverError::Internal(e.clone()))
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

    /// Resolve one line — through a path that DOES NOT EXIST, which is the
    /// oracle for "the answer came from the caller's bytes". Every test below
    /// therefore also asserts the resolver never reads the filesystem.
    fn resolve(source: &str, line: u32) -> Option<String> {
        let resolver = TreesitterRustResolver;
        resolver
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
        let batched = TreesitterRustResolver
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
        let out = TreesitterRustResolver
            .resolve_symbols_at(Path::new("/no/such/file.rs"), "fn alpha() {}\n", &[0, 1])
            .unwrap();
        assert_eq!(out.get(&1).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0), "no answer for a line that cannot be");
    }
}
