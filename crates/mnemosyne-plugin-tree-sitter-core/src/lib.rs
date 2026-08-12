//! THE GRAMMAR-INDEPENDENT HALF OF A TREE-SITTER `SymbolResolver`.
//!
//! Every tree-sitter backend answers the same question the same way: parse the
//! caller's bytes ONCE, find the declarations, and for each requested line take
//! the SMALLEST declaration whose extent covers it. What differs between
//! languages is four things, and only four — the grammar, the query that names
//! its declaration nodes, how a name is extracted from one of them, and whether
//! a comment sitting immediately above a declaration counts as documenting it.
//!
//! Those four are a [`LanguageSpec`]. Everything else lives here once.
//!
//! WHY THIS EXISTS. Round 1151 published the gap SCE reported: the extension
//! table routes `.go` and `.py` to languages this build ships no resolver for,
//! and `.kt` is on no row at all. Closing it means three more backends. The two
//! that existed were 290 and 481 lines of which the overwhelming majority was
//! the identical walk, so three more copies would have been the fourth, fifth
//! and sixth transcription of one algorithm — and a defect found in one copy
//! would then have to be found five more times. The specs that replace them are
//! declarations: a query, a `name_of`, and a doc-comment kind list.
//!
//! NOTHING HERE READS THE FILESYSTEM. The bytes come from the caller, which is
//! what keeps the symbol answer about the same file revision the citation was
//! extracted from — a correctness property, not only a saving (Round 1141).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use mnemosyne_core::{ResolverError, SymbolResolver, VersionSurface};
use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};

/// The four things that differ between one tree-sitter backend and the next.
pub struct LanguageSpec {
    /// The `[plugins.symbol_resolver.<lang>] backend = "…"` value that selects
    /// this backend.
    pub backend_key: &'static str,
    /// The plugin crate's name, as reported in its [`VersionSurface`].
    pub plugin_name: &'static str,
    /// The plugin crate's version, as reported in its [`VersionSurface`].
    pub plugin_version: &'static str,
    /// The symbol-axis language this backend resolves — the only
    /// `[plugins.symbol_resolver.<lang>]` key it may be registered under.
    pub symbol_axis_language: &'static str,
    /// The grammar.
    pub language: fn() -> tree_sitter::Language,
    /// A query whose captures are DECLARATION nodes — the node whose EXTENT is
    /// the declaration's span, not the node holding its name. The engine picks
    /// the smallest covering capture, so the extent is what decides which
    /// declaration wins when they nest.
    pub query_source: &'static str,
    /// The declared name of a captured declaration node, or `None` for a shape
    /// this backend does not name (an anonymous struct; a form whose name node
    /// is not the kind the language's authors record).
    pub name_of: fn(Node, &[u8]) -> Option<String>,
    /// Declaration kinds a comment immediately above may be documenting.
    ///
    /// EMPTY DISABLES THE RULE, which is a per-language decision and not an
    /// oversight: it changes where a `§` citation written in a doc comment
    /// binds, and the two languages that shipped before this crate disagreed
    /// about it. See [`TreesitterResolver::resolve_symbols_at`].
    pub documented_kinds: &'static [&'static str],
    /// What this grammar calls a comment node. Unused when
    /// `documented_kinds` is empty.
    pub comment_kind: &'static str,
    /// Where this backend's compiled query lives for the life of the PROCESS.
    ///
    /// Owned by the language crate rather than by the resolver instance so the
    /// guarantee is the one Round 1141 measured and published — one
    /// `Query::new` per process, not one per run and not one per citation.
    pub query_cache: &'static OnceLock<Result<Query, String>>,
}

impl LanguageSpec {
    /// The compiled query, built at most once for the process.
    ///
    /// # Errors
    ///
    /// A query source the grammar rejects. It is a constant, so this is a build
    /// defect surfacing at first use rather than a runtime condition.
    pub fn query(&self) -> Result<&'static Query, ResolverError> {
        self.query_cache
            .get_or_init(|| {
                let lang = (self.language)();
                Query::new(&lang, self.query_source).map_err(|e| format!("query compile: {e}"))
            })
            .as_ref()
            .map_err(|e| ResolverError::Internal(e.clone()))
    }
}

/// A `SymbolResolver` built from one [`LanguageSpec`].
pub struct TreesitterResolver {
    spec: &'static LanguageSpec,
}

impl TreesitterResolver {
    #[must_use]
    pub const fn new(spec: &'static LanguageSpec) -> Self {
        Self { spec }
    }

    /// The spec this resolver answers from.
    #[must_use]
    pub const fn spec(&self) -> &'static LanguageSpec {
        self.spec
    }
}

impl SymbolResolver for TreesitterResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: self.spec.plugin_name.into(),
            plugin_version: self.spec.plugin_version.into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    /// `(source, lines) -> {line: symbol_name}`, one parse and one query
    /// traversal for the whole file.
    ///
    /// TWO PASSES, IN THIS ORDER, and only when the spec asks for the first.
    ///
    /// 1. DOCUMENTED-SYMBOL. A `§<id>` citation in source is conventionally a
    ///    doc comment placed ABOVE the declaration it documents. Lexically that
    ///    comment is a preceding sibling, so the smallest node ENCLOSING it is
    ///    the outer scope (a namespace, a class) rather than the thing being
    ///    documented. When the cited line is a comment that — through a
    ///    contiguous run of comment lines with no blank line between — is
    ///    immediately followed by a declaration in
    ///    [`LanguageSpec::documented_kinds`], the answer is that declaration.
    ///    A blank line or a non-declaration sibling breaks the association, so
    ///    a file-header comment falls through to pass 2 and resolves coarsely,
    ///    which is correct.
    ///
    /// 2. SMALLEST COVERING DECLARATION. For every line pass 1 did not answer,
    ///    the declaration with the smallest extent covering it.
    ///
    /// Best-effort by construction: macro-expanded code, generated files and
    /// code behind conditional-compilation gates resolve under their textual
    /// name rather than any post-expansion form.
    ///
    /// tree-sitter rows are 0-indexed and callers pass 1-indexed lines (the
    /// editor / grep convention). Line 0 has no row and is DROPPED rather than
    /// shifted onto line 1.
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
        let mut parser = Parser::new();
        parser
            .set_language(&(self.spec.language)())
            .map_err(|e| ResolverError::Internal(format!("set_language: {e}")))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ResolverError::Internal("parse returned None".into()))?;
        let root = tree.root_node();

        let rows: Vec<(u32, usize)> = lines
            .iter()
            .filter(|l| **l > 0)
            .map(|l| (*l, (*l - 1) as usize))
            .collect();
        if rows.is_empty() {
            return Ok(out);
        }

        let mut pending: Vec<(u32, usize)> = Vec::new();
        if self.spec.documented_kinds.is_empty() {
            pending = rows;
        } else {
            for (line, row) in rows {
                match self.documented_symbol(root, source, row) {
                    Some(name) => {
                        out.insert(line, name);
                    }
                    None => pending.push((line, row)),
                }
            }
            if pending.is_empty() {
                return Ok(out);
            }
        }

        let query = self.spec.query()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root, source.as_bytes());
        // row -> (extent of the smallest covering declaration, its name)
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
                let Some(name) = (self.spec.name_of)(node, source.as_bytes()) else {
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

impl TreesitterResolver {
    /// Pass 1 — see [`SymbolResolver::resolve_symbols_at`].
    fn documented_symbol(&self, root: Node, source: &str, row: usize) -> Option<String> {
        let line = source.split('\n').nth(row)?;
        // Byte column of the first non-whitespace character. Comment openers
        // are ASCII in every grammar this serves, so byte == char offset here.
        let col = line.len() - line.trim_start().len();
        let pt = Point::new(row, col);
        // CLIMB TO THE COMMENT, do not demand to land on it. The descendant at
        // a point is the SMALLEST node covering it, anonymous tokens included,
        // and grammars disagree about whether a comment is a leaf: C++'s
        // `comment` is one, so asking for the node's kind directly worked;
        // Rust's `line_comment` has children, so the same question answered
        // `//` and the rule never fired for any language whose comments carry
        // structure. That is invisible from a language crate — it looks like a
        // spec that chose not to have the rule — and every backend added after
        // this one would have inherited it.
        let node = comment_ancestor(
            root.descendant_for_point_range(pt, pt)?,
            self.spec.comment_kind,
        )?;
        let mut last = node;
        let mut sib = node.next_named_sibling();
        while let Some(s) = sib {
            // Adjacency: the next sibling must begin on the line immediately
            // after the previous comment ends — a blank line breaks the
            // association.
            if s.start_position().row != last.end_position().row + 1 {
                return None;
            }
            if s.kind() == self.spec.comment_kind {
                last = s;
                sib = s.next_named_sibling();
                continue;
            }
            if self.spec.documented_kinds.contains(&s.kind()) {
                return (self.spec.name_of)(s, source.as_bytes());
            }
            return None;
        }
        None
    }
}

/// `node` itself, or its nearest ancestor, whose kind is `comment_kind`.
///
/// `None` when the point is not inside a comment at all — the climb accepts
/// only an exact kind match, so it cannot mistake an enclosing item for one.
fn comment_ancestor<'tree>(node: Node<'tree>, comment_kind: &str) -> Option<Node<'tree>> {
    let mut cur = node;
    loop {
        if cur.kind() == comment_kind {
            return Some(cur);
        }
        cur = cur.parent()?;
    }
}

/// The text of `node`'s named field `field`, when that child exists and is one
/// of `kinds`. An empty `kinds` accepts any child kind.
///
/// The kind filter is what lets a spec say "this form is named, that one is
/// not" without writing a descent of its own: Rust records an `impl` block
/// under a plain type name and has no spelling for a generic one, so
/// `impl Foo<T>` resolves to the enclosing scope rather than to the text
/// `Foo<T>`, which no author records as an `Implementation.symbol`.
#[must_use]
pub fn field_text(node: Node, field: &str, kinds: &[&str], src: &[u8]) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    if !kinds.is_empty() && !kinds.contains(&child.kind()) {
        return None;
    }
    child.utf8_text(src).ok().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_of(node: Node, src: &[u8]) -> Option<String> {
        match node.kind() {
            "function_item" => field_text(node, "name", &["identifier"], src),
            "struct_item" => field_text(node, "name", &["type_identifier"], src),
            _ => None,
        }
    }

    const QUERY_SRC: &str = "(function_item) @item (struct_item) @item";

    static SILENT_CACHE: OnceLock<Result<Query, String>> = OnceLock::new();
    /// A spec with NO doc-comment rule.
    static SILENT: LanguageSpec = LanguageSpec {
        backend_key: "test-silent",
        plugin_name: "test-silent",
        plugin_version: "0.0.0",
        symbol_axis_language: "silent",
        language: || tree_sitter_rust::LANGUAGE.into(),
        query_source: QUERY_SRC,
        name_of,
        documented_kinds: &[],
        comment_kind: "line_comment",
        query_cache: &SILENT_CACHE,
    };

    static DOCUMENTING_CACHE: OnceLock<Result<Query, String>> = OnceLock::new();
    /// The same four things, with the doc-comment rule switched ON.
    static DOCUMENTING: LanguageSpec = LanguageSpec {
        backend_key: "test-documenting",
        plugin_name: "test-documenting",
        plugin_version: "0.0.0",
        symbol_axis_language: "documenting",
        language: || tree_sitter_rust::LANGUAGE.into(),
        query_source: QUERY_SRC,
        name_of,
        documented_kinds: &["function_item", "struct_item"],
        comment_kind: "line_comment",
        query_cache: &DOCUMENTING_CACHE,
    };

    fn at(spec: &'static LanguageSpec, source: &str, line: u32) -> Option<String> {
        TreesitterResolver::new(spec)
            .resolve_symbols_at(Path::new("/no/such/file"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    /// THE SWITCH IS REAL, AND IT IS THE ONLY DIFFERENCE. Two specs over one
    /// grammar, one query and one `name_of`, differing in nothing but
    /// `documented_kinds` — and the same source line resolves two different
    /// ways. That asymmetry is not hypothetical: it is exactly how the shipped
    /// Rust and C++ backends differ, so an engine that quietly applied pass 1
    /// to everything would have changed one of them without any language test
    /// noticing.
    #[test]
    fn an_empty_documented_kinds_disables_the_doc_comment_pass() {
        let src = "// documents alpha\nfn alpha() {}\n";
        assert_eq!(at(&SILENT, src, 1), None);
        assert_eq!(at(&DOCUMENTING, src, 1).as_deref(), Some("alpha"));
        // …and pass 2 answers identically for both, so the difference above is
        // the rule and not the walk.
        assert_eq!(at(&SILENT, src, 2).as_deref(), Some("alpha"));
        assert_eq!(at(&DOCUMENTING, src, 2).as_deref(), Some("alpha"));
    }

    /// A blank line breaks the association even with the rule on, so a file
    /// header does not become the name of whatever follows it.
    #[test]
    fn a_blank_line_breaks_the_doc_association() {
        let src = "// a file header\n\nfn alpha() {}\n";
        assert_eq!(at(&DOCUMENTING, src, 1), None);
    }

    /// The smallest covering declaration wins when declarations nest.
    #[test]
    fn the_smallest_covering_declaration_wins() {
        let src = "struct Outer {\n    f: u32,\n}\nfn inner() {\n    let _ = 1;\n}\n";
        assert_eq!(at(&SILENT, src, 2).as_deref(), Some("Outer"));
        assert_eq!(at(&SILENT, src, 5).as_deref(), Some("inner"));
    }

    /// Line 0 has no row: dropped, never shifted onto line 1. An empty request
    /// answers empty without parsing at all.
    #[test]
    fn line_zero_is_dropped_and_no_lines_is_no_work() {
        let r = TreesitterResolver::new(&SILENT);
        let out = r
            .resolve_symbols_at(Path::new("/no/such/file"), "fn alpha() {}\n", &[0, 1])
            .unwrap();
        assert_eq!(out.get(&1).map(String::as_str), Some("alpha"));
        assert!(!out.contains_key(&0));
        assert!(r
            .resolve_symbols_at(Path::new("/no/such/file"), "fn alpha() {}\n", &[])
            .unwrap()
            .is_empty());
    }

    /// ONE `Query::new` PER PROCESS, which is the property Round 1141 measured
    /// and published — not one per run, and not the one per citation it was
    /// before. Pointer identity is the only thing that can say so: a cache
    /// rebuilt per call would return equal queries at different addresses.
    #[test]
    fn the_query_is_compiled_once_for_the_process() {
        let first = SILENT.query().expect("compiles");
        let second = SILENT.query().expect("compiles");
        assert!(std::ptr::eq(first, second));
    }

    /// The kind filter is what lets a spec decline to name a form its language
    /// has no spelling for, without writing a descent of its own.
    #[test]
    fn the_kind_filter_declines_a_child_of_the_wrong_kind() {
        // `impl Foo<T>` has a `generic_type` where `impl Foo` has a
        // `type_identifier`; asking for the latter must answer nothing.
        let src = "impl Foo<T> {}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(src, None).unwrap();
        let node = tree.root_node().named_child(0).unwrap();
        assert_eq!(node.kind(), "impl_item");
        assert_eq!(
            field_text(node, "type", &["type_identifier"], src.as_bytes()),
            None
        );
        assert_eq!(
            field_text(node, "type", &[], src.as_bytes()).as_deref(),
            Some("Foo<T>"),
            "with no filter the same field answers, so the filter is what refused"
        );
    }
}
