//! Which `match` expressions enumerate an enum this workspace owns and still
//! carry a catch-all.
//!
//! # What is read, and what is deliberately not
//!
//! The population is every tracked `.rs` file, parsed. Two passes: the first
//! collects the enums this workspace DEFINES, with their variant names; the
//! second walks every `match` and asks which of those enums its arms name.
//!
//! AN ENUM FROM OUTSIDE IS NOT THE SUBJECT. `syn::Expr` has ninety-odd variants
//! and is `#[non_exhaustive]`; a match over one is obliged to carry a wildcard
//! and adding an arm upstream is not this repository's event. What this gate is
//! about is an enum whose variants THIS tree adds, where the compiler would have
//! asked every reader and a catch-all is what stops it.
//!
//! NOR IS ONE THIS TREE MARKS `#[non_exhaustive]`, and that exclusion was
//! learned from the compiler rather than reasoned out: the attribute says adding
//! a variant must not break readers, rustc requires a wildcard in every other
//! crate accordingly, and the repair this gate would license does not build. See
//! `collect_enums`.
//!
//! A PATH IS MATCHED BY ITS LAST TWO SEGMENTS. `Declared::ThisRepository`,
//! `rust::Declared::ThisRepository` and `ci_plan::rust::Declared::ThisRepository`
//! are one pattern written three ways, and the enum's name plus the variant's is
//! what all three share. The cost is stated rather than hidden: two enums of the
//! same name in different modules are read as one, which over-counts density and
//! therefore reports MORE matches rather than fewer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use syn::visit::Visit;

/// One place that names variants of an enum this workspace defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enumeration {
    /// The tracked path it is written in.
    pub source: String,
    /// The line it sits on.
    pub line: usize,
    /// Which spelling it is written in — see [`Shape`].
    pub shape: Shape,
    /// The enum its arms name.
    pub enum_name: String,
    /// Which variants of that enum the arms name.
    pub named: BTreeSet<String>,
    /// How many variants the enum has, when this walk found its definition.
    pub variants: usize,
    /// The catch-all it carries, or `None` when every way out names a pattern.
    pub catch_all: Option<CatchAll>,
    /// Whether that catch-all's body DOES NOTHING AT ALL — `{}` or `()`.
    ///
    /// R1283, AND IT IS THE ONLY PART OF "DIRECTION" A PROGRAM CAN SETTLE. What
    /// a catch-all does with the value it did not name is the second half of
    /// every finding here — R1278's answered `return None`, which a caller reads
    /// as nothing to judge, and R1279's pushed to a list of failures, which is a
    /// refusal — and the two are the same clause with opposite consequences. But
    /// `false` is a refusal in `Verdict::is_failure` and an acceptance in
    /// `Origin::fetched`, so which direction a BODY points is semantic, and
    /// `CLAUDE.md` rules that out for v1. An EMPTY body is not: it produces no
    /// value, performs no effect and makes no report, whatever the surrounding
    /// code means. That is the part worth counting.
    pub discards: bool,
}

/// Which spelling an enumeration is written in.
///
/// THREE SPELLINGS, ONE SILENCE, AND R1283 MEASURED THAT THE FIRST WAS NOT THE
/// COMMON ONE. R1282's law read `match` only, which is the shape its two
/// motivating defects happened to wear. The question it asks — does adding a
/// variant reach this reader — is a question about any construct that sorts a
/// value by variant, and this repository writes two more of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    /// `match x { … }`.
    Match,
    /// `matches!(x, A | B)`. ITS CATCH-ALL IS INHERENT AND CANNOT BE WRITTEN
    /// AWAY: everything the pattern does not name answers `false`, so a variant
    /// added tomorrow is silently not-one-of-these. There is no exhaustive way
    /// to spell it, which is why a finding here is a rewrite to `match` rather
    /// than an added arm.
    MatchesMacro,
    /// `if let A = x { … } else if let B = x { … } else { … }`. The chain is the
    /// enumeration and the trailing `else` is the catch-all; a chain with no
    /// trailing `else` still has one, because the value falls through to
    /// nothing.
    IfLetChain,
}

impl Enumeration {
    /// Where it is, for a gate's own output.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}:{} `{}`", self.source, self.line, self.enum_name)
    }
}

/// What a catch-all arm does with the value it did not name.
///
/// THE DIRECTION IS THE SECOND HALF OF THE FINDING. R1278's catch-all answered
/// `return None` — a PASS for a site whose commands nobody could then judge.
/// R1279's answered by pushing to a list of failures — a REFUSAL, which is the
/// direction a catch-all has to point when it is the only one. Counting the two
/// as one clause would rank the harmful one with the harmless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchAll {
    /// `_ => …`, naming nothing.
    Wildcard,
    /// `other => …`, binding the value under a name. The same reach as a
    /// wildcard, and it reads as deliberate, which is why it is told apart.
    Bound,
    /// No arm at all: `matches!` answering `false` for everything it did not
    /// name, or an `if let` chain running off the end. UNWRITTEN AND
    /// UNWRITEABLE, which is the reason it is a kind of its own — there is no
    /// arm here to spell out, so the repair is a different construct.
    Inherent,
}

/// Every tracked Rust file's `match` expressions over this workspace's own
/// enums.
///
/// # Panics
///
/// When a tracked `.rs` file does not parse. Deliberate, and the same stance
/// `ci_plan::rust::cargo_commands` takes: a file this walk skips is a file whose
/// matches are invisible, and the skip would be silent.
#[must_use]
pub fn enumerations(root: &Path) -> Census {
    let mut sources = ci_plan::tracked_files(root, &["ls-files", "*.rs"]);
    sources.sort();
    assert!(
        !sources.is_empty(),
        "this repository tracks no Rust source at all — the empty answer that \
         looks like a clean one"
    );

    let mut parsed = Vec::new();
    let mut owned: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    for path in sources {
        let text = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|why| panic!("read {path}: {why}"));
        let file = syn::parse_file(&text)
            .unwrap_or_else(|why| panic!("{path} does not parse as Rust: {why}"));
        collect_enums(&file, &mut owned, &mut sizes);
        parsed.push((path, file));
    }
    let ambiguous = owned
        .iter()
        .filter(|(name, variants)| sizes.get(*name).copied().unwrap_or(0) != variants.len())
        .count();

    let mut census = Census {
        files: parsed.len(),
        enums: owned.len(),
        ambiguous,
        ..Census::default()
    };
    for (path, file) in &parsed {
        let mut walk = Matches {
            source: path.clone(),
            owned: &owned,
            sizes: &sizes,
            found: Vec::new(),
            matches: 0,
            unreadable: 0,
            inside_a_chain: false,
        };
        walk.visit_file(file);
        census.matches += walk.matches;
        census.unreadable += walk.unreadable;
        census.found.extend(walk.found);
    }
    census
}

/// What the walk saw, halves included — the denominators that make the findings
/// mean something.
#[derive(Debug, Clone, Default)]
pub struct Census {
    /// Every place over an enum this workspace defines, in any of the three
    /// spellings.
    pub found: Vec<Enumeration>,
    /// Every tracked Rust file parsed.
    pub files: usize,
    /// Every enum this workspace defines.
    pub enums: usize,
    /// Names that mean more than one enum in this tree, so the reader unions
    /// their variants for membership and takes the SMALLEST count for size.
    /// COUNTED, because a limit without its size is a limit nobody can weigh —
    /// and because the first draft's reasoning about this one was half wrong
    /// (R1283, `collect_enums`).
    pub ambiguous: usize,
    /// Every sorting-by-variant construct seen, over anything at all. The
    /// denominator.
    pub matches: usize,
    /// `matches!` calls whose pattern this reader could not separate from the
    /// scrutinee — a guard, or a shape it does not know. COUNTED, so the limit
    /// has a size rather than a silence (R1190's rule).
    pub unreadable: usize,
}

impl Census {
    /// The ones that ENUMERATE — name at least `floor` variants of one enum —
    /// and still carry a catch-all.
    #[must_use]
    pub fn enumerating_with_a_catch_all(&self, floor: usize) -> Vec<&Enumeration> {
        self.found
            .iter()
            .filter(|e| e.catch_all.is_some() && e.named.len() >= floor)
            .collect()
    }
}

/// Every `enum` item, with its variant names, keyed by the enum's own name.
fn collect_enums(
    file: &syn::File,
    into: &mut BTreeMap<String, BTreeSet<String>>,
    sizes: &mut BTreeMap<String, usize>,
) {
    struct Enums<'a>(
        &'a mut BTreeMap<String, BTreeSet<String>>,
        &'a mut BTreeMap<String, usize>,
    );
    impl<'ast> Visit<'ast> for Enums<'_> {
        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            // `#[non_exhaustive]` IS A DECISION ALREADY WRITTEN DOWN, AND THE
            // COMPILER TAUGHT R1282 THAT THE HARD WAY. An enum carrying it says
            // that adding a variant must NOT be a compile error for its readers,
            // and rustc enforces exactly that: a match in any other crate has to
            // end in a wildcard however many variants it names. The first draft
            // of this walk excluded only enums from OUTSIDE the workspace, found
            // `mnemosyne-render::door_label` naming all three `Door` variants
            // beside a catch-all, and the repair it licensed did not build.
            // Demanding exhaustiveness over such an enum is demanding the
            // impossible, and a gate that does it teaches people to ignore it.
            if item
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("non_exhaustive"))
            {
                return;
            }
            let variants = item
                .variants
                .iter()
                .map(|v| v.ident.to_string())
                .collect::<BTreeSet<_>>();
            // A NAME THAT MEANS TWO THINGS IS READ TWO WAYS, AND THE FIRST
            // DRAFT'S REASONING WAS HALF WRONG (R1283). It unioned the variants
            // of same-named enums on the argument that a denser-looking match
            // makes the gate report MORE and never fewer. True of the count;
            // false of the FRACTION, which is the other half of the rule — a
            // union makes the DENOMINATOR bigger, and a bigger denominator hides
            // a finding. Measured: `item-citations`' `Verdict` has four variants
            // and shares its name with one that has more, so a `matches!` naming
            // two of its four was read as two of sixteen and fell under the
            // third-of-the-enum threshold. An injection restoring that defect
            // came back GREEN, which is how it was found.
            //
            // SO MEMBERSHIP UNIONS AND SIZE TAKES THE SMALLEST. Both choices
            // point the same way: a variant either name can carry is recognised,
            // and the tightest denominator is used, so an ambiguous name reports
            // more rather than fewer. A false finding is visible and actionable;
            // a hidden one is neither.
            let name = item.ident.to_string();
            let smallest = self.1.entry(name.clone()).or_insert(variants.len());
            *smallest = (*smallest).min(variants.len());
            self.0.entry(name).or_default().extend(variants);
            syn::visit::visit_item_enum(self, item);
        }
    }
    Enums(into, sizes).visit_file(file);
}

struct Matches<'a> {
    source: String,
    owned: &'a BTreeMap<String, BTreeSet<String>>,
    sizes: &'a BTreeMap<String, usize>,
    found: Vec<Enumeration>,
    matches: usize,
    unreadable: usize,
    inside_a_chain: bool,
}

impl<'ast> Visit<'ast> for Matches<'_> {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.matches += 1;
        let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut catch_all = None;
        let mut discards = false;
        for arm in &node.arms {
            match &arm.pat {
                syn::Pat::Wild(_) => {
                    catch_all = Some(CatchAll::Wildcard);
                    discards = does_nothing(&arm.body);
                }
                // A BINDING WITH NO SUBPATTERN REACHES EVERYTHING A WILDCARD
                // DOES. `other => …` is the spelling R1279's own match uses.
                syn::Pat::Ident(ident) if ident.subpat.is_none() => {
                    catch_all = Some(CatchAll::Bound);
                    discards = does_nothing(&arm.body);
                }
                pattern => names_in(pattern, self.owned, &mut named),
            }
        }
        self.record(
            Shape::Match,
            node.match_token.span.start().line,
            named,
            catch_all,
            discards,
        );
        syn::visit::visit_expr_match(self, node);
    }

    /// `matches!(x, A | B)` — the enumeration whose catch-all cannot be written
    /// out (R1283).
    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if node
            .mac
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "matches")
        {
            self.matches += 1;
            match pattern_of_a_matches_call(&node.mac.tokens) {
                Some(pattern) => {
                    let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                    names_in(&pattern, self.owned, &mut named);
                    self.record(
                        Shape::MatchesMacro,
                        node.mac.path.segments[0].ident.span().start().line,
                        named,
                        Some(CatchAll::Inherent),
                        // AN INHERENT CATCH-ALL HAS NO BODY TO BE EMPTY. It
                        // answers `false` and there is nowhere for a reader to
                        // have written anything, which is a different limit from
                        // a body somebody left blank.
                        false,
                    );
                }
                // A `matches!` THIS READER CANNOT PARSE IS COUNTED RATHER THAN
                // PASSED OVER, because a shape it silently skipped would be the
                // very silence it exists to name.
                None => self.unreadable += 1,
            }
        }
        syn::visit::visit_expr_macro(self, node);
    }

    /// `if let A = x { … } else if let B = x { … } else { … }` — the chain is
    /// the enumeration (R1283).
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        // ONLY THE HEAD OF A CHAIN STARTS ONE. An `else if let` is reached
        // through its parent's `else_branch` and counting it again would report
        // the same enumeration once per link, each time one variant shorter.
        if !self.inside_a_chain && matches!(*node.cond, syn::Expr::Let(_)) {
            self.matches += 1;
            let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            let mut link = node;
            // BOTH ANSWERS COME OUT OF THE LOOP TOGETHER, because every way it
            // ends decides both — a `let mut` seeded before it is a value
            // nothing reads, which clippy says out loud.
            let (catch_all, trailing_else_does_nothing) = loop {
                if let syn::Expr::Let(binding) = &*link.cond {
                    names_in(&binding.pat, self.owned, &mut named);
                }
                match link.else_branch.as_ref().map(|(_, otherwise)| &**otherwise) {
                    Some(syn::Expr::If(next)) if matches!(*next.cond, syn::Expr::Let(_)) => {
                        link = next;
                    }
                    // A TRAILING `else` IS THE CATCH-ALL, and so is having none:
                    // a chain that runs off the end hands the value to nothing,
                    // which is the same reach with less to read.
                    Some(otherwise) => break (CatchAll::Bound, does_nothing(otherwise)),
                    // NO `else` AT ALL IS THE EMPTIEST BODY THERE IS: the value
                    // reaches nothing, and there is not even a brace to read.
                    None => break (CatchAll::Inherent, true),
                }
            };
            self.record(
                Shape::IfLetChain,
                node.if_token.span.start().line,
                named,
                Some(catch_all),
                trailing_else_does_nothing,
            );
            self.inside_a_chain = true;
            syn::visit::visit_expr_if(self, node);
            self.inside_a_chain = false;
            return;
        }
        syn::visit::visit_expr_if(self, node);
    }
}

impl Matches<'_> {
    fn record(
        &mut self,
        shape: Shape,
        line: usize,
        named: BTreeMap<String, BTreeSet<String>>,
        catch_all: Option<CatchAll>,
        discards: bool,
    ) {
        for (enum_name, variants) in named {
            // THE TIGHTEST DENOMINATOR AMONG SAME-NAMED ENUMS (R1283) — see
            // `collect_enums` for why the union was the wrong one.
            let all = self.sizes.get(&enum_name).copied().unwrap_or(0);
            self.found.push(Enumeration {
                source: self.source.clone(),
                line,
                shape,
                enum_name,
                named: variants,
                variants: all,
                catch_all,
                discards,
            });
        }
    }
}

/// The PATTERN half of `matches!(scrutinee, pattern)`, or `None` when this
/// reader cannot separate the two.
///
/// SPLIT AT THE FIRST TOP-LEVEL COMMA, which is exact rather than approximate:
/// a `TokenStream`'s iterator hands back a bracketed group as ONE token, so a
/// comma inside `Some(a, b)` is never seen here. What follows may carry an `if`
/// guard, which `Pat` will not parse — such a call is counted as unread rather
/// than guessed at.
fn pattern_of_a_matches_call(tokens: &proc_macro2::TokenStream) -> Option<syn::Pat> {
    use syn::parse::Parser as _;
    let mut after = proc_macro2::TokenStream::new();
    let mut past_the_comma = false;
    for token in tokens.clone() {
        if !past_the_comma {
            if matches!(&token, proc_macro2::TokenTree::Punct(punct) if punct.as_char() == ',') {
                past_the_comma = true;
            }
            continue;
        }
        after.extend(std::iter::once(token));
    }
    if !past_the_comma {
        return None;
    }
    syn::Pat::parse_multi_with_leading_vert.parse2(after).ok()
}

/// Does this expression produce nothing, do nothing and report nothing?
///
/// `{}` and `()` and nothing else. NOT a judgement about DIRECTION, which is
/// semantic — `false` refuses in one predicate and accepts in the next — but the
/// one part of it a program can settle: a body with no statements performs no
/// effect and yields no value, so the variant it caught reaches nothing at all.
#[must_use]
fn does_nothing(body: &syn::Expr) -> bool {
    match body {
        syn::Expr::Block(block) => block.block.stmts.is_empty(),
        syn::Expr::Tuple(tuple) => tuple.elems.is_empty(),
        _ => false,
    }
}

/// Which of this workspace's enums a pattern names, and which variants.
fn names_in(
    pattern: &syn::Pat,
    owned: &BTreeMap<String, BTreeSet<String>>,
    into: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let mut take = |path: &syn::Path| {
        let mut segments = path.segments.iter().rev();
        let Some(variant) = segments.next().map(|s| s.ident.to_string()) else {
            return;
        };
        let Some(enum_name) = segments.next().map(|s| s.ident.to_string()) else {
            return;
        };
        if owned
            .get(&enum_name)
            .is_some_and(|variants| variants.contains(&variant))
        {
            into.entry(enum_name).or_default().insert(variant);
        }
    };
    match pattern {
        syn::Pat::Path(path) => take(&path.path),
        syn::Pat::TupleStruct(tuple) => take(&tuple.path),
        syn::Pat::Struct(structure) => take(&structure.path),
        // AN `|` PATTERN IS SEVERAL NAMES IN ONE ARM, and it is how five of the
        // six arms of the enumerations this gate exists for are written.
        syn::Pat::Or(or) => {
            for case in &or.cases {
                names_in(case, owned, into);
            }
        }
        syn::Pat::Reference(reference) => names_in(&reference.pat, owned, into),
        syn::Pat::Paren(paren) => names_in(&paren.pat, owned, into),
        // `declared @ (A | B | C)` — the binding R1278's own enumeration uses.
        syn::Pat::Ident(ident) => {
            if let Some((_, subpattern)) = &ident.subpat {
                names_in(subpattern, owned, into);
            }
        }
        _ => {}
    }
}
