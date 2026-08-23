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

/// One `match` that names variants of an enum this workspace defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enumeration {
    /// The tracked path it is written in.
    pub source: String,
    /// The line the `match` sits on.
    pub line: usize,
    /// The enum its arms name.
    pub enum_name: String,
    /// Which variants of that enum the arms name.
    pub named: BTreeSet<String>,
    /// How many variants the enum has, when this walk found its definition.
    pub variants: usize,
    /// The catch-all it carries, rendered, or `None` when every arm is a
    /// pattern.
    pub catch_all: Option<CatchAll>,
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
    for path in sources {
        let text = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|why| panic!("read {path}: {why}"));
        let file = syn::parse_file(&text)
            .unwrap_or_else(|why| panic!("{path} does not parse as Rust: {why}"));
        collect_enums(&file, &mut owned);
        parsed.push((path, file));
    }

    let mut census = Census {
        files: parsed.len(),
        enums: owned.len(),
        ..Census::default()
    };
    for (path, file) in &parsed {
        let mut walk = Matches {
            source: path.clone(),
            owned: &owned,
            found: Vec::new(),
            matches: 0,
        };
        walk.visit_file(file);
        census.matches += walk.matches;
        census.found.extend(walk.found);
    }
    census
}

/// What the walk saw, halves included — the denominators that make the findings
/// mean something.
#[derive(Debug, Clone, Default)]
pub struct Census {
    /// Every `match` over an enum this workspace defines.
    pub found: Vec<Enumeration>,
    /// Every tracked Rust file parsed.
    pub files: usize,
    /// Every enum this workspace defines.
    pub enums: usize,
    /// Every `match` seen, over anything at all. The denominator.
    pub matches: usize,
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
fn collect_enums(file: &syn::File, into: &mut BTreeMap<String, BTreeSet<String>>) {
    struct Enums<'a>(&'a mut BTreeMap<String, BTreeSet<String>>);
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
            // A NAME THAT MEANS TWO THINGS MEANS BOTH HERE, which over-counts
            // rather than under-counts: the union of two same-named enums'
            // variants makes a match look denser, so the gate reports more and
            // never fewer. The alternative — dropping such names, as
            // `ci_plan::rust` does for functions — would make a real
            // enumeration invisible the day somebody reuses a name.
            self.0
                .entry(item.ident.to_string())
                .or_default()
                .extend(variants);
            syn::visit::visit_item_enum(self, item);
        }
    }
    Enums(into).visit_file(file);
}

struct Matches<'a> {
    source: String,
    owned: &'a BTreeMap<String, BTreeSet<String>>,
    found: Vec<Enumeration>,
    matches: usize,
}

impl<'ast> Visit<'ast> for Matches<'_> {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.matches += 1;
        let mut named: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut catch_all = None;
        for arm in &node.arms {
            match &arm.pat {
                syn::Pat::Wild(_) => catch_all = Some(CatchAll::Wildcard),
                // A BINDING WITH NO SUBPATTERN REACHES EVERYTHING A WILDCARD
                // DOES. `other => …` is the spelling R1279's own match uses.
                syn::Pat::Ident(ident) if ident.subpat.is_none() => {
                    catch_all = Some(CatchAll::Bound);
                }
                pattern => names_in(pattern, self.owned, &mut named),
            }
        }
        for (enum_name, variants) in named {
            let all = self.owned.get(&enum_name).map_or(0, BTreeSet::len);
            self.found.push(Enumeration {
                source: self.source.clone(),
                line: node.match_token.span.start().line,
                enum_name,
                named: variants,
                variants: all,
                catch_all,
            });
        }
        syn::visit::visit_expr_match(self, node);
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
