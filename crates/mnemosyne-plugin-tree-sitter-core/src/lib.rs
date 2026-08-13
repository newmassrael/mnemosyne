//! THE GRAMMAR-INDEPENDENT HALF OF A TREE-SITTER `SymbolResolver`.
//!
//! Every tree-sitter backend answers the same question the same way: parse the
//! caller's bytes ONCE, find the declarations, and for each requested line take
//! the SMALLEST declaration whose extent covers it. What differs between
//! languages is four things, and only four — the grammar, the query that names
//! its declaration nodes, how a name is extracted from one of them, and where a
//! comment sitting immediately above a declaration binds.
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
//! declarations: a query, a `name_of`, and a [`DocCommentRule`].
//!
//! NOTHING HERE READS THE FILESYSTEM. The bytes come from the caller, which is
//! what keeps the symbol answer about the same file revision the citation was
//! extracted from — a correctness property, not only a saving (Round 1141).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use mnemosyne_core::{ResolverError, SymbolResolver, VersionSurface};
use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

/// WHERE A CITATION WRITTEN IN A COMMENT BINDS — the doc-comment half of a
/// [`LanguageSpec`], and the criterion a new backend answers.
///
/// A `§` citation in source is conventionally written in a comment placed
/// immediately above the declaration it is about. Lexically that comment is a
/// PRECEDING SIBLING, so the smallest node enclosing it is the outer scope — a
/// class, a module, or nothing at all at the top level. Without a rule for it,
/// an author who recorded the declaration's own name as its
/// `Implementation.symbol` gets a `symbol_mismatch` against a name nobody
/// wrote.
///
/// THE RULE IS NOT OPTIONAL, AND UNTIL ROUND 1162 IT WAS. What this struct
/// replaces was a bare kind list whose EMPTY value switched the pass off, so a
/// backend declined the rule by writing `&[]` and nothing asked why. Four of
/// the five shipped backends switched it on and the fifth off — each
/// defensibly, none comparably, and the fifth's own doc said "which one is
/// right is a question with an answer" and left it there. The three questions
/// below are that answer, and each is answered with DATA that a corpus law
/// witnesses: a backend that claims a kind it cannot bind is red.
///
/// 1. WHAT DOES THIS GRAMMAR CALL A COMMENT? — `comment_kinds`, ALL of the
///    spellings, because a citation may sit in any of them. A single spelling
///    was this wire's own limit once: C++, Go and Python each call every
///    comment `comment`, so one string carried the whole answer and nothing
///    said otherwise, while Kotlin calls `//` a `line_comment` and KDoc a
///    `block_comment` and the field could not have held both.
///
/// 2. WHICH OF THOSE SPELLINGS DOCUMENT THE SCOPE THEY ARE INSIDE? —
///    `inward_markers`. A language may spell "documents my container" the same
///    way it spells "documents what follows": Rust's `//!` and `///` are BOTH
///    `line_comment`, as are `/*! */` and `/** */` as `block_comment`. Binding
///    a `//!` citation to the item under it would be wrong — that citation is
///    about the module — so a language with such a spelling must be able to
///    say which node marks it. Rust's grammar does mark it
///    (`inner_doc_comment_marker`), and naming that is what lets the rule serve
///    the language instead of being switched off for it.
///
/// 3. WHICH DECLARATIONS MAY A COMMENT ABOVE BE DOCUMENTING? —
///    `documented_kinds`. Containers a comment merely sits inside are excluded,
///    and so are function-body locals, so a citation inside a body binds to the
///    function. Every kind listed must be one [`LanguageSpec::name_of`] can
///    name — a kind it cannot is a claim with no answer behind it.
///
/// THE QUESTION WITH NO FIELD, which is the one an empty list was hiding: is
/// there a spelling that means something ELSE in the SAME POSITION and that the
/// grammar does not mark? If there is, the answer is to extend this wire, not
/// to switch the rule off silently — an off switch and an unserved language
/// look identical from outside, which is how Rust's stayed for five rounds.
/// C++'s Doxygen `///<` is the near case: it documents the PRECEDING member and
/// the grammar makes it a plain `comment`. It is not in the same position — the
/// rule starts from the first non-whitespace character of the cited row, and on
/// a trailing comment's row that character is code, so the pass declines and
/// the citation binds to the declaration it trails. Measured over the C++
/// corpus this repository is put to: 78 trailing-doc comments, 0 of them
/// starting their own line.
pub struct DocCommentRule {
    /// Question 1 — every node kind this grammar calls a comment.
    pub comment_kinds: &'static [&'static str],
    /// Question 2 — child node kinds marking a comment as documenting the
    /// scope it is INSIDE. Empty asserts the language has no such spelling.
    pub inward_markers: &'static [&'static str],
    /// Question 3 — declaration kinds a comment immediately above may be
    /// documenting.
    pub documented_kinds: &'static [&'static str],
}

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
    /// Where a citation written in a comment binds — see [`DocCommentRule`].
    pub doc_comments: DocCommentRule,
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

    /// This grammar's parse of a source it was handed.
    ///
    /// ONE PLACE, AND THE SWEEPS GATE IS WHY IT IS ONE. Round 1161 added a
    /// second caller and copied these four lines to do it, which took the
    /// `the-resolver-goes-back-to-the-disk` injection from landing once to
    /// landing twice — the gate refused, and the repair is the duplication
    /// rather than the anchor. A parse is also exactly the thing that must not
    /// have two implementations here: the whole point of the source being passed
    /// in is that the answer comes from the bytes the citation was taken from,
    /// and a second parse is a second chance to read something else.
    ///
    /// # Errors
    ///
    /// A grammar the parser rejects, or a parse that returns nothing.
    pub fn parse(&self, source: &str) -> Result<Tree, ResolverError> {
        let mut parser = Parser::new();
        parser
            .set_language(&(self.language)())
            .map_err(|e| ResolverError::Internal(format!("set_language: {e}")))?;
        parser
            .parse(source, None)
            .ok_or_else(|| ResolverError::Internal("parse returned None".into()))
    }

    /// How many declaration patterns this backend's query declares.
    ///
    /// THE DENOMINATOR OF WHAT A CORPUS COVERS, and it comes from the compiled
    /// query rather than from anybody's list. Round 1161 needed a population for
    /// "which shapes does this backend CLAIM to answer", and the query source is
    /// where that claim is already written — counting it here is asking the
    /// program, where a constant beside it would be a second answer free to
    /// drift.
    ///
    /// # Errors
    ///
    /// See [`LanguageSpec::query`].
    pub fn pattern_count(&self) -> Result<usize, ResolverError> {
        Ok(self.query()?.pattern_count())
    }

    /// Which of this backend's query patterns a source actually exercises.
    ///
    /// THE NUMERATOR, TAKEN THE SAME WAY. `tree_sitter` hands every match its
    /// `pattern_index`, so "did the corpus reach this shape" is a fact the
    /// matcher already knows and nothing here has to infer from node kinds or
    /// from reading the query text.
    ///
    /// WHY IT IS NOT ENOUGH TO COUNT ANSWERS. A resolver returns NAMES, and two
    /// patterns can produce the same name while a third produces none at all
    /// ([`LanguageSpec::name_of`] returns `None` for shapes a backend declines
    /// to name). A corpus that never reaches a pattern and a corpus that reaches
    /// one it cannot name are different states, and the answer map cannot tell
    /// them apart — this can.
    ///
    /// # Errors
    ///
    /// See [`LanguageSpec::query`]; also a grammar the parser rejects.
    pub fn patterns_exercised(&self, source: &str) -> Result<BTreeSet<usize>, ResolverError> {
        let tree = self.parse(source)?;
        let query = self.query()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        let mut seen = BTreeSet::new();
        while let Some(m) = matches.next() {
            seen.insert(m.pattern_index);
        }
        Ok(seen)
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
    /// TWO PASSES, IN THIS ORDER.
    ///
    /// 1. DOCUMENTED-SYMBOL. A `§<id>` citation in source is conventionally a
    ///    doc comment placed ABOVE the declaration it documents. Lexically that
    ///    comment is a preceding sibling, so the smallest node ENCLOSING it is
    ///    the outer scope (a namespace, a class) rather than the thing being
    ///    documented. When the cited line is a comment that — through a
    ///    contiguous run of comment lines with no blank line between — is
    ///    immediately followed by a declaration in
    ///    [`DocCommentRule::documented_kinds`], the answer is that declaration.
    ///    A blank line or a non-declaration sibling breaks the association, so
    ///    a file-header comment falls through to pass 2 and resolves coarsely,
    ///    which is correct. So does a comment the language marks as documenting
    ///    the scope it is inside ([`DocCommentRule::inward_markers`]) — Rust's
    ///    `//!` is about the module, and the module is what pass 2 answers.
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
        let tree = self.spec.parse(source)?;
        let root = tree.root_node();

        let rows: Vec<(u32, usize)> = lines
            .iter()
            .filter(|l| **l > 0)
            .map(|l| (*l, (*l - 1) as usize))
            .collect();
        if rows.is_empty() {
            return Ok(out);
        }

        // THE SOURCE'S LINES, SPLIT ONCE FOR THE WHOLE CALL. Pass 1 needs the
        // text of two rows and used to reach them with `split('\n').nth(row)`,
        // which walks the file from the start — once per requested line, so a
        // whole-file request was quadratic in the file's length. Nothing saw it
        // while the one language whose citations are all module-level was also
        // the one language with the pass switched off; putting this repository
        // itself under the real-tree pass is what priced it.
        let text: Vec<&str> = source.split('\n').collect();

        let mut pending: Vec<(u32, usize)> = Vec::new();
        for (line, row) in rows {
            match self.documented_symbol(root, source, &text, row) {
                Some(name) => {
                    out.insert(line, name);
                }
                None => pending.push((line, row)),
            }
        }
        if pending.is_empty() {
            return Ok(out);
        }

        let query = self.spec.query()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root, source.as_bytes());
        // row -> (extent of the smallest covering declaration, its name)
        let mut best: BTreeMap<usize, (usize, String)> = BTreeMap::new();
        // THE PENDING ROWS, SORTED, SO A CAPTURE FINDS THE ONES IT COVERS BY
        // BINARY SEARCH. Scanning every pending row per capture is quadratic in
        // a whole-file request, which is the request the real-tree pass makes of
        // every file it reads and the one a `--filter-id`-less run makes of a
        // large source. The set is what matters here, not the caller's order.
        let mut sorted: Vec<usize> = pending.iter().map(|(_, row)| *row).collect();
        sorted.sort_unstable();
        sorted.dedup();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node: Node = cap.node;
                let start = node.start_position().row;
                let end = node.end_position().row;
                let lo = sorted.partition_point(|row| *row < start);
                let hi = sorted.partition_point(|row| *row <= end);
                if lo == hi {
                    continue;
                }
                let Some(name) = (self.spec.name_of)(node, source.as_bytes()) else {
                    continue;
                };
                let span = end.saturating_sub(start);
                for row in &sorted[lo..hi] {
                    match best.get(row) {
                        Some((cur_span, _)) if span >= *cur_span => {}
                        _ => {
                            best.insert(*row, (span, name.clone()));
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
    fn documented_symbol(
        &self,
        root: Node,
        source: &str,
        text: &[&str],
        row: usize,
    ) -> Option<String> {
        let rule = &self.spec.doc_comments;
        let line = *text.get(row)?;
        // Byte column of the first non-whitespace character. Comment openers
        // are ASCII in every grammar this serves, so byte == char offset here.
        // IT IS ALSO WHAT KEEPS A TRAILING COMMENT OUT OF THIS PASS: on
        // `int x; ///< the x` that character is code, so the climb below finds
        // no comment and the citation binds to the declaration it trails.
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
        let node = comment_ancestor(root.descendant_for_point_range(pt, pt)?, rule.comment_kinds)?;
        // A COMMENT ABOUT ITS CONTAINER IS NOT ABOUT WHAT FOLLOWS IT. Rust
        // spells both with one node kind and marks which is which, so the
        // question is asked of the CITED comment rather than of the run: a
        // `//!` line declines here and falls through to pass 2, which answers
        // the enclosing module — the thing that comment is documenting.
        if documents_inward(node, rule.inward_markers) {
            return None;
        }
        // The contiguous run of comment lines this one belongs to. Consecutive
        // comments ARE siblings in every grammar here, so the tree is the right
        // instrument for this half.
        let mut last = node;
        let mut sib = node.next_named_sibling();
        while let Some(s) = sib {
            if !rule.comment_kinds.contains(&s.kind())
                || s.start_position().row != last_text_row(last) + 1
            {
                break;
            }
            last = s;
            sib = s.next_named_sibling();
        }

        // ASK THE ROW, NOT THE SIBLING, for what the run documents. A sibling
        // walk assumes the declaration sits beside the comment, and grammars
        // put a container between them: inside a Python class the body `block`
        // begins at the first STATEMENT, so a leading comment is a child of the
        // `class_definition` and its next sibling is the block — the rule read
        // "not a declaration" and bound the whole method to the class. The row
        // after the run is where the documented thing must BEGIN, whatever the
        // shape of the tree above it.
        let next_row = last_text_row(last) + 1;
        let next_line = *text.get(next_row)?;
        // A blank line breaks the association, so a file header does not become
        // the name of whatever follows it.
        if next_line.trim().is_empty() {
            return None;
        }
        let next_col = next_line.len() - next_line.trim_start().len();
        let start = Point::new(next_row, next_col);
        let mut cur = root.descendant_for_point_range(start, start)?;
        // THE OUTERMOST DECLARATION THAT STILL BEGINS ON THIS ROW, not the
        // innermost. Measured over 43095 comment lines of a real C++ corpus:
        // taking the innermost moved 151 answers, and the ones it moved were
        // wrong. `// doc` above `struct mobj_s *snext;` reaches the elaborated
        // TYPE `struct mobj_s` before it reaches the field, so the comment
        // documenting a field bound to the struct being pointed at; the same
        // shape put a class's leading macro in place of the class name.
        //
        // Leaving this row is the stop condition, and it is what keeps a
        // comment above a statement inside a function from climbing out to the
        // function: nothing on that row is a declaration, and the enclosing one
        // began earlier.
        let mut found: Option<Node> = None;
        loop {
            if cur.start_position().row != next_row {
                break;
            }
            if rule.documented_kinds.contains(&cur.kind()) {
                found = Some(cur);
            }
            match cur.parent() {
                Some(parent) => cur = parent,
                None => break,
            }
        }
        (self.spec.name_of)(found?, source.as_bytes())
    }
}

/// The last row this node has text on.
///
/// NOT `end_position().row`, AND ONE GRAMMAR IS ENOUGH TO SHOW WHY. A node
/// whose extent ends at column 0 has swallowed the newline that ends the row
/// before, so its `end_position().row` is a row it holds nothing on.
/// tree-sitter-rust does this for DOC comments (`///`, `//!`) and not for
/// ordinary ones, and the difference is invisible until a language turns the
/// doc-comment pass on: with the raw end row, the pass looked one row past the
/// declaration for exactly the comments the rule exists to serve, and answered
/// nothing. A rule that silently never fires is indistinguishable from a
/// language that declined it — which is the state Round 1162 found Rust in.
fn last_text_row(node: Node) -> usize {
    let end = node.end_position();
    if end.column == 0 {
        end.row.saturating_sub(1)
    } else {
        end.row
    }
}

/// Whether the language marks this comment as documenting the scope it is
/// INSIDE — see [`DocCommentRule::inward_markers`].
///
/// The marker is a CHILD of the comment, not the comment's own kind, because
/// the grammar that has this distinction records the two spellings under one
/// kind and separates them by what they open with.
fn documents_inward(comment: Node, markers: &[&str]) -> bool {
    (0..u32::try_from(comment.named_child_count()).unwrap_or(u32::MAX))
        .filter_map(|i| comment.named_child(i))
        .any(|child| markers.contains(&child.kind()))
}

/// `node` itself, or its nearest ancestor, whose kind is one of
/// `comment_kinds`.
///
/// `None` when the point is not inside a comment at all — the climb accepts
/// only an exact kind match, so it cannot mistake an enclosing item for one.
fn comment_ancestor<'tree>(node: Node<'tree>, comment_kinds: &[&str]) -> Option<Node<'tree>> {
    let mut cur = node;
    loop {
        if comment_kinds.contains(&cur.kind()) {
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

    const DOCUMENTED_KINDS: &[&str] = &["function_item", "struct_item"];

    static UNMARKED_CACHE: OnceLock<Result<Query, String>> = OnceLock::new();
    /// A spec that names NO inward marker — the answer four of the five shipped
    /// languages give, and the wrong answer for a grammar that has one.
    static UNMARKED: LanguageSpec = LanguageSpec {
        backend_key: "test-unmarked",
        plugin_name: "test-unmarked",
        plugin_version: "0.0.0",
        symbol_axis_language: "unmarked",
        language: || tree_sitter_rust::LANGUAGE.into(),
        query_source: QUERY_SRC,
        name_of,
        doc_comments: DocCommentRule {
            comment_kinds: &["line_comment"],
            inward_markers: &[],
            documented_kinds: DOCUMENTED_KINDS,
        },
        query_cache: &UNMARKED_CACHE,
    };

    static MARKED_CACHE: OnceLock<Result<Query, String>> = OnceLock::new();
    /// The same four things, with the grammar's inward marker named.
    static MARKED: LanguageSpec = LanguageSpec {
        backend_key: "test-marked",
        plugin_name: "test-marked",
        plugin_version: "0.0.0",
        symbol_axis_language: "marked",
        language: || tree_sitter_rust::LANGUAGE.into(),
        query_source: QUERY_SRC,
        name_of,
        doc_comments: DocCommentRule {
            comment_kinds: &["line_comment"],
            inward_markers: &["inner_doc_comment_marker"],
            documented_kinds: DOCUMENTED_KINDS,
        },
        query_cache: &MARKED_CACHE,
    };

    fn at(spec: &'static LanguageSpec, source: &str, line: u32) -> Option<String> {
        TreesitterResolver::new(spec)
            .resolve_symbols_at(Path::new("/no/such/file"), source, &[line])
            .unwrap()
            .remove(&line)
    }

    /// THE MARKER IS THE ONLY DIFFERENCE, AND IT DECIDES. Two specs over one
    /// grammar, one query and one `name_of`, differing in nothing but
    /// `inward_markers` — and a `//!` line resolves two different ways while an
    /// ordinary comment resolves the same way for both. Without the marker the
    /// wire cannot tell "documents my module" from "documents the item below",
    /// which is the state Rust's backend was in when it answered by switching
    /// the whole pass off.
    #[test]
    fn an_inward_marker_is_what_keeps_a_module_comment_off_the_item_below() {
        let src = "//! documents the module\nfn alpha() {}\n";
        assert_eq!(at(&UNMARKED, src, 1).as_deref(), Some("alpha"));
        assert_eq!(at(&MARKED, src, 1), None);
        // An OUTER doc comment is not marked inward, so both specs bind it to
        // the item below: the difference above is the marker and not the pass.
        let outer = "/// documents alpha\nfn alpha() {}\n";
        assert_eq!(at(&UNMARKED, outer, 1).as_deref(), Some("alpha"));
        assert_eq!(at(&MARKED, outer, 1).as_deref(), Some("alpha"));
        // …and pass 2 answers identically for both in either source, so nothing
        // else moved.
        assert_eq!(at(&UNMARKED, src, 2).as_deref(), Some("alpha"));
        assert_eq!(at(&MARKED, src, 2).as_deref(), Some("alpha"));
    }

    /// A blank line breaks the association, so a file header does not become
    /// the name of whatever follows it.
    #[test]
    fn a_blank_line_breaks_the_doc_association() {
        let src = "// a file header\n\nfn alpha() {}\n";
        assert_eq!(at(&MARKED, src, 1), None);
    }

    /// A TRAILING COMMENT IS NOT IN THE POSITION THE RULE IS ABOUT. The pass
    /// starts from the first non-whitespace character of the cited row, and on
    /// a trailing comment's row that character is code — so the citation binds
    /// to the declaration it trails rather than to the next one. This is what
    /// lets a language keep the rule while spelling "documents the PRECEDING
    /// member" with the same comment kind, which is C++'s Doxygen `///<`.
    #[test]
    fn a_trailing_comment_binds_to_the_declaration_it_trails() {
        let src = "fn alpha() {} // about alpha\nfn beta() {}\n";
        assert_eq!(at(&MARKED, src, 1).as_deref(), Some("alpha"));
    }

    /// The smallest covering declaration wins when declarations nest.
    #[test]
    fn the_smallest_covering_declaration_wins() {
        let src = "struct Outer {\n    f: u32,\n}\nfn inner() {\n    let _ = 1;\n}\n";
        assert_eq!(at(&MARKED, src, 2).as_deref(), Some("Outer"));
        assert_eq!(at(&MARKED, src, 5).as_deref(), Some("inner"));
    }

    /// Line 0 has no row: dropped, never shifted onto line 1. An empty request
    /// answers empty without parsing at all.
    #[test]
    fn line_zero_is_dropped_and_no_lines_is_no_work() {
        let r = TreesitterResolver::new(&UNMARKED);
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
        let first = UNMARKED.query().expect("compiles");
        let second = UNMARKED.query().expect("compiles");
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
