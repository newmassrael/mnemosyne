//! Spec binding: §code-citation-defense, §code-citation-defense/bidirectional-binding.
//!
//! code citation verification (Stage 2 of the 3-stage
//! code-citation defense — introduced the agent-time CLAUDE.md
//! rule, this module backs the validator-time `validate-code-refs`
//! subcommand, + 258 wire pre-commit / cascade triggers).
//!
//! extends the scanner with the spec ↔ code bidirectional
//! binding check (Path B substrate from 's
//! `AtomicSection.bindings`). The scanner now also extracts
//! `§<id>` citations and applies set-equality against each section's
//! `bindings` set (OPTION D pattern lifted from the
//! cross-ref orphan ledger).
//!
//! ## Pattern derivation
//!
//! `Round NNN`-shaped citations use the configured `entry_id_prefix`
//!:
//!
//! ```text
//! \b<prefix><digits>(\.<digits>)?\b
//! ```
//!
//! `§<id>`-shaped citations use a fixed `§` sigil + opaque token shape
//! `[A-Za-z0-9._/-]+` (covers numeric ids ``, fractional ``,
//! kebab + slash slugs `§atomic-store/changelog-atomic-ledger`), with `.`
//! and `/` INTERIOR-only — a separator that is not flanked ends the id
//! rather than joining it, so `§1/§3` is two cites (Round 799):
//!
//! ```text
//! §[A-Za-z0-9._/-]+ (trailing `.` not consumed)
//! ```
//!
//! Word-boundary discipline excludes identifier-like incidental hits.
//!
//! ## Violation taxonomy
//!
//! `Round NNN` axis (existing — /258):
//! - `Missing` — entry_id not in `changelog_entries`
//! - `Decay` — `--filter-id` cascade scan match
//!
//! `§<id>` axis:
//! - `SectionMissing` — §<id> not in `atomic_section_id_set`
//! - `CitationUnbound` — §<id> exists but citing file F not in
//! §<id>.`bindings` (code-side; spec doesn't agree)
//! - `BindingUnbacked` — (file F, sym?) in
//! §<id>.`bindings` but F has no §<id> citation (spec-side;
//! code doesn't agree)
//! - `ImplementationMissing` — §<id> whose `decision_status` is NOT
//! axiom-exempt (`DecisionStatus::is_axiom_exempt` = `Removed` |
//! `Open`) but has zero `implements` bindings. Third edge of the
//! Path B set-equality, complementing the two file-grained binding
//! directions above.
//!
//! This said "non-`Removed`" and glossed the axiom as "Active = backed
//! by code" until Round 666. The first went stale at Round 578, which
//! added `Open` to the exemption set. The second was NEVER true: the
//! axiom does not key on `Active` — `Superseded` and a `None` status
//! trigger it too (Round 269). The gloss cost a design round: R666
//! reasoned from it to a principle ("the axiom fires ON a ratified
//! state") that inverts what the code does (it is EXEMPT during an
//! unratified one), and was refuted. State the predicate, not a slogan.
//!
//! The binding directions are *asymmetric in shape*: code-side
//! violations have a concrete (file, line, entry_id); the
//! `BindingUnbacked` spec-side variant has no line and carries
//! the impl-entry symbol; the `ImplementationMissing` spec-side variant
//! has neither file nor symbol (it is a section-level absence). This is
//! modeled as a 3-variant `CodeRefViolation` enum rather than collapsing
//! the directions into one struct with sentinel fields — the shape
//! differences are domain facts, not encoding accidents.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_config::{OrphanKind, OrphanLedgerEntry, SetEqualityValidatorConfig};
use mnemosyne_core::DecisionStatus;
use serde::Serialize;

/// One `Round NNN` / `§<id>` citation candidate extracted from a source
/// file. `entry_id` retains the cite shape verbatim (`""` or
/// `""` — `§` prefix kept so the kind axis is readable from the id
/// alone).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Citation {
    pub file: PathBuf,
    pub line: usize,
    pub entry_id: String,
}

/// WHAT AN AXIS READ AT A CITATION, AND WHAT THE STORE SAYS IT SHOULD BE.
///
/// Both halves or neither: a drift is a PAIR, and one half of it is not a
/// diagnosis. `expected` is the set the store records for this (section, file) —
/// a section legitimately names more than one symbol in a file, so the
/// comparison is membership and the report prints the whole set rather than
/// picking one to look definite.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReadSymbol {
    /// The symbol the resolver answered for this line.
    pub found: String,
    /// The symbols the store records for this citation, in store order.
    pub expected: Vec<String>,
}

/// WHAT THE AXIS THAT JUDGED A CITATION READ THERE, for the axes that read
/// anything at all.
///
/// ONE ENUM AND NOT A FIELD PER AXIS (Round 1167). Round 1158 gave the symbol
/// axis a `read: Option<ReadSymbol>` and pinned its population to one kind;
/// two more axes have a payload they were dropping, and three parallel
/// `Option`s would be eight states of which five are nonsense — a shape whose
/// invariant has to be restated once per field. A single `Option<Self>` has
/// exactly four, and [`AuditAxis::evidence`] says which one each kind must be.
///
/// WHAT QUALIFIES. Every variant here is a value the axis ALREADY COMPUTED and
/// then threw away: the resolver's answer, the binding set the unbound test
/// consults, the verb the prose rule matched. Nothing is inferred for the
/// report's sake — an axis that reads only an identifier and asks the store
/// whether it exists carries nothing, and says so by name in the table rather
/// than by an absence a reader has to interpret.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CitationEvidence {
    /// [`ViolationKind::SymbolMismatch`] — the name the resolver answered and
    /// the set the store records (Round 1158).
    SymbolDrift(ReadSymbol),
    /// [`ViolationKind::CitationUnbound`] — the files the cited section DOES
    /// bind, sorted. The citing file is not among them; that is the violation.
    ///
    /// EMPTY IS AN ANSWER, not a missing one: a section with no binding at all
    /// and a section bound to somebody else are different repairs — register
    /// this file, or ask why the section claims no code — and a consumer that
    /// cannot tell them apart opens the store to find out. This is the `0` vs
    /// `null` distinction of Round 1141 one level down.
    SectionBindings { files: Vec<String> },
    /// [`ViolationKind::ProseFactAssertion`] — the verb that made the comment
    /// an assertion rather than a pointer, as spelled in
    /// [`PROSE_FACT_ASSERTION_VERBS`]. The rule is a list this repository owns,
    /// so a consumer reading a flagged line otherwise has to guess which of its
    /// words tripped it, in whichever of the list's two languages.
    AssertionVerb { verb: String },
}

/// WHICH EVIDENCE, named without the value — the type [`AuditAxis::evidence`]
/// declares in and [`CitationEvidence::shape`] answers in, so "this axis
/// carries that payload" is a checkable equality rather than a sentence in a
/// doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceShape {
    /// The axis compared an identifier against the store and read nothing at
    /// the citation site. Declared, not defaulted.
    Nothing,
    /// [`CitationEvidence::SymbolDrift`].
    SymbolDrift,
    /// [`CitationEvidence::SectionBindings`].
    SectionBindings,
    /// [`CitationEvidence::AssertionVerb`].
    AssertionVerb,
}

impl EvidenceShape {
    /// THE `--json` KEYS A VIOLATION OF THIS SHAPE CARRIES.
    ///
    /// Published so a law can ASK what the wire is called instead of restating
    /// it — the end-to-end law in `symbol_axis_reach.rs` walks the binary's own
    /// output and needs the expected key set for each kind, and a second copy of
    /// these names in a test is a copy free to drift from the serializer the day
    /// a key is renamed. This declaration is itself checked against
    /// [`CodeRefViolation::to_cli_json`] by
    /// `the_wire_names_are_the_ones_the_serializer_writes`, so the chain is
    /// serializer → these names → the law, with no link stated twice.
    #[must_use]
    pub const fn wire_keys(self) -> &'static [&'static str] {
        match self {
            Self::Nothing => &[],
            Self::SymbolDrift => &["found", "expected"],
            Self::SectionBindings => &["bound_files"],
            Self::AssertionVerb => &["assertion_verb"],
        }
    }
}

impl CitationEvidence {
    /// Which shape this value is. Exhaustive, so a new variant does not compile
    /// until [`AuditAxis::evidence`] has an axis that declares it.
    #[must_use]
    pub const fn shape(&self) -> EvidenceShape {
        match self {
            Self::SymbolDrift(_) => EvidenceShape::SymbolDrift,
            Self::SectionBindings { .. } => EvidenceShape::SectionBindings,
            Self::AssertionVerb { .. } => EvidenceShape::AssertionVerb,
        }
    }

    /// The shape of an optional evidence — `None` is [`EvidenceShape::Nothing`],
    /// which is the whole reason the enum has that variant: "read nothing" is
    /// one of the four answers, not the absence of an answer.
    #[must_use]
    pub const fn shape_of(evidence: Option<&Self>) -> EvidenceShape {
        match evidence {
            Some(e) => e.shape(),
            None => EvidenceShape::Nothing,
        }
    }
}

/// One verification failure surfaced to the caller.
///
/// Three variants — code-side citations (`Citation`), file-grained
/// spec-side claims (`BindingUnbacked`), and section-level
/// spec-side absences (`ImplementationMissing`) have structurally
/// different evidence (a concrete file:line vs an impl-entry without a
/// code witness vs a section with no impl entries at all), so the enum
/// splits at those natural boundaries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CodeRefViolation {
    /// Citation-side violation — there is a concrete cite at file:line,
    /// and the cite is wrong in some way (`kind` distinguishes how).
    ///
    /// `evidence` IS WHAT THE AXIS READ AT THIS SITE, when the axis that judged
    /// it read anything there at all. Round 1158 — until then the symbol axis
    /// resolved a name, compared it with the one the store records, emitted a
    /// violation and DROPPED the name it had read: a consumer learned that a
    /// citation had drifted but not to WHAT, and had to open the file to find
    /// out. Round 1154 made that specific rather than merely inconvenient, by
    /// repairing the doc-comment rule and so binding thirty citations in the
    /// consumer's tree that had previously resolved to an enclosing scope —
    /// someone meeting one of those as a fresh mismatch could not tell from the
    /// report whether their code had moved or this resolver had started
    /// answering. Round 1167 found the same drop on two more axes and made the
    /// field an enum rather than three parallel `Option`s.
    ///
    /// AN OPTION WHOSE POPULATION IS PINNED, which is what keeps it from being
    /// the smell a shared field otherwise is: its shape is exactly the one
    /// [`AuditAxis::evidence`] declares for the kind, checked over a real scan
    /// by `every_citation_axis_publishes_what_it_read` here and on the wire by
    /// the end-to-end law in `symbol_axis_reach.rs`. The kind's own payload
    /// would be the tidier home, but [`ViolationKind`] is a `Copy` tag read as a
    /// key by the severity map, the axis map and `kind_tag`; giving it a
    /// `String` moves that cost onto every one of those readers to spare this
    /// one field.
    Citation {
        citation: Citation,
        kind: ViolationKind,
        evidence: Option<CitationEvidence>,
    },
    /// Spec-side violation — the atomic store records a binding (of ANY
    /// kind) in `§section_id.bindings` naming (file, symbol?), but the file
    /// has no `§section_id` citation. The spec asserts a code↔spec edge the
    /// code does not witness — for an `implements` binding that is an
    /// unwitnessed implementation claim, for a `references` binding an
    /// unwitnessed trace link; either way the binding is unbacked.
    BindingUnbacked {
        section_id: String,
        file: PathBuf,
        symbol: Option<String>,
    },
    /// Spec-side coverage axiom — `§section_id` exists in the atomic
    /// store with a non-`Removed` `decision_status` but has zero
    /// `implements` bindings (a `references`-only section counts as
    /// uncovered): the section asserts a decision without naming any code
    /// that *satisfies* it.
    ///
    /// `decision_status` is kept as the raw `Option<DecisionStatus>`
    /// (not pre-resolved to `Active`) so the audit-trail consumer can
    /// distinguish "no atomic override, parser default applies" from
    /// "atomic override = Active"; the None → Active fallback is a
    /// consumer-side convention (Round 265) and resolving it at
    /// emission time would discard authoring intent.
    ImplementationMissing {
        section_id: String,
        decision_status: Option<DecisionStatus>,
    },
    /// Spec-side verification axiom (R413, opt-in) — `§section_id` is a
    /// `Normative` + `Dedicated`, non-`Removed` section with zero `verifies`
    /// bindings: it expects dedicated test/report evidence and names none.
    /// Emitted only when the verify axis is enabled
    /// (`severity_verification` configured); `ByConstruction` and
    /// `Informative` sections are exempt. `decision_status` is preserved as the
    /// raw `Option` for the same audit-trail reason as `ImplementationMissing`.
    VerificationMissing {
        section_id: String,
        decision_status: Option<DecisionStatus>,
    },
    /// Coverage-invariant violation (R423, opt-in) — `§section_id` is an EXEMPT
    /// section (`OutOfScopeHere` | `Informational`) that carries an `implements`
    /// or `verifies` binding, violating design sec 6's
    /// `has-implements/verifies ⟹ Normative`. Either the section is mislabeled
    /// (should be `Normative`) or the binding is wrong. Emitted only when
    /// `severity_classification` is set. The 3-state `coverage_expectation` enum
    /// adds the label; this gate enforces label↔binding consistency (catches the
    /// "exempt but actually implemented" mislabel the enum alone misses).
    MisclassifiedCoverage {
        section_id: String,
        decision_status: Option<DecisionStatus>,
    },
    /// Blanket-binding violation (R425, opt-in — SCE field-report P1): ONE test
    /// artifact (`file`, `symbol`) carries `verifies` bindings on more than one
    /// section. A conformance test almost always verifies one section; N>1 is
    /// the blanket-binding smell (one test stamped across siblings it does not
    /// exercise). Emitted once per ARTIFACT (not per section), carrying the
    /// full sorted list of bound sections. Emitted only when
    /// `severity_blanket` is set.
    BlanketVerifies {
        file: PathBuf,
        symbol: Option<String>,
        section_ids: Vec<String>,
    },
}

/// Whether a binding `kind` satisfies the coverage axiom (`ImplementationMissing`).
/// Only «satisfy» counts; «trace» and «verify» do not. Written as an exhaustive
/// `match` (not `== Implements`) so adding a `BindingKind` variant (e.g.
/// `refines`) is a compile error here until its coverage semantics is decided
/// — the "free single-step" extension claim's one non-obvious touch-point.
fn counts_as_coverage(kind: mnemosyne_core::BindingKind) -> bool {
    match kind {
        mnemosyne_core::BindingKind::Implements => true,
        mnemosyne_core::BindingKind::References => false,
        mnemosyne_core::BindingKind::Verifies => false,
    }
}

/// Whether a binding `kind` asserts a *code↔spec citation* edge — i.e. its
/// `file` is expected to carry a `§<id>` citation and so participates in the
/// bidirectional citation set-equality (`citation_unbound` / `binding_unbacked`
/// / `symbol_mismatch`). `Implements` and `References` do; `Verifies` does NOT
/// — a verifies binding points at a test/evidence artifact whose link to the
/// section is sourced externally (e.g. a conformance manifest), not from a
/// `§<id>` citation in the file, so requiring it to be witnessed by a citation
/// would be a spurious `binding_unbacked`. Exhaustive (not `!= Verifies`) so a
/// new kind forces this decision too.
fn is_citation_edge(kind: mnemosyne_core::BindingKind) -> bool {
    match kind {
        mnemosyne_core::BindingKind::Implements => true,
        mnemosyne_core::BindingKind::References => true,
        mnemosyne_core::BindingKind::Verifies => false,
    }
}

/// Coverage classification of a single section (Round 390). This is the
/// single source of truth for "what counts as a coverage gap": both the
/// Step-4 axiom (which emits [`CodeRefViolation::ImplementationMissing`]) and
/// the positive `report-coverage` projection ([`classify_coverage`]) route
/// through [`classify_section_coverage`], so the negative finding and the
/// positive aggregate can never drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageClass {
    /// `Normative`, non-`Removed`, with at least one `implements` binding.
    Implemented,
    /// `Normative`, non-`Removed`, with zero `implements` coverage — exactly
    /// the set the Step-4 axiom emits as `ImplementationMissing`.
    NormativeGap,
    /// `Informative` and live — prose-only, exempt from the coverage axiom.
    InformativeExempt,
    /// Lifecycle-excluded from the coverage denominator entirely — a `Removed`
    /// tombstone or an `Open` (not-yet-decided) section (`is_axiom_exempt`).
    RemovedExcluded,
}

/// Classify one section against the Round 269 coverage axiom (refined by the
/// Round 389 applicability gate). The `Informative` check comes first — it is
/// the section's declared nature, exactly as the Step-4 axiom gates on it
/// before the `Removed` lifecycle check. A `Removed` section is a tombstone
/// for reporting regardless of expectation; `NormativeGap` is the only class
/// the axiom flags, and it is `{Normative, !Removed, no implements coverage}`
/// under either ordering.
fn classify_section_coverage(section: &mnemosyne_core::SectionView) -> CoverageClass {
    let exempt = section
        .decision_status
        .unwrap_or(DecisionStatus::Active)
        .is_axiom_exempt();
    match section.coverage_expectation {
        // Both exempt classes (R421 3-state) leave the coverage axiom: a section
        // out-of-scope here, or inherently informational, expects no implements.
        mnemosyne_core::CoverageExpectation::OutOfScopeHere
        | mnemosyne_core::CoverageExpectation::Informational => {
            if exempt {
                CoverageClass::RemovedExcluded
            } else {
                CoverageClass::InformativeExempt
            }
        }
        mnemosyne_core::CoverageExpectation::Normative => {
            if exempt {
                CoverageClass::RemovedExcluded
            } else if section.bindings.iter().any(|b| counts_as_coverage(b.kind)) {
                CoverageClass::Implemented
            } else {
                CoverageClass::NormativeGap
            }
        }
    }
}

/// Single source of truth for the verify-axis gap (R413), mirroring
/// [`classify_section_coverage`]. A section is a verification gap iff it is
/// `Normative` (the implements axis applies), `Dedicated` (it expects dedicated
/// test/report evidence), non-`Removed`, and has zero `verifies` bindings.
/// `ByConstruction` / `Informative` / `Removed` are exempt. The opt-in gate
/// (`VerificationMissing`) and any future positive projection both bottom out
/// here so they cannot drift — the same single-source discipline the coverage
/// axis uses. The kind predicate is the exhaustive [`counts_as_coverage`]'s
/// sibling: `Verifies` is the only kind that satisfies it.
fn is_verification_gap(section: &mnemosyne_core::SectionView) -> bool {
    let exempt = section
        .decision_status
        .unwrap_or(DecisionStatus::Active)
        .is_axiom_exempt();
    !exempt
        && matches!(
            section.coverage_expectation,
            mnemosyne_core::CoverageExpectation::Normative
        )
        && matches!(
            section.verification_expectation,
            mnemosyne_core::VerificationExpectation::Dedicated
        )
        && !section
            .bindings
            .iter()
            .any(|b| matches!(b.kind, mnemosyne_core::BindingKind::Verifies))
}

/// Whether a section violates the coverage invariant (R423, design sec 6): an
/// EXEMPT section (`OutOfScopeHere` | `Informational`, non-`Removed`) that
/// carries an `implements` or `verifies` binding. Such a binding asserts the
/// section IS implemented/verified here, contradicting the exempt label — so
/// either the label is wrong (should be `Normative`) or the binding is.
/// `references` bindings are fine on an exempt section (a «trace» edge, not a
/// fulfillment claim). Mirrors [`is_verification_gap`] — opt-in, predicate-only.
fn is_coverage_misclassified(section: &mnemosyne_core::SectionView) -> bool {
    let lifecycle_exempt = section
        .decision_status
        .unwrap_or(DecisionStatus::Active)
        .is_axiom_exempt();
    if lifecycle_exempt {
        return false;
    }
    let exempt = matches!(
        section.coverage_expectation,
        mnemosyne_core::CoverageExpectation::OutOfScopeHere
            | mnemosyne_core::CoverageExpectation::Informational
    );
    exempt
        && section.bindings.iter().any(|b| {
            matches!(
                b.kind,
                mnemosyne_core::BindingKind::Implements | mnemosyne_core::BindingKind::Verifies
            )
        })
}

/// Blanket-binding scan (R425, SCE field-report P1) — one test artifact
/// (`file`, `symbol`) carrying `verifies` bindings on MORE THAN ONE
/// non-`Removed` section. A conformance test almost always verifies one
/// section; N>1 is the blanket smell (one test stamped across siblings it does
/// not exercise — the shape behind the 84/126 wrong-binding episode). Emits one
/// violation per ARTIFACT with the sorted section list. Single source for the
/// Step-7 gate and the unit test.
fn scan_blanket_verifies(snapshot: &mnemosyne_core::AtomicSnapshot) -> Vec<CodeRefViolation> {
    let mut by_artifact: BTreeMap<(String, Option<String>), Vec<String>> = BTreeMap::new();
    for (section_id, section) in &snapshot.sections {
        let exempt = section
            .decision_status
            .unwrap_or(DecisionStatus::Active)
            .is_axiom_exempt();
        if exempt {
            continue;
        }
        for b in &section.bindings {
            if matches!(b.kind, mnemosyne_core::BindingKind::Verifies) {
                by_artifact
                    .entry((b.file.clone(), b.symbol.clone()))
                    .or_default()
                    .push(section_id.clone());
            }
        }
    }
    let mut out = Vec::new();
    for ((file, symbol), mut section_ids) in by_artifact {
        if section_ids.len() > 1 {
            section_ids.sort_unstable();
            out.push(CodeRefViolation::BlanketVerifies {
                file: PathBuf::from(file),
                symbol,
                section_ids,
            });
        }
    }
    out
}

/// The positive coverage projection (Round 390): the 3-way breakdown of every
/// section into implemented / normative-gap / informative-exempt, plus the
/// `Removed` tombstones excluded from the denominator. Read-only — derived
/// from an [`mnemosyne_core::AtomicSnapshot`] with no authoritative state of its own (an L3
/// view, mirroring `report-binding-migration`). The `validate-code-refs`
/// coverage axis already emits the precise gap list; this is its positive
/// aggregate counterpart, so a maintainer can read coverage as a ratio rather
/// than infer it from the absence of findings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoverageReport {
    /// Section ids classified [`CoverageClass::Implemented`], sorted.
    pub implemented: Vec<String>,
    /// Section ids classified [`CoverageClass::NormativeGap`], sorted — the
    /// same set `validate-code-refs` reports as `impl_missing`.
    pub normative_gap: Vec<String>,
    /// Section ids classified [`CoverageClass::InformativeExempt`], sorted.
    pub informative_exempt: Vec<String>,
    /// Section ids classified [`CoverageClass::RemovedExcluded`], sorted.
    pub removed_excluded: Vec<String>,
}

impl CoverageReport {
    /// Sections subject to the coverage axiom (the ratio denominator):
    /// implemented + normative gap.
    pub fn applicable(&self) -> usize {
        self.implemented.len() + self.normative_gap.len()
    }

    /// Coverage ratio over the applicable set, in `[0.0, 1.0]`. `None` when no
    /// section is applicable (0/0 — an empty or all-`Informative` ledger has
    /// no coverage obligation to express as a percentage).
    pub fn coverage_ratio(&self) -> Option<f64> {
        let applicable = self.applicable();
        if applicable == 0 {
            None
        } else {
            Some(self.implemented.len() as f64 / applicable as f64)
        }
    }
}

/// Build the positive [`CoverageReport`] from a snapshot. Pure projection over
/// `snapshot.sections`; no file scan (a section's coverage class is a function
/// of its own decision_status / coverage_expectation / bindings only).
/// `BTreeMap` iteration yields section ids already sorted within each bucket.
pub fn classify_coverage(snapshot: &mnemosyne_core::AtomicSnapshot) -> CoverageReport {
    let mut report = CoverageReport::default();
    for (section_id, section) in &snapshot.sections {
        let bucket = match classify_section_coverage(section) {
            CoverageClass::Implemented => &mut report.implemented,
            CoverageClass::NormativeGap => &mut report.normative_gap,
            CoverageClass::InformativeExempt => &mut report.informative_exempt,
            CoverageClass::RemovedExcluded => &mut report.removed_excluded,
        };
        bucket.push(section_id.clone());
    }
    report
}

/// One axis of the citation audit — the unit at which a run either JUDGES or
/// does not.
///
/// # Why the audit needs the axis as a value
///
/// Every axis reports a clean result as an absence: nothing printed, count
/// zero. So does an axis the run never reached. Three modes were already
/// printing a measured-looking `0` for an axis they structurally skip:
/// `--filter-id` suppresses every axis but `decay`, `decay` itself is only
/// judged WHEN an id is named, and the four opt-in axes emit nothing while
/// their severity is unset. A consumer reading `impl_missing=0` cannot tell
/// those apart from a judged-and-clean tree, and the reading they will take is
/// the reassuring one.
///
/// So a run publishes a verdict per axis ([`SetEqualityValidator::axis_verdicts`]),
/// every skip inside [`SetEqualityValidator::scan`] is taken by ASKING that map
/// rather than by re-deriving the condition beside the code it guards, and the
/// report prints a count only where a count was measured. The kind tag lives
/// here, so the name a violation carries and the name a verdict carries are one
/// string ([`CodeRefViolation::kind_tag`] delegates to [`AuditAxis::kind_tag`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditAxis {
    Missing,
    Decay,
    SectionMissing,
    CitationUnbound,
    SymbolMismatch,
    InventoryMissing,
    InventoryDeprecated,
    ProseFactAssertion,
    BindingUnbacked,
    ImplementationMissing,
    VerificationMissing,
    MisclassifiedCoverage,
    BlanketVerifies,
}

/// Which half of the bidirectional audit an axis lives on — the split the SCE
/// lift request turns on, stated once here instead of in each caller's head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSide {
    /// Decidable from ONE FILE plus the store. A path-scoped run judges these.
    Citation,
    /// Asks the reverse question — does the store's claim have a witness
    /// anywhere — and so needs the whole tree. A path-scoped run does not
    /// judge these and says so.
    Spec,
}

impl AuditAxis {
    /// Where [`Self::all`] starts walking.
    const FIRST: Self = Self::Missing;

    /// Stable tag: the string this axis's violations carry in `--json`, and the
    /// stem of its `<tag>_count` report field.
    #[must_use]
    pub const fn kind_tag(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Decay => "decay",
            Self::SectionMissing => "section_missing",
            Self::CitationUnbound => "citation_unbound",
            Self::SymbolMismatch => "symbol_mismatch",
            Self::InventoryMissing => "inventory_missing",
            Self::InventoryDeprecated => "inventory_deprecated",
            Self::ProseFactAssertion => "prose_fact_assertion",
            Self::BindingUnbacked => "binding_unbacked",
            Self::ImplementationMissing => "impl_missing",
            Self::VerificationMissing => "verification_missing",
            Self::MisclassifiedCoverage => "misclassified_coverage",
            Self::BlanketVerifies => "blanket_verifies",
        }
    }

    /// Which half of the audit this axis belongs to.
    #[must_use]
    pub const fn side(self) -> AuditSide {
        match self {
            Self::Missing
            | Self::Decay
            | Self::SectionMissing
            | Self::CitationUnbound
            | Self::SymbolMismatch
            | Self::InventoryMissing
            | Self::InventoryDeprecated
            | Self::ProseFactAssertion => AuditSide::Citation,
            Self::BindingUnbacked
            | Self::ImplementationMissing
            | Self::VerificationMissing
            | Self::MisclassifiedCoverage
            | Self::BlanketVerifies => AuditSide::Spec,
        }
    }

    /// WHAT A VIOLATION OF THIS AXIS MUST CARRY (Round 1167).
    ///
    /// The declaration, and the emit sites are checked against it —
    /// `every_citation_axis_publishes_what_it_read` compares the shape of what
    /// the scan produced with the shape named here, so an axis that starts
    /// dropping its payload, or starts inventing one, reddens. Exhaustive over
    /// the whole axis space, which is what makes the population of
    /// evidence-bearing axes DERIVABLE (`all().filter(…)`) instead of a list a
    /// test would have to restate — a new axis does not compile until it says
    /// which of the four it is.
    ///
    /// SPEC-SIDE AXES ANSWER [`EvidenceShape::Nothing`] because this is the
    /// citation variant's field: their evidence is the named fields of their
    /// own [`CodeRefViolation`] variant (a file, a symbol, a section list), not
    /// something read at a citation site. [`AuditSide`] is the predicate that
    /// separates them where that matters.
    #[must_use]
    pub const fn evidence(self) -> EvidenceShape {
        match self {
            Self::SymbolMismatch => EvidenceShape::SymbolDrift,
            Self::CitationUnbound => EvidenceShape::SectionBindings,
            Self::ProseFactAssertion => EvidenceShape::AssertionVerb,
            // Compares a cited identifier against the store's key set and reads
            // nothing at the site — the id in `citation.entry_id` IS the whole
            // of what it looked at.
            Self::Missing
            | Self::Decay
            | Self::SectionMissing
            | Self::InventoryMissing
            | Self::InventoryDeprecated => EvidenceShape::Nothing,
            // Spec-side: not a citation variant at all.
            Self::BindingUnbacked
            | Self::ImplementationMissing
            | Self::VerificationMissing
            | Self::MisclassifiedCoverage
            | Self::BlanketVerifies => EvidenceShape::Nothing,
        }
    }

    /// The next axis in enumeration order, or `None` at the end.
    ///
    /// Exhaustive, so a new variant does not compile until it is given a place
    /// in the order — which is what makes [`Self::all`] a derivation instead of
    /// a hand list of the kind Round 777 removed. The residue this leaves,
    /// stated rather than hidden: a new arm returning `None` that nothing else
    /// points at would be unreachable from `FIRST`, and only the surrounding
    /// tests would notice.
    const fn next(self) -> Option<Self> {
        match self {
            Self::Missing => Some(Self::Decay),
            Self::Decay => Some(Self::SectionMissing),
            Self::SectionMissing => Some(Self::CitationUnbound),
            Self::CitationUnbound => Some(Self::SymbolMismatch),
            Self::SymbolMismatch => Some(Self::InventoryMissing),
            Self::InventoryMissing => Some(Self::InventoryDeprecated),
            Self::InventoryDeprecated => Some(Self::ProseFactAssertion),
            Self::ProseFactAssertion => Some(Self::BindingUnbacked),
            Self::BindingUnbacked => Some(Self::ImplementationMissing),
            Self::ImplementationMissing => Some(Self::VerificationMissing),
            Self::VerificationMissing => Some(Self::MisclassifiedCoverage),
            Self::MisclassifiedCoverage => Some(Self::BlanketVerifies),
            Self::BlanketVerifies => None,
        }
    }

    /// Every axis, in enumeration order.
    #[must_use]
    pub fn all() -> Vec<Self> {
        std::iter::successors(Some(Self::FIRST), |a| a.next()).collect()
    }
}

impl serde::Serialize for AuditAxis {
    /// As the kind tag — an axis and the violations it emits must be one name
    /// on the wire, or a consumer joining `not_judged` to a violation list has
    /// to hold a translation table this code does not publish.
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.kind_tag())
    }
}

/// Why a run did not judge an axis. Present in the report beside the axis name,
/// because "not judged" without a reason is only half an answer — the consumer's
/// next question is whether it is their config, their flags, or the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotJudged {
    /// The run was narrowed to a file list (`--paths`) and this axis asks a
    /// question about the whole tree.
    PathScope,
    /// `--filter-id` narrows the run to the decay axis of one entry id.
    DecayFilter,
    /// Decay is judged only when a caller names the id to scan for.
    NoDecayFilter,
    /// The axis is opt-in and its severity is unset in config and flags.
    AxisDisabled,
    /// The symbol axis has no instrument: no `[plugins.symbol_resolver.<lang>]`
    /// entry at all, so the demand this run collected was put to nobody.
    ///
    /// A CONFIG FACT, deliberately, like the four above — not a fact about the
    /// data. The alternative considered was "there was demand and none of it
    /// was answered", which is more precise and makes the verdict depend on
    /// whether some section happens to record a symbol: the axis would then
    /// appear and disappear from a consumer's `not_judged` list as the store is
    /// edited, and the answer to "is this run judging symbols" would not be
    /// derivable from the run's own configuration. Partial reach keeps the
    /// count (see [`SetEqualityValidator::symbol_axis_coverage`], which is
    /// where how-far belongs).
    NoResolver,
}

impl NotJudged {
    /// Machine-readable reason tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PathScope => "path_scope",
            Self::DecayFilter => "decay_filter",
            Self::NoDecayFilter => "no_decay_filter",
            Self::AxisDisabled => "axis_disabled",
            Self::NoResolver => "no_resolver",
        }
    }

    /// The same reason as a sentence, for the human report.
    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Self::PathScope => {
                "asks whether the store's claim has a witness anywhere, which needs the \
                 whole tree; this run was given a file list"
            }
            Self::DecayFilter => "--filter-id narrows the run to the decay axis of one entry",
            Self::NoDecayFilter => "decay is scanned only for an entry id a caller names",
            Self::AxisDisabled => "opt-in axis; its severity is unset",
            Self::NoResolver => {
                "no [plugins.symbol_resolver.<lang>] entry is configured, so every citation \
                 this axis would judge was put to nobody"
            }
        }
    }
}

impl serde::Serialize for NotJudged {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// What one run judged, by axis. `None` = judged; `Some(reason)` = not.
///
/// Built once per run from the mode and the config, then read by both the scan
/// (as its skip conditions) and the report (as its `not_judged` list). One
/// decision, two readers — the alternative is the report re-deriving the scan's
/// conditions, which is the two-write-paths shape this project treats as no
/// invariant at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisVerdicts(BTreeMap<AuditAxis, Option<NotJudged>>);

impl AxisVerdicts {
    /// Whether this run judges `axis`.
    #[must_use]
    pub fn judges(&self, axis: AuditAxis) -> bool {
        self.0.get(&axis).copied().flatten().is_none()
    }

    /// Whether this run judges any of `axes`.
    #[must_use]
    pub fn judges_any(&self, axes: &[AuditAxis]) -> bool {
        axes.iter().any(|a| self.judges(*a))
    }

    /// The axes this run did not judge, each with its reason, in axis order.
    #[must_use]
    pub fn not_judged(&self) -> Vec<(AuditAxis, NotJudged)> {
        self.0
            .iter()
            .filter_map(|(axis, reason)| reason.map(|r| (*axis, r)))
            .collect()
    }

    /// Every axis and its verdict, in axis order.
    pub fn iter(&self) -> impl Iterator<Item = (AuditAxis, Option<NotJudged>)> + '_ {
        self.0.iter().map(|(a, r)| (*a, *r))
    }
}

/// The file list a run was narrowed to — SCE lift-request 4-B.
///
/// # The law this type exists to keep
///
/// A scoped run's answer about the named files is EXACTLY the whole run's
/// answer about those files. That is why the scope filters the read set rather
/// than replacing it: a path outside the configured `paths` is not part of the
/// unscoped answer, so judging it would make the scoped run a different gate
/// with the same name. Such a path is reported by
/// [`PathScopeCoverage::out_of_read_set`] instead — the consumer hands this
/// flag every file a commit touched, and "this gate never reads that one" is an
/// answer they need and cannot get from silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathScope {
    /// Workspace-relative, sorted, deduplicated.
    requested: Vec<PathBuf>,
}

/// What a [`PathScope`] selected, and what it did not — reported every scoped
/// run, at every value, for the Round 819 reason: an empty answer is the shape
/// of "clean" and the shape of "nothing was read".
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct PathScopeCoverage {
    /// Every path the caller named, normalized workspace-relative.
    pub requested: Vec<String>,
    /// Read-set files the scope selected — what this run actually judged.
    pub matched_files: Vec<String>,
    /// Requested paths that exist on disk but that the configured `paths` do
    /// not cover. Judged by nobody, in either mode.
    pub out_of_read_set: Vec<String>,
    /// Requested paths that are not on disk at all — a typo, or a file the
    /// commit deleted.
    pub not_found: Vec<String>,
    /// How many files the same run would have read unscoped. The narrowing as
    /// a number rather than as an adjective.
    pub read_set_total: usize,
}

fn display_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

impl PathScope {
    /// Normalize a caller's path list against `root`.
    ///
    /// # Errors
    ///
    /// [`std::io::ErrorKind::InvalidInput`] when the list is empty (an empty
    /// scope reads as "everything" to one reader and "nothing" to the next,
    /// and both are silent), when an entry is the empty string, or when an
    /// absolute entry names a path outside the workspace.
    pub fn new(root: &Path, requested: &[String]) -> std::io::Result<Self> {
        let invalid = |msg: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, msg);
        if requested.is_empty() {
            return Err(invalid(
                "--paths needs at least one path — an empty scope reports the same clean \
                 as a clean tree"
                    .to_string(),
            ));
        }
        let mut out = Vec::with_capacity(requested.len());
        for raw in requested {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(invalid("--paths was given an empty path".to_string()));
            }
            let p = Path::new(trimmed);
            let rel = if p.is_absolute() {
                p.strip_prefix(root)
                    .map_err(|_| {
                        invalid(format!(
                            "--paths `{trimmed}` is outside the workspace {}",
                            root.display()
                        ))
                    })?
                    .to_path_buf()
            } else {
                p.strip_prefix("./").unwrap_or(p).to_path_buf()
            };
            if rel.as_os_str().is_empty() {
                return Err(invalid(format!(
                    "--paths `{trimmed}` names the workspace root, which is not a narrowing"
                )));
            }
            out.push(rel);
        }
        out.sort();
        out.dedup();
        Ok(Self { requested: out })
    }

    /// Whether a workspace-relative file is in scope — named exactly, or lying
    /// under a named directory.
    #[must_use]
    pub fn selects(&self, rel: &Path) -> bool {
        self.requested
            .iter()
            .any(|req| rel == req || rel.starts_with(req))
    }

    /// The read set narrowed to this scope.
    #[must_use]
    pub fn select(&self, root: &Path, files: Vec<PathBuf>) -> Vec<PathBuf> {
        files
            .into_iter()
            .filter(|abs| self.selects(abs.strip_prefix(root).unwrap_or(abs)))
            .collect()
    }

    /// Classify every requested path against the UNSCOPED read set.
    #[must_use]
    pub fn coverage(&self, root: &Path, read_set: &[PathBuf]) -> PathScopeCoverage {
        let rels: Vec<PathBuf> = read_set
            .iter()
            .map(|abs| abs.strip_prefix(root).unwrap_or(abs).to_path_buf())
            .collect();
        let mut cov = PathScopeCoverage {
            requested: self.requested.iter().map(|p| display_path(p)).collect(),
            read_set_total: read_set.len(),
            ..PathScopeCoverage::default()
        };
        for req in &self.requested {
            let hit = rels.iter().any(|rel| rel == req || rel.starts_with(req));
            if hit {
                continue;
            }
            if root.join(req).exists() {
                cov.out_of_read_set.push(display_path(req));
            } else {
                cov.not_found.push(display_path(req));
            }
        }
        cov.matched_files = rels
            .iter()
            .filter(|rel| self.selects(rel))
            .map(|rel| display_path(rel))
            .collect();
        cov.matched_files.sort();
        cov
    }
}

impl CodeRefViolation {
    /// The audit axis this violation belongs to.
    ///
    /// Exhaustive over both enums, so a new violation shape cannot be added
    /// without deciding which axis it is — and therefore whether a path-scoped
    /// run can judge it.
    #[must_use]
    pub fn axis(&self) -> AuditAxis {
        match self {
            CodeRefViolation::Citation { kind, .. } => match kind {
                ViolationKind::Missing => AuditAxis::Missing,
                ViolationKind::Decay => AuditAxis::Decay,
                ViolationKind::SectionMissing => AuditAxis::SectionMissing,
                ViolationKind::CitationUnbound => AuditAxis::CitationUnbound,
                ViolationKind::InventoryMissing => AuditAxis::InventoryMissing,
                ViolationKind::InventoryDeprecated => AuditAxis::InventoryDeprecated,
                ViolationKind::SymbolMismatch => AuditAxis::SymbolMismatch,
                ViolationKind::ProseFactAssertion => AuditAxis::ProseFactAssertion,
            },
            CodeRefViolation::BindingUnbacked { .. } => AuditAxis::BindingUnbacked,
            CodeRefViolation::ImplementationMissing { .. } => AuditAxis::ImplementationMissing,
            CodeRefViolation::VerificationMissing { .. } => AuditAxis::VerificationMissing,
            CodeRefViolation::MisclassifiedCoverage { .. } => AuditAxis::MisclassifiedCoverage,
            CodeRefViolation::BlanketVerifies { .. } => AuditAxis::BlanketVerifies,
        }
    }

    /// Stable kind tag for JSON output / CLI rendering — the axis's tag, so a
    /// violation and the verdict about its axis can never be two names.
    pub fn kind_tag(&self) -> &'static str {
        self.axis().kind_tag()
    }

    /// Defect class — drives `--severity-missing` vs
    /// `--severity-binding` bucketing. Hallucination-class = cited
    /// identifier doesn't exist (Missing, SectionMissing). Binding-class
    /// = set-equality violation (CitationUnbound, BindingUnbacked,
    /// ImplementationMissing — all three edges of the Path B
    /// bidirectional binding). Decay is its own informational class —
    /// never reject-bucketed.
    pub fn defect_class(&self) -> DefectClass {
        match self {
            CodeRefViolation::Citation { kind, .. } => match kind {
                ViolationKind::Missing | ViolationKind::SectionMissing => {
                    DefectClass::Hallucination
                }
                ViolationKind::CitationUnbound | ViolationKind::SymbolMismatch => {
                    DefectClass::Binding
                }
                ViolationKind::Decay => DefectClass::Decay,
                ViolationKind::InventoryMissing | ViolationKind::InventoryDeprecated => {
                    DefectClass::Inventory
                }
                ViolationKind::ProseFactAssertion => DefectClass::ProseFactAssertion,
            },
            CodeRefViolation::BindingUnbacked { .. } => DefectClass::Binding,
            CodeRefViolation::ImplementationMissing { .. } => DefectClass::Binding,
            CodeRefViolation::VerificationMissing { .. } => DefectClass::Verification,
            CodeRefViolation::MisclassifiedCoverage { .. } => DefectClass::Classification,
            CodeRefViolation::BlanketVerifies { .. } => DefectClass::Blanket,
        }
    }

    /// Render the violation as a flat JSON object — the shape
    /// `mnemosyne-cli validate-code-refs --json` emits per violation:
    /// `{"kind": <tag>, "file": <path>, "line": <n>, "section_id": <id>,
    /// "entry_id": <id>, "symbol": <name>, "decision_status": <status>}`,
    /// with optional fields omitted when absent. The default Serialize
    /// derive on `CodeRefViolation` produces a nested
    /// variant-tagged form intended for the `ErasedValidator` dispatch
    /// boundary; this method is the CLI-stable flat shape.
    pub fn to_cli_json(&self) -> serde_json::Value {
        use serde_json::{Map, Value};
        let mut obj = Map::new();
        let kind_tag = self.kind_tag();
        obj.insert("kind".into(), Value::String(kind_tag.into()));
        match self {
            CodeRefViolation::Citation {
                citation, evidence, ..
            } => {
                obj.insert(
                    "file".into(),
                    Value::String(citation.file.to_string_lossy().into_owned()),
                );
                obj.insert("line".into(), Value::Number(citation.line.into()));
                obj.insert("entry_id".into(), Value::String(citation.entry_id.clone()));
                // ABSENT rather than null when the axis read nothing, so the
                // presence of the key is itself the answer to "did anything read
                // the code here" — the same distinction the axis counts draw
                // between `0` and `null` (Round 1141). One key per payload and
                // no envelope: a consumer already switching on `kind` would gain
                // nothing from a second tag, and `found` / `expected` are the
                // names Round 1158 shipped.
                match evidence {
                    Some(CitationEvidence::SymbolDrift(r)) => {
                        obj.insert("found".into(), Value::String(r.found.clone()));
                        obj.insert(
                            "expected".into(),
                            Value::Array(
                                r.expected
                                    .iter()
                                    .map(|s| Value::String(s.clone()))
                                    .collect(),
                            ),
                        );
                    }
                    Some(CitationEvidence::SectionBindings { files }) => {
                        obj.insert(
                            "bound_files".into(),
                            Value::Array(files.iter().map(|s| Value::String(s.clone())).collect()),
                        );
                    }
                    Some(CitationEvidence::AssertionVerb { verb }) => {
                        obj.insert("assertion_verb".into(), Value::String(verb.clone()));
                    }
                    None => {}
                }
            }
            CodeRefViolation::BindingUnbacked {
                section_id,
                file,
                symbol,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                obj.insert(
                    "file".into(),
                    Value::String(file.to_string_lossy().into_owned()),
                );
                if let Some(s) = symbol {
                    obj.insert("symbol".into(), Value::String(s.clone()));
                }
            }
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                obj.insert("decision_status".into(), Value::String(status_str));
            }
            CodeRefViolation::VerificationMissing {
                section_id,
                decision_status,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                obj.insert("decision_status".into(), Value::String(status_str));
            }
            CodeRefViolation::MisclassifiedCoverage {
                section_id,
                decision_status,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                obj.insert("decision_status".into(), Value::String(status_str));
            }
            CodeRefViolation::BlanketVerifies {
                file,
                symbol,
                section_ids,
            } => {
                obj.insert(
                    "file".into(),
                    Value::String(file.to_string_lossy().into_owned()),
                );
                if let Some(s) = symbol {
                    obj.insert("symbol".into(), Value::String(s.clone()));
                }
                obj.insert(
                    "section_ids".into(),
                    Value::Array(
                        section_ids
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
        }
        Value::Object(obj)
    }
}

impl std::fmt::Display for CodeRefViolation {
    /// Render the human-readable CLI line for one violation. Format
    /// mirrors the legacy `violation_to_finding` message output:
    /// `[<kind>] <file>:<line> <entry_id>` for citations,
    /// `[<kind>] <file>:<no-cite> §<section_id> (<symbol>)` for
    /// implementation-unbacked, and `[<kind>] §<section_id>
    /// (status=<status>)` for implementation-missing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind_tag = self.kind_tag();
        match self {
            // THE HUMAN LINE CARRIES THE EVIDENCE TOO (Round 1158, extended to
            // every axis that has any in Round 1167). The R1045 lesson is that a
            // claim proved only against `--json` leaves the line a person reads
            // free to say less; the whole point of this payload is that a reader
            // does not have to go looking, and the reader of this line is the
            // one who would.
            CodeRefViolation::Citation {
                citation,
                evidence: Some(e),
                ..
            } => {
                let clause = match e {
                    CitationEvidence::SymbolDrift(r) => format!(
                        "code says `{}`, store records {}",
                        r.found,
                        if r.expected.is_empty() {
                            "nothing".to_string()
                        } else {
                            r.expected
                                .iter()
                                .map(|s| format!("`{s}`"))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        }
                    ),
                    CitationEvidence::SectionBindings { files } => {
                        if files.is_empty() {
                            "the section binds no file".to_string()
                        } else {
                            format!(
                                "the section binds {}",
                                files
                                    .iter()
                                    .map(|s| format!("`{s}`"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    }
                    CitationEvidence::AssertionVerb { verb } => {
                        format!("the prose asserts `{verb}`")
                    }
                };
                write!(
                    f,
                    "[{}] {}:{} {} — {}",
                    kind_tag,
                    citation.file.to_string_lossy(),
                    citation.line,
                    citation.entry_id,
                    clause
                )
            }
            CodeRefViolation::Citation { citation, .. } => write!(
                f,
                "[{}] {}:{} {}",
                kind_tag,
                citation.file.to_string_lossy(),
                citation.line,
                citation.entry_id
            ),
            CodeRefViolation::BindingUnbacked {
                section_id,
                file,
                symbol,
            } => write!(
                f,
                "[{}] {}:<no-cite> §{}{}",
                kind_tag,
                file.to_string_lossy(),
                section_id,
                symbol
                    .as_deref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default()
            ),
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                write!(f, "[{}] §{} (status={})", kind_tag, section_id, status_str)
            }
            CodeRefViolation::VerificationMissing {
                section_id,
                decision_status,
            } => {
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                write!(
                    f,
                    "[{}] §{} (status={}, no verifies evidence)",
                    kind_tag, section_id, status_str
                )
            }
            CodeRefViolation::MisclassifiedCoverage {
                section_id,
                decision_status,
            } => {
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                write!(
                    f,
                    "[{}] §{} (status={}, exempt but has implements/verifies — must be normative)",
                    kind_tag, section_id, status_str
                )
            }
            CodeRefViolation::BlanketVerifies {
                file,
                symbol,
                section_ids,
            } => write!(
                f,
                "[{}] {}{} verifies {} sections: {}",
                kind_tag,
                file.to_string_lossy(),
                symbol
                    .as_deref()
                    .map(|s| format!(":{}", s))
                    .unwrap_or_default(),
                section_ids.len(),
                section_ids
                    .iter()
                    .map(|s| format!("§{}", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// semantic axis that drives CLI severity flag bucketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectClass {
    /// Cited identifier doesn't exist (Missing, SectionMissing).
    Hallucination,
    /// Set-equality violation (CitationUnbound, BindingUnbacked,
    /// ImplementationMissing — all three edges of the Path B
    /// bidirectional binding).
    Binding,
    /// Cascade scan informational surface (Decay).
    Decay,
    /// Round 275 — Inventory axis violations (InventoryMissing,
    /// InventoryDeprecated). Distinct from Hallucination because the
    /// inventory genre has a different lifecycle vocabulary (Active /
    /// Deprecated / Reserved) and a separate severity knob
    /// (`severity_inventory`) for per-project tuning.
    Inventory,
    /// R413 — verification axis violation (`VerificationMissing`): a
    /// Normative + Dedicated section with no `verifies` evidence. Its own
    /// class with a separate, opt-in `severity_verification` knob, because
    /// requirement→test traceability is a per-project commitment, not a
    /// universal axiom.
    Verification,
    /// R423 — coverage-invariant violation (`MisclassifiedCoverage`): an exempt
    /// section carries an implements/verifies binding. Its own opt-in
    /// `severity_classification` knob — label↔binding consistency layered on the
    /// 3-state `coverage_expectation` enum.
    Classification,
    /// R425 — blanket-binding violation (`BlanketVerifies`): one test artifact
    /// bound `verifies` to >1 section. Its own opt-in `severity_blanket` knob
    /// (SCE field-report P1 — the cheap, metadata-free granularity fence).
    Blanket,
    /// Structured-fact SSOT violation (`ProseFactAssertion`): a code comment
    /// restates a store-homed structured fact (relation/status verb adjacent to
    /// a `§<id>`) instead of pointing to it. Its own opt-in
    /// `severity_prose_fact_assertion` knob — prose is read-side only, the fact
    /// lives once in the store. See claudedocs/structured-fact-ssot-design.md.
    ProseFactAssertion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ViolationKind {
    /// `entry_id` not in the atomic store `changelog_entries` map
    /// (hallucinated or refers to a removed entry).
    Missing,
    /// citation matches an explicit decay filter (e.g. an
    /// entry_id the cascade caller knows just transitioned to Superseded).
    /// Surfaced regardless of whether the id is still in the valid set —
    /// the entry exists, but author should review whether the code is
    /// still accurate against the new decision.
    Decay,
    /// `§<id>` citation where `<id>` is not in the atomic
    /// store's section_id set (analog of `Missing` on the section axis).
    SectionMissing,
    /// `§<id>` citation where `<id>` exists in the atomic
    /// store but the citing file is not registered in
    /// `§<id>.bindings`. The code-side half of the bidirectional
    /// set-equality violation (spec disagrees with code).
    CitationUnbound,
    /// Round 275 — Inventory ID citation where the cited id is not in
    /// `AtomicStore.inventory_entries`. Hallucination-class on the
    /// inventory axis (Phase 1A 5th entity).
    InventoryMissing,
    /// Round 275 — Inventory ID citation where the cited id exists but
    /// `InventoryEntry.status == Deprecated`. Author should update or
    /// remove the cite; the inventory entry is no longer in active use.
    /// `Reserved` status does not trigger this — Reserved is "set aside,
    /// cite permitted" by R275 design.
    InventoryDeprecated,
    /// Round 306 — RFC-002 FR-3 symbol-level enforcement.
    ///
    /// At a `§<id>` citation site (`file`:`line` carrying the cite), the
    /// `SymbolResolver` plugin's `resolve_symbols_at` answer for that line is a
    /// name that is NOT a member of the set of `Implementation.symbol`
    /// values the cited section records for the citing file. A section may
    /// be implemented by several symbols in one file, so the registered
    /// symbols form a set and the cite is bound iff its enclosing symbol is
    /// one of them. The binding exists at file granularity (R260) but no
    /// registered symbol covers this line — code drifted under the spec's
    /// claim, or the symbol set is stale.
    SymbolMismatch,
    /// Structured-fact SSOT violation — a current-state code comment RESTATES a
    /// structured fact instead of POINTING to it: a relation/status assertion
    /// verb (`supersedes` / `decided in` / `deferred to` / ...) sits adjacent
    /// to a `§<id>` citation on the same comment line. Such facts have a single
    /// store home (`decision_status` / `superseded_by` / bindings, authored via
    /// the mutate API), so prose acting as their source is a second source of
    /// truth. Emitted only when `severity_prose_fact_assertion` is set. See
    /// claudedocs/structured-fact-ssot-design.md.
    ProseFactAssertion,
}

/// Walk configured paths under `root`, collecting all readable files.
///
/// A path may name a `*` as a WHOLE segment (`crates/*/src/`, Round 777), which
/// matches every directory sitting at that position. This exists because the
/// alternative — a hand-enumerated list of sibling directories — is a copy of
/// the very bug class this validator was built to catch: the list drifts from
/// the tree the moment a sibling is added, and it drifts SILENTLY, because a
/// path that is merely absent from the list looks exactly like a path deliberately
/// excluded. It had already drifted by four crates when this was found, one of
/// them the crate that a dozen consecutive rounds had been landing in, so their
/// `Round NNN` citations were never checked while the documented contract said
/// they were. A pattern derives the set from the tree; a list restates it and
/// hopes.
///
/// Skips hidden directories (`.git/`, `.mnemosyne/`), `target/`, and
/// `node_modules/` — these never carry author-written citations, and a `*` skips
/// them too, through the same predicate rather than a second copy of the rule.
/// SOME non-existent configured paths are silently skipped; the design gives
/// external users a way to declare intent for a path that may exist in some
/// checkouts but not others. ALL of them resolving to nothing is different and
/// is an error (Round 777): a configured-but-empty scan set means the validator
/// examines no file and therefore reports no violation, which is the same
/// "clean" a genuinely clean tree produces. A pattern makes that reachable in a
/// new way — an older binary with no `*` support reads `crates/*/src/` as a
/// literal path, finds nothing, and gates vacuously — so the vacuity is refused
/// where the paths are resolved rather than left for each caller to notice.
///
/// # Errors
///
/// [`std::io::ErrorKind::NotFound`] if `paths` is non-empty and no entry
/// resolves to anything on disk; any I/O error while descending.
pub fn walk_paths(root: &Path, paths: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let roots = expand_paths(root, paths);
    if !paths.is_empty() && roots.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "configured scan paths resolve to nothing under {}: {paths:?} — \
                 a validator with no file to read reports the same clean as a clean tree",
                root.display()
            ),
        ));
    }
    let mut out = Vec::new();
    for abs in roots {
        collect_files(&abs, &mut out, true)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The concrete roots a configured path list resolves to under `root` (Round
/// 777) — every `*` already expanded and every non-existent entry dropped, i.e.
/// exactly what [`walk_paths`] will descend into.
///
/// Public because a reporter must be able to print the COVERAGE rather than the
/// configuration. That distinction is not cosmetic: the drift this round fixed
/// was invisible for as long as it was, because every report showed the list it
/// had been handed and never the tree that list covered. Same function as the
/// walk uses, so the printed answer cannot drift from the scanned one either.
#[must_use]
pub fn expand_paths(root: &Path, paths: &[String]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = paths
        .iter()
        .flat_map(|p| expand_segments(root, p))
        .filter(|p| p.exists())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// What the configured scan paths cover, and what they leave out (Round 783).
///
/// # Why this exists
///
/// Round 777 replaced a hand list of sixteen scan directories with a derived
/// `crates/*/src/`, because a list restates the tree and then drifts from it in
/// silence. The derivation fixed the drift one level down and left it one level
/// up: `crates/*/src/` is itself a claim about WHICH trees hold citations, and
/// that claim went stale the same silent way. Every `crates/*/build.rs` — four
/// files of production code — and all of `tools/*/src/` were unscanned, and the
/// only reason anyone noticed is that a round went looking.
///
/// The fix is not another path. It is making the omission LOUD: every Rust
/// source in the workspace is either covered by a scan path or named by a
/// declared exclusion, and anything else is reported. A tree merely absent from
/// the config now fails; only a tree someone wrote down stays out.
///
/// # Why stale exclusions are reported too
///
/// An exclusion that matches nothing is a rotting entry, and the workspace
/// already treats that as a failure on the orphan-ledger axis (a ledger row
/// whose orphan resolved must be deleted). The same rule here keeps the
/// exclusion list from accumulating names of trees that no longer exist —
/// otherwise the list decays into folklore that reads as policy.
/// # Two axes, two universes (Round 854)
///
/// The counts below answer the Round 783 question — "is every RUST source
/// scanned or declared" — and are Rust-only by design: a language-agnostic
/// `unscanned` would demand a declaration for every `.md` and `.toml` in the
/// tree. The two file SETS answer a different question, the Round 840 one
/// ("does the gate still read this citation anywhere"), and that answer depends
/// on every language a consumer's `paths` enrol, so they are language-agnostic.
/// Both are derived from the same walk, so the two axes cannot disagree about a
/// file (the Round 777 rule).
#[derive(Debug)]
pub struct ScanCoverage {
    /// Rust sources found under `root`. A run that considered none proves
    /// nothing, so the count is reported rather than assumed.
    pub considered: usize,
    /// Of those, the ones a configured scan path covers.
    pub scanned: usize,
    /// Rust sources neither scanned nor excluded — the drift this catches.
    pub unscanned: Vec<PathBuf>,
    /// Declared exclusions that match no file under `root`, ANY language
    /// (Round 854): an exclusion over a C++ tree is doing work, and reporting
    /// it stale told a consumer to delete the declaration that was doing it.
    pub stale_exclusions: Vec<String>,
    /// Files a configured path COVERS and a declared exclusion also names
    /// (Round 860) — a contradiction the config cannot resolve, because
    /// `scan_exclusions` declares intent for the coverage axis and does NOT
    /// subtract from what the citation gate reads.
    ///
    /// Reported from the field: the consumer's `paths` enrolled a parent
    /// directory holding build output, so they added four exclusion prefixes to
    /// quiet it. The counts did not move and `stale_exclusions` stayed 0 — the
    /// prefixes matched real files and changed nothing, which is config that
    /// looks like it works. The repair is to narrow `paths`; nothing said so.
    pub excluded_but_scanned: Vec<PathBuf>,
    /// EVERY file an exclusion removed, any language (Round 840 set, Round 854
    /// universe) — carried so [`swallowed_citations`] reads exactly the set this
    /// walk excluded, rather than re-deriving it and being free to disagree (the
    /// Round 777 rule).
    pub excluded_files: BTreeSet<PathBuf>,
    /// EVERY file the configured paths cover, any language — what the gate
    /// reads, carried for the same reason.
    pub scanned_files: BTreeSet<PathBuf>,
}

/// Compute [`ScanCoverage`] for `root` under the configured `paths` and
/// `exclusions`.
///
/// Uses [`walk_paths`] for all three sets rather than a second traversal, so the
/// files judged "covered" are exactly the files the gate will read — the R777
/// discipline that a reporter and a walk must not be able to disagree.
///
/// # Errors
///
/// Whatever the underlying directory walk fails with.
pub fn scan_coverage(
    root: &Path,
    paths: &[String],
    exclusions: &[String],
) -> std::io::Result<ScanCoverage> {
    let is_rust = |p: &Path| p.extension().is_some_and(|e| e == "rs");
    // Language-agnostic first, Rust view derived below. The other order — filter
    // to `.rs` at the walk — is what Round 854 fixed: it made the Round 840
    // reachability axis blind to C++, and blind in BOTH directions.
    let all: Vec<PathBuf> = walk_paths(root, &[String::new()])?;
    let scanned_files: BTreeSet<PathBuf> = walk_paths(root, paths)?.into_iter().collect();

    let mut excluded_files: BTreeSet<PathBuf> = BTreeSet::new();
    let mut stale_exclusions = Vec::new();
    for one in exclusions {
        let hit: Vec<PathBuf> = walk_paths(root, std::slice::from_ref(one)).unwrap_or_default();
        if hit.is_empty() {
            stale_exclusions.push(one.clone());
        }
        excluded_files.extend(hit);
    }

    // The Round 783 axis. Membership is tested against the language-agnostic
    // sets, which for a Rust path is the same answer the filtered sets gave —
    // so this axis is unchanged, by derivation rather than by a second walk.
    let rust: Vec<&PathBuf> = all.iter().filter(|p| is_rust(p)).collect();
    let unscanned: Vec<PathBuf> = rust
        .iter()
        .filter(|p| !scanned_files.contains(**p) && !excluded_files.contains(**p))
        .map(|p| (*p).clone())
        .collect();
    // Round 860 — the two sets are supposed to be disjoint: `paths` says what
    // the gate reads, an exclusion says which Rust source is deliberately NOT
    // covered by it. A file in both says both at once, and the exclusion is the
    // half that cannot win.
    let excluded_but_scanned: Vec<PathBuf> = excluded_files
        .intersection(&scanned_files)
        .cloned()
        .collect();
    Ok(ScanCoverage {
        considered: rust.len(),
        scanned: scanned_files.iter().filter(|p| is_rust(p)).count(),
        unscanned,
        stale_exclusions,
        excluded_but_scanned,
        excluded_files,
        scanned_files,
    })
}

/// A citation that exists ONLY inside an excluded tree (Round 840).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SwallowedCitation {
    /// The cited section id, exactly as written.
    pub section_id: String,
    /// An excluded file citing it — the first in path order, so the message is
    /// deterministic and points somewhere to start.
    pub file: PathBuf,
    pub line: usize,
    /// How many excluded files cite it, so a reader can tell one stray comment
    /// from a whole tree that was un-gated.
    pub occurrences: usize,
}

/// Citations that a `scan_exclusions` entry SWALLOWED (Round 840) — cited inside
/// an excluded tree and cited nowhere the gate reads.
///
/// # Why
///
/// A `scan_exclusions` entry asserts "no citation this ledger gates lives here",
/// and until now nothing checked that assertion. Reported from the field by the
/// external spec consumer, who followed our own advice to add four exclusion
/// globs, then checked the trees BY HAND before declaring them and found 55
/// hand-authored citations that the exclusions would have silently un-gated. The
/// advice produced a green gate and would have cost real coverage.
///
/// # Why "only" is the axis, and not "any"
///
/// The same consumer's other exclusions are HONEST and must stay silent: their
/// codegen output cites the same section ids as the templates that generate it,
/// and the templates remain scanned, so excluding the output re-checks one string
/// twice. A detector that fired on any citation inside an excluded tree would
/// reject that and teach people to stop declaring exclusions at all.
///
/// The distinction that separates the two cases is whether the gate still SEES
/// the id somewhere: a citation also present in a scanned file loses nothing when
/// a copy is excluded, and a citation present only in excluded trees is gone from
/// the gate entirely. So the axis is set difference, not membership.
///
/// This is the vacuity Round 783 already refuses, one level up: that round made a
/// configured-but-empty scan set loud, and this makes a scan set emptied BY
/// DECLARATION loud.
///
/// # Why the universe is every language (Round 854)
///
/// "Nowhere the gate reads" is a claim about the gate, and the gate walks
/// `paths` without looking at extensions — C++ headers, Jinja templates,
/// whatever a consumer enrols. The first version inherited a Rust-only
/// [`ScanCoverage`], which broke the claim in both directions and was reported
/// from the field: seven sections cited and bound in scanned C++ were named as
/// reachable only from an excluded tree (the consumer dropped the citation from
/// its templates to get green), while a citation living only in an excluded C++
/// file — the loss this exists to catch — could not be seen at all.
///
/// The rule that keeps this honest is unchanged and now runs on every file: the
/// detector reads exactly what the gate reads, no more and no less. That is why
/// an unreadable file is SKIPPED here rather than raising — the gate skips it
/// too, so its citations were never coverage.
#[must_use]
pub fn swallowed_citations(
    coverage: &ScanCoverage,
    known_sections: &BTreeSet<String>,
    attribution: &CitationAttribution,
) -> Vec<SwallowedCitation> {
    // THE shared predicate (Round 867), not a private copy of it. This axis used
    // to take the prefix registries and `comment_only` and apply them itself,
    // which is how it ended up honouring the registries and NOT
    // `section_namespace` while the gate honoured both.
    let cites = |path: &PathBuf| -> Vec<(usize, String)> {
        attribution
            .attribute_file(path)
            .map(|a| a.cited)
            .unwrap_or_default()
    };

    let mut still_seen: BTreeSet<String> = BTreeSet::new();
    for path in &coverage.scanned_files {
        for (_, id) in cites(path) {
            still_seen.insert(id);
        }
    }
    // First site in path order + a count, so the message is deterministic.
    let mut found: BTreeMap<String, (PathBuf, usize, usize)> = BTreeMap::new();
    for path in &coverage.excluded_files {
        for (line, id) in cites(path) {
            // SCOPED TO THIS LEDGER. An exclusion asserts that no citation
            // THIS ledger gates lives in the tree — a citation of some other
            // ledger's namespace is not this one's coverage to lose, and
            // reporting it would make a correct config loud. Found by running
            // the first version against a real consumer: on their WIRE ledger it
            // named fifteen `§10.2`-shaped SCXML citations that the SCXML ledger
            // gates and this one never could.
            if !known_sections.contains(&id) || still_seen.contains(&id) {
                continue;
            }
            found
                .entry(id)
                .and_modify(|e| e.2 += 1)
                .or_insert((path.clone(), line, 1));
        }
    }
    found
        .into_iter()
        .map(
            |(section_id, (file, line, occurrences))| SwallowedCitation {
                section_id,
                file,
                line,
                occurrences,
            },
        )
        .collect()
}

/// One store id cited in code that the store does not hold (Round 819) — an
/// ADVISORY finding, never a violation. See [`scan_id_citations`] for why the
/// axis stops at advisory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdCiteFinding {
    pub file: PathBuf,
    pub line: usize,
    /// The token as written. The report never guesses what was meant.
    pub token: String,
}

/// What the fact/entity citation axis saw (Round 819) — counted every run,
/// because an axis that quietly covers nothing reads exactly like an axis that
/// passes (the Round 807/811 rule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct IdCiteReport {
    /// Namespace prefixes DERIVED from the store's own key space, with the
    /// number of ids each covers. Empty = the axis has nothing to run on.
    pub namespaces: BTreeMap<String, usize>,
    pub files_scanned: usize,
    /// Files holding at least one citation of a store id.
    pub files_citing: usize,
    pub fact_sites: usize,
    pub entity_sites: usize,
    /// Distinct ids cited, over the store's total, per axis.
    pub facts_cited: usize,
    pub facts_total: usize,
    pub entities_cited: usize,
    pub entities_total: usize,
    /// Tokens carrying a derived namespace that name nothing in the store.
    pub unknown: Vec<IdCiteFinding>,
}

/// Derive namespace prefixes from a store's own key space (Round 819).
///
/// A prefix must cover at least TWO ids: one id does not establish a namespace,
/// and admitting it would put every hyphenated word in the corpus on the axis.
/// Derived rather than configured because a hand list is itself a claim about
/// which namespaces exist, and it goes stale silently — the Round 783 lesson,
/// applied one axis over.
fn derive_id_namespaces<'a>(ids: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for id in ids {
        let Some(cut) = id.find('-') else { continue };
        let head = &id[..cut];
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_lowercase()) {
            *counts.entry(format!("{head}-")).or_insert(0) += 1;
        }
    }
    counts.retain(|_, n| *n >= 2);
    counts
}

/// Every store-id-shaped token in `content`, as `(line, token)`.
///
/// A store id is lowercase, hyphenated, and bounded by something that is not an
/// identifier character — which is what keeps `f-jiun-holds` inside
/// `some_ident-f-jiun` from being read as a citation, and what lets a token
/// sitting against Korean prose be read as one. Hand-rolled rather than a regex
/// because the boundary rule needs lookaround the regex crate does not have.
fn id_shaped_tokens(content: &str) -> Vec<(usize, String)> {
    let is_id_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let mut out = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if !chars[i].is_ascii_lowercase() || (i > 0 && is_id_char(chars[i - 1])) {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_lowercase() || chars[i].is_ascii_digit() || chars[i] == '-')
            {
                i += 1;
            }
            // An identifier character just past the run means this was part of a
            // longer name (`f-x_y`), never a bare id.
            let clean_end = i >= chars.len() || !is_id_char(chars[i]);
            let token: String = chars[start..i].iter().collect();
            if clean_end && token.contains('-') && !token.ends_with('-') {
                out.push((idx + 1, token));
            }
        }
    }
    out
}

/// The fact/entity citation axis (Round 819) — ADVISORY, never gating.
///
/// Round 803 named the gap: the citation gate covers sections, rounds and
/// inventory, while the north-star consumer cites FACTS, so an invented `f-…`
/// in a `.rs` file draws no violation at all.
///
/// This is the unmarked half of the Round 819 design and it is advisory by
/// EVIDENCE, not by timidity. Measured on the first playable consumer: of the
/// ten id-shaped tokens in its sources that name nothing in its store, eight are
/// ids invented on purpose to test rejection (`ent-does-not-exist` and its
/// kind), and a deliberately invented id is indistinguishable BY SHAPE from a
/// hallucinated one. A ninth is the shared prefix of four real ids, used to
/// assert that no id leaks to the screen — and the prefix of a real id is itself
/// id-shaped, so no shape rule avoids it. The tenth was a real decayed citation.
/// Gating on that ratio would switch the gate off; naming it costs nothing.
///
/// There is deliberately NO suppression device. An id allow-list would
/// institutionalize the gate's reason for existing (the Round 783 refusal), and
/// a per-file fixture declaration is worse: the same test file cites real ids two
/// lines above its invented one, so silencing the file opens a false-negative
/// surface by config — and a false positive is visible where a false negative is
/// not.
///
/// The scan boundary is inherited, not invented: [`walk_paths`] already skips
/// build artifacts, which is what keeps a baked projection's thousands of
/// emitted ids out of an axis about what an author wrote.
///
/// # Why the universe is every language (Round 856)
///
/// That last sentence was true of the directory boundary and false of the file
/// boundary, which this axis narrowed to `.rs` and said nothing about — the
/// third site of the class Round 854 closed for the exclusion axis and Round
/// 855 for the symbol axis, and the only one whose doc comment did not even
/// admit the filter. The claim it publishes is a coverage claim
/// (`facts_cited` of `facts_total`, over `files_scanned`), so a Rust-only
/// universe understates it silently.
///
/// Measured on the first playable consumer, whose store this axis was built
/// for: its `.scxml` scenario files carry HAND-AUTHORED fact citations in
/// prose — `f-bell-counts`, `f-knots-are-sins`, `f-unconfessed-stays-bound` —
/// and every one of them was invisible here. The axis was blank in exactly the
/// files where that author writes fact citations by hand.
///
/// Noise stays bounded by two properties already in place rather than by an
/// extension list: the namespace prefixes are DERIVED from the store's own key
/// space (a prefix needs two ids), and the axis is advisory, so a shape match in
/// data costs a printed line and never a red gate.
///
/// Takes the run's READ SET rather than the configured paths, so a run narrowed
/// by [`PathScope`] reports this axis over the files it read.
#[must_use]
pub fn scan_id_citations(
    root: &Path,
    read_set: &[PathBuf],
    comment_only: bool,
    facts: &BTreeSet<String>,
    entities: &BTreeSet<String>,
) -> IdCiteReport {
    let mut report = IdCiteReport {
        facts_total: facts.len(),
        entities_total: entities.len(),
        namespaces: derive_id_namespaces(facts.iter().chain(entities).map(String::as_str)),
        ..IdCiteReport::default()
    };
    if report.namespaces.is_empty() {
        return report;
    }
    let mut facts_seen: BTreeSet<&str> = BTreeSet::new();
    let mut entities_seen: BTreeSet<&str> = BTreeSet::new();
    for abs in read_set {
        // Every file the gate reads, in any language (Round 856). Unreadable
        // files are skipped exactly where the gate skips them, so `files_scanned`
        // counts what was examined rather than what was walked.
        let Ok(raw) = std::fs::read_to_string(abs) else {
            continue;
        };
        report.files_scanned += 1;
        let content = if comment_only {
            strip_to_comments(&raw, comment_syntax_for(abs))
        } else {
            raw
        };
        let rel = abs
            .strip_prefix(root)
            .map_or_else(|_| abs.clone(), Path::to_path_buf);
        let mut cited_here = false;
        for (line, token) in id_shaped_tokens(&content) {
            if !report.namespaces.keys().any(|p| token.starts_with(p)) {
                continue;
            }
            if let Some(id) = facts.get(&token) {
                report.fact_sites += 1;
                facts_seen.insert(id);
                cited_here = true;
            } else if let Some(id) = entities.get(&token) {
                report.entity_sites += 1;
                entities_seen.insert(id);
                cited_here = true;
            } else {
                report.unknown.push(IdCiteFinding {
                    file: rel.clone(),
                    line,
                    token,
                });
            }
        }
        if cited_here {
            report.files_citing += 1;
        }
    }
    report.facts_cited = facts_seen.len();
    report.entities_cited = entities_seen.len();
    report
}

/// Never carries author-written citations: a VCS/tool directory, a build
/// artifact tree, or a vendored dependency. One home for the rule, read both
/// when descending into a directory and when expanding a `*` segment — two
/// copies would let a `*` scan what a walk skips.
fn is_skipped_dir(name: &str) -> bool {
    name.starts_with('.') || name == "target" || name == "node_modules"
}

/// Expand a configured path into the concrete paths it names, resolving each
/// `*` segment against the tree (Round 777). A path with no `*` yields itself,
/// so the non-pattern case is unchanged.
fn expand_segments(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let segments: Vec<&str> = pattern
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();
    if !segments.contains(&"*") {
        return vec![root.join(pattern)];
    }
    let mut out = Vec::new();
    expand_into(root.to_path_buf(), &segments, &mut out);
    out
}

fn expand_into(base: PathBuf, segments: &[&str], out: &mut Vec<PathBuf>) {
    let Some((head, rest)) = segments.split_first() else {
        out.push(base);
        return;
    };
    if *head != "*" {
        expand_into(base.join(head), rest, out);
        return;
    }
    let Ok(entries) = std::fs::read_dir(&base) else {
        return;
    };
    let mut matched: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            !p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(is_skipped_dir)
        })
        .collect();
    // Deterministic: the scan order must not depend on the filesystem's.
    matched.sort();
    for dir in matched {
        expand_into(dir, rest, out);
    }
}

fn collect_files(p: &Path, out: &mut Vec<PathBuf>, is_root: bool) -> std::io::Result<()> {
    if p.is_file() {
        out.push(p.to_path_buf());
        return Ok(());
    }
    if !p.is_dir() {
        return Ok(());
    }
    if !is_root {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if is_skipped_dir(name) {
            return Ok(());
        }
    }
    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        collect_files(&entry.path(), out, false)?;
    }
    Ok(())
}

/// Extract every `<prefix><digits>(.<digits>)?` citation candidate from
/// `content`, with 1-indexed line numbers. The `prefix` argument is the
/// `[schema].entry_id_prefix` value (default `"Round "`).
///
/// Round 810 — `external_ledgers` gives this axis the external escape hatch the
/// section axis has had since Round 277. A citation whose same-line prose ends
/// with a registered ledger name (`mnemosyne Round 780`) names ANOTHER
/// project's ledger and is not a candidate here at all; without it, a consumer
/// citing an upstream round got a `Missing` — the hallucination class — with no
/// way to say whose ledger it meant. The verdict is [`is_external_section_cite`]
/// itself rather than a second matcher, so both axes answer "is this citation
/// external?" the same way and cannot drift apart. Empty slice = every citation
/// resolves locally (the pre-Round-810 behavior).
pub fn extract_citations(
    prefix: &str,
    content: &str,
    external_ledgers: &[String],
) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    if prefix.is_empty() {
        return out;
    }
    for (line_idx, line) in content.lines().enumerate() {
        let mut start = 0;
        while start <= line.len() {
            let rel = match line[start..].find(prefix) {
                Some(r) => r,
                None => break,
            };
            let i = start + rel;
            let prev_ok = i == 0
                || !line[..i]
                    .chars()
                    .last()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false);
            if !prev_ok {
                // Advance past the matched char by its full UTF-8 width, never
                // a hardcoded +1: a non-ASCII `entry_id_prefix` puts `i` at a
                // multibyte boundary, and `i + 1` would land mid-codepoint so
                // the next `line[start..]` slice panics (same class as the
                // Round 279 Bug #1 fix in extract_inventory_citations_with_tail).
                let advance = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                start = i + advance;
                continue;
            }
            let after = &line[i + prefix.len()..];
            match scan_round_number(after) {
                Some(num) => {
                    let next_idx = i + prefix.len() + num.len();
                    let next_ok = next_idx >= line.len()
                        || !line[next_idx..]
                            .chars()
                            .next()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    // Round 810 — the numeric axis is passed empty: a round
                    // number IS the citation, so there is no `<name> <number>
                    // Round <n>` shape for it to answer.
                    let external = is_external_section_cite(&line[..i], &[], external_ledgers);
                    if next_ok && !external {
                        out.push((line_idx + 1, format!("{}{}", prefix, num)));
                    }
                    start = next_idx;
                }
                None => {
                    start = i + prefix.len();
                }
            }
        }
    }
    out
}

/// extract every `§<id>` citation candidate from `content`.
///
/// Token shape: `§` followed by 1+ chars from `[A-Za-z0-9._/-]`, in which
/// `.` and `/` are INTERIOR-only. Tail trailing `.` is not consumed
/// (mirrors `scan_round_number` so `.` at end of sentence yields `39`, not
/// `39.`), and a `/` not flanked by id chars ends the id instead of joining
/// it, so `§1/§3` reads as two cites rather than `1/` and `3` (Round 799). Returned entries use the bare
/// id (no `§` prefix) so callers can directly index `AtomicSection` keys.
/// Line numbers are 1-indexed.
///
/// `§` is itself a non-ASCII / non-identifier character, so prefix-side
/// word-boundary is implicit. Tail-side boundary: id terminates on any
/// char outside the token shape.
///
/// `§<id>` extractor with two external-standard skip axes:
/// *numeric* (RFC / IEEE / ISO/IEC, `<PREFIX> <NUMERIC> §<id>`) via
/// `external_prefixes_numeric` and *bare* (AUTOSAR family,
/// `<PREFIX> §<id>` without numeric) via `external_prefixes_bare`.
///
/// The two axes are independent — same prefix may appear in both if the
/// standard supports both forms; matching tries the axis that applies
/// based on the shape of the token preceding `§`.
///
/// Empty slices = the corresponding axis disabled. Both empty = no
/// external skip, every `§<id>` is treated as internal to this
/// workspace's atomic store.
///
/// Round 380 — external context also propagates across two citation
/// scopes wider than a single immediately-preceding prefix: (c) same-line
/// chains (`<prefix> §A / §B / §C` — cites after the first inherit when
/// separated only by a chain separator, i.e. whitespace, `/`, or the CJK
/// list joiners `·` / `・` (Round 801); a comma or word breaks the chain)
/// and (d) comment-block wraps (a sigil that is the first content on its
/// line inherits when the previous comment line *ends with* the prefix,
/// e.g. `/// WAI-ARIA 1.2` then `/// §6.6.6`). Both still require a
/// registered prefix verbatim, so a citation never skips without one.
/// Forbidden structured-fact-assertion verbs (lowercased substrings). A
/// current-state prose surface (a code comment) may POINT to a section
/// (`§<id>`) but must not RESTATE a structured fact about it: relation and
/// status assertions have a single store home (authored via the mutate API),
/// and prose must only project them. Matched as case-insensitive substring
/// containment, so a stem (`supersede`) also covers its inflections
/// (`supersedes` / `superseded`).
///
/// **Curation rule (sec 6):** a verb is listed IFF the fact it asserts has a
/// store home, so the author has a structured alternative — otherwise the lint
/// would be an alternative-less ban. Homes:
/// - supersede / 폐기 / 대체 → `superseded_by`
/// - deferred to → `resolved_by` (R579, sec 12a)
/// - decided in / ratified in → `decision_status` (Active)
/// - open question / still open → `decision_status` (Open, R578)
///
/// Deliberately EXCLUDED: `depends on` / `refines` / `conflicts with` have no
/// typed-relation home yet (build per sec 6 when first used, then add here);
/// bare `open` is too noisy a word (false positives) for too little
/// fabrication risk — only the specific `open question` / `still open` phrases
/// are linted; `implements` is the binding idiom (binding axis + max-rigor).
/// See claudedocs/structured-fact-ssot-design.md.
const PROSE_FACT_ASSERTION_VERBS: &[&str] = &[
    "supersede",
    "폐기",
    "대체",
    "deferred to",
    "decided in",
    "ratified in",
    "open question",
    "still open",
];

/// Scan comment text for the structured-fact SSOT violation "a fact-assertion
/// verb restates a fact about a `§<id>` citation in prose". Returns
/// `(line_number, section_id, verb)` per hit: on a single comment line a
/// forbidden verb (see [`PROSE_FACT_ASSERTION_VERBS`]) occurs before a `§<id>`
/// citation, so the prose is sourcing a store-homed fact instead of pointing to
/// it. Backtick code-spans are skipped (a `§<id>` inside `` `…` `` is a
/// documentation example, mirroring [`extract_section_citations`]). At most one
/// hit per line — the violation is per comment, not per token. Independent of
/// whether the cited id resolves: restating a fact in prose is the violation
/// regardless of the id's validity.
pub fn extract_prose_fact_assertions(content: &str) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let mut in_backtick = false;
        let mut in_quote = false;
        for (i, c) in line.char_indices() {
            if c == '`' {
                in_backtick = !in_backtick;
                continue;
            }
            // A `§ref` inside a double-quoted span is a meta-mention / quoted
            // example (e.g. a corrective `overclaimed "decided in §X"`), not an
            // assertion — skip it like a backtick code-span (FP-1, R582).
            if c == '"' {
                in_quote = !in_quote;
                continue;
            }
            if in_backtick || in_quote || c != '§' {
                continue;
            }
            let preceding = line[..i].to_lowercase();
            let verb = match PROSE_FACT_ASSERTION_VERBS
                .iter()
                .find(|v| preceding.contains(**v))
            {
                Some(v) => *v,
                None => continue,
            };
            // Parse the cited id tail with the same char class as
            // `extract_section_citations` (trailing `.` dropped).
            let tail = &line[i + c.len_utf8()..];
            let id: String = tail
                .chars()
                .take_while(|t| is_section_id_char(*t) || *t == '.')
                .collect();
            let id = id.trim_end_matches('.').to_string();
            if id.is_empty() {
                continue;
            }
            out.push((line_idx + 1, id, verb.to_string()));
            break;
        }
    }
    out
}

/// One section-prose structured-fact-assertion finding (sec 12b). A section's
/// own prose field RESTATES a structured fact (a verb from
/// [`PROSE_FACT_ASSERTION_VERBS`] next to a `§<id>`) instead of pointing to it —
/// the same SSOT violation as a code comment, on the store-side surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionProseFinding {
    /// The section whose prose carries the assertion.
    pub section_id: String,
    /// Which settable prose field carried it (`intent` / `rationale` /
    /// `inputs` / `outputs`). The append-only `caveats` field is exempt — see
    /// [`scan_section_prose_fact_assertions`].
    pub field: &'static str,
    /// The matched fact-assertion verb. The finding deliberately does NOT pin a
    /// single target section ref: free-prose source→target binding is unreliable
    /// (passive voice — "X superseded by Y" — produced source==target
    /// self-loops), so claiming one is false precision (FP-2, R582). This is an
    /// advisory heuristic; the author reads the named field to act.
    pub verb: String,
}

fn collect_section_prose(
    out: &mut Vec<SectionProseFinding>,
    section_id: &str,
    field: &'static str,
    text: &str,
) {
    // The matched section ref is dropped, not stored as an authoritative target —
    // free-prose binding is unreliable (it produced self-loops), so the advisory
    // finding names only (section, field, verb).
    for (_, _matched_ref, verb) in extract_prose_fact_assertions(text) {
        out.push(SectionProseFinding {
            section_id: section_id.to_string(),
            field,
            verb,
        });
    }
}

/// Scan every section's CURRENT-STATE prose fields for the structured-fact SSOT
/// violation (sec 12b) — the store-side counterpart of the code-comment lint.
/// Reuses [`extract_prose_fact_assertions`] so the verb set and detection are
/// identical across both surfaces (one rule, two places). Gated by the same
/// `severity_prose_fact_assertion` axis at the call site.
///
/// Only fields with a SETTER (`intent` / `rationale` / `inputs` / `outputs` —
/// each replaceable, so a flagged assertion can be remediated) are scanned.
/// `caveats_bullets` is **deliberately exempt**: it is append-only (only
/// `add_section_caveat`, no set/remove), an audit ledger that accumulates
/// per-round decisions — so a homed-verb inside a caveat has no sanctioned
/// remediation path and would be permanent noise. Exempting it applies the same
/// audit-vs-current-state line that exempts the changelog (R584, confirming the
/// sec 9 irreducible residual with the concrete append-only mechanism a pinion
/// field report surfaced). `alternatives_rejected` (a struct surface) stays
/// deferred.
pub fn scan_section_prose_fact_assertions(
    store: &mnemosyne_atomic::AtomicStore,
) -> Vec<SectionProseFinding> {
    let mut out = Vec::new();
    for (section_id, section) in &store.sections {
        if let Some(intent) = &section.intent {
            collect_section_prose(&mut out, section_id.as_str(), "intent", intent);
        }
        for b in &section.rationale_bullets {
            collect_section_prose(&mut out, section_id.as_str(), "rationale", b);
        }
        for b in &section.inputs_bullets {
            collect_section_prose(&mut out, section_id.as_str(), "inputs", b);
        }
        for b in &section.outputs_bullets {
            collect_section_prose(&mut out, section_id.as_str(), "outputs", b);
        }
    }
    out
}

pub fn extract_section_citations(
    content: &str,
    external_prefixes_numeric: &[String],
    external_prefixes_bare: &[String],
) -> Vec<(usize, String)> {
    let external_enabled =
        !external_prefixes_numeric.is_empty() || !external_prefixes_bare.is_empty();
    let mut out = Vec::new();
    // R380 — previous physical line, for the comment-block-wrap carry (d).
    // In comment-only mode `strip_to_comments` preserves line numbers (code
    // lines become spaces), so this is the previous *comment* line whenever
    // the carry could legitimately fire.
    let mut prev_line = "";
    for (line_idx, line) in content.lines().enumerate() {
        // — single-line backtick state. `` inside a code-span
        // is documentation example, not a citation. Toggled on each backtick
        // and reset at line end (multi-line fenced code spans are not
        // recognized; the comment-only stripper already gates this for
        // most source files, and inline backtick spans cover the doc-comment
        // example case that survives stripping).
        let mut in_backtick = false;
        // R380 — line-local chain state for `<prefix> §A / §B / §C` (c).
        let mut chain_external = false;
        let mut last_cite_end = 0usize;
        let mut chars = line.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c == '`' {
                in_backtick = !in_backtick;
                continue;
            }
            if in_backtick {
                continue;
            }
            if c != '§' {
                continue;
            }
            // Tail: read [A-Za-z0-9._/-]+ starting at the byte after `§`.
            // `.` is constrained to digit-digit boundaries so
            // `.bindings` parses as `39` (the prose-style field
            // reference suffix is not part of the section_id) while
            // `` (fractional id) remains intact. Parsed first (before the
            // external verdict) so the chain bookkeeping (c) always has the
            // cite's byte extent.
            let tail_start = i + c.len_utf8();
            let tail = &line[tail_start..];
            let tail_chars: Vec<(usize, char)> = tail.char_indices().collect();
            let mut last_byte = 0usize;
            for (idx, &(j, t)) in tail_chars.iter().enumerate() {
                if t == '.' {
                    let prev_is_digit = idx > 0 && tail_chars[idx - 1].1.is_ascii_digit();
                    let next_is_digit = tail_chars
                        .get(idx + 1)
                        .map(|(_, c)| c.is_ascii_digit())
                        .unwrap_or(false);
                    if !(prev_is_digit && next_is_digit) {
                        break;
                    }
                    last_byte = j + t.len_utf8();
                    continue;
                }
                // EVERY separator in the id charset is INTERIOR-ONLY, which is
                // the rule `.` above already had in a stricter form (Round 799
                // for `/`, generalized in Round 800).
                //
                // The charset carries four non-alphanumerics — `.`, `_`, `/`,
                // `-` — and each is also how a human separates two cites.
                // Unconstrained, `§1/§3` parsed as `1/` and `3`: the first is no
                // section, so the gate returned SectionMissing, the HALLUCINATION
                // class, against a comment that had cited two real sections
                // correctly. The grammar was inconsistent with itself too, since
                // the second cite past the same separator parsed fine.
                //
                // Naming ONE separator here would be a hand list of one, and the
                // next reader would meet `§1-§3` — a range, more common in prose
                // than the slash — with the same defect. So the test is on the
                // CHARACTER CLASS: an id may not begin or end with a separator,
                // whichever it is. Slugs are unaffected because a slug never
                // starts or ends with one, and a separator between two cites
                // never has an id char on its far side.
                // Separators only — a character OUTSIDE the charset still ends
                // the id outright, which is what the terminator below is for.
                if !t.is_ascii_alphanumeric() && is_section_id_char(t) {
                    let prev_ok = idx > 0 && is_section_id_char(tail_chars[idx - 1].1);
                    let next_ok = tail_chars
                        .get(idx + 1)
                        .is_some_and(|(_, c)| is_section_id_char(*c));
                    if !(prev_ok && next_ok) {
                        break;
                    }
                    last_byte = j + t.len_utf8();
                    continue;
                }
                if !is_section_id_char(t) {
                    break;
                }
                last_byte = j + t.len_utf8();
            }
            if last_byte == 0 {
                continue;
            }
            let mut end = last_byte;
            if tail[..end].ends_with('.') {
                end -= 1;
            }
            if end == 0 {
                continue;
            }
            let id = tail[..end].to_string();
            let cite_end = tail_start + end;

            // External-standard verdict — three context paths (R277/284 +
            // R380), all still gated on a verbatim-registered prefix:
            //  - direct: `<prefix>` immediately precedes the sigil (R277/284)
            //  - chained (c): the previous same-line cite was external and
            //    only chain separators sit between (`is_chain_separator`)
            //  - carried (d): the sigil is the first content on its line and
            //    the previous comment line ends with the prefix (wrapped)
            let is_external = external_enabled
                && (is_external_section_cite(
                    &line[..i],
                    external_prefixes_numeric,
                    external_prefixes_bare,
                ) || (chain_external && gap_is_chain_only(&line[last_cite_end..i]))
                    || (line_prose_is_marker_only(&line[..i])
                        && prev_line_ends_with_prefix(
                            prev_line,
                            external_prefixes_numeric,
                            external_prefixes_bare,
                        )));

            // skip metavariable placeholders like `§N`, `§X`,
            // `§Y` used in doc-comments to mean "any section id". A real
            // section_id is either multi-char or starts with lowercase /
            // digit; a single uppercase letter is metasyntax.
            let is_metavar = id.chars().count() == 1
                && id
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false);
            if !is_metavar && !is_external {
                out.push((line_idx + 1, id));
            }
            // A metavar carries no external context forward; an internal
            // cite breaks the chain; an external cite continues it.
            chain_external = is_external && !is_metavar;
            last_cite_end = cite_end;

            // Advance the outer iterator past what we consumed.
            // (peekable / char_indices doesn't have skip-to-byte, so we
            // re-seek by consuming until we pass `cite_end`.)
            while let Some(&(k, _)) = chars.peek() {
                if k < cite_end {
                    chars.next();
                } else {
                    break;
                }
            }
        }
        prev_line = line;
    }
    out
}

/// Round 380 — is `gap` (the text between two same-line `§` cites) made of
/// only chain separators? See [`is_chain_separator`] for the set and why it
/// is a closed one; a comma, word, or any other char breaks the chain so a
/// distinct cite after `, ` / ` and ` is still validated as internal.
///
/// Round 808 — a PARENTHETICAL GLOSS on a cite does not break the chain:
/// `설계 §2-1(구역 개폐)·§2-4(마을=데이터)·§4` is one document enumerated
/// three times, exactly like the un-glossed form Round 801 closed. The rule
/// is the Round 380 one applied to the gap with its balanced groups removed:
/// **strip balanced `(…)` runs, and what remains must still be a non-empty
/// run of chain separators.** So a gloss chains, and a WORD outside the gloss
/// still breaks (`§A(x) 그리고 §B`) — the annotation is subordinate to the
/// cite it follows, while a bare word starts a new thought.
///
/// Two deliberate conservatisms, because widening this predicate is the same
/// false-negative surface [`is_chain_separator`] warns about — the cite
/// BEHIND the gap stops being checked at all:
///
/// - **Unbalanced breaks.** An unclosed `(` leaves no way to know where the
///   gloss ends, so the chain dies rather than swallowing the rest of the line.
/// - **ASCII parens only.** Every measured instance in the reporting corpus
///   and in this one uses `(` / `)`; `（）` / `[]` / `【】` have ZERO. Adding
///   them by symmetry is precisely the move Round 801 refused for `-` —
///   resume on a measured instance, not on the shape of the rule.
///
/// This is the chain-side of the Round 802 correction: what a shape selects is
/// where the prefix's slice ENDS, not which axis is allowed. Reported by the
/// same downstream workspace as Round 801, whose remaining two violations were
/// this one line, and reproduced here before it was believed.
fn gap_is_chain_only(gap: &str) -> bool {
    let mut depth = 0usize;
    let mut outside = String::with_capacity(gap.len());
    for c in gap.chars() {
        match c {
            '(' => depth += 1,
            ')' => match depth.checked_sub(1) {
                Some(d) => depth = d,
                // A close with nothing open: the gap is not a glossed chain.
                None => return false,
            },
            _ if depth > 0 => {}
            _ => outside.push(c),
        }
    }
    depth == 0 && !outside.is_empty() && outside.chars().all(is_chain_separator)
}

/// Round 801 — does `c` join two citations of the SAME document, rather
/// than end the thought that named it?
///
/// This set is SEMANTIC, which is what separates it from the id charset
/// [`is_section_id_char`], whose interior-only rule Round 800 could derive
/// mechanically (no id may begin or end with a separator, whichever the
/// separator is). No such derivation exists here: `/` joins two sections of
/// one standard and `.` ends a sentence, and nothing about either character
/// says which. So the set stays a closed list and its test pins BOTH
/// classes — what chains and what breaks — since a rule that cannot derive
/// its oracle has to state the contrast instead.
///
/// Widening it is a FALSE-NEGATIVE surface: every char added here stops the
/// cite behind it from being checked at all. That is the mirror of the id
/// charset, where a wrong widening costs false positives, which are at least
/// visible. It is also why this set is deliberately not workspace-
/// configurable — a workspace could otherwise disable the axis by listing
/// `.` and never see a violation again.
///
/// `·` (U+00B7) and `・` (U+30FB) are the Korean and Japanese list joiner:
/// `첫날 §4·§6·§7·§13` enumerates one document, the exact role `/` plays in
/// `UAX #9 §6.6.8 / §6.6.9`. Reported by a downstream workspace whose prose
/// is Korean, where the ASCII-only set turned a correct enumeration into
/// three `SectionMissing` hallucination verdicts. `、` (U+3001) is the CJK
/// comma and is NOT here, matching the `,` rule Round 380 already stated.
///
/// `-` is NOT here either, though Round 800 did add it to the id charset's
/// interior rule. A spaced hyphen is a dash — `RFC 2131 §3 - our §5-4` is
/// two documents, not two sections of one — so chaining it would silently
/// stop checking the cite after every dash in a comment; and the corpus
/// that reported this gap holds 5 `·` chains and 0 `-` chains. Resume on a
/// measured instance, not on the symmetry with Round 800 — that round's
/// lesson was about a structural rule and does not transfer to this one.
fn is_chain_separator(c: char) -> bool {
    c.is_whitespace() || c == '/' || c == '·' || c == '・'
}

/// Round 380 — is the text before a `§` only a comment marker (leading
/// whitespace + a run of `/` or `#`) with no prose? Such a sigil is the
/// first content on its line and may be a wrapped-citation continuation.
fn line_prose_is_marker_only(before: &str) -> bool {
    before
        .trim_start()
        .trim_start_matches(['/', '#'])
        .trim()
        .is_empty()
}

/// Round 380 — does the previous comment line *end with* an external
/// prefix (so a wrapped `/// <prefix>\n/// §id` continuation inherits it)?
/// Reuses [`is_external_section_cite`] by appending a space, so the same
/// numeric/bare/multi-word matching applies; only fires when the prefix is
/// the literal trailing content of the line.
fn prev_line_ends_with_prefix(
    prev_line: &str,
    prefixes_numeric: &[String],
    prefixes_bare: &[String],
) -> bool {
    if prev_line.trim().is_empty() {
        return false;
    }
    let mut ctx = String::with_capacity(prev_line.len() + 1);
    ctx.push_str(prev_line);
    ctx.push(' ');
    is_external_section_cite(&ctx, prefixes_numeric, prefixes_bare)
}

/// Byte offset of the start of the last whitespace-delimited token in `s`.
/// Splits on the last Unicode-whitespace char and advances past its full
/// UTF-8 width (not a hardcoded +1): `char::is_whitespace` matches multibyte
/// whitespace (U+00A0, U+2028, …), so `rfind(..).map(|i| i + 1)` could land
/// mid-codepoint and panic the following slice. Returns 0 when `s` has no
/// whitespace.
fn last_whitespace_token_start(s: &str) -> usize {
    s.char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0)
}

/// Round 277 + 284 — detect external-standard context preceding a `§`
/// sigil.
///
/// Two recognized forms, keyed on the shape of the token immediately
/// before the `§`:
///
/// - **Numeric mode** (R277): `<prefix> <number> §<id>` where `<number>`
/// is a document-number token (`2131`, `802.3`, `14882`, `R1345`).
/// Prefix matched verbatim against `prefixes_numeric` after punctuation
/// strip (R281). Used by RFC / IEEE / ISO/IEC.
/// - **Bare mode** (R284): `<prefix> §<id>` — no document number between
/// prefix and sigil. Prefix matched verbatim against `prefixes_bare`
/// after punctuation strip. Used by AUTOSAR family (TR_SOMEIP,
/// SOMEIPSD, SWS_SD) and other doc-name-only standards.
///
/// Round 802 — THE TOKEN SHAPE SELECTS WHICH SLICE A PREFIX MUST END,
/// NOT WHICH AXIS IS ALLOWED. The two modes were mutually exclusive, so a
/// document-number-shaped last token committed to the numeric axis and
/// returned false there rather than trying bare — which made a registered
/// bare prefix that happens to carry digits (`ISO9001 §3`) unusable, and
/// made widening the token shape impossible without breaking it. Numeric
/// is tried where the shape allows and bare answers otherwise. The axis
/// invariant is untouched: neither path skips a citation without a
/// verbatim-registered prefix, so the widening is bounded by an explicit
/// act of the workspace rather than coming for free.
///
/// Round 379 — prefixes may be multi-word (`"CSS Color"`, `"Unicode
/// Standard"`): the prose before the document-number (numeric mode) or
/// before the sigil (bare mode) is matched against each registered
/// prefix as a token-boundary *suffix*.
fn is_external_section_cite(
    line_before_sigil: &str,
    prefixes_numeric: &[String],
    prefixes_bare: &[String],
) -> bool {
    // Both forms require whitespace between the trigger and the sigil;
    // otherwise this is an inline reference (`RFC2131§3`) which is not
    // the recognized form.
    let trimmed = line_before_sigil.trim_end();
    if trimmed.len() == line_before_sigil.len() {
        return false;
    }
    let last_token_start = last_whitespace_token_start(trimmed);
    let last_token = &trimmed[last_token_start..];
    if last_token.is_empty() {
        return false;
    }
    // Numeric mode (R277, widened R379). The document-number token may
    // carry a leading `#` (`UAX #9`), a leading name (`R1345`), or
    // trailing letters (`802.11ax`); the prose *before* it must end with a
    // registered numeric prefix (which may itself be multi-word, e.g.
    // `CSS Color`).
    //
    // Round 809 — the slice before the number is matched against BOTH registries,
    // not only the numeric one. Which registry a document's NAME is declared in
    // says how it is usually written, not whether it may ever carry an instance
    // number: `필드 리포트 ③ §4.5` names a bare-registered document and numbers
    // it, and before this round that shape reached NEITHER axis — the numeric one
    // declined because the name is not in its registry, and the bare one saw
    // prose ending in the number rather than in the name. This is the same
    // correction Round 802 made one level down (what a shape selects is where the
    // prefix's slice ENDS, never which axis is allowed); Round 802 closed it for
    // a prefix whose own last token looks like a number, and this closes it for a
    // prefix followed by a separate number token. Still bounded by an explicit
    // act of the workspace: the slice must end with a VERBATIM-registered prefix.
    if is_document_number_token(last_token) {
        let before_num = trimmed[..last_token_start].trim_end();
        if !before_num.is_empty()
            && (prose_ends_with_prefix(before_num, prefixes_numeric)
                || prose_ends_with_prefix(before_num, prefixes_bare))
        {
            return true;
        }
    }
    // Bare mode (R284, widened R379). The prose must end with a registered
    // bare prefix (which may be multi-word, e.g. `Unicode Standard`).
    !prefixes_bare.is_empty() && prose_ends_with_prefix(trimmed, prefixes_bare)
}

/// Round 379 — does `tok` look like a standard's *document-number* token?
///
/// Accepts an optional leading `#` (Unicode Annex form `UAX #9`), then an
/// optional ASCII-alphabetic run, then requires an ASCII digit, over an
/// all-alphanumeric-or-dot body (`791`, `802.3`, `1.2`, `9`, `802.11ax`,
/// `R1345`). Rejects names (`Color`, `Standard`) and hyphenated tokens
/// (`WAI-ARIA`), which select bare mode.
///
/// Round 802 — the leading-name run is what admits `pinion R1345 §5`.
/// `R` plus digits is the standard document-name shape in this ecosystem,
/// the way `#9` is Unicode's and `802.11ax` is IEEE's, and requiring the
/// token to *start* with a digit read it as a name and sent the citation
/// down the bare axis, where the prose ends with `R1345` rather than with
/// the registered prefix. Widening it is only safe because the caller no
/// longer treats the two modes as exclusive: a bare prefix that carries
/// digits still resolves on the bare axis after the numeric one declines.
/// Round 809 — is `c` a circled digit (`①` … `⑳`, U+2460..=U+2473)?
///
/// One glyph that IS a number, so a token made of them is a document number
/// the way `3` is. Unlike the chain-separator set (whose members had to be
/// enumerated one measured instance at a time because `/` joins and `.` ends
/// and nothing about either character says which), this range is DERIVABLE:
/// every member is the same character class with the same reading, and there
/// is no `②`-versus-`③` distinction to be drawn. So taking the whole
/// contiguous block is not the add-by-symmetry move Round 801 refused for the
/// dash — that one traded a semantic judgment for a shape.
///
/// Other numeric glyph families (parenthesized `⑴`, fullwidth `３`) are NOT
/// here: they are different classes, and neither corpus holds one.
fn is_circled_digit(c: char) -> bool {
    ('\u{2460}'..='\u{2473}').contains(&c)
}

fn is_document_number_token(tok: &str) -> bool {
    let body = tok.strip_prefix('#').unwrap_or(tok);
    // Round 809 — a circled-digit run is a document number written as glyphs.
    // The reporting corpus writes its own ledgers that way (`필드 리포트 ③`),
    // and the enumeration reading of the same character never collides here:
    // an enumeration marker is not preceded by a registered document name, and
    // measured across that corpus, its enumeration markers are never followed
    // by a section sigil at all.
    if !body.is_empty() && body.chars().all(is_circled_digit) {
        return true;
    }
    let number = body.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    number.starts_with(|c: char| c.is_ascii_digit())
        && body.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// Round 379 — does `prose` end with one of `prefixes` on a token
/// boundary? A prefix may be multi-word (`"CSS Color"`, `"Unicode
/// Standard"`); the char before the match must be a non-alphanumeric
/// boundary, so `(RFC` / `[RFC` / a leading whitespace all match but
/// `FOORFC` does not. Verbatim suffix match — no domain knowledge, the
/// engine never learns what any prefix means.
fn prose_ends_with_prefix(prose: &str, prefixes: &[String]) -> bool {
    let prose = prose.trim();
    for p in prefixes {
        if p.is_empty() || !prose.ends_with(p.as_str()) {
            continue;
        }
        let idx = prose.len() - p.len();
        if !prose.is_char_boundary(idx) {
            continue;
        }
        let boundary_ok = idx == 0
            || !prose[..idx]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        if boundary_ok {
            return true;
        }
    }
    false
}

/// The namespace segment of a `§<id>` citation: the part before the first
/// `-`, or the whole id when it has no `-`. Pure, offline, no domain
/// knowledge — the engine never learns what any particular namespace means.
///
/// `scxml-6.4` → `scxml` · `mesh-16.7` → `mesh` · `D` → `D` ·
/// `scxml-D-interpret` → `scxml`. Used by the workspace `section_namespace`
/// scope to decide whether a citation falls under this ledger's jurisdiction.
fn citation_namespace(section_id: &str) -> &str {
    // `split_once` makes the no-hyphen case explicit (the whole id is its own
    // namespace) instead of an unreachable `split('-').next()` fallback.
    section_id
        .split_once('-')
        .map_or(section_id, |(namespace, _)| namespace)
}

fn is_section_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '/' || c == '-' || c == '_'
}

/// Round 275 — Extract inventory ID citations from `content` (Phase 1A).
///
/// For each `prefix` in `prefixes`, scans `<prefix><tail>` tokens where
/// `<tail>` matches `[A-Z0-9_]+` *and ends in a digit*. The digit-terminus
/// rule distinguishes inventory IDs (e.g., `ARP_07`,
/// `TCP_RETRANSMISSION_TO_04`) from coding-convention identifiers
/// (`TCP_BUFFER_SIZE`, `ARP_PROTO_TYPE`) — the dominant false-positive
/// surface when scanning C/Rust/Java codebases.
///
/// Word-boundary rules mirror `extract_citations`: the char before
/// `<prefix>` must be non-alphanumeric/non-underscore, and the char after
/// `<tail>` must be the same. Backtick code-span skipping mirrors
/// `extract_section_citations` (the comment-only filter handles the
/// dominant string-literal surface; this is the inline doc-example
/// guard).
///
/// Output: `(line_idx_1_based, full_inventory_id)` pairs, deduped on
/// `(line, id)` so that a single token matched by multiple registered
/// prefixes (e.g., `SOMEIP_` and `SOMEIP_ETS_` both registered, token =
/// `SOMEIP_ETS_BASICS_01`) surfaces once with the longest-prefix match
/// recorded. Returns empty when `prefixes.is_empty()` (axis disabled).
pub fn extract_inventory_citations(prefixes: &[String], content: &str) -> Vec<(usize, String)> {
    extract_inventory_citations_with_tail(prefixes, content, InventoryTailMode::IdToken)
}

/// Extract *section-path-shaped* inventory citations.
///
/// Companion axis to [`extract_inventory_citations`] for external-spec
/// mirror adopters (W3C SCXML, IETF RFC, IEEE, AUTOSAR, …) whose
/// citation tail uses section-path characters (`A-Za-z0-9./-_`) instead
/// of the opaque-ID shape (`[A-Z0-9_]+ ending in digit`). Token form:
/// `<prefix><tail>` where `<tail>` matches `[A-Za-z0-9./-_]+` with no
/// digit-terminus requirement — `3.13`, `test144`, `D.2.selectTransitions`
/// all match.
///
/// Word-boundary, backtick-skip, longest-prefix-first ordering, and
/// dedup semantics are identical to [`extract_inventory_citations`].
/// Returns empty when `prefixes.is_empty()` (axis disabled).
///
/// Use case: an adopter mirroring W3C SCXML registers
/// `inventory_path_prefixes = ["W3C SCXML "]` and a W3C SCXML section
/// like `3.13` gets registered as `InventoryEntry { id = "W3C SCXML
/// 3.13", … }` in the atomic store. Citations of the form
/// `// W3C SCXML 3.13` in code resolve against the inventory axis
/// without forcing a mass cite migration to the sigil-prefixed form.
pub fn extract_inventory_path_citations(
    prefixes: &[String],
    content: &str,
) -> Vec<(usize, String)> {
    extract_inventory_citations_with_tail(prefixes, content, InventoryTailMode::SectionPath)
}

/// Inventory citation tail shape — distinguishes opaque-ID citations
/// from section-path identifiers. Internal to the extractor; callers
/// pick the public function (`extract_inventory_citations` vs
/// `extract_inventory_path_citations`) and the corresponding mode is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InventoryTailMode {
    /// `[A-Z0-9_]+` with tail ending in a digit. Targets opaque
    /// inventory IDs (`ARP_07`, `TCP_RETRANSMISSION_TO_04`).
    IdToken,
    /// `[A-Za-z0-9./-_]+` with no digit-terminus requirement. Targets
    /// section paths (`3.13`, `test144`, `D.2.selectTransitions`).
    SectionPath,
}

fn extract_inventory_citations_with_tail(
    prefixes: &[String],
    content: &str,
    tail_mode: InventoryTailMode,
) -> Vec<(usize, String)> {
    if prefixes.is_empty() {
        return Vec::new();
    }
    // Longest-prefix-first ordering so that overlapping registrations
    // (`SOMEIP_` and `SOMEIP_ETS_`) yield the longer match — the more
    // specific ID is what the author intended.
    let mut ordered: Vec<&String> = prefixes.iter().collect();
    ordered.sort_by_key(|p| std::cmp::Reverse(p.len()));

    let mut seen: BTreeSet<(usize, String)> = BTreeSet::new();
    for (line_idx, line) in content.lines().enumerate() {
        let mut in_backtick = false;
        let bytes = line.as_bytes();
        // Round 279 Bug #1 fix — drive the outer loop with `char_indices`
        // instead of raw byte indexing. A non-ASCII char in the comment
        // (em-dash `—`, Korean, CJK, …) previously left `i` mid-multibyte,
        // and the next `line[i..].starts_with(prefix)` call panicked at
        // a UTF-8 char-boundary check. `char_indices` yields only valid
        // boundaries, so `line[i..]` is always safe; advancement after a
        // match is done via `peek/next` until past the matched byte span.
        let mut chars = line.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if c == '`' {
                in_backtick = !in_backtick;
                continue;
            }
            if in_backtick {
                continue;
            }
            let mut matched_len: Option<usize> = None;
            let mut matched_id: Option<String> = None;
            for prefix in &ordered {
                if !line[i..].starts_with(prefix.as_str()) {
                    continue;
                }
                // word boundary before the prefix
                let prev_ok = i == 0
                    || !line[..i]
                        .chars()
                        .last()
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false);
                if !prev_ok {
                    continue;
                }
                let tail_start = i + prefix.len();
                // tail char class differs per mode:
                //   IdToken    → [A-Z0-9_]+ (uppercase, digits, underscore)
                //   SectionPath → [A-Za-z0-9./-_]+ (alnum + . / - _; mirrors
                //                 `is_section_id_char` used by the section-citation axis)
                let tail_bytes = &bytes[tail_start..];
                let mut t = 0usize;
                while t < tail_bytes.len() {
                    let c = tail_bytes[t];
                    let is_tail = match tail_mode {
                        InventoryTailMode::IdToken => {
                            c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_'
                        }
                        InventoryTailMode::SectionPath => {
                            c.is_ascii_alphanumeric()
                                || c == b'.'
                                || c == b'/'
                                || c == b'-'
                                || c == b'_'
                        }
                    };
                    if is_tail {
                        t += 1;
                    } else {
                        break;
                    }
                }
                if t == 0 {
                    continue;
                }
                let tail_end = tail_start + t;
                // word boundary after the tail
                let next_ok = tail_end >= line.len()
                    || !line[tail_end..]
                        .chars()
                        .next()
                        .map(|c| c.is_alphanumeric() || c == '_')
                        .unwrap_or(false);
                if !next_ok {
                    continue;
                }
                // IdToken mode: tail must end in a digit (TC8 / ISO test-spec
                // convention; suppresses identifier-shaped false positives).
                // SectionPath mode: no digit-terminus — section paths can end
                // in a letter (`D.2.selectTransitions`) or a digit (`3.13`).
                if tail_mode == InventoryTailMode::IdToken && !tail_bytes[t - 1].is_ascii_digit() {
                    continue;
                }
                let id = format!("{}{}", prefix, &line[tail_start..tail_end]);
                matched_len = Some(prefix.len() + t);
                matched_id = Some(id);
                break; // longest-first ordering — first match wins
            }
            if let (Some(consumed), Some(id)) = (matched_len, matched_id) {
                seen.insert((line_idx + 1, id));
                // Advance past the consumed bytes — `peek/next` until we pass
                // `i + consumed`. char_indices keeps the iterator on valid
                // char boundaries even when prefix-length advance lands on
                // an ASCII byte (tails in both modes are ASCII by design).
                let target_byte = i + consumed;
                while let Some(&(k, _)) = chars.peek() {
                    if k < target_byte {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
        }
    }
    seen.into_iter().collect()
}

// ============================================================================
// Comment-only filtering.
//
// The scanner pattern-matches the entire file body, which surfaces
// string-literal fixtures (e.g. test markdown that contains "" as
// data) as false-positive citations. The comment-only layer strips
// non-comment chars to a single space so that line numbers are preserved
// 1:1 while only language-comment text reaches the citation extractor.
//
// This is a *heuristic*, not a full parser: ~95% accuracy with ~100 LOC,
// which keeps the 5-min setup promise (no AST dependency). Limitations:
// - Rust raw strings (`r"..."`, `r#"..."#`) treated as normal strings;
// - Python triple-quoted strings not recognized;
// - shell heredocs not recognized;
// - escape rules simplified (`\X` skips one char inside strings).
// These miss cases are deliberately deferred — when they bite, opt-out via
// `[plugins.set_equality_validator] comment_only = false` restores the whole-text scan.
// ============================================================================

/// Per-language comment recognition mode. The dispatcher in
/// [`comment_syntax_for`] maps file extensions onto these variants;
/// `Unknown` extensions fall through to whole-text scan (back-compat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentSyntax {
    /// C-family: `// line` + `/* block */` (Rust, C/C++, Go, JS/TS, Java, Kotlin, Swift, Scala).
    Slash,
    /// Hash-family: `# line` only, no block syntax (Python, shell, Ruby, TOML, YAML).
    Hash,
    /// No filtering — whole text is scanned (back-compat for unknown extensions).
    Unknown,
}

/// Map a file path's extension to the appropriate [`CommentSyntax`].
/// Case-insensitive on the extension. Files with no extension fall to
/// [`CommentSyntax::Unknown`].
pub fn comment_syntax_for(path: &Path) -> CommentSyntax {
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return CommentSyntax::Unknown,
    };
    match ext.as_str() {
        "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" | "go" | "js" | "ts"
        | "jsx" | "tsx" | "mjs" | "cjs" | "java" | "scala" | "kt" | "kts" | "swift" => {
            CommentSyntax::Slash
        }
        "py" | "sh" | "bash" | "zsh" | "rb" | "toml" | "yaml" | "yml" => CommentSyntax::Hash,
        _ => CommentSyntax::Unknown,
    }
}

/// How `comment_only = true` actually applied, per file (Round 856).
///
/// [`CommentSyntax::Unknown`] means the whole text is scanned, so the knob's
/// meaning depends on the file's extension — and that dependency was invisible.
/// It is load-bearing in both directions: it is why a consumer's `.scxml`
/// scenario files put their fact citations in prose and have them read, and it
/// is also why a citation-shaped token in a data file counts as a citation under
/// a `reject`-level severity. Reporting it costs a line; guessing costs a field
/// report.
///
/// Behaviour deliberately unchanged. Making `Unknown` scan nothing would drop
/// the prose citations this same round measured as real coverage.
///
/// # Why the unreadable files are counted apart (Round 860)
///
/// The Round 856 version of this report counted every unknown-extension file as
/// "read whole", including files the gate CANNOT read: the citation scan does
/// `read_to_string` and skips whatever is not valid UTF-8, so a `.class` or a
/// `.pyc` is walked and then dropped. The consumer's first run printed
/// `1138 of 1684 … read whole {"class": 368, "pyc": 23, "bin": 10, "jar": 1}`
/// and every one of those is a compiled artifact that no author cites. The count
/// overstated the exposure and named the wrong files, so the split is now
/// measured rather than inferred from the extension.
///
/// This is also what makes the number STABLE between a developer and CI, which
/// is the consumer's own framing and the sharper half of their report: a binary
/// artifact is excluded by the UTF-8 property it inherently lacks, not by a name
/// on a skip list that would drift the way every hand list here has.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct CommentModeCoverage {
    /// Files under the configured paths.
    pub scanned: usize,
    /// Of those, the ones with no known comment syntax AND readable as text —
    /// the files whose whole content the citation extractors see.
    pub whole_text: usize,
    /// Extension → count for those files, so the answer to "which of mine?" is
    /// in the report rather than in this function's source.
    pub whole_text_extensions: BTreeMap<String, usize>,
    /// Files the gate walks and cannot read as UTF-8 — no axis sees them, in
    /// any language. Counted because a configured path full of build artifacts
    /// is a real finding about the config (Round 860).
    pub unreadable: usize,
    /// Extension → count for those, same reason.
    pub unreadable_extensions: BTreeMap<String, usize>,
}

/// Compute [`CommentModeCoverage`] over the files a run reads.
///
/// Takes the READ SET rather than the configured paths, so a run narrowed by
/// [`PathScope`] reports the mode of what it actually read. A coverage report
/// that re-walks the configuration would describe a tree the judgement never
/// touched — the Round 777 defect with the roles swapped.
///
/// Reads each unknown-extension file, because "the gate reads this whole" and
/// "the gate cannot read this at all" are the two answers a reader needs and
/// only the bytes can tell them apart (Round 860). Files whose extension HAS a
/// comment syntax are not read — they are already accounted for and their
/// content changes nothing here.
#[must_use]
pub fn comment_mode_coverage(read_set: &[PathBuf]) -> CommentModeCoverage {
    let mut out = CommentModeCoverage::default();
    for abs in read_set {
        out.scanned += 1;
        if comment_syntax_for(abs) != CommentSyntax::Unknown {
            continue;
        }
        let ext = abs
            .extension()
            .map_or_else(|| "<none>".to_string(), |e| e.to_string_lossy().to_string());
        // The SAME question the gate asks of the same file, asked the same way:
        // `read_to_string` succeeds or the file is invisible to every axis.
        if std::fs::read_to_string(abs).is_ok() {
            out.whole_text += 1;
            *out.whole_text_extensions.entry(ext).or_insert(0) += 1;
        } else {
            out.unreadable += 1;
            *out.unreadable_extensions.entry(ext).or_insert(0) += 1;
        }
    }
    out
}

/// What the tree's own version control says about the files the gate reads
/// (Round 864).
///
/// # Why this axis exists
///
/// [`is_skipped_dir`] is a HAND LIST — `.`-prefixed, `target`, `node_modules` —
/// and it is the only thing keeping build output out of the read set. It knows
/// the two ecosystems this repo is written in and no others. Measured at this
/// round against our own configured paths: 6454 of 6554 files are ignored by
/// git, and the single word `target` is what removes them. The list is load
/// bearing and it has already drifted — a consumer's `__pycache__` walks
/// straight in (23 files, measured), and a generated Go tree has no
/// conventional directory name for any list to hold at all (126 files, 182
/// citations, reported from the field).
///
/// Rounds 856 and 860 each named part of this class and structurally could not
/// name the rest: Round 856 names files with no known comment syntax, Round 860
/// names files that are not valid UTF-8, and a generated `.go` file is neither.
///
/// The property read here has the shape Round 860 chose, one level up. A binary
/// is excluded by the UTF-8 property it inherently lacks; build output is named
/// by the tree under audit rather than by a list in this file. A list restates
/// the tree and then drifts from it in silence (the Round 777 rule); an answer
/// asked of the tree cannot.
///
/// # Why `--others --ignored` and not `check-ignore` (corrected, Round 865)
///
/// The predicate is untracked AND ignored. Round 864 justified that by claiming
/// `git check-ignore` reports a file committed with `git add -f` and this does
/// not — and that is FALSE. `check-ignore` consults the index by default and
/// stays silent on a tracked path; reporting it needs `--no-index`. Measured on
/// git 2.34.1 with an ignore-matching `add -f` file in the index: `check-ignore`
/// names only the untracked one, `check-ignore --no-index` names both, and
/// `ls-files --others --ignored` names only the untracked one. The two commands
/// agree, at every value of tracked-but-ignored, and the consumer caught the
/// claim by running it.
///
/// So the choice is OPERATIONAL, not a correctness argument: this states the
/// intent in the flags themselves, walks the tree in one call instead of being
/// handed a path list, and has no `--no-index` footgun a later edit could trip.
/// Nothing in the counts depends on it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VcsIgnoreAxis {
    /// The VCS answered for this tree.
    Measured {
        /// Files the caller handed in — the read set, or the excluded set, or
        /// any other set the config produced. Named for what it is rather than
        /// for one caller's use, since Round 866 gave it a second (the phrasing
        /// "scanned" belongs to the report, not to the answer).
        considered: usize,
        /// Of those, the ones the VCS calls build output, in path order.
        ignored: Vec<PathBuf>,
        /// Extension → count for those, so "which of mine?" is in the report.
        ignored_extensions: BTreeMap<String, usize>,
    },
    /// No VCS answer for this tree: git absent, not a repository, or the query
    /// failed. A DISTINCT value from zero ignored files, because a report where
    /// "nothing is build output" and "nobody asked" look alike is the silence
    /// Round 856 was written to remove.
    NotDetermined {
        /// What went wrong, in the VCS's own words where it gave any.
        reason: String,
    },
}

/// The `top` most common extensions, largest first, plus a count of the rest.
///
/// The read set usually names one or two; a consumer's excluded set named SIXTY,
/// and a report line that lists all of them is a line the next person scrolls
/// past. Ties break on the extension name, so the string is deterministic and
/// two runs of the same tree are diffable. The full map stays in `--json`.
#[must_use]
pub fn summarize_extensions(counts: &BTreeMap<String, usize>, top: usize) -> String {
    let mut pairs: Vec<(&String, &usize)> = counts.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let shown: Vec<String> = pairs
        .iter()
        .take(top)
        .map(|(k, v)| format!("{k:?}: {v}"))
        .collect();
    let rest = pairs.len().saturating_sub(top);
    if rest == 0 {
        format!("{{{}}}", shown.join(", "))
    } else {
        format!("{{{}, +{rest} more}}", shown.join(", "))
    }
}

/// The fewest directories under `root` that still contain every file in `files`.
///
/// Used to scope the VCS query, and DERIVED from the file set rather than from
/// the config that produced it: a scope handed in separately could name a tree
/// the set does not live under, and the query would then answer about the wrong
/// files while looking correct (the Round 777 rule that a reporter and a walk
/// must not be able to disagree). Anything extra the scope sweeps in is removed
/// by the intersection, so widening is safe and narrowing is not.
///
/// `BTreeSet` order puts an ancestor immediately before its descendants, so
/// keeping a directory only when it does not extend the last kept one collapses
/// the set in a single pass.
fn covering_roots(files: &BTreeSet<PathBuf>, root: &Path) -> Vec<PathBuf> {
    let dirs: BTreeSet<PathBuf> = files
        .iter()
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .collect();
    let mut out: Vec<PathBuf> = Vec::new();
    for d in dirs {
        if out.last().is_some_and(|prev| d.starts_with(prev)) {
            continue;
        }
        out.push(d);
    }
    out.iter()
        .filter_map(|p| p.strip_prefix(root).ok().map(Path::to_path_buf))
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

/// Compute [`VcsIgnoreAxis`] for a set of files the config already produced.
///
/// Takes the SET rather than the config that made it, because both callers
/// already hold one and re-deriving it here would let this axis disagree with
/// the walk it is reporting on (Round 777). `validate-code-refs` passes the read
/// set; `validate-workspace` passes [`ScanCoverage::excluded_files`], which is
/// the set Round 840 reads citations out of and therefore the set whose
/// developer-versus-CI difference decides whether that answer is stable.
///
/// Never fails: an unanswerable tree is [`VcsIgnoreAxis::NotDetermined`] carrying
/// the reason, following the Round 377 commit-scan precedent in this workspace,
/// which degrades rather than aborting when `git` cannot answer.
#[must_use]
pub fn vcs_ignored_among(root: &Path, files: &BTreeSet<PathBuf>) -> VcsIgnoreAxis {
    match ls_files_others_among(root, files, &["--ignored"]) {
        Ok(ignored) => VcsIgnoreAxis::Measured {
            considered: files.len(),
            ignored_extensions: extension_histogram(&ignored),
            ignored,
        },
        Err(reason) => VcsIgnoreAxis::NotDetermined { reason },
    }
}

/// WHAT THE RECORD DOES NOT HOLD, AMONG THE FILES THE GATE READ (Round 984).
///
/// A sibling of [`vcs_ignored_among`] with ONE argument different, and the
/// difference is the whole point: that axis asks which read files the VCS calls
/// BUILD OUTPUT (`--others --ignored`), and a work-in-progress source is neither
/// ignored nor in the record, so it is invisible there. This asks which read
/// files are simply ABSENT FROM THE RECORD (`--others`, ignore-respecting), which
/// is the population question — Round 978 established that a count reported off
/// the disk is one nobody with a clone can reproduce, and `validate-workspace`
/// prints this gate's file counts.
///
/// ADVISORY, AND THE DIRECTION IS WHY. Reading an untracked source is not a
/// defect: it is the gate biting at the writing moment, before the file is
/// staged, which is the cheapest place to catch a hallucinated citation. A
/// superset can only scan more, never miss. What was wrong was that the printed
/// count did not say how much of itself a clone would not have.
#[must_use]
pub fn vcs_absent_from_record_among(root: &Path, files: &BTreeSet<PathBuf>) -> VcsRecordAxis {
    match ls_files_others_among(root, files, &[]) {
        Ok(absent) => VcsRecordAxis::Measured {
            considered: files.len(),
            absent_extensions: extension_histogram(&absent),
            absent,
        },
        Err(reason) => VcsRecordAxis::NotDetermined { reason },
    }
}

/// `git ls-files --others [extra] --exclude-standard` intersected with `files`.
///
/// ONE query site for both axes. Two hand-copied bodies differing by a single
/// flag is how one of them silently stops matching the other's scoping or its
/// `-z` handling; the flag is the argument because the flag is the only thing
/// that differs.
fn ls_files_others_among(
    root: &Path,
    files: &BTreeSet<PathBuf>,
    extra: &[&str],
) -> Result<Vec<PathBuf>, String> {
    // Scope the query, so a workspace inside a large monorepo does not pay for
    // its siblings' untracked files.
    let pathspecs = covering_roots(files, root);
    let mut args = vec!["ls-files", "--others"];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--exclude-standard", "-z"]);
    let output = std::process::Command::new("git")
        .args(&args)
        .arg("--")
        .args(&pathspecs)
        .current_dir(root)
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            let said = String::from_utf8_lossy(&o.stderr);
            let first = said
                .lines()
                .next()
                .unwrap_or("no message")
                .trim()
                .to_string();
            return Err(format!("git exited {}: {first}", o.status));
        }
        Err(e) => return Err(format!("git could not be run: {e}")),
    };
    // `-z`, because a path may contain a newline and a line-split report would
    // then name a file that does not exist. Paths arrive relative to the cwd the
    // command ran in, which is `root`.
    let mut hit: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| root.join(s))
        .filter(|p| files.contains(p))
        .collect();
    hit.sort();
    hit.dedup();
    Ok(hit)
}

fn extension_histogram(paths: &[PathBuf]) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for p in paths {
        let ext = p
            .extension()
            .map_or_else(|| "<none>".to_string(), |e| e.to_string_lossy().to_string());
        *out.entry(ext).or_insert(0) += 1;
    }
    out
}

/// What the tree's own version control says is ABSENT FROM THE RECORD among a
/// set of files (Round 984). Three states for the Round 856 reason: "none
/// absent" and "nobody asked" are different facts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VcsRecordAxis {
    Measured {
        /// Files the caller handed in — here, the set the citation gate read.
        considered: usize,
        /// Of those, the ones no clone of this repository would have.
        absent: Vec<PathBuf>,
        /// Extension → count for those.
        absent_extensions: BTreeMap<String, usize>,
    },
    NotDetermined {
        reason: String,
    },
}

/// Which subtrees speak ANOTHER document's `§N.M` numbering (Round 867).
///
/// # Why a FILE carries this, and not a token
///
/// [`SetEqualityValidatorConfig::external_section_prefixes`] and its bare sibling
/// answer the same question one TOKEN at a time, keyed on the citing prose:
/// `UAX #9 §6.7` names its document, so the registry can skip it. That reaches
/// only a citation whose author named the document, and inside another project's
/// own documents the numbering is IMPLICIT — a vendored `SCE_MESH.md` says
/// `per §6.2` about a section of its own, with nothing in the token to match.
/// (Both quotations are in code spans on purpose: this file is scanned, and a
/// bare one would be exactly the misattribution described here. Our own gate
/// caught the first draft of this paragraph.) Two consumers on
/// this machine sat on exactly that: one was rejected for four such citations and
/// pinned itself behind Round 840 to stay green, the other for five whose ids are
/// SINGLE DIGITS, so every `§N` in its 8727 vendored files is a collision
/// candidate. Only where the file lives distinguishes them.
///
/// # Why derived, and not declared
///
/// The shape the consumer asked for was a `numbering = "foreign"` attribute on a
/// `scan_exclusions` entry: a claim nothing can check, on the wrong axis — the
/// same misattribution is live on the SCANNED side, where a third consumer
/// suppresses it with seven hand-registered prefixes and no exclusion exists at
/// all. And this codebase has the Round 777 and Round 783 record of
/// hand-maintained lists drifting from the tree they describe, its own included
/// (Round 864). The tree's own VCS already answers it for the case that has
/// consumers: `git ls-files --stage` marks a submodule with mode `160000`, and a
/// file under that path belongs to ANOTHER REPOSITORY. Since a store is
/// per-workspace, "another repository" and "not this store's numbering" are one
/// statement here.
///
/// # Why three states, and why the third attributes nothing
///
/// "No foreign subtree" and "no VCS to ask" are different facts, and a report
/// where they look alike is the silence Round 856 was written to remove — so
/// [`NumberingOriginAxis::NotDetermined`] is its own value, as Round 864
/// established for the sibling axis. It yields NO foreign subtrees, which keeps
/// the gate exactly as tight as it was before this axis existed: unlike every
/// other axis in this family, this one LOOSENS — it makes citations disappear —
/// so an unanswerable VCS must never be able to un-gate anything.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NumberingOriginAxis {
    /// The VCS answered for this tree.
    Measured {
        /// Workspace-relative subtree paths, in path order.
        foreign_subtrees: Vec<PathBuf>,
    },
    /// No VCS answer: git absent, not a repository, or the query failed.
    NotDetermined {
        /// What went wrong, in the VCS's own words where it gave any.
        reason: String,
    },
}

impl NumberingOriginAxis {
    /// Ask the tree's own VCS which subtrees are other repositories.
    ///
    /// Never fails, following the Round 377 precedent in this workspace: a tree
    /// git cannot answer for degrades to [`Self::NotDetermined`] rather than
    /// aborting a validate run.
    #[must_use]
    pub fn derive(root: &Path) -> Self {
        let output = std::process::Command::new("git")
            .args(["ls-files", "--stage", "-z"])
            .current_dir(root)
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            Ok(o) => {
                let said = String::from_utf8_lossy(&o.stderr);
                let first = said
                    .lines()
                    .next()
                    .unwrap_or("no message")
                    .trim()
                    .to_string();
                return Self::NotDetermined {
                    reason: format!("git exited {}: {first}", o.status),
                };
            }
            Err(e) => {
                return Self::NotDetermined {
                    reason: format!("git could not be run: {e}"),
                };
            }
        };
        // `<mode> <object> <stage>\t<path>`, records NUL-separated. Split on the
        // TAB rather than on whitespace: `-z` leaves paths unquoted, so a
        // submodule directory containing a space would otherwise be truncated.
        // Measured on git 2.34.1, including that space case.
        let mut foreign_subtrees: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .filter_map(|record| record.split_once('\t'))
            .filter(|(meta, _)| meta.starts_with("160000 "))
            .map(|(_, path)| PathBuf::from(path))
            .collect();
        foreign_subtrees.sort();
        foreign_subtrees.dedup();
        Self::Measured { foreign_subtrees }
    }

    /// The subtrees to attribute elsewhere — NONE when nobody could be asked.
    #[must_use]
    pub fn foreign_subtrees(&self) -> &[PathBuf] {
        match self {
            Self::Measured { foreign_subtrees } => foreign_subtrees,
            Self::NotDetermined { .. } => &[],
        }
    }
}

/// What one file's `§N.M` tokens turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct FileCitations {
    /// Tokens that cite THIS store, `(line, section_id)`.
    pub cited: Vec<(usize, String)>,
    /// The foreign subtree this file lives under. `Some` implies `cited` is
    /// empty, by construction.
    pub foreign_origin: Option<PathBuf>,
    /// Citation-shaped tokens the foreign origin stopped attributing here.
    ///
    /// Counted AFTER the prefix registries and the namespace scope, so a
    /// registered `UAX #9` reference is not among them. Not all of them would
    /// have RESOLVED — in a consumer tree most name nothing in this store and
    /// would have been hallucination-class violations — so this is the size of
    /// what went quiet rather than a count of coverage lost. That is the number
    /// the reader needs: it is what an over-broad derivation would hide.
    pub foreign_skipped: usize,
}

/// THE answer to "does this `§N.M` token, in THIS file, cite this store"
/// (Round 867).
///
/// # Why one resolver
///
/// Five production readers asked that question with THREE different predicates:
/// [`SetEqualityValidator::scan`], [`SetEqualityValidator::propose_implementations`]
/// and [`SetEqualityValidator::citation_index`] honoured both the prefix
/// registries and `section_namespace`; [`swallowed_citations`] honoured the
/// prefixes and not the namespace; [`scan_section_decay`] honoured neither. Both
/// divergences measured zero on every tree available when they were found — the
/// decay axis needs a superseded section and no workspace here sets a namespace —
/// so they were latent rather than live, and recorded as latent. A half-enforced
/// predicate is still no predicate: it is the shape Round 305 caught when two
/// write paths to one field carried different caps, and the parity test that
/// answered it there is the substrate here.
pub struct CitationAttribution<'a> {
    root: &'a Path,
    prefixes_numeric: &'a [String],
    prefixes_bare: &'a [String],
    namespace: Option<&'a str>,
    comment_only: bool,
    origin: NumberingOriginAxis,
}

impl<'a> CitationAttribution<'a> {
    /// Build the one attribution for a tree. Callers derive the origin ONCE per
    /// command and pass this down, so no two axes in a run can hold different
    /// answers to the same question.
    #[must_use]
    pub fn new(
        root: &'a Path,
        config: &'a SetEqualityValidatorConfig,
        origin: NumberingOriginAxis,
    ) -> Self {
        Self {
            root,
            prefixes_numeric: config.external_section_prefixes.as_slice(),
            prefixes_bare: config.external_section_prefixes_bare.as_slice(),
            namespace: config.section_namespace.as_deref(),
            comment_only: config.comment_only,
            origin,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.root
    }

    #[must_use]
    pub fn origin(&self) -> &NumberingOriginAxis {
        &self.origin
    }

    /// The foreign subtree `file` lives under, if any.
    ///
    /// Component-wise [`Path::starts_with`], never a string prefix: `vendor/sce`
    /// must not swallow `vendor/scenarios`, which is the collapse Round 866 had
    /// to inject against on the sibling axis.
    #[must_use]
    fn foreign_subtree_of(&self, file: &Path) -> Option<PathBuf> {
        let rel = file.strip_prefix(self.root).unwrap_or(file);
        self.origin
            .foreign_subtrees()
            .iter()
            .find(|sub| rel.starts_with(sub))
            .cloned()
    }

    /// Attribute the tokens in `content`, which the caller has already read and
    /// stripped the way the gate reads it.
    #[must_use]
    pub fn citations_in(&self, file: &Path, content: &str) -> FileCitations {
        let foreign_origin = self.foreign_subtree_of(file);
        let mut cited = Vec::new();
        let mut foreign_skipped = 0usize;
        for (line, section_id) in
            extract_section_citations(content, self.prefixes_numeric, self.prefixes_bare)
        {
            // Namespace scope — a citation whose namespace segment (the part
            // before the first `-`) is not exactly the declared one belongs to a
            // different ledger of the same store (Round 376).
            if let Some(ns) = self.namespace {
                if citation_namespace(&section_id) != ns {
                    continue;
                }
            }
            if foreign_origin.is_some() {
                foreign_skipped += 1;
                continue;
            }
            cited.push((line, section_id));
        }
        FileCitations {
            cited,
            foreign_origin,
            foreign_skipped,
        }
    }

    /// Read `file` the way the gate reads it, then attribute it.
    ///
    /// `None` = the gate cannot read it either, so its citations were never
    /// coverage (the Round 854 rule, with the Round 860 correction that the
    /// question is `read_to_string` and not an extension guess).
    #[must_use]
    pub fn attribute_file(&self, file: &Path) -> Option<FileCitations> {
        let raw = std::fs::read_to_string(file).ok()?;
        let content = if self.comment_only {
            strip_to_comments(&raw, comment_syntax_for(file))
        } else {
            raw
        };
        Some(self.citations_in(file, &content))
    }
}

/// What the numbering-origin derivation removed, printed every run.
///
/// # Why this axis must be loud where its siblings may be silent
///
/// Every other axis in this family TIGHTENS: it finds a citation the gate was
/// missing. This one loosens — a subtree derived foreign makes its citations
/// vanish — so a wrong verdict silently un-gates real ones, which is Round 840's
/// own class inverted. The counted line is what makes that visible instead of
/// quiet, and it is also the discovery path for the one shape deliberately not
/// built: a monorepo that shares numbering across a submodule reads this line and
/// asks for an override, rather than the override being speculated into existence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NumberingOriginReport {
    pub axis: NumberingOriginAxis,
    /// Files handed in — the set whose attribution this describes.
    pub files_considered: usize,
    /// Of those, the ones living under a foreign subtree.
    pub files_foreign: usize,
    /// Citation-shaped tokens no longer attributed to this store. See
    /// [`FileCitations::foreign_skipped`] for what the number does and does not
    /// claim.
    pub citations_skipped: usize,
    /// Subtree → citations removed, so one stray vendored comment reads
    /// differently from a whole tree going quiet.
    pub skipped_per_subtree: BTreeMap<String, usize>,
}

/// Compute [`NumberingOriginReport`] over a set of files the config produced.
///
/// Takes the SET rather than the config that made it, for the Round 866 reason:
/// a reporter that re-derives its own file list is free to disagree with the walk
/// it is reporting on.
#[must_use]
pub fn numbering_origin_coverage(
    attribution: &CitationAttribution,
    files: &BTreeSet<PathBuf>,
) -> NumberingOriginReport {
    let mut files_foreign = 0usize;
    let mut citations_skipped = 0usize;
    let mut skipped_per_subtree: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        let Some(attributed) = attribution.attribute_file(file) else {
            continue;
        };
        if let Some(subtree) = attributed.foreign_origin {
            files_foreign += 1;
            *skipped_per_subtree
                .entry(subtree.to_string_lossy().to_string())
                .or_insert(0) += attributed.foreign_skipped;
            citations_skipped += attributed.foreign_skipped;
        }
    }
    NumberingOriginReport {
        axis: attribution.origin().clone(),
        files_considered: files.len(),
        files_foreign,
        citations_skipped,
        skipped_per_subtree,
    }
}

/// Replace non-comment characters with spaces so citation extractors see
/// only comment text. Line breaks are preserved 1:1 so line numbers stay
/// accurate. Unknown syntax returns the input unchanged.
pub fn strip_to_comments(content: &str, syntax: CommentSyntax) -> String {
    match syntax {
        CommentSyntax::Unknown => content.to_string(),
        CommentSyntax::Slash => strip_slash(content),
        CommentSyntax::Hash => strip_hash(content),
    }
}

fn strip_slash(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_block = false;
    for (line_idx, line) in content.lines().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        let mut in_string = false;
        let mut chars = line.char_indices().peekable();
        while let Some((_, c)) = chars.next() {
            if in_block {
                if c == '*' && chars.peek().map(|(_, n)| *n) == Some('/') {
                    out.push('*');
                    chars.next();
                    out.push('/');
                    in_block = false;
                } else {
                    out.push(c);
                }
                continue;
            }
            if in_string {
                if c == '\\' {
                    out.push(' ');
                    if chars.next().is_some() {
                        out.push(' ');
                    }
                    continue;
                }
                if c == '"' {
                    in_string = false;
                }
                out.push(' ');
                continue;
            }
            // Code state — look for comment openers.
            if c == '/' && chars.peek().map(|(_, n)| *n) == Some('/') {
                out.push('/');
                chars.next();
                out.push('/');
                for (_, rest) in chars.by_ref() {
                    out.push(rest);
                }
                break;
            }
            if c == '/' && chars.peek().map(|(_, n)| *n) == Some('*') {
                out.push('/');
                chars.next();
                out.push('*');
                in_block = true;
                continue;
            }
            if c == '"' {
                in_string = true;
                out.push(' ');
                continue;
            }
            out.push(' ');
        }
        // EOL — single-line strings auto-close (we don't carry in_string
        // across lines; multi-line raw strings are an accepted miss case).
    }
    out
}

fn strip_hash(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for (line_idx, line) in content.lines().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        let mut in_single = false;
        let mut in_double = false;
        let mut chars = line.char_indices().peekable();
        while let Some((_, c)) = chars.next() {
            if in_single || in_double {
                if c == '\\' {
                    out.push(' ');
                    if chars.next().is_some() {
                        out.push(' ');
                    }
                    continue;
                }
                if in_single && c == '\'' {
                    in_single = false;
                } else if in_double && c == '"' {
                    in_double = false;
                }
                out.push(' ');
                continue;
            }
            if c == '#' {
                out.push('#');
                for (_, rest) in chars.by_ref() {
                    out.push(rest);
                }
                break;
            }
            if c == '"' {
                in_double = true;
                out.push(' ');
                continue;
            }
            if c == '\'' {
                in_single = true;
                out.push(' ');
                continue;
            }
            out.push(' ');
        }
    }
    out
}

/// Normalize a changelog identifier of EITHER stored shape to the citation
/// shape `<prefix><number>` — the one resolver for "which round is this?".
///
/// The ledger holds two key shapes: short-form (`"Round 292"`) and long-form
/// (`"Round 293 — <title>"`). A citation names only the number, so both must
/// reduce to the same key before comparison. This accepts a stored key OR a
/// citation, which is what makes it a resolver rather than a formatter: pass
/// both sides through it and compare the results.
///
/// Returns `None` when `s` does not carry the configured prefix followed by a
/// number — such a string cannot collide with the cited shape.
///
/// Round 638 — this rule previously lived inline in the code-refs gate AND, in
/// a DIFFERENT and BROKEN form, in CLAUDE.md's hand-executed citation-hygiene
/// procedure (which prefix-matched `"Round NNN "` WITH A TRAILING SPACE, so it
/// answered "hallucinated" for all 96 short-form entries — a quarter of the
/// ledger). Two statements of one rule is the drift class Round 636 convicted;
/// this is the single home, and prose now names the verb instead of restating
/// the rule.
pub fn normalize_entry_citation(prefix: &str, s: &str) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }
    let rest = s.strip_prefix(prefix)?;
    let num = scan_round_number(rest)?;
    Some(format!("{}{}", prefix, num))
}

/// Read `<digits>(.<digits>)?` from the start of `s`. Returns the
/// matched substring, or `None` if `s` does not start with a digit.
/// Trailing `.` without fractional digits is not consumed.
fn scan_round_number(s: &str) -> Option<String> {
    let mut chars = s.chars().peekable();
    let mut buf = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            buf.push(c);
            chars.next();
        } else {
            break;
        }
    }
    if buf.is_empty() {
        return None;
    }
    if chars.peek() == Some(&'.') {
        let mut probe = chars.clone();
        probe.next();
        let mut frac = String::new();
        while let Some(&c) = probe.peek() {
            if c.is_ascii_digit() {
                frac.push(c);
                probe.next();
            } else {
                break;
            }
        }
        if !frac.is_empty() {
            buf.push('.');
            buf.push_str(&frac);
        }
    }
    Some(buf)
}

/// full Path B scan: Round NNN axis + §<id> axis +
/// bidirectional set-equality check + orphan ledger suppression for
/// `OrphanKind::CodeCitation` rows.
///
/// Algorithm (per scanned file F):
/// 1. Extract `<prefix>NNN` citations → `Missing` (or `Decay` under
/// `filter_id`) using existing /258 path.
/// 2. Extract `§<id>` citations:
/// - `<id>` not in `store.atomic_section_id_set()` → `SectionMissing`
/// - `<id>` exists but F not in `§<id>.bindings` files →
/// `CitationUnbound`
/// - else OK (record F in `cited_by[<id>]` for step 3)
/// 3. After all files scanned, walk `store.sections`. For each §X, for
/// each `Binding { file, symbol, kind }` in `§X.bindings`:
/// if `file` ∉ `cited_by[X]` → `BindingUnbacked`.
/// 4. Same walk: for each §X with `decision_status != Removed` and
/// zero `implements` bindings → `ImplementationMissing` (spec-side
/// coverage axiom — Round 269).
///
/// `filter_id` is the decay-scan toggle. When `Some`, only
/// Round NNN citations matching the filter are surfaced (as `Decay`);
/// all other Round NNN citations are suppressed, and the §<id> axis
/// stays silent for symmetry (a Superseded-decision cascade caller is
/// asking "where is this entry_id mentioned?", not "audit the whole
/// store" — keep the surface narrow). Steps 3 and 4 are also skipped
/// under decay-filter mode for the same surface-narrowing reason.
///
/// `orphan_ledger` rows with `kind = CodeCitation` suppress any §<id>
/// violation matching `(from = file, to = id)`. Other kinds are
/// ignored by this scanner (they belong to the atomic-internal /
/// markdown axes).
///
/// `comment_only` toggles the comment-only filtering layer.
/// When `true`, each file's content is passed through [`strip_to_comments`]
/// (per-extension dispatch via [`comment_syntax_for`]) so the citation
/// extractor only sees comment text. Unknown extensions fall through to
/// whole-text scan regardless of the flag.
/// Scanner with all four cite axes wired in:
///
/// 1. `Round NNN` axis — `<entry_id_prefix><number>` (decay-aware via
///    `filter_id`).
/// 2. `§<id>` axis with two external-standard skip modes —
///    *numeric* (`<PREFIX> <NUMERIC> §<id>`) via
///    `external_section_prefixes_numeric` and *bare*
///    (`<PREFIX> §<id>` doc-name only) via `external_section_prefixes_bare`.
/// 3. Inventory axis with two tail shapes — *opaque-ID*
///    (`<prefix><[A-Z0-9_]+ ending in digit>`) via `inventory_prefixes`
///    and *section-path* (`<prefix><[A-Za-z0-9./-_]+>`) via
///    `inventory_path_prefixes`. Both feed the same `InventoryEntry`
///    store and share `severity_inventory`.
/// 4. Bidirectional set-equality (Path B) — `§X.bindings` files
///    vs cited-by sets — surfaces `CitationUnbound`,
///    `BindingUnbacked`, and `ImplementationMissing` (R269
///    coverage axiom).
///
/// `orphan_ledger` rows with `kind = CodeCitation` suppress
/// section-citation-axis violations and rows with `kind =
/// InventoryCitation` (R285) suppress inventory-axis violations.
///
/// Pass an empty slice on any axis to disable it. `filter_id` is the
/// decay-scan toggle (Steps 3-4 stay silent under decay mode for
/// surface-narrowing).
/// File extension → the language ID used as the
/// `[plugins.symbol_resolver.<lang>]` key. Round 306 wired `SymbolResolver`
/// plugins per extension; Round 855 made this a TABLE rather than a match, so
/// the set of legal config keys is derived from it ([`symbol_axis_languages`])
/// instead of being restated somewhere free to drift (the Round 777 rule).
///
/// `.c` was absent until Round 855 — reported from the field, where a C runtime
/// of 226 files would have taken file-level binding on its `.c` files and
/// symbol-level on its `.h` files, with nothing saying so while
/// `severity_binding = reject` read as symbol-level throughout. The omission
/// was visible in this file: [`comment_syntax_for`] one screen away has always
/// known `.c`.
///
/// `.kt` / `.kts` were absent until Round 1155, and they were the SAME omission
/// with a longer reach: `comment_syntax_for` has always known both, so a Kotlin
/// file's citations were scanned and spell-checked while this table said the
/// extension mapped to no language at all — the census's "extension maps to no
/// language" bucket, which reads as a file the symbol axis was never meant to
/// judge rather than as a language nobody had wired. The consumer whose spec
/// ledger enrols a Kotlin runtime wrote that sentence out by hand.
///
/// A ROW HERE IS A PROMISE THIS BUILD CAN KEEP. Since Round 1154 the reach
/// contract requires `languages_without_backend` to be EMPTY, so adding a row
/// for a language with no resolver fails at the moment the row is added rather
/// than at the moment a consumer notices their citations took file-level
/// binding.
const SYMBOL_AXIS_EXTENSIONS: &[(&str, &str)] = &[
    ("c", "cpp"),
    ("cc", "cpp"),
    ("cpp", "cpp"),
    ("cxx", "cpp"),
    ("go", "go"),
    ("h", "cpp"),
    ("hh", "cpp"),
    ("hpp", "cpp"),
    ("hxx", "cpp"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("py", "python"),
    ("rs", "rust"),
];

/// The extension table itself, `(extension, language)` in extension order.
///
/// Published because the consumer's question is often about a FILE rather than
/// a language: SCE's spec ledger enrols a Kotlin runtime, and the sentence they
/// needed was "`.kt` is not on the table", which no derived language set can
/// say. Reading it out of this repository's source is what they did instead,
/// and prose about the inside of another tree decays without a reader.
#[must_use]
pub fn symbol_axis_extensions() -> &'static [(&'static str, &'static str)] {
    SYMBOL_AXIS_EXTENSIONS
}

/// Every language ID the extension table can produce — the legal key set for
/// `[plugins.symbol_resolver.<lang>]`. Derived, so a resolver keyed to a
/// language no file can ever map to is refusable rather than dead config.
#[must_use]
pub fn symbol_axis_languages() -> BTreeSet<&'static str> {
    SYMBOL_AXIS_EXTENSIONS
        .iter()
        .map(|(_, lang)| *lang)
        .collect()
}

/// Map a file path to its symbol-axis language, or `None` when no resolver
/// could apply. A `None` is NOT silent: [`SetEqualityValidator::symbol_axis_coverage`]
/// counts the file under its extension and the gate prints that every run.
///
/// Case-insensitive on the extension, like [`comment_syntax_for`] — a `.H`
/// header was skipped by one of the two and read by the other.
fn lang_for_file(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    SYMBOL_AXIS_EXTENSIONS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, lang)| *lang)
}

/// The symbols a citation must resolve into, or `None` when this citation is
/// not one the symbol axis puts to a resolver.
///
/// ONE DEFINITION OF "A RESOLVER CALL". [`SetEqualityValidator::scan`] judges
/// with it and [`SetEqualityValidator::symbol_axis_coverage`] counts with it, so
/// the cost the census publishes is the cost the gate will pay. A census free to
/// disagree with the gate about what a resolver call is would be pricing some
/// other tool, and the consumer whose payoff estimate needs that ratio has no
/// way to tell.
///
/// `index` is `section_id -> file -> {symbols}`, built from the citation-edge
/// bindings that record a `symbol`. A section legitimately has more than one
/// symbol in a file, so membership — not equality — is the test at the call
/// site.
fn symbol_expectation<'a>(
    section_id: &str,
    rel: &str,
    index: &'a BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>>,
) -> Option<&'a BTreeSet<&'a str>> {
    index.get(section_id).and_then(|m| m.get(rel))
}

/// RFC-002 FR-3 symbol-level enforcement index — `section_id -> file ->
/// {symbols}` over every citation-edge binding that records a `symbol`.
///
/// Built here rather than at each reader, for the reason [`symbol_expectation`]
/// exists: the scan and the census must not be able to disagree about which
/// citations the symbol axis judges. A `verifies` binding is excluded because it
/// points at an externally mapped test artifact rather than at a `§<id>`-citing
/// file, so it neither defends a citation nor enters the set-equality.
fn symbols_by_section_file(
    snapshot: &mnemosyne_core::AtomicSnapshot,
) -> BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> {
    snapshot
        .sections
        .iter()
        .map(|(sid, sec)| {
            let mut m: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
            for b in &sec.bindings {
                if !is_citation_edge(b.kind) {
                    continue;
                }
                if let Some(s) = b.symbol.as_deref() {
                    m.entry(b.file.as_str()).or_default().insert(s);
                }
            }
            (sid.as_str(), m)
        })
        .collect()
}

/// First-class Validator plugin embodying the set-equality citation
/// audit. Routes through `PluginRegistry` so the validator-class trait
/// surface is reached from production code (`cmd_validate_code_refs`
/// constructs, registers, and dispatches), closing R306 carry item #1.
///
/// Field rationale:
/// - `config` — paths / severity / comment_only / inventory + external
///   prefix axes (in-place from `SetEqualityValidatorConfig`).
/// - `entry_id_prefix` — schema-driven (`<entry_id_prefix><number>`
///   cite shape). Cached at construction so `Validator::validate` does
///   not re-discover from `ValidationContext`.
/// - `orphan_ledger` — workspace-config-driven `[[orphan_ledger]]` rows.
/// - `symbol_resolvers` — BindingClass plugin map keyed by language ID
///   (`rust`/`python`/`go`). Owned (not registry-borrowed) so
///   `Validator::validate` is self-contained — no registry parameter on
///   `ValidationContext`. Empty map = symbol axis disabled.
/// - `filter_id` — decay-cascade caller's per-instance toggle. `None`
///   for normal runs; `Some(<entry_id>)` for cascade-mode callers
///   narrowing to one entry's decay scan.
/// - `path_scope` — file-list narrowing (`validate-code-refs --paths`).
///   `None` for whole-tree runs. Narrows the READ SET, never the
///   judgement applied to a file, so the scoped answer stays equal to
///   the whole run's answer about those files; the spec-side axes it
///   cannot judge are named in [`Self::axis_verdicts`] rather than
///   reported as zero.
pub struct SetEqualityValidator {
    pub config: SetEqualityValidatorConfig,
    pub entry_id_prefix: String,
    pub orphan_ledger: Vec<OrphanLedgerEntry>,
    pub symbol_resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>>,
    pub filter_id: Option<String>,
    pub path_scope: Option<PathScope>,
}

impl SetEqualityValidator {
    /// The files this run reads: the configured walk, narrowed by
    /// [`Self::path_scope`] when one is set.
    ///
    /// Every read inside this type goes through here, so the scan, the symbol
    /// census and the citation index cannot disagree about what the run
    /// covered — a coverage report free to describe a wider tree than the
    /// judgement did is the Round 777 defect with the roles swapped.
    ///
    /// # Errors
    ///
    /// Whatever [`walk_paths`] fails with.
    pub fn read_set(&self, workspace_root: &Path) -> std::io::Result<Vec<PathBuf>> {
        let files = walk_paths(workspace_root, &self.config.paths)?;
        Ok(match &self.path_scope {
            None => files,
            Some(scope) => scope.select(workspace_root, files),
        })
    }

    /// What [`Self::path_scope`] selected and what it did not reach — `None`
    /// for an unscoped run, so a report cannot print an empty scope block that
    /// reads like a narrowing nobody asked for.
    ///
    /// Measured against the UNSCOPED walk: the question "does this gate read
    /// that file at all" is exactly the difference between the two sets, and it
    /// is the question a commit hook handing over a changed-file list needs
    /// answered.
    ///
    /// # Errors
    ///
    /// Whatever [`walk_paths`] fails with.
    pub fn scope_coverage(
        &self,
        workspace_root: &Path,
    ) -> std::io::Result<Option<PathScopeCoverage>> {
        match &self.path_scope {
            None => Ok(None),
            Some(scope) => {
                let full = walk_paths(workspace_root, &self.config.paths)?;
                Ok(Some(scope.coverage(workspace_root, &full)))
            }
        }
    }

    /// Which axes THIS run judges, and why not for the rest.
    ///
    /// The single decision point: [`Self::scan`] takes every one of its skips
    /// by asking this, and the report prints its `not_judged` list from the
    /// same map. A count may therefore be published only for an axis this map
    /// calls judged — `0` means measured-and-clean, everywhere, in every mode.
    #[must_use]
    pub fn axis_verdicts(&self) -> AxisVerdicts {
        let decay_mode = self.filter_id.is_some();
        let scoped = self.path_scope.is_some();
        AxisVerdicts(
            AuditAxis::all()
                .into_iter()
                .map(|axis| {
                    let reason = if decay_mode && axis != AuditAxis::Decay {
                        Some(NotJudged::DecayFilter)
                    } else if !decay_mode && axis == AuditAxis::Decay {
                        Some(NotJudged::NoDecayFilter)
                    } else if scoped && axis.side() == AuditSide::Spec {
                        Some(NotJudged::PathScope)
                    } else if axis == AuditAxis::SymbolMismatch && self.symbol_resolvers.is_empty()
                    {
                        Some(NotJudged::NoResolver)
                    } else if self.axis_severity_unset(axis) {
                        Some(NotJudged::AxisDisabled)
                    } else {
                        None
                    };
                    (axis, reason)
                })
                .collect(),
        )
    }

    /// Whether `axis` is one of the opt-in axes and its severity is unset.
    ///
    /// Exhaustive, so an axis added later must state whether it is opt-in
    /// rather than inheriting "always on" by omission.
    fn axis_severity_unset(&self, axis: AuditAxis) -> bool {
        match axis {
            AuditAxis::VerificationMissing => self.config.severity_verification.is_none(),
            AuditAxis::MisclassifiedCoverage => self.config.severity_classification.is_none(),
            AuditAxis::BlanketVerifies => self.config.severity_blanket.is_none(),
            AuditAxis::ProseFactAssertion => self.config.severity_prose_fact_assertion.is_none(),
            // Always judged when the mode allows it. `symbol_mismatch` is here
            // rather than with the opt-ins because there is no severity that
            // turns it off — how far it reaches is reported as coverage by
            // `symbol_axis_coverage` (Round 855) instead of as one on/off bit.
            // Whether it reaches AT ALL is a separate question, answered one
            // branch up in `axis_verdicts` by `NotJudged::NoResolver`: an empty
            // resolver map is the one state in which this axis judges nothing,
            // and it used to publish that as `0`.
            AuditAxis::Missing
            | AuditAxis::Decay
            | AuditAxis::SectionMissing
            | AuditAxis::CitationUnbound
            | AuditAxis::SymbolMismatch
            | AuditAxis::InventoryMissing
            | AuditAxis::InventoryDeprecated
            | AuditAxis::BindingUnbacked
            | AuditAxis::ImplementationMissing => false,
        }
    }

    /// Rich scan returning `CodeRefViolation`. The plugin trait method
    /// `validate(ctx)` calls into this and maps each variant to a
    /// `ValidationFinding` for cross-plugin dispatch; direct callers
    /// (the decay-cascade trigger after a Superseded transition) keep
    /// the structured shape.
    ///
    /// Algorithm: Round NNN axis + §<id> axis with two external-skip
    /// modes + Inventory axis with two tail shapes + bidirectional
    /// set-equality (Path B) + spec-side coverage axiom. See
    /// [`CodeRefViolation`] doc for per-variant evidence.
    pub fn scan(
        &self,
        attribution: &CitationAttribution,
        snapshot: &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        // The tree comes from the attribution rather than beside it (Round 867):
        // a root passed separately could name a tree the numbering origin was not
        // derived from, and the answer would be wrong while looking right.
        let workspace_root = attribution.root();
        let prefix = self.entry_id_prefix.as_str();
        let filter_id = self.filter_id.as_deref();
        // Every skip below asks this map rather than re-deriving its condition,
        // so the report's `not_judged` list and the code that did the skipping
        // are the same decision read twice.
        let verdicts = self.axis_verdicts();
        let comment_only = self.config.comment_only;
        let inventory_prefixes = self.config.inventory_prefixes.as_slice();
        let inventory_path_prefixes = self.config.inventory_path_prefixes.as_slice();
        // Empty resolver map = symbol axis silently skipped; identical
        // semantic to the pre-R307 `Option<&BTreeMap>` shape where None
        // bypassed lookup entirely.
        let symbol_resolvers_opt = if self.symbol_resolvers.is_empty() {
            None
        } else {
            Some(&self.symbol_resolvers)
        };
        let orphan_ledger = self.orphan_ledger.as_slice();

        // valid_entry_ids must match the shape produced by `extract_citations`,
        // which returns `<prefix><number>` (e.g. "Round 293"). Both ledger key
        // shapes normalize through the one shared resolver.
        let valid_entry_ids: BTreeSet<String> = snapshot
            .changelog_entry_ids
            .iter()
            .filter_map(|k| normalize_entry_citation(prefix, k))
            .collect();
        let section_id_set = &snapshot.section_ids_with_implied_parents;

        // Pre-index §X.bindings files by section_id for O(log n) per-cite
        // membership check + step 3 universe enumeration. Restricted to
        // citation-edge kinds (implements OR references): a verifies binding
        // points at an externally-mapped test artifact, not a §<id>-citing
        // file, so it neither defends a citation nor enters the set-equality.
        let impl_files_by_section: BTreeMap<&str, BTreeSet<&str>> = snapshot
            .sections
            .iter()
            .map(|(sid, sec)| {
                let files: BTreeSet<&str> = sec
                    .bindings
                    .iter()
                    .filter(|b| is_citation_edge(b.kind))
                    .map(|b| b.file.as_str())
                    .collect();
                (sid.as_str(), files)
            })
            .collect();

        // RFC-002 FR-3 symbol-level enforcement index — section_id → file →
        // {symbols} (every Implementation.symbol that is Some). A section is
        // legitimately realized by more than one symbol in a file (e.g. a
        // typed-throw contract spread across parse entry points), so the
        // index is set-valued: a cite is bound at symbol granularity iff its
        // resolved enclosing symbol is a MEMBER of the registered set. Drives
        // SymbolMismatch where the file IS bound (R260) but no registered
        // symbol covers the cited line.
        let impl_symbols_by_section_file = symbols_by_section_file(snapshot);

        // Orphan ledger lookup: (file, id) pairs explicitly registered as
        // known-stale code citations on the `§`-axis vs the inventory axis.
        // Independent indices so `CodeCitation` rows don't suppress inventory
        // violations and `InventoryCitation` rows don't suppress `§`-axis.
        let ledger_index: BTreeSet<(&str, &str)> = orphan_ledger
            .iter()
            .filter(|e| e.kind == OrphanKind::CodeCitation)
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        let inventory_ledger_index: BTreeSet<(&str, &str)> = orphan_ledger
            .iter()
            .filter(|e| e.kind == OrphanKind::InventoryCitation)
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();

        let files = self.read_set(workspace_root)?;
        let mut violations: Vec<CodeRefViolation> = Vec::new();

        // file_path → BTreeSet<section_id> citations actually observed.
        // Drives step 3's bidirectional check.
        let mut cited_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for abs in files {
            let raw = match std::fs::read_to_string(&abs) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // `raw` outlives `content` because the symbol axis needs it: a
            // resolver must parse the PROGRAM, and `comment_only` strips the
            // text down to comments, which no parser can read. Borrowed rather
            // than cloned in the un-stripped case — this loop runs over every
            // file in the read set.
            let content: std::borrow::Cow<'_, str> = if comment_only {
                std::borrow::Cow::Owned(strip_to_comments(&raw, comment_syntax_for(&abs)))
            } else {
                std::borrow::Cow::Borrowed(&raw)
            };
            let rel = abs
                .strip_prefix(workspace_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(abs.clone());
            let rel_str = rel.to_string_lossy().to_string();
            // The citations of THIS file that reach the symbol axis, collected
            // so the resolver is asked once for the file rather than once per
            // citation (SCE 4-A).
            let mut symbol_demand: Vec<(usize, String, &BTreeSet<&str>)> = Vec::new();

            // ---- Round NNN axis ----
            for (line, entry_id) in
                extract_citations(prefix, &content, &self.config.external_changelog_prefixes)
            {
                let matches_filter = filter_id.map(|f| entry_id == f).unwrap_or(false);
                let is_missing = !valid_entry_ids.contains(&entry_id);
                let kind = if matches_filter && verdicts.judges(AuditAxis::Decay) {
                    ViolationKind::Decay
                } else if is_missing && verdicts.judges(AuditAxis::Missing) {
                    ViolationKind::Missing
                } else {
                    continue;
                };
                violations.push(CodeRefViolation::Citation {
                    citation: Citation {
                        file: rel.clone(),
                        line,
                        entry_id,
                    },
                    kind,
                    // This axis compares an id against the store's key set and
                    // reads nothing at the site, which is what
                    // `AuditAxis::evidence` declares for both its kinds.
                    evidence: None,
                });
            }

            // ---- §<id> axis ----
            // This block does two jobs: it judges the three section axes, and it
            // records `cited_by` for the spec-side half below. So it is skipped
            // only when NEITHER is wanted — decay-filter mode, where the cascade
            // caller's question is Round NNN alone. A path-scoped run still walks
            // it: the section axes are decidable from one file plus the store.
            if !verdicts.judges_any(&[
                AuditAxis::SectionMissing,
                AuditAxis::CitationUnbound,
                AuditAxis::SymbolMismatch,
            ]) && !verdicts.judges(AuditAxis::BindingUnbacked)
            {
                continue;
            }
            // The prefix registries, the namespace scope and the file's numbering
            // origin all live in the one resolver (Round 867). A citation this
            // resolver drops leaves no trace on either side of the set-equality:
            // no SectionMissing, and no `cited_by` record, because step 3 must
            // not treat another document's number as this workspace's binding.
            for (line, section_id) in attribution.citations_in(&abs, &content).cited {
                // Ledger suppression — if (file, id) is explicitly registered as a
                // known-stale code citation, treat as if the binding were correct
                // (record in `cited_by` so step 3 doesn't double-fire).
                let suppressed = ledger_index.contains(&(rel_str.as_str(), section_id.as_str()));
                cited_by
                    .entry(rel_str.clone())
                    .or_default()
                    .insert(section_id.clone());
                if suppressed {
                    continue;
                }
                if !section_id_set.contains(&section_id) {
                    if verdicts.judges(AuditAxis::SectionMissing) {
                        violations.push(CodeRefViolation::Citation {
                            citation: Citation {
                                file: rel.clone(),
                                line,
                                entry_id: format!("§{}", section_id),
                            },
                            kind: ViolationKind::SectionMissing,
                            // The id is not in the store's section set, so
                            // there is no section whose bindings could be read.
                            evidence: None,
                        });
                    }
                    continue;
                }
                // Section exists — check spec-side membership of (file in
                // §<id>.bindings files). Matching is by `file` string only;
                // symbol is opaque metadata not in the bidirectional set-equality.
                let binds = impl_files_by_section.get(section_id.as_str());
                let bound = binds
                    .map(|files| files.contains(rel_str.as_str()))
                    .unwrap_or(false);
                if !bound {
                    if verdicts.judges(AuditAxis::CitationUnbound) {
                        violations.push(CodeRefViolation::Citation {
                            citation: Citation {
                                file: rel.clone(),
                                line,
                                entry_id: format!("§{}", section_id),
                            },
                            kind: ViolationKind::CitationUnbound,
                            // WHAT THE SECTION DOES BIND (Round 1167). This
                            // lookup is the test itself — the boolean above is
                            // the only thing it used to keep, and a consumer was
                            // left to query the store for the set that decided
                            // their violation. A section binding nobody yields
                            // an EMPTY list rather than no evidence: "you are
                            // not in the list" and "there is no list" are
                            // different repairs.
                            evidence: Some(CitationEvidence::SectionBindings {
                                files: binds
                                    .map(|files| files.iter().map(|f| (*f).to_string()).collect())
                                    .unwrap_or_default(),
                            }),
                        });
                    }
                } else if let Some(expected_syms) =
                    symbol_expectation(&section_id, &rel_str, &impl_symbols_by_section_file)
                {
                    // RFC-002 FR-3 symbol-level enforcement. File-level binding
                    // passed and the cited section records symbols for this
                    // file, so this citation is one the symbol axis judges. The
                    // resolver is NOT called here — the demand is collected and
                    // put to it once for the whole file below (SCE 4-A).
                    symbol_demand.push((line, section_id.clone(), expected_syms));
                }
            }

            // ---- Symbol axis: ONE resolver call for this file (SCE 4-A) ----
            //
            // The resolver used to be called per citation, and each call read
            // and parsed the whole file and recompiled its query: a consumer
            // measured 108.9 seconds of gate time with 99.4% of it here. The
            // demand is now batched per file, and the SOURCE goes with it —
            // `raw`, the bytes this loop read, not a second read of the path.
            // (`raw` and not `content`: under `comment_only` the content is
            // stripped to comments, which is not a program any parser can read.
            // Stripping preserves line numbering, so the lines still agree.)
            if !symbol_demand.is_empty() && verdicts.judges(AuditAxis::SymbolMismatch) {
                if let Some(resolvers) = symbol_resolvers_opt {
                    if let Some(resolver) = lang_for_file(&rel).and_then(|l| resolvers.get(l)) {
                        let lines: Vec<u32> = symbol_demand
                            .iter()
                            .map(|(line, _, _)| *line as u32)
                            .collect();
                        let abs_for_resolve = workspace_root.join(&rel);
                        // A resolver error is silent, as it was per citation:
                        // an unparseable file is not a citation defect.
                        if let Ok(resolved) =
                            resolver.resolve_symbols_at(&abs_for_resolve, &raw, &lines)
                        {
                            for (line, section_id, expected_syms) in &symbol_demand {
                                let Some(found) = resolved.get(&(*line as u32)) else {
                                    continue;
                                };
                                if !expected_syms.contains(found.as_str())
                                    && verdicts.judges(AuditAxis::SymbolMismatch)
                                {
                                    violations.push(CodeRefViolation::Citation {
                                        citation: Citation {
                                            file: rel.clone(),
                                            line: *line,
                                            entry_id: format!("§{}", section_id),
                                        },
                                        kind: ViolationKind::SymbolMismatch,
                                        // THE ONE SITE THAT RESOLVED A NAME, so
                                        // the one that carries it. Both names,
                                        // because a drift is a pair — and
                                        // `expected` is the whole recorded set,
                                        // sorted, rather than whichever member
                                        // the comparison happened to reject.
                                        evidence: Some(CitationEvidence::SymbolDrift(ReadSymbol {
                                            found: found.clone(),
                                            expected: {
                                                let mut e: Vec<String> = expected_syms
                                                    .iter()
                                                    .map(|s| (*s).to_string())
                                                    .collect();
                                                e.sort_unstable();
                                                e
                                            },
                                        })),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // ---- Prose-fact-assertion axis (structured-fact SSOT, opt-in) ----
            // A current-state code comment must POINT to a section, not RESTATE
            // a structured fact about it (a relation/status verb adjacent to a
            // `§<id>`). Such facts live once in the store (decision_status /
            // superseded_by / bindings, authored via the mutate API) and prose
            // only projects them. OFF unless `severity_prose_fact_assertion` is
            // set. Reached only in filter_id.is_none() mode (the section-axis
            // guard above already `continue`s the decay-filter pass). See
            // claudedocs/structured-fact-ssot-design.md.
            if verdicts.judges(AuditAxis::ProseFactAssertion) {
                for (line, section_id, verb) in extract_prose_fact_assertions(&content) {
                    violations.push(CodeRefViolation::Citation {
                        citation: Citation {
                            file: rel.clone(),
                            line,
                            entry_id: format!("§{}", section_id),
                        },
                        kind: ViolationKind::ProseFactAssertion,
                        // THE VERB THAT MADE IT AN ASSERTION (Round 1167). The
                        // extractor has always returned it and this site has
                        // always bound it to `_verb`: the rule is a list of
                        // eight spellings in two languages that lives in this
                        // crate, so a reader of the flagged line was left
                        // guessing which of its words the gate objected to.
                        evidence: Some(CitationEvidence::AssertionVerb { verb }),
                    });
                }
            }

            // ---- Inventory ID axis (Phase 1A) ----
            // Active / Reserved → silent; Deprecated → InventoryDeprecated;
            // missing IDs → InventoryMissing. `[[orphan_ledger]] kind =
            // InventoryCitation` suppresses both. Chain section-path
            // inventory axis (`inventory_path_prefixes`); dedup on (line, id)
            // so a prefix registered in both axes surfaces once.
            let mut inventory_cites = extract_inventory_citations(inventory_prefixes, &content);
            inventory_cites.extend(extract_inventory_path_citations(
                inventory_path_prefixes,
                &content,
            ));
            inventory_cites.sort();
            inventory_cites.dedup();
            for (line, inventory_id) in inventory_cites {
                let kind = match snapshot.inventory.get(&inventory_id).copied() {
                    None if verdicts.judges(AuditAxis::InventoryMissing) => {
                        Some(ViolationKind::InventoryMissing)
                    }
                    Some(mnemosyne_core::InventoryStatus::Deprecated)
                        if verdicts.judges(AuditAxis::InventoryDeprecated) =>
                    {
                        Some(ViolationKind::InventoryDeprecated)
                    }
                    // Active / Reserved — cite-permitted; and an axis this run
                    // does not judge emits nothing rather than a quiet pass.
                    _ => None,
                };
                if let Some(k) = kind {
                    if inventory_ledger_index.contains(&(rel_str.as_str(), inventory_id.as_str())) {
                        continue;
                    }
                    violations.push(CodeRefViolation::Citation {
                        citation: Citation {
                            file: rel.clone(),
                            line,
                            entry_id: inventory_id,
                        },
                        kind: k,
                        // Both inventory kinds are decided by the entry's
                        // status in the store; nothing is read at the site.
                        evidence: None,
                    });
                }
            }
        }

        // ---- Step 3: spec-side bidirectional half ----
        // Skipped under decay-filter mode, and under a path scope: `cited_by`
        // holds only the scoped files, so every binding naming a file outside
        // the scope would look unwitnessed. That is the whole reason the axis
        // is reported as not judged rather than judged clean.
        if verdicts.judges(AuditAxis::BindingUnbacked) {
            for (section_id, section) in &snapshot.sections {
                // A citation-edge binding (implements OR references) asserts a
                // code↔spec edge and so must be witnessed by a citation. A
                // verifies binding is externally mapped (test → section), not a
                // §<id> citation, so it is excluded from this half.
                for impl_entry in &section.bindings {
                    if !is_citation_edge(impl_entry.kind) {
                        continue;
                    }
                    let suppressed =
                        ledger_index.contains(&(impl_entry.file.as_str(), section_id.as_str()));
                    if suppressed {
                        continue;
                    }
                    let cited = cited_by
                        .get(&impl_entry.file)
                        .map(|set| set.contains(section_id))
                        .unwrap_or(false);
                    if !cited {
                        violations.push(CodeRefViolation::BindingUnbacked {
                            section_id: section_id.clone(),
                            file: PathBuf::from(&impl_entry.file),
                            symbol: impl_entry.symbol.clone(),
                        });
                    }
                }
            }
        }

        // ---- Step 4: spec-side coverage axiom ----
        // Workspace-wide: a `Normative`, non-`Removed` section with zero
        // `implements` coverage is the "Active = backed by code" axiom
        // violation. `Informative` sections (terminology / overview /
        // references) are prose-only and exempt (Round 389); `Removed` is a
        // lifecycle tombstone, also exempt. The gap definition lives in the
        // single source of truth `classify_section_coverage` (Round 390), so
        // this negative finding and the positive `report-coverage` projection
        // cannot drift; both bottom out in the exhaustive `counts_as_coverage`
        // / `coverage_expectation` matches that force a compile-time decision
        // for any future variant. decision_status is preserved as the raw
        // `Option` on the emitted variant (None → Active is a consumer-side
        // convention, Round 265).
        if verdicts.judges(AuditAxis::ImplementationMissing) {
            for (section_id, section) in &snapshot.sections {
                if classify_section_coverage(section) == CoverageClass::NormativeGap {
                    violations.push(CodeRefViolation::ImplementationMissing {
                        section_id: section_id.clone(),
                        decision_status: section.decision_status,
                    });
                }
            }
        }

        // ---- Step 5: spec-side verification axiom (R413, opt-in) ----
        // OFF unless the verify axis is enabled (`severity_verification` set):
        // requirement→test-evidence traceability is a per-project commitment,
        // not a universal axiom, so a workspace that registers no `verifies`
        // bindings pays no cost and sees no noise. When on, a Normative +
        // Dedicated, non-`Removed` section with zero `verifies` bindings is the
        // gap. `ByConstruction` (no per-unit oracle) and `Informative` sections
        // are exempt — the classification is SCE-supplied; the gate only
        // enforces it.
        if verdicts.judges(AuditAxis::VerificationMissing) {
            for (section_id, section) in &snapshot.sections {
                if is_verification_gap(section) {
                    violations.push(CodeRefViolation::VerificationMissing {
                        section_id: section_id.clone(),
                        decision_status: section.decision_status,
                    });
                }
            }
        }

        // ---- Step 6: coverage invariant (R423, opt-in) ----
        // OFF unless `severity_classification` is set. Enforces design sec 6: an
        // exempt section (OutOfScope | Informational) must NOT carry an
        // implements/verifies binding — else it is mislabeled (should be
        // Normative) or the binding is wrong. The 3-state enum adds the label;
        // this gate enforces label↔binding consistency.
        if verdicts.judges(AuditAxis::MisclassifiedCoverage) {
            for (section_id, section) in &snapshot.sections {
                if is_coverage_misclassified(section) {
                    violations.push(CodeRefViolation::MisclassifiedCoverage {
                        section_id: section_id.clone(),
                        decision_status: section.decision_status,
                    });
                }
            }
        }

        // ---- Step 7: blanket-binding detector (R425, opt-in — SCE P1) ----
        // OFF unless `severity_blanket` is set.
        if verdicts.judges(AuditAxis::BlanketVerifies) {
            violations.extend(scan_blanket_verifies(snapshot));
        }

        sort_violations(&mut violations);
        Ok(violations)
    }

    /// Curation support for Path B adoption. Scans the configured paths for
    /// `§<id>` citations and, per `(section_id, file)`, resolves each
    /// citation's enclosing symbol via the file's language `SymbolResolver`,
    /// returning the proposed implementation symbol set plus a count of
    /// citations whose symbol could not be resolved (file-only fallback).
    /// Attributes citations through the SAME resolver as [`Self::scan`]
    /// (Round 867), so a proposal can never bind a file on a citation the gate
    /// does not count. Unknown-section citations are skipped here (they are
    /// hallucinations for `scan` to flag, not bindings).
    ///
    /// The result is a *proposal* reflecting the current code state. The
    /// maintainer ratifies it into `§X.bindings` as design intent —
    /// the act of review is also an audit of where each section is cited.
    pub fn propose_implementations(
        &self,
        attribution: &CitationAttribution,
        snapshot: &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<Vec<ProposedImplementation>> {
        let workspace_root = attribution.root();
        let comment_only = self.config.comment_only;
        let known_section = &snapshot.section_ids_with_implied_parents;

        // (section_id, file) -> (resolved symbols, unresolved cite count)
        let mut acc: BTreeMap<(String, String), (BTreeSet<String>, usize)> = BTreeMap::new();

        let files = self.read_set(workspace_root)?;
        for abs in files {
            let raw = match std::fs::read_to_string(&abs) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // Same reason as the scan's: the resolver parses the PROGRAM, so
            // `raw` has to outlive the comment-stripped view.
            let content: std::borrow::Cow<'_, str> = if comment_only {
                std::borrow::Cow::Owned(strip_to_comments(&raw, comment_syntax_for(&abs)))
            } else {
                std::borrow::Cow::Borrowed(&raw)
            };
            let rel = abs
                .strip_prefix(workspace_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(abs.clone());
            let rel_str = rel.to_string_lossy().to_string();

            // Every known-section citation in this file, then ONE resolver call
            // for all their lines (SCE 4-A) — a proposal over a file with forty
            // citations used to parse it forty times.
            let cited: Vec<(usize, String)> = attribution
                .citations_in(&abs, &content)
                .cited
                .into_iter()
                .filter(|(_, section_id)| known_section.contains(section_id.as_str()))
                .collect();
            if cited.is_empty() {
                continue;
            }
            let resolved: BTreeMap<u32, String> = lang_for_file(&rel)
                .and_then(|lang| self.symbol_resolvers.get(lang))
                .and_then(|resolver| {
                    let lines: Vec<u32> = cited.iter().map(|(line, _)| *line as u32).collect();
                    resolver
                        .resolve_symbols_at(&workspace_root.join(&rel), &raw, &lines)
                        .ok()
                })
                .unwrap_or_default();
            for (line, section_id) in cited {
                let entry = acc.entry((section_id, rel_str.clone())).or_default();
                match resolved.get(&(line as u32)) {
                    Some(sym) => {
                        entry.0.insert(sym.clone());
                    }
                    None => entry.1 += 1,
                }
            }
        }

        let mut out: Vec<ProposedImplementation> = acc
            .into_iter()
            .map(
                |((section_id, file), (symbols, unresolved))| ProposedImplementation {
                    section_id,
                    file,
                    symbols,
                    unresolved_citations: unresolved,
                },
            )
            .collect();
        out.sort();
        Ok(out)
    }

    /// Reverse citation index for citation-density reporting (the
    /// `report-spec-map` projection): `section_id -> [{file, line}]` for every
    /// code site that cites the section. Reuses the same file walk and the same
    /// attribution resolver as [`Self::propose_implementations`], but aggregates
    /// the raw cite locations per section without symbol resolution. Read-only —
    /// an L3 view substrate, never a mutation.
    ///
    /// Only sections present in `snapshot.section_ids_with_implied_parents` are
    /// counted: a cite to a non-existent section is a hallucination
    /// (`validate-code-refs` flags it as `section_missing`), not a density data
    /// point. Result is stably ordered — `BTreeMap` by section id, and each
    /// site list sorted by `(file, line)`.
    pub fn citation_index(
        &self,
        attribution: &CitationAttribution,
        snapshot: &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<BTreeMap<String, Vec<CitationSite>>> {
        let workspace_root = attribution.root();
        let comment_only = self.config.comment_only;
        let known_section = &snapshot.section_ids_with_implied_parents;

        let mut index: BTreeMap<String, Vec<CitationSite>> = BTreeMap::new();
        let files = self.read_set(workspace_root)?;
        for abs in files {
            let raw = match std::fs::read_to_string(&abs) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let content = if comment_only {
                strip_to_comments(&raw, comment_syntax_for(&abs))
            } else {
                raw
            };
            let rel = abs
                .strip_prefix(workspace_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(abs.clone());
            let rel_str = rel.to_string_lossy().to_string();

            for (line, section_id) in attribution.citations_in(&abs, &content).cited {
                if !known_section.contains(section_id.as_str()) {
                    continue;
                }
                index.entry(section_id).or_default().push(CitationSite {
                    file: rel_str.clone(),
                    line,
                });
            }
        }
        for sites in index.values_mut() {
            sites.sort();
        }
        Ok(index)
    }

    /// What the symbol axis can and cannot cover under this config (Round 855).
    ///
    /// # Why
    ///
    /// `severity_binding = reject` reads as symbol-level enforcement, and for
    /// any file the axis cannot reach it is file-level — a materially weaker
    /// claim, made silently. Two ways to be unreachable, and a consumer hit
    /// both: an extension the table does not map (their C runtime's `.c`
    /// files, while the `.h` files beside them resolved), and a language with
    /// no configured resolver (their Go, Kotlin and Python runtimes, holding
    /// 181 / 235 / 194 hand-authored citations between them).
    ///
    /// Reported, not rejected. Rejecting would fail every workspace that
    /// enrols a directory holding a `.md` or a manifest, which is most of
    /// them; the defect was never that those files are unreachable, it was
    /// that nothing said so. This is the Round 819 shape: an axis that covers
    /// nothing prints the same silence as an axis where everything passed, so
    /// the counts go out every run.
    ///
    /// `citing` is the load-bearing half of each pair. A hundred unreachable
    /// files that cite nothing cost no coverage; one that carries a citation
    /// this ledger gates is a symbol-level claim the run did not check.
    ///
    /// # Errors
    ///
    /// Whatever the directory walk or [`Self::citation_index`] fails with.
    pub fn symbol_axis_coverage(
        &self,
        attribution: &CitationAttribution,
        snapshot: &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<SymbolAxisCoverage> {
        let workspace_root = attribution.root();
        // The SAME extraction, namespace scoping and known-section filter the
        // gate applies, by calling it rather than by repeating it: a coverage
        // report free to disagree with the gate about what a citation is would
        // be reporting some other tool's coverage.
        let index = self.citation_index(attribution, snapshot)?;
        let citing: BTreeSet<String> = index
            .values()
            .flatten()
            .map(|site| site.file.clone())
            .collect();
        let mut cov = SymbolAxisCoverage::default();

        // The axis's PRICE, from the same predicate the scan judges by
        // ([`symbol_expectation`]) over the same citation index the coverage
        // above is derived from. Counted here rather than reported by the scan
        // because a consumer needs it BEFORE paying it, and — the case that
        // decided the shape — a tree with no resolver configured never enters
        // the scan's symbol branch at all and so could never report from there.
        let symbols_by_section_file = symbols_by_section_file(snapshot);
        let mut checked_files: BTreeSet<&str> = BTreeSet::new();
        for (section_id, sites) in &index {
            for site in sites {
                if symbol_expectation(section_id, &site.file, &symbols_by_section_file).is_none() {
                    continue;
                }
                let Some(lang) = lang_for_file(Path::new(&site.file)) else {
                    continue;
                };
                cov.checked_citations += 1;
                checked_files.insert(site.file.as_str());
                // Round 1144 — of that demand, the part NO configured resolver
                // can answer. Counted here rather than derived from
                // `unresolved_languages` below, which counts FILES and would
                // answer a different question: a file may be unreachable and
                // carry no symbol-level claim, which costs nothing.
                if !self.symbol_resolvers.contains_key(lang) {
                    cov.unchecked_citations += 1;
                    cov.unchecked_languages.insert(lang.to_string());
                }
            }
        }
        cov.checked_files = checked_files.len();
        for abs in self.read_set(workspace_root)? {
            let rel = abs.strip_prefix(workspace_root).unwrap_or(&abs);
            let cites = citing.contains(&rel.to_string_lossy().to_string());
            let bucket = match lang_for_file(rel) {
                None => {
                    let ext = rel
                        .extension()
                        .map_or_else(|| "<none>".to_string(), |e| e.to_string_lossy().to_string());
                    cov.unmapped_extensions.entry(ext).or_default()
                }
                Some(lang) if self.symbol_resolvers.contains_key(lang) => {
                    cov.covered.entry(lang.to_string()).or_default()
                }
                Some(lang) => cov
                    .unresolved_languages
                    .entry(lang.to_string())
                    .or_default(),
            };
            bucket.files += 1;
            if cites {
                bucket.citing_files += 1;
            }
        }
        Ok(cov)
    }
}

/// Files an axis reached, and how many of them carry a citation it gates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct AxisFileCount {
    pub files: usize,
    /// Of those, the ones holding at least one citation this ledger gates —
    /// the only files whose unreachability costs coverage.
    pub citing_files: usize,
}

/// What the symbol axis covers under a given config (Round 855). See
/// [`SetEqualityValidator::symbol_axis_coverage`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct SymbolAxisCoverage {
    /// Extension → count, for extensions no resolver can apply to. A citation
    /// in one of these binds at FILE level whatever `severity_binding` says.
    pub unmapped_extensions: BTreeMap<String, AxisFileCount>,
    /// Language → count, for languages the table maps but no
    /// `[plugins.symbol_resolver.<lang>]` entry covers.
    pub unresolved_languages: BTreeMap<String, AxisFileCount>,
    /// Language → count, for languages a configured resolver covers.
    pub covered: BTreeMap<String, AxisFileCount>,
    /// Citations a judging run puts to a resolver: the cite's section exists,
    /// the file is bound to it, and the section records symbols for that file
    /// ([`symbol_expectation`], the same predicate the scan judges by).
    ///
    /// This is the axis's PRICE, published whether or not a resolver is
    /// configured — a consumer estimating what one-parse-per-file buys needs
    /// `checked_citations / checked_files`, and the tree that hosts this gate
    /// configures no resolver at all, so it could never have measured the ratio
    /// by timing itself (SCE 4-A).
    pub checked_citations: usize,
    /// Distinct files among those citations — the number of resolver CALLS a
    /// run makes now that the demand is batched per file. Before Round 1141 the
    /// call count was `checked_citations`, and each call re-read and re-parsed
    /// the whole file.
    pub checked_files: usize,
    /// Of `checked_citations`, the ones no configured resolver can answer —
    /// symbol-level claims this run would leave unjudged (Round 1144).
    ///
    /// A run in this state is refused rather than reported, for the reason
    /// Round 855 refuses an unbuildable resolver entry: `severity_binding =
    /// reject` reads as symbol-level enforcement while the run performs none.
    /// It is also the state a consumer reaches by pricing the axis the way SCE
    /// did — deleting the resolver blocks and re-running — which left every
    /// symbol binding in their store unchecked and every run green.
    pub unchecked_citations: usize,
    /// The languages those claims are written in: what to configure.
    pub unchecked_languages: BTreeSet<String>,
}

impl SymbolAxisCoverage {
    /// Files carrying a gated citation that the symbol axis cannot reach, for
    /// either reason. The one number that says how much weaker
    /// `severity_binding` is than it reads.
    #[must_use]
    pub fn unreachable_citing_files(&self) -> usize {
        self.unmapped_extensions
            .values()
            .chain(self.unresolved_languages.values())
            .map(|c| c.citing_files)
            .sum()
    }
}

/// One code site citing a spec section, for the reverse citation index
/// ([`SetEqualityValidator::citation_index`], the `report-spec-map`
/// citation-density dimension). `file` is workspace-relative; `line` is
/// 1-indexed. `Ord` sorts by `(file, line)` for stable output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct CitationSite {
    pub file: String,
    pub line: usize,
}

/// A proposed `§<section_id>.bindings` entry derived from the
/// current code citations, for maintainer ratification (Path B curation).
/// `symbols` is the set of resolved enclosing symbols across every cite of
/// the section in `file`; `unresolved_citations` counts cites whose symbol
/// the resolver could not name (no resolver for the language, or a cite
/// sitting outside any declaration) — those bind at file granularity only.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct ProposedImplementation {
    pub section_id: String,
    pub file: String,
    pub symbols: BTreeSet<String>,
    pub unresolved_citations: usize,
}

impl mnemosyne_core::Validator for SetEqualityValidator {
    type Finding = CodeRefViolation;

    fn version_surface(&self) -> mnemosyne_core::VersionSurface {
        mnemosyne_core::VersionSurface {
            plugin_name: "mnemosyne-validate::SetEqualityValidator".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn validate(
        &self,
        ctx: &mnemosyne_core::ValidationContext<'_>,
    ) -> Result<Vec<CodeRefViolation>, mnemosyne_core::ValidatorError> {
        let snapshot = ctx.store.snapshot();
        // The trait hands over a root, not an attribution, so this is where the
        // numbering origin gets derived for a plugin-dispatched run — once, here,
        // rather than per file or per axis (Round 867).
        let attribution = CitationAttribution::new(
            ctx.workspace_root,
            &self.config,
            NumberingOriginAxis::derive(ctx.workspace_root),
        );
        self.scan(&attribution, &snapshot)
            .map_err(|e| mnemosyne_core::ValidatorError::Internal(e.to_string()))
    }
}

/// Round 266 — auto-cascade trigger primitive (Stage B freshness).
///
/// Targeted decay scan for §<section_id> citations of *one* section,
/// returned as a flat list of [`Citation`]. Used by the mutate-time hook
/// in `set-section-decision-status` CLI: when a section transitions
/// to Superseded/Removed, this surfaces the source-side citations that
/// will need authoring follow-up (no rejection — informational only).
///
/// Skips file-read failures silently (consistent with the bidirectional
/// scanner's behavior).
///
/// Attributes through the SHARED resolver (Round 867). It used to call the
/// extractor with two EMPTY prefix slices and no namespace scope, which made it
/// the loosest of the five readers: a `UAX #9 §6.7` comment counted as decay of
/// a section of ours numbered the same. Latent rather than live when found — this
/// axis only runs for
/// a superseded or removed section, and the trees available measured zero — but a
/// predicate held by one reader and not its siblings is no predicate.
///
/// `paths` is workspace-relative; symbol-side bindings are not consulted
/// (decay is about cite locations, not implementation universe).
pub fn scan_section_decay(
    attribution: &CitationAttribution,
    paths: &[String],
    section_id: &str,
) -> std::io::Result<Vec<Citation>> {
    let workspace_root = attribution.root();
    let files = walk_paths(workspace_root, paths)?;
    let mut hits = Vec::new();
    for abs in files {
        let Some(attributed) = attribution.attribute_file(&abs) else {
            continue;
        };
        let rel = abs
            .strip_prefix(workspace_root)
            .map(|p| p.to_path_buf())
            .unwrap_or(abs.clone());
        for (line, sid) in attributed.cited {
            if sid == section_id {
                hits.push(Citation {
                    file: rel.clone(),
                    line,
                    entry_id: format!("§{}", sid),
                });
            }
        }
    }
    hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(hits)
}

/// Round 276 — Inventory axis cascade trigger primitive (Phase 1A).
///
/// Targeted decay scan for a single inventory ID's citations across
/// `paths`. Mirrors [`scan_section_decay`] on the §<id> axis. Used by
/// the mutate-time hook in the `add-inventory-entry` (registered
/// Deprecated), `set-inventory-status` (transition to Deprecated), and
/// `remove-inventory-entry` CLI surfaces — the cascade surfaces author-
/// follow-up sites without rejecting the mutate.
///
/// `inventory_prefixes` are required for the extractor lookup; an empty
/// slice yields no hits regardless of input. `comment_only` toggles the
/// shared filter so fixture string literals don't generate noise.
///
/// Skips file-read failures silently (consistent with the bidirectional
/// scanner). Returns hits sorted by `(file, line)`.
///
/// Decay scan covers both inventory axes: opaque-ID via
/// `inventory_prefixes` and section-path via `inventory_path_prefixes`.
/// Cascade trigger calls this after an `InventoryEntry` transitions to
/// a status that needs cite-side notification, so a path-shape ID
/// rename / deprecation surfaces its cite-sites too. An empty slice
/// disables the corresponding axis.
pub fn scan_inventory_decay(
    workspace_root: &Path,
    paths: &[String],
    inventory_id: &str,
    inventory_prefixes: &[String],
    inventory_path_prefixes: &[String],
    comment_only: bool,
) -> std::io::Result<Vec<Citation>> {
    if inventory_prefixes.is_empty() && inventory_path_prefixes.is_empty() {
        return Ok(Vec::new());
    }
    let files = walk_paths(workspace_root, paths)?;
    let mut hits = Vec::new();
    for abs in files {
        let raw = match std::fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let content = if comment_only {
            strip_to_comments(&raw, comment_syntax_for(&abs))
        } else {
            raw
        };
        let rel = abs
            .strip_prefix(workspace_root)
            .map(|p| p.to_path_buf())
            .unwrap_or(abs.clone());
        // Chain opaque-ID + section-path axes; dedup on (line, id) so a
        // prefix registered in both axes surfaces once.
        let mut cites = extract_inventory_citations(inventory_prefixes, &content);
        cites.extend(extract_inventory_path_citations(
            inventory_path_prefixes,
            &content,
        ));
        cites.sort();
        cites.dedup();
        for (line, id) in cites {
            if id == inventory_id {
                hits.push(Citation {
                    file: rel.clone(),
                    line,
                    entry_id: id,
                });
            }
        }
    }
    hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(hits)
}

/// Total-order key for the deterministic violation sort. `rank` (the variant
/// declaration order) is the primary axis — Citation < BindingUnbacked <
/// ImplementationMissing < VerificationMissing — so reports keep relative diff
/// stability as edges surface. The remaining fields carry each variant's
/// secondary ordering and are *rank-gated*: cross-rank pairs are separated by
/// `rank` and never compare them, so a field slot legitimately means different
/// things per variant (e.g. `primary` = file for Citation/BindingUnbacked, but
/// section_id for the two Missing variants).
///
/// Derived `Ord` compares fields in declaration order. The key is produced by
/// the single exhaustive [`CodeRefViolation::sort_key`] match, so adding a
/// `CodeRefViolation` variant is a compile error there — restoring the
/// exhaustiveness guarantee the previous `match (a, b) { _ => unreachable!() }`
/// tiebreaker silently lost (it compiled with a missing same-rank arm and
/// panicked at runtime when two of the new variant were compared).
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ViolationSortKey {
    rank: u8,
    /// file (Citation / BindingUnbacked) | section_id (the Missing variants).
    primary: String,
    /// line (Citation) | 0 otherwise.
    line: usize,
    /// entry_id (Citation) | section_id (BindingUnbacked) | "" otherwise.
    secondary: String,
    /// symbol (BindingUnbacked) | "" otherwise.
    tertiary: String,
}

impl CodeRefViolation {
    /// Compute the [`ViolationSortKey`]. ONE exhaustive match — the
    /// compiler-enforced extension point for variant ordering.
    fn sort_key(&self) -> ViolationSortKey {
        match self {
            CodeRefViolation::Citation { citation, .. } => ViolationSortKey {
                rank: 0,
                primary: citation.file.to_string_lossy().into_owned(),
                line: citation.line,
                secondary: citation.entry_id.clone(),
                tertiary: String::new(),
            },
            CodeRefViolation::BindingUnbacked {
                section_id,
                file,
                symbol,
            } => ViolationSortKey {
                rank: 1,
                primary: file.to_string_lossy().into_owned(),
                line: 0,
                secondary: section_id.clone(),
                // Symbols are validated non-empty, so "" uniquely encodes
                // `None` and preserves the `Option` ordering (None < Some).
                tertiary: symbol.clone().unwrap_or_default(),
            },
            CodeRefViolation::ImplementationMissing { section_id, .. } => ViolationSortKey {
                rank: 2,
                primary: section_id.clone(),
                line: 0,
                secondary: String::new(),
                tertiary: String::new(),
            },
            CodeRefViolation::VerificationMissing { section_id, .. } => ViolationSortKey {
                rank: 3,
                primary: section_id.clone(),
                line: 0,
                secondary: String::new(),
                tertiary: String::new(),
            },
            CodeRefViolation::MisclassifiedCoverage { section_id, .. } => ViolationSortKey {
                rank: 4,
                primary: section_id.clone(),
                line: 0,
                secondary: String::new(),
                tertiary: String::new(),
            },
            CodeRefViolation::BlanketVerifies { file, symbol, .. } => ViolationSortKey {
                rank: 5,
                primary: file.to_string_lossy().into_owned(),
                line: 0,
                secondary: symbol.clone().unwrap_or_default(),
                tertiary: String::new(),
            },
        }
    }
}

/// Deterministic ordering — see [`ViolationSortKey`]. `sort_by_cached_key`
/// computes each key once (the keys allocate), then sorts.
fn sort_violations(violations: &mut [CodeRefViolation]) {
    violations.sort_by_cached_key(|v| v.sort_key());
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{
        add_section_binding, set_section_coverage_expectation,
        set_section_verification_expectation, AtomicStore, BindingKind,
    };
    use mnemosyne_core::VerificationExpectation;
    use tempfile::TempDir;

    /// A fixture tree STATES its numbering environment rather than asking the
    /// disk (Round 867). These fixtures are bare temp directories with no VCS,
    /// so deriving would answer `NotDetermined` and the tests would then be
    /// asserting about a degraded path instead of the ordinary one.
    fn no_foreign_subtree<'a>(
        root: &'a Path,
        config: &'a SetEqualityValidatorConfig,
    ) -> CitationAttribution<'a> {
        CitationAttribution::new(
            root,
            config,
            NumberingOriginAxis::Measured {
                foreign_subtrees: Vec::new(),
            },
        )
    }

    /// The decay axis consults only `comment_only` and the attribution.
    fn decay_config(comment_only: bool) -> SetEqualityValidatorConfig {
        SetEqualityValidatorConfig {
            comment_only,
            ..Default::default()
        }
    }

    /// Round 783 — the coverage check exists because `paths` is itself a claim
    /// about which trees hold citations, and that claim drifted exactly the way
    /// the hand list Round 777 removed had drifted.
    ///
    /// Both halves matter. A tree that is neither scanned nor declared must be
    /// REPORTED, and a tree that is declared must then be silent — a check that
    /// only ever answered "unscanned" would be as useless as one that only ever
    /// answered "clean", and it is the same file asserting both so neither can
    /// pass while the other is broken.
    #[test]
    fn a_tree_that_is_neither_scanned_nor_declared_is_reported() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("crates/alpha/src")).unwrap();
        std::fs::write(root.join("crates/alpha/src/lib.rs"), "// Round 1\n").unwrap();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::fs::write(root.join("elsewhere/stray.rs"), "// Round 2\n").unwrap();

        let paths = vec!["crates/*/src/".to_string()];
        let found = scan_coverage(root, &paths, &[]).unwrap();
        assert_eq!(found.considered, 2, "both sources must be counted");
        assert_eq!(found.scanned, 1);
        assert_eq!(
            found.unscanned,
            vec![root.join("elsewhere/stray.rs")],
            "a Rust source outside every configured path must be named"
        );

        // Declared: the same tree, now written down, must go quiet. Nothing else
        // changed, so this isolates the exclusion as the cause.
        let declared = vec!["elsewhere/".to_string()];
        let after = scan_coverage(root, &paths, &declared).unwrap();
        assert!(after.unscanned.is_empty(), "a declared tree must be silent");
        assert!(after.stale_exclusions.is_empty());
    }

    /// Round 854 — the two axes, and the fact that only one of them is about
    /// Rust.
    ///
    /// `considered` / `scanned` / `unscanned` answer the Round 783 question and
    /// are Rust-only on purpose: a language-agnostic `unscanned` would demand a
    /// declaration for every `.md` in the tree. The file SETS answer the Round
    /// 840 question — what the gate reads — and a consumer's `paths` enrol C++
    /// headers and Jinja templates. One walk, two views, asserted together so a
    /// future edit cannot quietly narrow one of them.
    #[test]
    fn the_rust_axis_and_the_file_sets_answer_different_questions() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("sce/src")).unwrap();
        std::fs::write(root.join("sce/src/lib.rs"), "// Round 1\n").unwrap();
        std::fs::write(root.join("sce/src/engine.cpp"), "// see 1.1\n").unwrap();
        std::fs::write(root.join("sce/src/tpl.jinja2"), "{# see 1.1 #}\n").unwrap();
        std::fs::create_dir_all(root.join("generated")).unwrap();
        std::fs::write(root.join("generated/out.cpp"), "// see 1.1\n").unwrap();

        let paths = vec!["sce/src/".to_string()];
        let exclusions = vec!["generated/".to_string()];
        let cov = scan_coverage(root, &paths, &exclusions).unwrap();

        assert_eq!(cov.considered, 1, "one Rust source in the tree");
        assert_eq!(cov.scanned, 1, "and the configured path covers it");
        assert!(cov.unscanned.is_empty());
        assert_eq!(
            cov.scanned_files.len(),
            3,
            "the gate reads every file under a configured path, whatever the \
             language: {:?}",
            cov.scanned_files
        );
        assert_eq!(
            cov.excluded_files,
            [root.join("generated/out.cpp")].into_iter().collect(),
            "an excluded C++ file is a file the exclusion removed — under a \
             Rust-only view this set was empty and the exclusion read as stale"
        );
        assert!(
            cov.stale_exclusions.is_empty(),
            "the exclusion matches a real tree: {:?}",
            cov.stale_exclusions
        );
    }

    /// An exclusion that matches nothing is a rotting entry, and the workspace
    /// already fails on the same shape one axis over (an orphan-ledger row whose
    /// orphan resolved must be deleted). Without this the list decays into
    /// folklore that still reads as policy.
    #[test]
    fn an_exclusion_matching_no_file_is_reported_as_stale() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("crates/alpha/src")).unwrap();
        std::fs::write(root.join("crates/alpha/src/lib.rs"), "// Round 1\n").unwrap();

        let paths = vec!["crates/*/src/".to_string()];
        let found = scan_coverage(root, &paths, &["gone/".to_string()]).unwrap();
        assert_eq!(found.stale_exclusions, vec!["gone/".to_string()]);
        assert!(
            found.unscanned.is_empty(),
            "a stale exclusion must not also manufacture an unscanned file"
        );
    }

    /// Round 777 — a `*` segment DERIVES the sibling set from the tree, which is
    /// the whole reason it exists: the hand list it replaced had silently stopped
    /// covering four crates. The non-vacuity is the second half of this test, not
    /// the first — a pattern that merely finds today's directories proves nothing
    /// a list could not also do, so a NEW sibling is created after the first scan
    /// and must appear without the pattern being touched.
    #[test]
    fn a_star_segment_derives_the_sibling_set_including_ones_added_later() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        for crate_name in ["alpha", "beta"] {
            std::fs::create_dir_all(root.join("crates").join(crate_name).join("src")).unwrap();
            std::fs::write(
                root.join("crates").join(crate_name).join("src/lib.rs"),
                "// Round 1\n",
            )
            .unwrap();
            // tests/ sits beside src/ and must stay OUT: `*/src/` is a policy
            // boundary, not just a convenience.
            std::fs::create_dir_all(root.join("crates").join(crate_name).join("tests")).unwrap();
            std::fs::write(
                root.join("crates").join(crate_name).join("tests/it.rs"),
                "// Round 2\n",
            )
            .unwrap();
        }
        // A build tree under the star position must not be scanned — the skip
        // predicate is shared with the walk rather than copied for the glob.
        std::fs::create_dir_all(root.join("crates/target/src")).unwrap();
        std::fs::write(root.join("crates/target/src/junk.rs"), "// Round 3\n").unwrap();

        let pattern = vec!["crates/*/src/".to_string()];
        let found = walk_paths(root, &pattern).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().display().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "crates/alpha/src/lib.rs".to_string(),
                "crates/beta/src/lib.rs".to_string(),
            ],
            "the star matches every crate's src/, and only that"
        );

        // The half a hand list cannot pass: a sibling that did not exist when the
        // pattern was written is scanned anyway, with nothing edited.
        std::fs::create_dir_all(root.join("crates/gamma/src")).unwrap();
        std::fs::write(root.join("crates/gamma/src/lib.rs"), "// Round 4\n").unwrap();
        let after = walk_paths(root, &pattern).unwrap();
        assert!(
            after.contains(&root.join("crates/gamma/src/lib.rs")),
            "a crate added after the pattern was written must still be gated"
        );
        assert_eq!(after.len(), 3);
    }

    /// Round 777 — a path with no `*` resolves exactly as it always did, so the
    /// pattern support is additive rather than a re-specification of every
    /// existing config in the wild.
    #[test]
    fn a_path_without_a_star_is_unchanged() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("crates/alpha/src")).unwrap();
        std::fs::write(root.join("crates/alpha/src/lib.rs"), "// Round 1\n").unwrap();

        let found = walk_paths(root, &["crates/alpha/src/".to_string()]).unwrap();
        assert_eq!(found, vec![root.join("crates/alpha/src/lib.rs")]);
        // SOME configured path missing is still skipped rather than erroring —
        // the declared-intent-for-optional-checkouts behaviour, unchanged.
        let partial = walk_paths(
            root,
            &[
                "crates/alpha/src/".to_string(),
                "crates/nowhere/src/".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(partial, vec![root.join("crates/alpha/src/lib.rs")]);
    }

    /// Round 777 — a scan set that resolves to NOTHING is an error, not zero
    /// violations. Both spellings of it: a pattern that matches no directory
    /// (what an older binary without `*` support does to `crates/*/src/`, reading
    /// it as a literal path) and a plain path that is simply not there. Either
    /// way the validator would read no file and report the same clean a clean
    /// tree reports, which is the failure mode this whole round is about.
    #[test]
    fn a_scan_set_that_resolves_to_nothing_is_an_error_not_a_clean_report() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();

        for empty in [
            vec!["crates/*/src/".to_string()],
            vec!["crates/mnemosyne-gone/src/".to_string()],
        ] {
            let err = walk_paths(root, &empty).expect_err("an empty scan set must fail loud");
            assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        }
        // Non-vacuity: an EMPTY config is not an empty scan set — it is a
        // workspace that declared no paths, and the callers already return early
        // on that rather than treating it as a fault.
        assert!(walk_paths(root, &[]).unwrap().is_empty());
    }

    /// Test-only wrapper that drives `SetEqualityValidator::scan` with no
    /// SymbolResolver registry — i.e., pre-R306 set-equality-only mode.
    /// Tests that specifically exercise R306 symbol-axis enforcement
    /// construct a `SetEqualityValidator` directly with a populated
    /// `symbol_resolvers` map.
    #[allow(clippy::too_many_arguments)]
    fn scan_paths_no_resolvers(
        workspace_root: &Path,
        paths: &[String],
        prefix: &str,
        store: &AtomicStore,
        orphan_ledger: &[OrphanLedgerEntry],
        filter_id: Option<&str>,
        comment_only: bool,
        inventory_prefixes: &[String],
        external_section_prefixes_numeric: &[String],
        external_section_prefixes_bare: &[String],
        inventory_path_prefixes: &[String],
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        // The common case carries no namespace scope; `_ns` is the single
        // implementation, so the existing call sites stay untouched.
        scan_paths_no_resolvers_ns(
            workspace_root,
            paths,
            prefix,
            store,
            orphan_ledger,
            filter_id,
            comment_only,
            inventory_prefixes,
            external_section_prefixes_numeric,
            external_section_prefixes_bare,
            inventory_path_prefixes,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_paths_no_resolvers_ns(
        workspace_root: &Path,
        paths: &[String],
        prefix: &str,
        store: &AtomicStore,
        orphan_ledger: &[OrphanLedgerEntry],
        filter_id: Option<&str>,
        comment_only: bool,
        inventory_prefixes: &[String],
        external_section_prefixes_numeric: &[String],
        external_section_prefixes_bare: &[String],
        inventory_path_prefixes: &[String],
        section_namespace: Option<&str>,
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        use mnemosyne_core::AtomicStoreView;
        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                scan_exclusions: Vec::new(),
                paths: paths.to_vec(),
                severity_missing: mnemosyne_config::Severity::Reject,
                severity_binding: mnemosyne_config::Severity::Reject,
                severity_coverage: None,
                severity_verification: None,
                severity_confirmation: None,
                severity_classification: None,
                severity_blanket: None,
                severity_prose_fact_assertion: None,
                severity_inventory: mnemosyne_config::Severity::Reject,
                comment_only,
                inventory_prefixes: inventory_prefixes.to_vec(),
                external_section_prefixes: external_section_prefixes_numeric.to_vec(),
                external_section_prefixes_bare: external_section_prefixes_bare.to_vec(),
                external_changelog_prefixes: vec![],
                inventory_path_prefixes: inventory_path_prefixes.to_vec(),
                section_namespace: section_namespace.map(String::from),
            },
            entry_id_prefix: prefix.to_string(),
            orphan_ledger: orphan_ledger.to_vec(),
            symbol_resolvers: BTreeMap::new(),
            filter_id: filter_id.map(String::from),
            path_scope: None,
        };
        let snapshot = store.snapshot();
        validator.scan(
            &no_foreign_subtree(workspace_root, &validator.config),
            &snapshot,
        )
    }

    #[test]
    fn scan_round_number_plain() {
        assert_eq!(scan_round_number("254 rest"), Some("254".to_string()));
    }

    #[test]
    fn scan_round_number_with_fraction() {
        assert_eq!(scan_round_number("33.5)"), Some("33.5".to_string()));
    }

    #[test]
    fn scan_round_number_trailing_dot_not_consumed() {
        assert_eq!(scan_round_number("254. End"), Some("254".to_string()));
    }

    #[test]
    fn scan_round_number_rejects_non_digit_start() {
        assert_eq!(scan_round_number("foo"), None);
        assert_eq!(scan_round_number(""), None);
    }

    /// Round 638 — the resolver must reduce BOTH stored key shapes to the
    /// cited shape. The short-form leg is the one CLAUDE.md's hand-executed
    /// procedure got wrong (it matched `"Round NNN "` WITH a trailing space,
    /// so all 96 short-form entries read as hallucinated); if this leg ever
    /// regresses, a quarter of the ledger becomes uncitable again.
    #[test]
    fn normalize_entry_citation_reduces_both_stored_key_shapes() {
        assert_eq!(
            normalize_entry_citation("Round ", "Round 568").as_deref(),
            Some("Round 568"),
            "short-form key (no title) must resolve — the 96-entry class"
        );
        assert_eq!(
            normalize_entry_citation("Round ", "Round 293 — the title").as_deref(),
            Some("Round 293"),
            "long-form key must reduce to the cited shape"
        );
        // A citation and its own stored key must land on the same string —
        // that identity is what lets a caller compare the two sides.
        assert_eq!(
            normalize_entry_citation("Round ", "Round 293"),
            normalize_entry_citation("Round ", "Round 293 — the title"),
        );
    }

    /// Round 638 — the boundary that keeps the resolver from being the very
    /// bug it replaces: a shorter number must NOT match a longer one.
    #[test]
    fn normalize_entry_citation_does_not_collide_on_a_number_prefix() {
        assert_ne!(
            normalize_entry_citation("Round ", "Round 56"),
            normalize_entry_citation("Round ", "Round 568"),
            "`Round 56` must never resolve to `Round 568`"
        );
        // An alpha-suffixed key (the Round 474 base-26 column, e.g.
        // `Round 311aa`) REDUCES to its base number — pinned as the real
        // behaviour, not wished away: it is pre-existing and load-bearing for
        // the gate, which asks only "is this number a real round?". It is also
        // exactly why the single-entry READ (`ops::query_changelog_entry`)
        // fails loud when one citation resolves to several entries instead of
        // picking one — a silently-arbitrary decision is the class this round
        // exists to kill.
        assert_eq!(
            normalize_entry_citation("Round ", "Round 311aa").as_deref(),
            Some("Round 311"),
        );
        assert_eq!(
            normalize_entry_citation("Round ", "Round 311aa"),
            normalize_entry_citation("Round ", "Round 311"),
            "the reduction is why the read guards ambiguity"
        );
    }

    /// Round 638 — non-citations are rejected, never coerced.
    #[test]
    fn normalize_entry_citation_rejects_what_is_not_a_citation() {
        assert_eq!(normalize_entry_citation("Round ", "568"), None);
        assert_eq!(normalize_entry_citation("Round ", "Section 5"), None);
        assert_eq!(normalize_entry_citation("", "Round 5"), None);
    }

    /// Round 638 — the prefix is CONFIG-driven (`[schema].entry_id_prefix`),
    /// never the hardcoded `"Round "`: a workspace that names its entries
    /// differently must resolve through the same one resolver.
    #[test]
    fn normalize_entry_citation_honours_a_configured_prefix() {
        assert_eq!(
            normalize_entry_citation("Sprint ", "Sprint 12 — a title").as_deref(),
            Some("Sprint 12")
        );
        assert_eq!(normalize_entry_citation("Sprint ", "Round 12"), None);
    }

    #[test]
    fn extract_citations_basic() {
        let src = "// Round 254 carry\n// see Round 33.5 for sub-round\n";
        let out = extract_citations("Round ", src, &[]);
        assert_eq!(
            out,
            vec![(1, "Round 254".to_string()), (2, "Round 33.5".to_string())]
        );
    }

    #[test]
    fn extract_citations_skips_identifier_like() {
        let src = "TestRound254Helper\nlet round_254_helper = 1;\n";
        let out = extract_citations("Round ", src, &[]);
        assert_eq!(out, vec![]);
    }

    #[test]
    fn extract_citations_post_boundary_excludes_alphanumeric_tail() {
        let src = "see Round 254a here\n";
        let out = extract_citations("Round ", src, &[]);
        assert_eq!(out, vec![]);
    }

    #[test]
    fn extract_citations_brackets_and_parens_ok() {
        let src = "(Round 254) [Round 100] {Round 1}\n";
        let out = extract_citations("Round ", src, &[]);
        assert_eq!(
            out,
            vec![
                (1, "Round 254".to_string()),
                (1, "Round 100".to_string()),
                (1, "Round 1".to_string())
            ]
        );
    }

    #[test]
    fn extract_citations_external_prefix() {
        let src = "ADR-0042 implements ADR-7\n";
        let out = extract_citations("ADR-", src, &[]);
        assert_eq!(
            out,
            vec![(1, "ADR-0042".to_string()), (1, "ADR-7".to_string())]
        );
    }

    #[test]
    fn extract_citations_empty_prefix_yields_empty() {
        assert!(extract_citations("", "Round 254\n", &[]).is_empty());
    }

    /// Round 810 — the `Round NNN` axis gains the external escape hatch the
    /// section axis has had since Round 277. Both classes are pinned, and the
    /// LOCAL half is what makes the widening safe: an unnamed round still
    /// resolves against this workspace's ledger, so a genuinely hallucinated
    /// round is still caught.
    #[test]
    fn a_named_ledger_takes_a_round_out_of_this_workspaces_jurisdiction_r810() {
        let ledgers = vec!["mnemosyne".to_string()];
        // Named: another project's ledger, not a candidate here at all.
        let named = "//! mnemosyne Round 780 baked the projection.\n";
        assert!(
            extract_citations("Round ", named, &ledgers).is_empty(),
            "a named ledger must leave this axis; got: {:?}",
            extract_citations("Round ", named, &ledgers)
        );
        // THE GUARD: the same line without the name is ours, and stays ours.
        let unnamed = "//! Round 780 baked the projection.\n";
        assert_eq!(
            extract_citations("Round ", unnamed, &ledgers),
            vec![(1usize, "Round 780".to_string())],
            "an unnamed round is this ledger's and must still be checked"
        );
        // An UNREGISTERED name does not skip — the registry is the permission,
        // exactly as on the section axis.
        let other = "//! elsewhere Round 780 baked the projection.\n";
        assert_eq!(
            extract_citations("Round ", other, &ledgers),
            vec![(1usize, "Round 780".to_string())]
        );
        // And with no registry at all the axis is off: every round is local,
        // the pre-Round-810 behavior exactly.
        assert_eq!(
            extract_citations("Round ", named, &[]),
            vec![(1usize, "Round 780".to_string())]
        );
        // Two ledgers on one line, one named and one not.
        let mixed = "//! mnemosyne Round 780 is why Round 12 moved.\n";
        assert_eq!(
            extract_citations("Round ", mixed, &ledgers),
            vec![(1usize, "Round 12".to_string())]
        );
    }

    #[test]
    fn extract_citations_non_ascii_prefix_no_panic() {
        // A non-ASCII `entry_id_prefix` (no config rule forbids one) puts the
        // match offset `i` on a multibyte boundary. When the prefix is
        // preceded by an alphanumeric (word-boundary reject), the old
        // `start = i + 1` advance landed mid-codepoint and the next slice
        // panicked. The first occurrence is a clean citation; the second is
        // glued to `x` and must be skipped — without panicking.
        let src = "라운드 254 and x라운드 7\n";
        let out = extract_citations("라운드 ", src, &[]);
        assert_eq!(out, vec![(1, "라운드 254".to_string())]);
    }

    #[test]
    fn is_external_section_cite_numeric_multibyte_whitespace_no_panic() {
        // U+2028 LINE SEPARATOR is Unicode whitespace (3 bytes). The old
        // `rfind(char::is_whitespace).map(|i| i + 1)` landed mid-codepoint and
        // panicked. The token after it ("791") is numeric and "RFC" precedes
        // it, so the numeric axis must still match across the multibyte gap.
        let prefixes = vec!["RFC".to_string()];
        assert!(is_external_section_cite("RFC\u{2028}791 ", &prefixes, &[]));
    }

    #[test]
    fn is_external_section_cite_bare_multibyte_whitespace_no_panic() {
        // U+00A0 NO-BREAK SPACE is Unicode whitespace (2 bytes). The bare axis
        // splits on the last whitespace to isolate the trailing token; the
        // advance must clear the full multibyte width, not +1.
        let bare = vec!["TR_SOMEIP".to_string()];
        assert!(is_external_section_cite("x\u{00A0}TR_SOMEIP ", &[], &bare));
    }

    // ============ §<id> extractor unit tests ============

    #[test]
    fn extract_section_citations_basic_numeric() {
        let src = "// §39 carry\n// also §61 for context\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "39".to_string()), (2, "61".to_string())]);
    }

    #[test]
    fn extract_section_citations_fractional_id() {
        let src = "// see §61.1 for sub-section\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "61.1".to_string())]);
    }

    #[test]
    fn extract_section_citations_slash_slug() {
        let src = "// §atomic-store/changelog-atomic-ledger anchor\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(
            out,
            vec![(1, "atomic-store/changelog-atomic-ledger".to_string())]
        );
    }

    /// Round 799 — `§1/§3` is two cites, not one broken id and one good one.
    ///
    /// Reported by a downstream workspace, which met it as a `SectionMissing`
    /// violation — the HALLUCINATION class — on a comment that cited two real
    /// sections. Their recommended repair was to close the id charset to
    /// numerics, which would have broken the slash slugs this very store uses;
    /// the rule landed instead is that a separator is interior or it is not part
    /// of the id. The slug case below is what the narrower rule would have cost.
    #[test]
    fn extract_section_citations_slash_between_cites_is_a_separator() {
        for (src, want) in [
            ("// a §1/§3 pair\n", vec!["1", "3"]),
            ("// b §1 / §3 spaced\n", vec!["1", "3"]),
            ("// d (§2/§4)\n", vec!["2", "4"]),
            // The shape this workspace's own ids take.
            ("// e §5.39/§6.3 pair\n", vec!["5.39", "6.3"]),
            // A trailing slash at end of line has nothing after it either.
            ("// f §7/\n", vec!["7"]),
        ] {
            let got: Vec<String> = extract_section_citations(src, &[], &[])
                .into_iter()
                .map(|(_, id)| id)
                .collect();
            assert_eq!(got, want, "for {src:?}");
        }
    }

    /// Round 800 — the rule is on the character CLASS, and the oracle for that
    /// is derived from the class rather than typed out.
    ///
    /// Round 799 fixed `/` alone, which was a hand list of one: `-` had the
    /// identical defect and is the MORE common prose separator, since `§1-§3`
    /// is how a range is written. A test naming the separators it checks would
    /// have passed then too. This one asks `is_section_id_char` which characters
    /// are separators and checks every one, so a character added to the charset
    /// later arrives here already covered.
    #[test]
    fn extract_section_citations_no_separator_may_end_an_id() {
        let separators: Vec<char> = (0u8..=127)
            .map(char::from)
            .filter(|c| !c.is_ascii_alphanumeric() && is_section_id_char(*c))
            .collect();
        assert_eq!(
            separators,
            vec!['-', '.', '/', '_'],
            "the charset moved; this test derives from it, but the assertion \
             below documents what it derived"
        );
        for sep in separators {
            let src = format!("// §1{sep}§3\n");
            let got: Vec<String> = extract_section_citations(&src, &[], &[])
                .into_iter()
                .map(|(_, id)| id)
                .collect();
            assert_eq!(got, vec!["1", "3"], "separator {sep:?} was swallowed");
        }
    }

    /// Round 799 — and the slug still parses whole, which is the half that
    /// stops the fix from being a different false positive. `§1/x` stays one id
    /// for the same reason: with an id char on both sides the slash IS interior,
    /// and nothing in the token can tell a two-segment slug from a pair.
    #[test]
    fn extract_section_citations_interior_slash_still_belongs_to_the_id() {
        for (src, want) in [
            (
                "// §atomic-store/changelog-atomic-ledger anchor\n",
                vec!["atomic-store/changelog-atomic-ledger"],
            ),
            (
                "// §code-citation-defense/bidirectional-binding\n",
                vec!["code-citation-defense/bidirectional-binding"],
            ),
            ("// c §1/x trailing\n", vec!["1/x"]),
            // Round 800 — the kebab shape is what the `-` half must not cost.
            (
                "// §atomic-store-mutate-api\n",
                vec!["atomic-store-mutate-api"],
            ),
            ("// §5.39 fractional\n", vec!["5.39"]),
        ] {
            let got: Vec<String> = extract_section_citations(src, &[], &[])
                .into_iter()
                .map(|(_, id)| id)
                .collect();
            assert_eq!(got, want, "for {src:?}");
        }
    }

    #[test]
    fn extract_section_citations_trailing_dot_not_consumed() {
        let src = "End of sentence §39. Next line\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "39".to_string())]);
    }

    #[test]
    fn extract_section_citations_brackets_and_parens() {
        let src = "(§39) [§61.1] {§atomic-store}\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(
            out,
            vec![
                (1, "39".to_string()),
                (1, "61.1".to_string()),
                (1, "atomic-store".to_string())
            ]
        );
    }

    #[test]
    fn extract_section_citations_solitary_sigil_no_id_skipped() {
        let src = "Just a § sigil with no id following\n";
        let out = extract_section_citations(src, &[], &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn extract_section_citations_underscore_allowed() {
        let src = "// §atomic_store snake case slug\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "atomic_store".to_string())]);
    }

    // ============ bidirectional scan integration tests ============

    fn build_store_with_impl(
        path: &Path,
        section_id: &str,
        impl_file: &str,
        symbol: Option<&str>,
    ) -> AtomicStore {
        let mut store = AtomicStore::new();
        // Round 287 fail-loud: seed Section before add_section_binding
        // (test fixture path — direct insert bypasses audit-receipt overhead).
        store.sections.insert(
            section_id.into(),
            mnemosyne_atomic::AtomicSection::default(),
        );
        add_section_binding(
            &mut store,
            path,
            section_id,
            impl_file,
            symbol,
            BindingKind::Implements,
        )
        .unwrap();
        store
    }

    /// Round 855 — the symbol axis says what it cannot reach, and `.c` is no
    /// longer one of those things.
    ///
    /// Reported from the field: `lang_for_file` had no `.c` arm, so a C runtime
    /// took file-level binding on its `.c` files and symbol-level on the `.h`
    /// files beside them, silently, under a config that says
    /// `severity_binding = reject`. Three destinations in one fixture because
    /// the distinction is the point — a file the axis reaches, a language it
    /// could reach but has no resolver for, and an extension no resolver can
    /// ever apply to — and `.c` moved between the first and the last.
    #[test]
    fn the_symbol_axis_reports_every_file_it_cannot_reach() {
        use mnemosyne_core::AtomicStoreView;
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        // Reachable: a cpp resolver is configured below, and `.c` maps to it.
        std::fs::write(src.join("runtime.c"), "// §39 cite in C\n").unwrap();
        std::fs::write(src.join("runtime.h"), "// §39 cite in a header\n").unwrap();
        // Mapped to a language, no resolver configured for it.
        std::fs::write(src.join("gen.py"), "# §39 cite in python\n").unwrap();
        // No language for the extension, and no citation either — the pair of
        // counts must tell these two apart.
        std::fs::write(src.join("README.md"), "§39 cite in prose\n").unwrap();
        std::fs::write(src.join("Cargo.toml"), "[package]\n").unwrap();

        let mut store = AtomicStore::new();
        store
            .sections
            .insert("39".into(), mnemosyne_atomic::AtomicSection::default());
        let mut resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>> =
            BTreeMap::new();
        resolvers.insert(
            "cpp".to_string(),
            Box::new(mnemosyne_plugin_tree_sitter_cpp::resolver()),
        );
        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                scan_exclusions: Vec::new(),
                paths: vec!["src/".to_string()],
                severity_missing: mnemosyne_config::Severity::Reject,
                severity_binding: mnemosyne_config::Severity::Reject,
                severity_coverage: None,
                severity_verification: None,
                severity_confirmation: None,
                severity_classification: None,
                severity_blanket: None,
                severity_prose_fact_assertion: None,
                severity_inventory: mnemosyne_config::Severity::Reject,
                comment_only: true,
                inventory_prefixes: vec![],
                external_section_prefixes: vec![],
                external_section_prefixes_bare: vec![],
                external_changelog_prefixes: vec![],
                inventory_path_prefixes: vec![],
                section_namespace: None,
            },
            entry_id_prefix: "Round ".to_string(),
            orphan_ledger: vec![],
            symbol_resolvers: resolvers,
            filter_id: None,
            path_scope: None,
        };
        let cov = validator
            .symbol_axis_coverage(
                &no_foreign_subtree(tmp.path(), &validator.config),
                &store.snapshot(),
            )
            .unwrap();

        assert_eq!(
            cov.covered.get("cpp").map(|c| (c.files, c.citing_files)),
            Some((2, 2)),
            "`.c` and `.h` both reach the cpp resolver, and both cite: {cov:?}"
        );
        assert_eq!(
            cov.unresolved_languages
                .get("python")
                .map(|c| (c.files, c.citing_files)),
            Some((1, 1)),
            "python is mapped but unconfigured, and the file carries a gated \
             citation — this is the coverage the reject-level knob does not \
             have: {cov:?}"
        );
        assert_eq!(
            cov.unmapped_extensions
                .get("md")
                .map(|c| (c.files, c.citing_files)),
            Some((1, 1)),
            "prose the gate reads but no resolver can parse: {cov:?}"
        );
        assert_eq!(
            cov.unmapped_extensions
                .get("toml")
                .map(|c| (c.files, c.citing_files)),
            Some((1, 0)),
            "an unreachable file with no citation costs no coverage, and the \
             report must say so rather than counting it as a gap: {cov:?}"
        );
        assert_eq!(
            cov.unreachable_citing_files(),
            2,
            "the python file and the markdown file, not the toml: {cov:?}"
        );
    }

    /// EVERY AXIS PUBLISHES WHAT IT READ (Round 1167).
    ///
    /// Round 1158 gave the symbol axis its payload and left a carry: the other
    /// seven axes carry nothing, and two of them read something they then throw
    /// away. `citation_unbound` looks up the files the section DOES bind and
    /// keeps only the boolean; `prose_fact_assertion` matches a verb and binds
    /// it to `_verb`. A consumer meeting either has to open the store or
    /// re-derive our verb list — the same file-opening Round 1158 spared on the
    /// symbol axis.
    ///
    /// Over the SCAN's own output, and on BOTH surfaces: the machine wire and
    /// the line a person reads (the R1045 lesson — a claim proved only against
    /// `--json` leaves the human line free to say less, and the reader of that
    /// line is exactly the one who would otherwise go looking).
    #[test]
    fn every_citation_axis_publishes_what_it_read() {
        use mnemosyne_core::AtomicStoreView;
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        // Bound, and the grammar answers a name the store does not record.
        std::fs::write(
            src.join("drift.rs"),
            "struct H;\n\nimpl H {\n    fn alpha(&self) {\n        // §39 cite\n    }\n}\n",
        )
        .unwrap();
        // Cites a section that binds a DIFFERENT file.
        std::fs::write(src.join("unbound.rs"), "// §39 cite from nowhere\n").unwrap();
        // Restates in prose a fact the store homes.
        std::fs::write(
            src.join("prose.rs"),
            "// supersede §39, which the store already records\n",
        )
        .unwrap();
        // Cites an id the store does not hold — an axis that reads nothing.
        std::fs::write(src.join("hallucinated.rs"), "// §404 cite of nothing\n").unwrap();

        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/drift.rs", Some("beta"));

        let mut resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>> =
            BTreeMap::new();
        resolvers.insert(
            "rust".to_string(),
            Box::new(mnemosyne_plugin_tree_sitter_rust::resolver()),
        );
        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                scan_exclusions: Vec::new(),
                paths: vec!["src/".to_string()],
                severity_missing: mnemosyne_config::Severity::Reject,
                severity_binding: mnemosyne_config::Severity::Reject,
                severity_coverage: None,
                severity_verification: None,
                severity_confirmation: None,
                severity_classification: None,
                severity_blanket: None,
                severity_prose_fact_assertion: Some(mnemosyne_config::Severity::Reject),
                severity_inventory: mnemosyne_config::Severity::Reject,
                comment_only: true,
                inventory_prefixes: vec![],
                external_section_prefixes: vec![],
                external_section_prefixes_bare: vec![],
                external_changelog_prefixes: vec![],
                inventory_path_prefixes: vec![],
                section_namespace: None,
            },
            entry_id_prefix: "Round ".to_string(),
            orphan_ledger: vec![],
            symbol_resolvers: resolvers,
            filter_id: None,
            path_scope: None,
        };
        let snapshot = store.snapshot();
        let violations = validator
            .scan(
                &no_foreign_subtree(tmp.path(), &validator.config),
                &snapshot,
            )
            .expect("the scan runs");

        let wire = |tag: &str| -> Vec<serde_json::Value> {
            violations
                .iter()
                .filter(|v| v.kind_tag() == tag)
                .map(|v| v.to_cli_json())
                .collect()
        };
        let lines = |tag: &str| -> Vec<String> {
            violations
                .iter()
                .filter(|v| v.kind_tag() == tag)
                .map(|v| v.to_string())
                .collect()
        };

        // ---- citation_unbound: the files the section DOES bind ----
        let unbound = wire("citation_unbound");
        assert!(
            !unbound.is_empty(),
            "the fixture must reach the unbound axis, or the claims below are \
             about an empty set: {violations:?}"
        );
        for v in &unbound {
            assert_eq!(
                v.get("bound_files"),
                Some(&serde_json::json!(["src/drift.rs"])),
                "an unbound citation must name what the section binds instead: {v}"
            );
        }
        for line in lines("citation_unbound") {
            assert!(
                line.contains("src/drift.rs"),
                "the human line must name it too: {line}"
            );
        }

        // ---- prose_fact_assertion: the verb that tripped the rule ----
        let prose = wire("prose_fact_assertion");
        assert_eq!(
            prose.len(),
            1,
            "the fixture must reach the prose axis exactly once: {violations:?}"
        );
        assert_eq!(
            prose[0].get("assertion_verb"),
            Some(&serde_json::json!("supersede")),
            "a prose-fact violation must name the verb it matched: {}",
            prose[0]
        );
        for line in lines("prose_fact_assertion") {
            assert!(
                line.contains("supersede"),
                "the human line must name the verb too: {line}"
            );
        }

        // ---- symbol_mismatch: Round 1158's pair, unchanged ----
        let drift = wire("symbol_mismatch");
        assert_eq!(drift.len(), 1, "one drift: {violations:?}");
        assert_eq!(drift[0].get("found"), Some(&serde_json::json!("alpha")));
        assert_eq!(drift[0].get("expected"), Some(&serde_json::json!(["beta"])));

        // ---- section_missing: an axis that reads nothing says nothing ----
        let missing = wire("section_missing");
        assert_eq!(
            missing.len(),
            1,
            "the fixture must reach an axis that reads NOTHING, or absence is \
             never exercised: {violations:?}"
        );
        for key in ["found", "expected", "bound_files", "assertion_verb"] {
            assert_eq!(
                missing[0].get(key),
                None,
                "an axis that read nothing must claim nothing (`{key}`): {}",
                missing[0]
            );
        }

        // ---- and the shape of every one of them is the DECLARED shape ----
        // This is the pin Round 1158 put on a one-kind `Option`, generalised:
        // the payload is not "whatever the emit site felt like attaching" but
        // exactly what `AuditAxis::evidence` says that axis carries, so an axis
        // that starts dropping its evidence — or inventing some — reddens here.
        for v in &violations {
            if let CodeRefViolation::Citation { evidence, .. } = v {
                assert_eq!(
                    CitationEvidence::shape_of(evidence.as_ref()),
                    v.axis().evidence(),
                    "a violation must carry the evidence its axis declares: {v:?}"
                );
            }
        }

        // NON-VACUITY, DERIVED FROM THE TABLE RATHER THAN SPELLED. Every
        // citation-side axis that declares evidence must be one this fixture
        // actually produced: a fourth payload added tomorrow is a red test here
        // until the tree above reaches it, which is the difference between a law
        // about three axes and a law about the axes that have evidence. The
        // `Nothing` half is required too — without it the equality above holds
        // trivially on a run where nothing declared anything.
        let reached: BTreeSet<AuditAxis> = violations.iter().map(|v| v.axis()).collect();
        let declaring: BTreeSet<AuditAxis> = AuditAxis::all()
            .into_iter()
            .filter(|a| a.side() == AuditSide::Citation)
            .filter(|a| a.evidence() != EvidenceShape::Nothing)
            .collect();
        assert!(
            declaring.is_subset(&reached),
            "every axis that declares evidence must be exercised here — missing \
             {:?} from {reached:?}",
            declaring.difference(&reached).collect::<Vec<_>>()
        );
        assert!(
            reached
                .iter()
                .any(|a| a.side() == AuditSide::Citation && a.evidence() == EvidenceShape::Nothing),
            "and an axis that declares none, or `None` is never exercised: \
             {reached:?}"
        );
    }

    /// Round 855 — the legal `[plugins.symbol_resolver.<lang>]` keys are derived
    /// from the extension table, so a config naming `c` — the obvious
    /// workaround for a `.c` tree — is refusable instead of dead.
    ///
    /// Round 1155 — THE ABSENCE HALF IS DERIVED NOW, NOT SPELLED. This asserted
    /// `!langs.contains("kotlin")` as its example of a name no extension maps
    /// to, and the round that added `.kt` made that sentence false. Naming a
    /// different language in its place would replant the same clock; the claim
    /// that does not decay is the EQUALITY — the key set is exactly the table's
    /// range — plus the rule the `c` case is an instance of: an extension whose
    /// spelling differs from its language is never itself a key.
    #[test]
    fn the_symbol_axis_language_set_comes_from_the_extension_table() {
        let langs = symbol_axis_languages();
        let range: BTreeSet<&str> = symbol_axis_extensions()
            .iter()
            .map(|(_, lang)| *lang)
            .collect();
        assert_eq!(
            langs, range,
            "the key set must be the extension table's range and nothing else"
        );
        assert!(
            langs.contains("cpp") && langs.contains("rust"),
            "non-vacuity: the range is a real set, not an empty one: {langs:?}"
        );
        for (ext, lang) in symbol_axis_extensions() {
            if ext == lang {
                continue;
            }
            assert!(
                !langs.contains(ext),
                "`{ext}` is an EXTENSION, not a language key — it maps to \
                 `{lang}`, and a resolver keyed `{ext}` would never be \
                 consulted: {langs:?}"
            );
        }
    }

    #[test]
    fn citation_index_groups_by_section_excludes_hallucinations() {
        use mnemosyne_core::AtomicStoreView;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §39 first cite\n// §39 second cite\n// §999 not a section\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("src/bar.rs"), "// §39 cite in bar\n").unwrap();

        // Only section id 39 exists in the store; id 999 is hallucinated.
        let mut store = AtomicStore::new();
        store
            .sections
            .insert("39".into(), mnemosyne_atomic::AtomicSection::default());

        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                scan_exclusions: Vec::new(),
                paths: vec!["src/".to_string()],
                severity_missing: mnemosyne_config::Severity::Reject,
                severity_binding: mnemosyne_config::Severity::Reject,
                severity_coverage: None,
                severity_verification: None,
                severity_confirmation: None,
                severity_classification: None,
                severity_blanket: None,
                severity_prose_fact_assertion: None,
                severity_inventory: mnemosyne_config::Severity::Reject,
                comment_only: true,
                inventory_prefixes: vec![],
                external_section_prefixes: vec![],
                external_section_prefixes_bare: vec![],
                external_changelog_prefixes: vec![],
                inventory_path_prefixes: vec![],
                section_namespace: None,
            },
            entry_id_prefix: "Round ".to_string(),
            orphan_ledger: vec![],
            symbol_resolvers: BTreeMap::new(),
            filter_id: None,
            path_scope: None,
        };
        let snapshot = store.snapshot();
        let index = validator
            .citation_index(
                &no_foreign_subtree(tmp.path(), &validator.config),
                &snapshot,
            )
            .unwrap();

        // id 999 not in the store: excluded (hallucination, not density).
        assert_eq!(index.len(), 1, "got: {:?}", index);
        let sites = &index["39"];
        assert_eq!(sites.len(), 3);
        // Sorted by (file, line): bar before foo, then foo lines ascending.
        assert_eq!(
            sites[0],
            CitationSite {
                file: "src/bar.rs".into(),
                line: 1
            }
        );
        assert_eq!(
            sites[1],
            CitationSite {
                file: "src/foo.rs".into(),
                line: 1
            }
        );
        assert_eq!(
            sites[2],
            CitationSite {
                file: "src/foo.rs".into(),
                line: 2
            }
        );
    }

    #[test]
    fn bidirectional_clean_codebase_no_violations() {
        // cite in src/foo.rs +.bindings contains src/foo.rs.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/foo.rs", Some("Foo"));
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §39 — Foo binds here\nfn main() {}\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(v.is_empty(), "unexpected violations: {:?}", v);
    }

    #[test]
    fn references_binding_satisfies_citation_but_not_coverage() {
        // A section bound ONLY by a `references` («trace») binding: the cite is
        // defended (no citation_unbound — presence is kind-agnostic) and the
        // binding's file is cited (no binding_unbacked), but coverage counts only
        // `implements`, so the section still trips impl_missing.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        store
            .sections
            .insert("39".into(), mnemosyne_atomic::AtomicSection::default());
        add_section_binding(
            &mut store,
            &store_path,
            "39",
            "src/foo.rs",
            Some("Foo"),
            BindingKind::References,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §39 — Foo references here\nfn main() {}\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            !v.iter().any(|x| x.kind_tag() == "citation_unbound"),
            "references binding must defend the citation: {:?}",
            v
        );
        assert!(
            !v.iter().any(|x| x.kind_tag() == "binding_unbacked"),
            "the cited references binding must not be binding_unbacked: {:?}",
            v
        );
        assert!(
            v.iter().any(|x| x.kind_tag() == "impl_missing"),
            "references-only section has no implements coverage → impl_missing: {:?}",
            v
        );
    }

    #[test]
    fn informative_section_exempt_from_coverage_axiom() {
        // Round 389: a Normative section with zero implements bindings trips
        // impl_missing (the R269 axiom); an Informative section (prose-only,
        // nothing to implement here) is exempt — no impl_missing for it.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        store
            .sections
            .insert("norm".into(), mnemosyne_atomic::AtomicSection::default());
        store
            .sections
            .insert("info".into(), mnemosyne_atomic::AtomicSection::default());
        set_section_coverage_expectation(
            &mut store,
            &store_path,
            "info",
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            "terminology — nothing to implement here",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "fn main() {}\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        let impl_missing_ids: Vec<&str> = v
            .iter()
            .filter_map(|x| match x {
                CodeRefViolation::ImplementationMissing { section_id, .. } => {
                    Some(section_id.as_str())
                }
                _ => None,
            })
            .collect();
        assert!(
            impl_missing_ids.contains(&"norm"),
            "Normative section with zero implements trips impl_missing: {:?}",
            v
        );
        assert!(
            !impl_missing_ids.contains(&"info"),
            "Informative section is exempt from the coverage axiom: {:?}",
            v
        );
    }

    #[test]
    fn verifies_binding_excluded_from_set_equality_and_coverage() {
        // A `verifies` binding points at a test artifact whose link to the
        // section is externally mapped (e.g. a conformance manifest), not a
        // §<id> citation. So the test file is NOT required to cite the section
        // (no binding_unbacked), and verifies does not satisfy the implements
        // coverage axiom (impl_missing still fires when it is the only binding).
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        store
            .sections
            .insert("39".into(), mnemosyne_atomic::AtomicSection::default());
        add_section_binding(
            &mut store,
            &store_path,
            "39",
            "tests/conformance/test144.rs",
            Some("fn test144"),
            BindingKind::Verifies,
        )
        .unwrap();
        // src/ is the only scanned code path; the verifies test artifact lives
        // outside it and never cites the section — the externally-mapped link.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "fn main() {}\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            !v.iter().any(|x| x.kind_tag() == "binding_unbacked"),
            "verifies binding's test file must not require a §-citation: {:?}",
            v
        );
        assert!(
            v.iter().any(|x| matches!(
                x,
                CodeRefViolation::ImplementationMissing { section_id, .. } if section_id == "39"
            )),
            "verifies-only section still trips impl_missing (verifies != coverage): {:?}",
            v
        );
    }

    /// Drive the verify axis (Step 5) directly with a chosen
    /// `severity_verification`, bypassing the always-off test helper.
    fn scan_verify_axis(
        workspace_root: &Path,
        store: &AtomicStore,
        severity_verification: Option<&str>,
    ) -> Vec<CodeRefViolation> {
        use mnemosyne_core::AtomicStoreView;
        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                scan_exclusions: Vec::new(),
                paths: vec![],
                severity_missing: mnemosyne_config::Severity::Reject,
                severity_binding: mnemosyne_config::Severity::Reject,
                severity_coverage: None,
                severity_verification: severity_verification
                    .map(|s| mnemosyne_config::Severity::from_tag(s).expect("valid severity tag")),
                severity_confirmation: None,
                severity_classification: None,
                severity_blanket: None,
                severity_prose_fact_assertion: None,
                severity_inventory: mnemosyne_config::Severity::Reject,
                comment_only: true,
                inventory_prefixes: vec![],
                external_section_prefixes: vec![],
                external_section_prefixes_bare: vec![],
                external_changelog_prefixes: vec![],
                inventory_path_prefixes: vec![],
                section_namespace: None,
            },
            entry_id_prefix: "Round ".to_string(),
            orphan_ledger: vec![],
            symbol_resolvers: BTreeMap::new(),
            filter_id: None,
            path_scope: None,
        };
        validator
            .scan(
                &no_foreign_subtree(workspace_root, &validator.config),
                &store.snapshot(),
            )
            .unwrap()
    }

    fn verification_missing_ids(v: &[CodeRefViolation]) -> Vec<&str> {
        v.iter()
            .filter_map(|x| match x {
                CodeRefViolation::VerificationMissing { section_id, .. } => {
                    Some(section_id.as_str())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn verification_axis_gate_fires_only_for_dedicated_without_verifies() {
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // Two Normative + Dedicated, no-verifies sections → the gaps. Two of
        // them exercise the (VerificationMissing, VerificationMissing) sort
        // tiebreaker (a single gap would not).
        store.sections.insert(
            "ded-novf".into(),
            mnemosyne_atomic::AtomicSection::default(),
        );
        store.sections.insert(
            "ded-novf2".into(),
            mnemosyne_atomic::AtomicSection::default(),
        );
        // Normative + Dedicated, WITH a verifies binding → satisfied.
        store
            .sections
            .insert("ded-vf".into(), mnemosyne_atomic::AtomicSection::default());
        add_section_binding(
            &mut store,
            &store_path,
            "ded-vf",
            "tests/t.rs",
            None,
            BindingKind::Verifies,
        )
        .unwrap();
        // Normative + ByConstruction → exempt.
        store
            .sections
            .insert("bycon".into(), mnemosyne_atomic::AtomicSection::default());
        set_section_verification_expectation(
            &mut store,
            &store_path,
            "bycon",
            VerificationExpectation::ByConstruction,
            "transcribed pseudocode, holistic coverage",
        )
        .unwrap();
        // Informative → exempt (not Normative).
        store
            .sections
            .insert("info".into(), mnemosyne_atomic::AtomicSection::default());
        set_section_coverage_expectation(
            &mut store,
            &store_path,
            "info",
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            "glossary",
        )
        .unwrap();

        // Axis OFF (severity_verification None): no VerificationMissing at all.
        let off = scan_verify_axis(tmp.path(), &store, None);
        assert!(
            verification_missing_ids(&off).is_empty(),
            "verify axis off must emit no VerificationMissing: {:?}",
            off
        );

        // Axis ON: fires for exactly the Dedicated-without-verifies section.
        let on = scan_verify_axis(tmp.path(), &store, Some("reject"));
        assert_eq!(
            verification_missing_ids(&on),
            vec!["ded-novf", "ded-novf2"],
            "verify gate must fire only for Normative+Dedicated+0-verifies (sorted): {:?}",
            on
        );
    }

    // ============ Round 390: report-coverage positive projection ============

    #[test]
    fn classify_coverage_partitions_all_four_classes() {
        // The positive projection assigns every section to exactly one of the
        // four coverage classes, and the ratio is over the applicable
        // (Normative, non-Removed) set only.
        use mnemosyne_core::{
            AtomicSnapshot, BindingKind as Bk, BindingRef, CoverageExpectation as Ce,
            DecisionStatus, SectionView,
        };
        let view = |status: Option<DecisionStatus>, exp: Ce, kinds: &[Bk]| SectionView {
            bindings: kinds
                .iter()
                .map(|&k| BindingRef {
                    file: "f.rs".to_string(),
                    symbol: None,
                    kind: k,
                })
                .collect(),
            decision_status: status,
            coverage_expectation: exp,
            verification_expectation: Default::default(),
        };
        let mut snap = AtomicSnapshot::default();
        // Normative + an implements binding → implemented.
        snap.sections.insert(
            "impl".to_string(),
            view(
                Some(DecisionStatus::Active),
                Ce::Normative,
                &[Bk::Implements],
            ),
        );
        // Normative, references-only (no implements) → normative gap.
        snap.sections.insert(
            "gap".to_string(),
            view(None, Ce::Normative, &[Bk::References]),
        );
        // Informative + live → exempt.
        snap.sections
            .insert("info".to_string(), view(None, Ce::OutOfScopeHere, &[]));
        // Removed Normative with no coverage → excluded, NOT a gap.
        snap.sections.insert(
            "dead".to_string(),
            view(Some(DecisionStatus::Removed), Ce::Normative, &[]),
        );
        // Open (not-yet-decided) Normative with no coverage → lifecycle-excluded
        // like Removed, NOT a normative gap (is_axiom_exempt, R578).
        snap.sections.insert(
            "openq".to_string(),
            view(Some(DecisionStatus::Open), Ce::Normative, &[]),
        );
        let r = classify_coverage(&snap);
        assert_eq!(r.implemented, vec!["impl".to_string()]);
        assert_eq!(r.normative_gap, vec!["gap".to_string()]);
        assert_eq!(r.informative_exempt, vec!["info".to_string()]);
        assert_eq!(
            r.removed_excluded,
            vec!["dead".to_string(), "openq".to_string()]
        );
        assert_eq!(r.applicable(), 2);
        assert_eq!(r.coverage_ratio(), Some(0.5));
    }

    #[test]
    fn coverage_invariant_flags_exempt_with_implements_or_verifies() {
        // R423 — design sec 6 invariant: an exempt section must NOT carry an
        // implements/verifies binding. This is the gate that was MISSING and let
        // SCE's scxml-3.11 (out_of_scope + implements) and 6.4.4 (exempt +
        // verifies) through.
        use mnemosyne_core::{
            BindingKind as Bk, BindingRef, CoverageExpectation as Ce, SectionView,
        };
        let view = |exp: Ce, kinds: &[Bk]| SectionView {
            bindings: kinds
                .iter()
                .map(|&k| BindingRef {
                    file: "f.rs".to_string(),
                    symbol: None,
                    kind: k,
                })
                .collect(),
            decision_status: None,
            coverage_expectation: exp,
            verification_expectation: Default::default(),
        };
        // exempt + implements → misclassified (the scxml-3.11 shape)
        assert!(is_coverage_misclassified(&view(
            Ce::OutOfScopeHere,
            &[Bk::Implements]
        )));
        // exempt + verifies → misclassified (the 6.4.4 shape)
        assert!(is_coverage_misclassified(&view(
            Ce::Informational,
            &[Bk::Verifies]
        )));
        // exempt + references only → clean (a trace edge is allowed on exempt)
        assert!(!is_coverage_misclassified(&view(
            Ce::OutOfScopeHere,
            &[Bk::References]
        )));
        // Normative + implements → clean (correctly classified)
        assert!(!is_coverage_misclassified(&view(
            Ce::Normative,
            &[Bk::Implements]
        )));
        // exempt + no bindings → clean
        assert!(!is_coverage_misclassified(&view(Ce::Informational, &[])));
    }

    #[test]
    fn blanket_verifies_flags_one_artifact_bound_to_many_sections() {
        // R425 (SCE field-report P1) — one test artifact verifies-bound to >1
        // section is the blanket smell (the Test215-on-five-siblings shape). One
        // violation per artifact, sorted section list; same-artifact references
        // bindings and single-section verifies stay clean.
        use mnemosyne_core::{
            AtomicSnapshot, BindingKind as Bk, BindingRef, CoverageExpectation as Ce, SectionView,
        };
        let sec = |kinds: &[(&str, Bk)]| SectionView {
            bindings: kinds
                .iter()
                .map(|&(file, kind)| BindingRef {
                    file: file.to_string(),
                    symbol: None,
                    kind,
                })
                .collect(),
            decision_status: None,
            coverage_expectation: Ce::Normative,
            verification_expectation: Default::default(),
        };
        let mut snap = AtomicSnapshot::default();
        // Test215 stamped onto two sibling sections → flagged once.
        snap.sections
            .insert("6.4.1".into(), sec(&[("t/Test215.h", Bk::Verifies)]));
        snap.sections
            .insert("6.4.2".into(), sec(&[("t/Test215.h", Bk::Verifies)]));
        // A single-section verifies → clean.
        snap.sections
            .insert("5.1".into(), sec(&[("t/Test100.h", Bk::Verifies)]));
        // The same file referenced (not verifies) from many sections → clean.
        snap.sections
            .insert("a".into(), sec(&[("src/lib.rs", Bk::References)]));
        snap.sections
            .insert("b".into(), sec(&[("src/lib.rs", Bk::References)]));
        let hits = scan_blanket_verifies(&snap);
        assert_eq!(hits.len(), 1, "exactly the blanket artifact: {hits:?}");
        match &hits[0] {
            CodeRefViolation::BlanketVerifies {
                file, section_ids, ..
            } => {
                assert_eq!(file.to_string_lossy(), "t/Test215.h");
                assert_eq!(section_ids, &vec!["6.4.1".to_string(), "6.4.2".to_string()]);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn classify_coverage_ratio_none_when_no_applicable_section() {
        // An empty or all-Informative ledger has no coverage obligation to
        // express as a percentage → ratio is None (not a 0/0 panic or 100%).
        use mnemosyne_core::{AtomicSnapshot, CoverageExpectation, SectionView};
        let mut snap = AtomicSnapshot::default();
        snap.sections.insert(
            "info".to_string(),
            SectionView {
                bindings: Vec::new(),
                decision_status: None,
                coverage_expectation: CoverageExpectation::OutOfScopeHere,
                verification_expectation: Default::default(),
            },
        );
        let r = classify_coverage(&snap);
        assert_eq!(r.applicable(), 0);
        assert_eq!(r.coverage_ratio(), None);
    }

    #[test]
    fn report_coverage_normative_gap_matches_impl_missing() {
        // Single-source guarantee: the positive projection's normative_gap set
        // is byte-for-byte the same section ids the scan emits as impl_missing.
        // Both route through `classify_section_coverage`, so this locks them
        // against drift. Mix all four classes in one store.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        // implemented: Normative + implements binding.
        let mut store = build_store_with_impl(&store_path, "bound", "src/foo.rs", Some("Foo"));
        // normative gap: Normative, zero bindings.
        store
            .sections
            .insert("gap".into(), mnemosyne_atomic::AtomicSection::default());
        // informative exempt.
        store
            .sections
            .insert("info".into(), mnemosyne_atomic::AtomicSection::default());
        set_section_coverage_expectation(
            &mut store,
            &store_path,
            "info",
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            "terminology — nothing to implement here",
        )
        .unwrap();
        // removed: Normative, zero bindings, but tombstoned → excluded.
        store
            .sections
            .insert("dead".into(), mnemosyne_atomic::AtomicSection::default());
        mnemosyne_atomic::set_section_decision_status(
            &mut store,
            &store_path,
            "dead",
            mnemosyne_core::DecisionStatus::Removed,
            None,
            None,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §bound — Foo binds here\nfn main() {}\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        let mut scan_gap: Vec<String> = v
            .iter()
            .filter_map(|x| match x {
                CodeRefViolation::ImplementationMissing { section_id, .. } => {
                    Some(section_id.clone())
                }
                _ => None,
            })
            .collect();
        scan_gap.sort();
        let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
        let report = classify_coverage(&snapshot);
        assert_eq!(
            report.normative_gap, scan_gap,
            "positive projection gap set must equal scan impl_missing set"
        );
        assert_eq!(report.normative_gap, vec!["gap".to_string()]);
        assert_eq!(report.implemented, vec!["bound".to_string()]);
        assert_eq!(report.informative_exempt, vec!["info".to_string()]);
        assert_eq!(report.removed_excluded, vec!["dead".to_string()]);
    }

    #[test]
    fn bidirectional_section_missing_when_id_not_in_store() {
        // cite but no in the store.
        let tmp = TempDir::new().unwrap();
        let store = AtomicStore::new();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// see §999 hallucinated\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1);
        match &v[0] {
            CodeRefViolation::Citation { citation, kind, .. } => {
                assert_eq!(*kind, ViolationKind::SectionMissing);
                assert_eq!(citation.entry_id, "§999");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn bidirectional_citation_unbound_when_file_not_in_impls() {
        // exists with impl src/bar.rs, but src/foo.rs cites.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/bar.rs", None);
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §39 — unauthorized cite\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("src/bar.rs"), "// §39 — authoritative\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { citation, kind, .. } => {
                assert_eq!(*kind, ViolationKind::CitationUnbound);
                assert_eq!(citation.entry_id, "§39");
                assert_eq!(citation.file.to_string_lossy(), "src/foo.rs");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn bidirectional_implementation_unbacked_when_impl_file_lacks_cite() {
        //.bindings contains src/foo.rs:Foo, but src/foo.rs has
        // no citation.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/foo.rs", Some("Foo"));
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// no spec citation at all\nfn foo() {}\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::BindingUnbacked {
                section_id,
                file,
                symbol,
            } => {
                assert_eq!(section_id, "39");
                assert_eq!(file.to_string_lossy(), "src/foo.rs");
                assert_eq!(symbol.as_deref(), Some("Foo"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn bidirectional_orphan_ledger_suppresses_citation_unbound() {
        //.bindings names src/bar.rs only; src/foo.rs cites
        // but is registered in the orphan ledger as a known-stale code
        // citation. Suppressed.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/bar.rs", None);
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// §39 cite\n").unwrap();
        std::fs::write(tmp.path().join("src/bar.rs"), "// §39 cite\n").unwrap();
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::CodeCitation,
            doc: "<code-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "39".to_string(),
            reason: "legacy carry".to_string(),
            since: "Round 260".to_string(),
        }];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(v.is_empty(), "expected suppression, got: {:?}", v);
    }

    #[test]
    fn bidirectional_orphan_ledger_suppresses_implementation_unbacked() {
        //.bindings names src/foo.rs, src/foo.rs has no cite,
        // but ledger registers (src/foo.rs, 39) as known-stale. Suppressed.
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/foo.rs", None);
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// no cite here\n").unwrap();
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::CodeCitation,
            doc: "<code-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "39".to_string(),
            reason: "legacy carry".to_string(),
            since: "Round 260".to_string(),
        }];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(v.is_empty(), "expected suppression, got: {:?}", v);
    }

    #[test]
    fn bidirectional_filter_id_silences_section_axis() {
        // Decay-filter narrows surface to Round NNN only; §<id> binding
        // violations should not surface even if present.
        let tmp = TempDir::new().unwrap();
        let store = AtomicStore::new();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// §999 hallucinated\n// Round 1 cite\n",
        )
        .unwrap();
        // is in the store; is not. With filter_id=,
        // we expect to surface as Decay and to stay silent.
        let mut s2 = store.clone();
        s2.changelog_entries.insert(
            "Round 1".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &s2,
            &[],
            Some("Round 1"),
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1);
        match &v[0] {
            CodeRefViolation::Citation { citation, kind, .. } => {
                assert_eq!(*kind, ViolationKind::Decay);
                assert_eq!(citation.entry_id, "Round 1");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // ============ Round 266 scan_section_decay tests ============

    #[test]
    fn scan_section_decay_surfaces_only_target_section() {
        // Round 266 — targeted §<id> decay scan returns only citations of
        // the requested section_id; other sections in the same file ignored.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("a.rs"),
            "// §39 here\n// §61 here\n// §39 again\n// §99 elsewhere\n",
        )
        .unwrap();
        let cfg = decay_config(true);
        let hits = scan_section_decay(
            &no_foreign_subtree(tmp.path(), &cfg),
            &["src/".to_string()],
            "39",
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].entry_id, "§39");
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 3);
    }

    #[test]
    fn scan_section_decay_empty_when_no_citations() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("clean.rs"), "fn main() {}\n").unwrap();
        let cfg = decay_config(true);
        let hits = scan_section_decay(
            &no_foreign_subtree(tmp.path(), &cfg),
            &["src/".to_string()],
            "39",
        )
        .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn scan_section_decay_respects_comment_only_flag() {
        // String-literal §X tokens must be excluded under comment_only=true
        // (consistent with the bidirectional scanner's behavior). When false,
        // the whole-text scan picks them up.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("fixture.rs"),
            "let s = \"§39 in string\";\n// §39 in comment\n",
        )
        .unwrap();
        let comment_cfg = decay_config(true);
        let comment_hits = scan_section_decay(
            &no_foreign_subtree(tmp.path(), &comment_cfg),
            &["src/".to_string()],
            "39",
        )
        .unwrap();
        assert_eq!(
            comment_hits.len(),
            1,
            "comment_only excludes string literal"
        );
        assert_eq!(comment_hits[0].line, 2);
        let raw_cfg = decay_config(false);
        let raw_hits = scan_section_decay(
            &no_foreign_subtree(tmp.path(), &raw_cfg),
            &["src/".to_string()],
            "39",
        )
        .unwrap();
        assert_eq!(raw_hits.len(), 2, "comment_only=false picks up both");
    }

    // ============ comment-only filtering tests ============

    #[test]
    fn comment_syntax_dispatch_by_extension() {
        use std::path::PathBuf;
        // Slash family.
        for ext in [
            "rs", "c", "h", "cc", "cpp", "hpp", "go", "js", "ts", "jsx", "tsx", "java", "kt",
            "swift",
        ] {
            let p = PathBuf::from(format!("a.{}", ext));
            assert_eq!(
                comment_syntax_for(&p),
                CommentSyntax::Slash,
                "expected Slash for .{}",
                ext
            );
        }
        // Hash family.
        for ext in ["py", "sh", "bash", "rb", "toml", "yaml", "yml"] {
            let p = PathBuf::from(format!("a.{}", ext));
            assert_eq!(
                comment_syntax_for(&p),
                CommentSyntax::Hash,
                "expected Hash for .{}",
                ext
            );
        }
        // Unknown / extensionless.
        assert_eq!(
            comment_syntax_for(&PathBuf::from("a.unknown")),
            CommentSyntax::Unknown
        );
        assert_eq!(
            comment_syntax_for(&PathBuf::from("a")),
            CommentSyntax::Unknown
        );
        // Case-insensitive.
        assert_eq!(
            comment_syntax_for(&PathBuf::from("a.RS")),
            CommentSyntax::Slash
        );
    }

    /// Round 856 — `comment_only = true` means two different things depending on
    /// the extension, and the report says which files got which.
    ///
    /// Found by sweeping every extension-dependent branch in this file after
    /// closing the same class on three axes: `CommentSyntax::Unknown` leaves the
    /// WHOLE text on every axis, so the knob is a comment filter for `.rs` and a
    /// no-op for `.scxml`. Load-bearing in both directions — it is why a
    /// consumer's prose fact citations are read, and why a citation-shaped token
    /// in a data file counts under a reject-level severity — so it is reported
    /// rather than changed.
    #[test]
    fn the_comment_mode_report_names_the_files_read_whole() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        // Known syntax, both families: these must NOT be counted, or the report
        // would say "everything is read whole" and mean nothing.
        std::fs::write(root.join("src/a.rs"), "// x\n").unwrap();
        std::fs::write(root.join("src/b.toml"), "# x\n").unwrap();
        // No known syntax, one with an extension and one without.
        std::fs::write(root.join("src/scene.scxml"), "<!-- x -->\n").unwrap();
        std::fs::write(root.join("src/LICENSE"), "x\n").unwrap();

        // Round 860 — a file the gate WALKS and cannot READ. `read_to_string`
        // fails on it, so no axis sees it, and counting it as "read whole" both
        // overstated the exposure and named a compiled artifact as if an author
        // had cited in it. Invalid UTF-8 by construction (a lone 0xFF byte).
        std::fs::write(root.join("src/Runtime.class"), [0xFFu8, 0xFE, 0x00, 0x01]).unwrap();

        let cov = comment_mode_coverage(&walk_paths(root, &["src".to_string()]).unwrap());
        assert_eq!(cov.scanned, 5);
        assert_eq!(
            cov.whole_text, 2,
            "the `.rs` and `.toml` files have a comment syntax, and the `.class` \
             is not readable: {cov:?}"
        );
        assert_eq!(
            cov.whole_text_extensions,
            [("scxml".to_string(), 1), ("<none>".to_string(), 1)]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            "the report must name WHICH extensions, not just how many: {cov:?}"
        );
        assert_eq!(
            (
                cov.unreadable,
                cov.unreadable_extensions.get("class").copied()
            ),
            (1, Some(1)),
            "an unreadable file is its own bucket, named by extension — the \
             consumer's first run reported 368 `.class` files as read whole: {cov:?}"
        );
    }

    /// Round 860 — `scan_exclusions` does not narrow what the gate READS, and a
    /// file in both sets says two things at once.
    ///
    /// Reported from the field, as the one thing they asked us to write down.
    /// Their `paths` enrolled a parent directory holding build output, so they
    /// added exclusion prefixes to quiet the Round 856 line. The counts did not
    /// move and `stale_exclusions` stayed 0 — the prefixes matched real files and
    /// changed nothing, which is config that looks like it works. The repair is
    /// to narrow `paths`.
    ///
    /// Asserted together with the DISJOINT case, because an advisory that fires
    /// on every workspace is one the next person switches off.
    #[test]
    fn an_exclusion_inside_a_scanned_path_is_named_as_the_contradiction_it_is() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("sce/src/gen")).unwrap();
        std::fs::write(root.join("sce/src/lib.cpp"), "// see 1.1\n").unwrap();
        std::fs::write(root.join("sce/src/gen/out.cpp"), "// see 1.1\n").unwrap();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::fs::write(root.join("elsewhere/stray.rs"), "// Round 1\n").unwrap();

        let paths = vec!["sce/src/".to_string()];

        // DISJOINT: the exclusion names a tree no configured path covers — the
        // Round 783 declaration, and it must stay silent.
        let honest = scan_coverage(root, &paths, &["elsewhere/".to_string()]).unwrap();
        assert!(
            honest.excluded_but_scanned.is_empty(),
            "a declaration about an unscanned tree is not a contradiction: {:?}",
            honest.excluded_but_scanned
        );
        assert!(honest.stale_exclusions.is_empty());

        // OVERLAPPING: the exclusion names a subtree INSIDE a configured path.
        // The gate still reads it, and nothing said so.
        let trap = scan_coverage(root, &paths, &["sce/src/gen/".to_string()]).unwrap();
        assert_eq!(
            trap.excluded_but_scanned,
            vec![root.join("sce/src/gen/out.cpp")],
            "the file the config claims twice must be named: {trap:?}"
        );
        assert!(
            trap.stale_exclusions.is_empty(),
            "and it is NOT stale — it matched a real file, which is exactly why \
             the trap is quiet: {:?}",
            trap.stale_exclusions
        );
        assert!(
            trap.scanned_files
                .contains(&root.join("sce/src/gen/out.cpp")),
            "the gate reads it regardless of the exclusion — that is the whole \
             finding: {trap:?}"
        );
    }

    /// The walk's file set for one configured path — what both production
    /// callers hand to `vcs_ignored_among` (a read set, or an excluded set).
    fn walked(root: &Path, path: &str) -> BTreeSet<PathBuf> {
        walk_paths(root, &[path.to_string()])
            .unwrap()
            .into_iter()
            .collect()
    }

    /// Initialise a git repository at `root` with no user identity required —
    /// tracked-ness is decided by the index, so nothing here needs a commit.
    fn git_init_at(root: &Path) {
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .expect("git must be runnable to test the VCS axis");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "--quiet"]);
    }

    /// Round 864 — the axis names build output that `is_skipped_dir` cannot.
    ///
    /// The hand list holds `.`-prefixed, `target` and `node_modules`: the two
    /// ecosystems this repo is written in. A consumer's `__pycache__` walks
    /// straight in, and a generated Go tree has no conventional name to hold at
    /// all. Both are in the fixture, because a fixture that only proves the axis
    /// agrees with the hand list proves nothing the hand list did not already do.
    ///
    /// The `add -f` file discriminates against SWAPPING `--others` for
    /// `--cached`: `ls-files -i -c --exclude-standard` reports the tracked file
    /// and this test goes red. Deleting `--others` outright is not that edit —
    /// git refuses `-i` without `-o` or `-c` and the axis reports
    /// `NotDetermined`, which the sibling test catches instead. Measured both
    /// ways by injection.
    ///
    /// Round 865 — it does NOT discriminate `--others --ignored` from
    /// `check-ignore`, which is what the Round 864 version of this comment
    /// claimed. `check-ignore` consults the index and stays silent on a tracked
    /// path, so both commands return the same set here and at every other value
    /// of tracked-but-ignored. The consumer measured that and sent it back: a
    /// fixture input can only discriminate between two things that disagree
    /// somewhere, and asserting it does is the R858 corroboration error inside
    /// our own test.
    #[test]
    fn the_vcs_axis_names_build_output_no_hand_list_holds() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        git_init_at(root);
        std::fs::write(root.join(".gitignore"), "*.gen\n__pycache__/\n").unwrap();
        std::fs::create_dir_all(root.join("src/__pycache__")).unwrap();

        // Authored source: neither ignored nor build output.
        std::fs::write(root.join("src/a.rs"), "// Round 1\n").unwrap();
        // Untracked AND ignored — generated, and named by no hand list.
        std::fs::write(root.join("src/out.gen"), "// Round 1\n").unwrap();
        // Untracked AND ignored, inside a source directory: the consumer's 23
        // `.pyc`, which narrowing `paths` cannot reach without enumerating the
        // sibling modules and dropping the next one added.
        std::fs::write(root.join("src/__pycache__/x.pyc"), "x\n").unwrap();
        // TRACKED and ignore-matching: swapping `--others` for `--cached`
        // reports it and this test goes red. It does NOT separate us from
        // `check-ignore`, which consults the index and is equally silent on it
        // (Round 865).
        std::fs::write(root.join("src/pinned.gen"), "// Round 1\n").unwrap();
        // Ignored, INSIDE the queried pathspec, and skipped by the walk. The VCS
        // reports it and the read set does not hold it, so the intersection is
        // what keeps the two from disagreeing about a file (the Round 777 rule).
        std::fs::create_dir_all(root.join("src/node_modules")).unwrap();
        std::fs::write(root.join("src/node_modules/dep.gen"), "// Round 1\n").unwrap();
        let add = std::process::Command::new("git")
            .args(["add", "-f", "src/pinned.gen"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(add.status.success());

        let axis = vcs_ignored_among(root, &walked(root, "src/"));
        let VcsIgnoreAxis::Measured {
            considered,
            ignored,
            ignored_extensions,
        } = axis
        else {
            panic!("a git tree must be measurable: {axis:?}");
        };
        assert_eq!(
            considered, 4,
            "the read set is the walk's — `node_modules` never entered it"
        );
        assert_eq!(
            ignored,
            vec![root.join("src/__pycache__/x.pyc"), root.join("src/out.gen"),],
            "exactly the untracked-and-ignored files the gate READS: not the \
             `add -f` one, and not the one the walk already skipped"
        );
        assert_eq!(
            ignored_extensions,
            [("gen".to_string(), 1), ("pyc".to_string(), 1)]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            "the report names WHICH extensions: {ignored_extensions:?}"
        );
    }

    /// Round 866 — the excluded set is a DIFFERENT set, and it can be build
    /// output while the read set is clean.
    ///
    /// This is the whole round: Round 840 reads citations out of the excluded
    /// set to decide what an exclusion swallowed, so a set that differs between
    /// a developer and CI makes that verdict differ too. Round 864's axis
    /// answered only for the read set, and on this fixture it answers zero —
    /// which is exactly the reassuring silence the excluded set does not earn.
    ///
    /// Both sets come from one `scan_coverage` call, the same values the two
    /// production callers pass, so this cannot pass while they diverge.
    #[test]
    fn the_excluded_set_can_be_build_output_when_the_read_set_is_not() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        git_init_at(root);
        std::fs::write(root.join(".gitignore"), "*.gen\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("gen")).unwrap();
        std::fs::write(root.join("src/a.rs"), "// Round 1\n").unwrap();
        // Declared out of coverage AND generated: present for a developer who
        // has built, absent in a fresh clone.
        std::fs::write(root.join("gen/out.gen"), "// Round 1\n").unwrap();
        // Declared out of coverage and NOT generated: an ordinary excluded file,
        // which must not be counted or the axis just measures set size.
        std::fs::write(root.join("gen/notes.md"), "Round 1\n").unwrap();

        let scan = scan_coverage(root, &["src/".to_string()], &["gen/".to_string()]).unwrap();
        assert_eq!(
            vcs_ignored_among(root, &scan.scanned_files),
            VcsIgnoreAxis::Measured {
                considered: 1,
                ignored: vec![],
                ignored_extensions: BTreeMap::new(),
            },
            "the read set is clean and Round 864's axis says so"
        );
        assert_eq!(
            vcs_ignored_among(root, &scan.excluded_files),
            VcsIgnoreAxis::Measured {
                considered: 2,
                ignored: vec![root.join("gen/out.gen")],
                ignored_extensions: [("gen".to_string(), 1)].into_iter().collect(),
            },
            "and the set the swallowed verdict is read out of is half build output"
        );
    }

    /// Round 866 — the query scope is derived from the file set, and collapses.
    ///
    /// One pathspec per file would be an argument list the size of the tree; one
    /// per directory with no collapsing is nearly that on a deep build tree. The
    /// collapse must not lose a directory, which is what the sibling case here
    /// guards: `a/b` covering `a/b/c` is right, `a/b` covering `a/bc` is not.
    #[test]
    fn covering_roots_collapses_to_the_fewest_directories() {
        let root = Path::new("/w");
        let files: BTreeSet<PathBuf> = [
            "/w/a/b/f1",
            "/w/a/b/c/f2",
            "/w/a/b/c/d/f3",
            "/w/a/bc/f4",
            "/w/z/f5",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            covering_roots(&files, root),
            vec![
                PathBuf::from("a/b"),
                PathBuf::from("a/bc"),
                PathBuf::from("z")
            ],
            "descendants collapse into their ancestor; a name that merely SHARES \
             a prefix does not"
        );
    }

    /// Round 866 — a sixty-extension histogram on one line is a line nobody
    /// reads, and a truncated one that does not say it truncated is worse.
    #[test]
    fn summarize_extensions_names_the_largest_and_counts_the_rest() {
        let counts: BTreeMap<String, usize> = [("a", 1), ("b", 9), ("c", 5), ("d", 9)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert_eq!(
            summarize_extensions(&counts, 2),
            r#"{"b": 9, "d": 9, +2 more}"#,
            "largest first, ties by name so two runs are diffable, and the \
             remainder is COUNTED rather than dropped"
        );
        assert_eq!(
            summarize_extensions(&counts, 9),
            r#"{"b": 9, "d": 9, "c": 5, "a": 1}"#,
            "no `+n more` when nothing was left out"
        );
        assert_eq!(summarize_extensions(&BTreeMap::new(), 5), "{}");
    }

    /// Round 864 — the two silences the axis must not share.
    ///
    /// A workspace whose read set holds no build output and a workspace no VCS
    /// can answer for are different facts, and a report that printed nothing for
    /// both would be the Round 856 silence again (a store with zero facts and a
    /// store whose every citation lands looked identical).
    #[test]
    fn a_clean_tree_and_an_unanswerable_one_are_different_answers() {
        let clean = TempDir::new().unwrap();
        git_init_at(clean.path());
        std::fs::create_dir_all(clean.path().join("src")).unwrap();
        std::fs::write(clean.path().join("src/a.rs"), "// Round 1\n").unwrap();
        assert_eq!(
            vcs_ignored_among(clean.path(), &walked(clean.path(), "src/")),
            VcsIgnoreAxis::Measured {
                considered: 1,
                ignored: vec![],
                ignored_extensions: BTreeMap::new(),
            },
            "a git tree with no build output is MEASURED at zero"
        );

        // No `git init`: the same shape of tree, with nobody to ask.
        let unasked = TempDir::new().unwrap();
        std::fs::create_dir_all(unasked.path().join("src")).unwrap();
        std::fs::write(unasked.path().join("src/a.rs"), "// Round 1\n").unwrap();
        let axis = vcs_ignored_among(unasked.path(), &walked(unasked.path(), "src/"));
        let VcsIgnoreAxis::NotDetermined { reason } = &axis else {
            panic!("a tree outside version control cannot be measured: {axis:?}");
        };
        assert!(
            reason.contains("git exited"),
            "the reason carries what the VCS said, not a shrug: {reason}"
        );
    }

    #[test]
    fn strip_slash_preserves_line_comment_content() {
        let src = "let x = 1; // Round 254 carry\nlet y = 2;\n";
        let out = strip_to_comments(src, CommentSyntax::Slash);
        // Comment text retained, code chars stripped to spaces.
        assert!(out.contains("// Round 254 carry"));
        assert!(!out.contains("let x = 1;"));
        assert!(!out.contains("let y = 2;"));
        // Line count preserved.
        assert_eq!(out.lines().count(), src.lines().count());
    }

    #[test]
    fn strip_slash_removes_round_inside_string_literal() {
        // `` inside string literal must NOT survive comment-only mode.
        let src = "let s = \"Round 254\";\n";
        let out = strip_to_comments(src, CommentSyntax::Slash);
        assert!(!out.contains("Round 254"));
        assert!(!out.contains("Round"));
    }

    #[test]
    fn strip_slash_block_comment_multiline() {
        let src = "let x = 1; /* Round 254\n carry */ let y = 2;\n";
        let out = strip_to_comments(src, CommentSyntax::Slash);
        assert!(out.contains("Round 254"));
        assert!(out.contains("carry"));
        // Code outside block stripped.
        assert!(!out.contains("let x = 1;"));
        assert!(!out.contains("let y = 2;"));
    }

    #[test]
    fn strip_slash_string_with_double_slash_not_treated_as_comment() {
        // The `//` inside a string is NOT a comment opener.
        let src = "let s = \"// not a comment\"; // real comment\n";
        let out = strip_to_comments(src, CommentSyntax::Slash);
        // The real comment survives.
        assert!(out.contains("// real comment"));
        // The fake one (inside string) does not.
        assert!(!out.contains("not a comment"));
    }

    #[test]
    fn strip_hash_preserves_line_comment_content() {
        let src = "x = 1 # Round 254 carry\ny = 2\n";
        let out = strip_to_comments(src, CommentSyntax::Hash);
        assert!(out.contains("# Round 254 carry"));
        assert!(!out.contains("x = 1"));
        assert_eq!(out.lines().count(), src.lines().count());
    }

    #[test]
    fn strip_hash_removes_hash_inside_string_literal() {
        // `#` inside a quoted string must NOT be treated as a comment opener.
        let src = "url = \"http://example.com/#anchor\" # real comment\n";
        let out = strip_to_comments(src, CommentSyntax::Hash);
        assert!(out.contains("# real comment"));
        // The url content stripped — `#anchor` should not survive as a hash-comment.
        assert!(!out.contains("anchor\""));
    }

    #[test]
    fn strip_unknown_is_passthrough() {
        let src = "raw text with Round 254 anywhere\n";
        let out = strip_to_comments(src, CommentSyntax::Unknown);
        assert_eq!(out, src);
    }

    #[test]
    fn bidirectional_comment_only_filters_string_literal_noise() {
        //.rs file: only the comment cite should fire; string-literal Round NNN
        // must NOT produce a Missing violation under comment_only=true.
        let tmp = TempDir::new().unwrap();
        let store = AtomicStore::new();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "let fixture = \"Round 999 is fixture data\";\n// Round 999 real cite\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        // Only one Missing (the line 2 comment); line 1 string literal suppressed.
        let missing: Vec<_> = v
            .iter()
            .filter(|x| {
                matches!(
                    x,
                    CodeRefViolation::Citation {
                        kind: ViolationKind::Missing,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(missing.len(), 1, "got: {:?}", v);
        if let CodeRefViolation::Citation { citation, .. } = missing[0] {
            assert_eq!(citation.line, 2, "comment is on line 2, not line 1");
        }
    }

    #[test]
    fn bidirectional_comment_only_false_legacy_back_compat() {
        // With comment_only=false, both string-literal and comment cites fire
        //.
        let tmp = TempDir::new().unwrap();
        let store = AtomicStore::new();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "let fixture = \"Round 999 fixture\";\n// Round 999 cite\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            false,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        // Whole-text scan picks up BOTH occurrences (line 1 and line 2).
        let missing: Vec<_> = v
            .iter()
            .filter(|x| {
                matches!(
                    x,
                    CodeRefViolation::Citation {
                        kind: ViolationKind::Missing,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(missing.len(), 2, "got: {:?}", v);
    }

    #[test]
    fn bidirectional_comment_only_unknown_extension_passthrough() {
        //.unknown extension → CommentSyntax::Unknown → whole-text scan even
        // under comment_only=true.
        let tmp = TempDir::new().unwrap();
        let store = AtomicStore::new();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/notes.unknown"),
            "raw text Round 999 anywhere\n",
        )
        .unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        // Unknown extension preserves /258 whole-text behavior.
        assert_eq!(v.len(), 1, "got: {:?}", v);
    }

    // ============ Round 269: ImplementationMissing (spec-side coverage axiom) ============

    /// Builds an empty workspace dir + a store whose `section_id` exists
    /// but has no `implements` bindings. `decision_status` lets the test pin
    /// the atomic override; pass `None` to exercise the parser-default
    /// fallback path.
    fn build_store_with_empty_section(
        section_id: &str,
        decision_status: Option<DecisionStatus>,
    ) -> AtomicStore {
        let mut store = AtomicStore::new();
        // Round 287 fail-loud: explicit Section creation via direct insert
        // (test fixture path — no audit-receipt needed).
        store.sections.insert(
            section_id.into(),
            mnemosyne_atomic::AtomicSection {
                skeleton: mnemosyne_core::SectionSkeleton {
                    decision_status,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // bindings stays at Vec::default() = []
        store
    }

    #[test]
    fn coverage_axiom_active_empty_impls_triggers() {
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", Some(DecisionStatus::Active));
        // No source files written — workspace is otherwise silent.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                assert_eq!(section_id, "39");
                assert_eq!(*decision_status, Some(DecisionStatus::Active));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn coverage_axiom_none_status_falls_back_to_active_triggers() {
        // Parser-default fallback (Round 265 convention) — None resolves
        // to Active for the trigger check, but the emitted variant
        // preserves the raw None so the audit-trail consumer can tell.
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", None);
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                assert_eq!(section_id, "39");
                assert_eq!(*decision_status, None, "raw Option preserved, not resolved");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn coverage_axiom_superseded_empty_impls_also_triggers() {
        // Superseded with empty impls = "marked dead but never recorded
        // where it lived" — audit gap, surfaced.
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", Some(DecisionStatus::Superseded));
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                assert_eq!(section_id, "39");
                assert_eq!(*decision_status, Some(DecisionStatus::Superseded));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn coverage_axiom_removed_empty_impls_does_not_trigger() {
        // Removed = tombstone genre, legitimately carries no impls.
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", Some(DecisionStatus::Removed));
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(v.is_empty(), "Removed must not trigger, got: {:?}", v);
    }

    #[test]
    fn coverage_axiom_non_empty_impls_does_not_trigger() {
        // Section with at least one implementation is exempt from the
        // coverage axiom regardless of citation match status (which is
        // the BindingUnbacked axis's job).
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = build_store_with_impl(&store_path, "39", "src/foo.rs", None);
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// §39 cite\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.iter()
                .all(|x| !matches!(x, CodeRefViolation::ImplementationMissing { .. })),
            "no ImplementationMissing expected, got: {:?}",
            v
        );
    }

    #[test]
    fn coverage_axiom_decay_filter_silences_surface() {
        // Symmetry with Steps 2-3: a Superseded-cascade caller asks
        // "where is THIS entry_id cited?", not "audit the whole store".
        // Coverage axiom stays silent under filter_id.
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", Some(DecisionStatus::Active));
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            Some("Round 99"),
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "filter_id should silence coverage axiom, got: {:?}",
            v
        );
    }

    // ============================================================================
    // Round 275 — Inventory axis tests (Phase 1A).
    // ============================================================================

    #[test]
    fn extract_inventory_citations_survives_non_ascii_comment_chars() {
        // Round 279 Bug #1 regression — the byte-index loop used to panic
        // at the first `line[i..].starts_with(prefix)` call when a multi-
        // byte char (em-dash `\u{2014}`, Korean, CJK) sat between earlier
        // ASCII and the prefix. The fixture replays the original tc8-
        // harness panic frame and exercises Korean + CJK as well.
        let prefixes = vec!["FOO_".to_string()];
        // Source uses \u{2014} so the test file itself stays ASCII-clean
        // (the self-application scan must not see an em-dash literal).
        let fixture = format!(
            "// SERVICE-ID-2 (0xF4E8) is the natural target {} FOO_01 cite\n\
  // \u{D55C}\u{AE00} \u{C8FC}\u{C11D} \u{C548} FOO_02\n\
  // \u{4E2D}\u{6587}\u{6CE8}\u{91CA} FOO_03\n",
            '\u{2014}'
        );
        let out = extract_inventory_citations(&prefixes, &fixture);
        assert_eq!(
            out,
            vec![
                (1, "FOO_01".to_string()),
                (2, "FOO_02".to_string()),
                (3, "FOO_03".to_string()),
            ],
            "all three cites must surface; no panic on multi-byte chars"
        );
    }

    #[test]
    fn scan_survives_non_ascii_comment_chars() {
        // Round 279 Bug #1 regression — full scan path (including
        // strip_to_comments) must not panic when a workspace source file
        // contains the original em-dash trigger from the tc8-harness
        // bug report.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let content = format!(
            "// SERVICE-ID-2 (0xF4E8) target {} DUT offers FOO_01\n",
            '\u{2014}'
        );
        std::fs::write(tmp.path().join("src/x.rs"), content).unwrap();
        let store = AtomicStore::new();
        let prefixes = vec!["FOO_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .expect("scan must not panic on multi-byte comment chars");
        // FOO_01 is the only cite and it's not registered, so it surfaces
        // as InventoryMissing. The point of the test is "no panic" plus
        // correct extraction past the em-dash.
        assert_eq!(v.len(), 1, "expected exactly the FOO_01 cite, got: {:?}", v);
    }

    #[test]
    fn extract_inventory_citations_basic() {
        let prefixes = vec!["ARP_".to_string()];
        let out = extract_inventory_citations(&prefixes, "// ARP_07 cite\nfn x() {}\n");
        assert_eq!(out, vec![(1, "ARP_07".to_string())]);
    }

    #[test]
    fn extract_inventory_citations_multi_prefix() {
        let prefixes = vec!["ARP_".to_string(), "TCP_".to_string()];
        let out =
            extract_inventory_citations(&prefixes, "// ARP_07 and TCP_RETRANSMISSION_TO_04\n");
        assert_eq!(
            out,
            vec![
                (1, "ARP_07".to_string()),
                (1, "TCP_RETRANSMISSION_TO_04".to_string()),
            ]
        );
    }

    #[test]
    fn extract_inventory_citations_tail_must_end_in_digit() {
        // Coding-convention identifiers (TCP_BUFFER_SIZE) are NOT inventory IDs.
        // Only tokens ending in a digit are treated as cites.
        let prefixes = vec!["TCP_".to_string()];
        let out = extract_inventory_citations(
            &prefixes,
            "// TCP_BUFFER_SIZE constant ; TCP_BUFFER_03 cite\n",
        );
        assert_eq!(out, vec![(1, "TCP_BUFFER_03".to_string())]);
    }

    #[test]
    fn extract_inventory_citations_longest_prefix_wins() {
        // When SOMEIP_ and SOMEIP_ETS_ are both registered, SOMEIP_ETS_BASICS_01
        // is reported once under the longer (more specific) prefix.
        let prefixes = vec!["SOMEIP_".to_string(), "SOMEIP_ETS_".to_string()];
        let out = extract_inventory_citations(&prefixes, "// SOMEIP_ETS_BASICS_01\n");
        assert_eq!(out, vec![(1, "SOMEIP_ETS_BASICS_01".to_string())]);
    }

    #[test]
    fn extract_inventory_citations_word_boundary_rejects_alphanumeric_prev() {
        // `MY_ARP_07` should NOT match ARP_ prefix — the prefix is not on a
        // word boundary.
        let prefixes = vec!["ARP_".to_string()];
        let out = extract_inventory_citations(&prefixes, "// MY_ARP_07 internal\n");
        assert!(out.is_empty(), "expected no match, got: {:?}", out);
    }

    #[test]
    fn extract_inventory_citations_empty_prefixes_disables_axis() {
        let out = extract_inventory_citations(&[], "// ARP_07 cite\n");
        assert!(out.is_empty());
    }

    #[test]
    fn extract_inventory_citations_skips_backtick_codespan() {
        let prefixes = vec!["ARP_".to_string()];
        let out = extract_inventory_citations(&prefixes, "// example: `ARP_07` literal\n");
        assert!(
            out.is_empty(),
            "backtick span should suppress, got: {:?}",
            out
        );
    }

    // ============================================================================
    // Section-path inventory axis tests (RFC-002 FR-4 narrow ext).
    // ============================================================================

    #[test]
    fn extract_inventory_path_citations_w3c_scxml_dotted_numeric() {
        // The motivating case — W3C SCXML 3.13 (dotted-numeric tail) must
        // match an inventory_path_prefix of "W3C SCXML ".
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out =
            extract_inventory_path_citations(&prefixes, "// see W3C SCXML 3.13 for <event>\n");
        assert_eq!(out, vec![(1, "W3C SCXML 3.13".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_lowercase_tail() {
        // IRP test144 — lowercase alpha + digits, no underscore. R275
        // axis rejects this (uppercase-only); section-path axis accepts.
        let prefixes = vec!["IRP ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// IRP test144 catalog\n");
        assert_eq!(out, vec![(1, "IRP test144".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_alpha_terminus() {
        // Section paths can end in a letter (`D.2.selectTransitions` in
        // SCXML Appendix D) — no digit-terminus requirement under section-path mode.
        let prefixes = vec!["SCXML-".to_string()];
        let out = extract_inventory_path_citations(
            &prefixes,
            "// SCXML-D.2.selectTransitions algorithm\n",
        );
        assert_eq!(out, vec![(1, "SCXML-D.2.selectTransitions".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_multi_prefix() {
        let prefixes = vec!["W3C SCXML ".to_string(), "IRP ".to_string()];
        let out = extract_inventory_path_citations(
            &prefixes,
            "// W3C SCXML 3.13 vs IRP test144 cross-ref\n",
        );
        assert_eq!(
            out,
            vec![
                (1, "IRP test144".to_string()),
                (1, "W3C SCXML 3.13".to_string()),
            ]
        );
    }

    #[test]
    fn extract_inventory_path_citations_word_boundary_rejects_alphanumeric_prev() {
        // `xW3C SCXML 3.13` should NOT match — prefix is not on a word
        // boundary (the preceding 'x' is alphanumeric).
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// xW3C SCXML 3.13 internal name\n");
        assert!(out.is_empty(), "expected no match, got: {:?}", out);
    }

    #[test]
    fn extract_inventory_path_citations_skips_backtick_codespan() {
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out =
            extract_inventory_path_citations(&prefixes, "// example: `W3C SCXML 3.13` literal\n");
        assert!(
            out.is_empty(),
            "backtick span should suppress, got: {:?}",
            out
        );
    }

    #[test]
    fn extract_inventory_path_citations_longest_prefix_wins() {
        // Both `W3C ` and `W3C SCXML ` registered — the longer specific
        // prefix wins for "W3C SCXML 3.13".
        let prefixes = vec!["W3C ".to_string(), "W3C SCXML ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// W3C SCXML 3.13\n");
        assert_eq!(
            out,
            vec![(1, "W3C SCXML 3.13".to_string())],
            "longer prefix must win"
        );
    }

    #[test]
    fn extract_inventory_path_citations_empty_prefixes_disables_axis() {
        let out = extract_inventory_path_citations(&[], "// W3C SCXML 3.13\n");
        assert!(out.is_empty());
    }

    #[test]
    fn extract_inventory_path_citations_no_id_token_axis_interference() {
        // The section-path axis axis must NOT swallow R275 opaque IDs — distinct tail
        // grammar even if the function were misused. Lowercase tail like
        // `arp_07` would not match R275 (uppercase-only) but would match
        // section-path axis if prefix is registered there. This test pins that section-path axis
        // does not auto-skip uppercase tails — `ARP_07` is still valid
        // under section-path mode because [A-Za-z0-9./-_] is a superset.
        let prefixes = vec!["ARP_".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// ARP_07 cite\n");
        assert_eq!(out, vec![(1, "ARP_07".to_string())]);
    }

    #[test]
    fn scan_section_path_inventory_missing() {
        // Full-scanner path: a path-shape cite (`W3C SCXML 3.13`) with
        // no matching atomic store entry must surface as InventoryMissing
        // via the section-path axis axis, not silently pass.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// W3C SCXML 3.13 cited but not registered\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let path_prefixes = vec!["W3C SCXML ".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &path_prefixes,
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation, .. } => {
                assert!(matches!(kind, ViolationKind::InventoryMissing));
                assert_eq!(citation.entry_id, "W3C SCXML 3.13");
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn scan_section_path_inventory_active_silent() {
        // Registered InventoryEntry with Active status — cite passes
        // silently on the section-path axis axis, same policy as R275.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// W3C SCXML 3.13 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "W3C SCXML 3.13".to_string(),
            InventoryEntry {
                status: InventoryStatus::Active,
                section_ref: None,
                source: None,
                reason: None,
            },
        );
        let path_prefixes = vec!["W3C SCXML ".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &path_prefixes,
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "Active section-path axis cite must pass silently, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_both_inventory_axes_dedup() {
        // A prefix registered in BOTH axes (e.g., legacy `ARP_` carried
        // into section-path axis for migration reasons) must surface a matching cite
        // once, not twice. Dedup on (line, id).
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
        let store = AtomicStore::new();
        let opaque = vec!["ARP_".to_string()];
        let path = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &opaque,
            &[],
            &[],
            &path,
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "ARP_07 in both axes must dedup to 1 InventoryMissing, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_inventory_missing_reject() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 not in store\n").unwrap();
        let store = AtomicStore::new();
        let prefixes = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation, .. } => {
                assert!(matches!(kind, ViolationKind::InventoryMissing));
                assert_eq!(citation.entry_id, "ARP_07");
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn scan_inventory_deprecated_reject() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "ARP_07".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                section_ref: None,
                source: None,
                reason: Some("superseded".to_string()),
            },
        );
        let prefixes = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, .. } => {
                assert!(matches!(kind, ViolationKind::InventoryDeprecated));
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn scan_inventory_active_and_reserved_silent() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// ARP_07 active\n// ARP_08 reserved\n",
        )
        .unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "ARP_07".to_string(),
            InventoryEntry {
                status: InventoryStatus::Active,
                ..Default::default()
            },
        );
        store.inventory_entries.insert(
            "ARP_08".to_string(),
            InventoryEntry {
                status: InventoryStatus::Reserved,
                ..Default::default()
            },
        );
        let prefixes = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "Active and Reserved must be cite-permitted, got: {:?}",
            v
        );
    }

    // ============================================================================
    // Round 277 — External-standard §<id> skip tests (Phase 1A P1).
    // ============================================================================

    #[test]
    fn extract_skips_rfc_external_cite() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC 2131 §3.5 is external\n", &prefixes, &[]);
        assert!(
            out.is_empty(),
            "RFC <num> §<id> must be skipped, got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_ieee_external_cite() {
        let prefixes = vec!["IEEE".to_string()];
        let out = extract_section_citations("// IEEE 802.3 §2.4 frame format\n", &prefixes, &[]);
        assert!(out.is_empty(), "IEEE skip failed, got: {:?}", out);
    }

    #[test]
    fn extract_skips_iso_iec_external_cite() {
        // ISO/IEC contains `/` and is itself a single non-whitespace token
        // — the single-token rule handles it natively.
        let prefixes = vec!["ISO/IEC".to_string()];
        let out = extract_section_citations("// ISO/IEC 14882 §1.5\n", &prefixes, &[]);
        assert!(out.is_empty(), "ISO/IEC skip failed, got: {:?}", out);
    }

    #[test]
    fn extract_keeps_internal_when_no_external_context() {
        let prefixes = vec!["RFC".to_string(), "IEEE".to_string()];
        let out = extract_section_citations("// §4.2.4 internal cite\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn extract_section_citations_empty_external_prefixes_treats_all_as_internal() {
        // With both external-skip axes empty, every §<id> is treated as
        // internal — `RFC 2131 §3.5` does NOT skip; both 3.5 and 4.2.4
        // surface as internal citations.
        let out = extract_section_citations("// RFC 2131 §3.5 and §4.2.4 mixed\n", &[], &[]);
        assert!(out.iter().any(|(_, id)| id == "3.5"));
        assert!(out.iter().any(|(_, id)| id == "4.2.4"));
    }

    #[test]
    fn extract_requires_whitespace_between_numeric_and_sigil() {
        // `RFC2131§3` (no whitespace) is NOT the recognized form — falls
        // through to the regular extractor. Source uses `\u{00a7}` so the
        // fixture string itself doesn't show up as a `§3` citation when
        // the self-application scan walks `code_refs.rs`.
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC2131\u{00a7}3 inline form\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "3".to_string())]);
    }

    // Round 281 Bug #5A — surrounding punctuation must not block the
    // external-prefix verbatim match. Comment prose commonly wraps the
    // standard reference in parens / brackets / quotes.

    #[test]
    fn extract_skips_paren_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// fragmentation fields (RFC 791 \u{00a7}3.1) per spec\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "(RFC 791) form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_bracket_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// see [RFC 793 \u{00a7}3.9] for retransmit semantics\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "[RFC 793] form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_quote_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// per \"RFC 2131 \u{00a7}3.4\" the client retransmits\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "\"RFC 2131\" form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_bare_rfc_form_still_skipped() {
        // Regression for the original Round 277 form — punctuation strip must
        // not regress the bare-token case.
        let prefixes = vec!["RFC".to_string()];
        let out =
            extract_section_citations("// RFC 2131 \u{00a7}3.5 client behavior\n", &prefixes, &[]);
        assert!(
            out.is_empty(),
            "bare RFC form must stay skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn is_external_section_cite_strips_leading_punctuation() {
        let prefixes = vec!["RFC".to_string()];
        // Unit-level coverage of the prev_token cleanse (numeric mode).
        assert!(is_external_section_cite("(RFC 791 ", &prefixes, &[]));
        assert!(is_external_section_cite("[RFC 793 ", &prefixes, &[]));
        assert!(is_external_section_cite("\"RFC 2131 ", &prefixes, &[]));
        assert!(is_external_section_cite("«RFC 826 ", &prefixes, &[]));
        assert!(is_external_section_cite("RFC 3927 ", &prefixes, &[]));
        // Negative: random suffix on the prefix word should still miss.
        assert!(!is_external_section_cite("RFCs 791 ", &prefixes, &[]));
    }

    // Round 284 — bare-prefix (doc-name) mode tests. AUTOSAR family
    // (TR_SOMEIP / SOMEIPSD / SWS_SD) lacks a numeric document number,
    // so the prefix sits directly before the sigil: `<PREFIX> §<id>`.

    #[test]
    fn extract_skips_bare_tr_someip() {
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations(
            "// drives a Nack with TTL=0 (TR_SOMEIP \u{00a7}6.7.4.2.4).\n",
            &[],
            &bare,
        );
        assert!(
            out.is_empty(),
            "TR_SOMEIP bare form must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_bare_someipsd() {
        let bare = vec!["SOMEIPSD".to_string()];
        let out = extract_section_citations(
            "// multicast reply per SOMEIPSD \u{00a7}6.7.5.2 path\n",
            &[],
            &bare,
        );
        assert!(
            out.is_empty(),
            "SOMEIPSD bare form must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_paren_wrapped_bare_prefix() {
        // R281 leading-punct strip applies in bare mode too.
        let bare = vec!["AUTOSAR".to_string()];
        let out = extract_section_citations(
            "// wire format (AUTOSAR \u{00a7}7.3) over UDP\n",
            &[],
            &bare,
        );
        assert!(
            out.is_empty(),
            "(AUTOSAR §X) form must skip in bare mode; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_bare_mode_negative_unregistered_prefix() {
        // Internal §X.Y must surface when the preceding word is not in
        // the bare-prefix registry.
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations("// see FOO \u{00a7}4.2.4 internal cite\n", &[], &bare);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn extract_numeric_and_bare_axes_independent() {
        // `RFC 791 §3.1` (numeric) + `TR_SOMEIP §6.7.4.2.4` (bare) on the
        // same line, both registered in their respective axes → both skip.
        let numeric = vec!["RFC".to_string()];
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations(
            "// RFC 791 \u{00a7}3.1 and TR_SOMEIP \u{00a7}6.7.4.2.4 both\n",
            &numeric,
            &bare,
        );
        assert!(out.is_empty(), "both forms must skip; got: {:?}", out);
    }

    #[test]
    fn extract_numeric_mode_unaffected_by_bare_registration() {
        // R277 / R281 regression: numeric path keeps working when only the
        // numeric axis is registered; an empty bare slice must not change
        // semantics for the numeric path.
        let numeric = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC 2131 \u{00a7}3.5 client\n", &numeric, &[]);
        assert!(
            out.is_empty(),
            "numeric RFC path must keep working; got: {:?}",
            out
        );
    }

    #[test]
    fn is_external_section_cite_bare_mode_strips_leading_punctuation() {
        let bare = vec!["TR_SOMEIP".to_string()];
        // Unit-level coverage of the bare-mode strip + verbatim match.
        assert!(is_external_section_cite("// (TR_SOMEIP ", &[], &bare));
        assert!(is_external_section_cite("// [TR_SOMEIP ", &[], &bare));
        assert!(is_external_section_cite("per TR_SOMEIP ", &[], &bare));
        // Negative: unregistered word.
        assert!(!is_external_section_cite("// FOO ", &[], &bare));
        // Negative: numeric mode trigger with empty numeric axis.
        assert!(!is_external_section_cite("RFC 791 ", &[], &bare));
    }

    #[test]
    fn is_external_section_cite_hash_document_number_r379() {
        // R379 (a): a hash-prefixed document number (UAX #9, UAX #15)
        // selects numeric mode and reads UAX as the prefix.
        let numeric = vec!["UAX".to_string()];
        assert!(is_external_section_cite("// UAX #9 ", &numeric, &[]));
        assert!(is_external_section_cite("per UAX #15 ", &numeric, &[]));
        // Letter-suffixed document number (802.11ax) also classifies.
        let ieee = vec!["IEEE".to_string()];
        assert!(is_external_section_cite("IEEE 802.11ax ", &ieee, &[]));
        // Negative: a hash number with no registered prefix must not skip.
        assert!(!is_external_section_cite("// see #9 ", &numeric, &[]));
    }

    #[test]
    fn is_external_section_cite_multi_word_prefix_r379() {
        // R379 (b): multi-word prefixes match as a token-boundary suffix.
        let numeric = vec!["CSS Color".to_string()];
        assert!(is_external_section_cite("// CSS Color 4 ", &numeric, &[]));
        // Bare multi-word: Unicode Standard.
        let bare = vec!["Unicode Standard".to_string()];
        assert!(is_external_section_cite("// Unicode Standard ", &[], &bare));
        // Negative: a different leading word must not skip (no over-reach).
        assert!(!is_external_section_cite(
            "// random Color 4 ",
            &numeric,
            &[]
        ));
        // Negative: suffix must match on a token boundary (SCSS is not CSS).
        let css = vec!["CSS".to_string()];
        assert!(!is_external_section_cite("// SCSS 3 ", &css, &[]));
    }

    /// Round 802 — a document number may be NAMED. `R` plus digits is this
    /// ecosystem's document-name shape the way `#9` is Unicode's, and the
    /// leading-digit requirement read it as a name and sent the citation to
    /// the bare axis, where the prose ends with the number rather than with
    /// the registered prefix.
    #[test]
    fn is_document_number_token_admits_a_leading_name_r802() {
        for tok in [
            "791", "802.3", "1.2", "9", "#9", "802.11ax", "R1345", "ISO9001",
        ] {
            assert!(
                is_document_number_token(tok),
                "{tok:?} is a document number and must select the numeric axis"
            );
        }
        for tok in ["Color", "Standard", "WAI-ARIA", "R", "", "설계", "-3"] {
            assert!(
                !is_document_number_token(tok),
                "{tok:?} is a name and must select the bare axis"
            );
        }
        let numeric = vec!["pinion".to_string()];
        assert!(is_external_section_cite("// pinion R1345 ", &numeric, &[]));
        // The prefix is still required verbatim: the name alone does not skip.
        assert!(!is_external_section_cite("// see R1345 ", &numeric, &[]));
    }

    /// Round 809 — a circled digit is a document NUMBER, and the slice before
    /// a document number is matched against BOTH registries. Both classes are
    /// pinned: what counts as the glyph-number, and what the widening still
    /// refuses.
    #[test]
    fn a_circled_digit_numbers_a_document_and_the_name_may_be_bare_r809() {
        for tok in ["\u{2461}", "\u{2462}", "\u{2460}", "\u{2473}"] {
            assert!(
                is_document_number_token(tok),
                "{tok:?} is a number written as one glyph"
            );
        }
        // Different glyph classes, and neither corpus holds one — a token that
        // merely LOOKS numeric to a reader is not admitted on that basis.
        for tok in ["\u{2474}", "\u{ff13}", "\u{2462}a", "a\u{2462}", ""] {
            assert!(
                !is_document_number_token(tok),
                "{tok:?} is not a circled-digit document number"
            );
        }
        let numeric = vec!["pinion".to_string()];
        let bare = vec!["\u{d544}\u{b4dc} \u{b9ac}\u{d3ec}\u{d2b8}".to_string()]; // 필드 리포트
                                                                                  // The reported shape: a BARE-registered name carrying an instance number.
        assert!(is_external_section_cite(
            "// \u{d544}\u{b4dc} \u{b9ac}\u{d3ec}\u{d2b8} \u{2462} ",
            &numeric,
            &bare
        ));
        // Same name WITHOUT the number still resolves on its own axis — the
        // control that shows only the number token was ever the problem.
        assert!(is_external_section_cite(
            "// \u{d544}\u{b4dc} \u{b9ac}\u{d3ec}\u{d2b8} ",
            &numeric,
            &bare
        ));
        // An ASCII number after a bare-registered name, the same shape in the
        // other script. Round 802 closed the case where the PREFIX ends in a
        // number; this is a prefix FOLLOWED by a separate number token.
        let design = vec!["\u{c124}\u{acc4}".to_string()]; // 설계
        assert!(is_external_section_cite(
            "// \u{c124}\u{acc4} 1.2 ",
            &[],
            &design
        ));
        // THE GUARD, and what bounds the widening: an UNREGISTERED name with a
        // circled number does not skip. The workspace's verbatim declaration is
        // the whole permission, exactly as for every other prefix shape.
        assert!(!is_external_section_cite(
            "// \u{c694}\u{cc2d} \u{2461} ", // 요청 ② — never declared
            &numeric,
            &bare
        ));
        // And an enumeration marker in running prose is not preceded by a
        // registered document name, so it never reaches the skip either.
        assert!(!is_external_section_cite(
            "// \u{ae4c}\u{b2ed} \u{b458}: \u{2460} \u{ac78} \u{bb38}\u{c774} \u{c788}\u{ace0} \u{2461} ",
            &numeric,
            &bare
        ));
    }

    /// Round 802 — and the widening is only safe because the axes stopped
    /// being exclusive. A bare prefix carrying digits is itself
    /// document-number-shaped, so under the old dispatch it committed to the
    /// numeric axis and returned false there instead of ever reaching the
    /// list it was registered in.
    #[test]
    fn bare_prefix_that_looks_like_a_number_still_resolves_r802() {
        let bare = vec!["ISO9001".to_string()];
        assert!(is_external_section_cite("// ISO9001 ", &[], &bare));
        // Both axes registered, each answering for its own shape.
        let numeric = vec!["pinion".to_string()];
        assert!(is_external_section_cite("// ISO9001 ", &numeric, &bare));
        assert!(is_external_section_cite(
            "// pinion R1345 ",
            &numeric,
            &bare
        ));
        // Neither axis matching is still not external.
        assert!(!is_external_section_cite(
            "// other R1345 ",
            &numeric,
            &bare
        ));
    }

    #[test]
    fn extract_skips_w3c_shapes_r379() {
        // End-to-end: UAX #9 and CSS Color 4 citations no longer surface
        // as internal once the prefix is registered; a bare internal cite
        // (5.16) still does.
        let numeric = vec!["UAX".to_string(), "CSS Color".to_string()];
        let content = "// UAX #9 \u{00a7}3.3.1 reorder\n// CSS Color 4 \u{00a7}8.1 oklch\n// \u{00a7}5.16 internal\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(
            out,
            vec![(3usize, "5.16".to_string())],
            "only the internal cite should remain; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_chains_multi_cite_same_line_r380() {
        // R380 (c): `UAX #9 §6.6.8 / §6.6.9 / §6.6.10` — only the first cite
        // carries the prefix; the rest inherit across `/` separators.
        let numeric = vec!["UAX".to_string()];
        let content = "// UAX #9 \u{00a7}6.6.8 / \u{00a7}6.6.9 / \u{00a7}6.6.10\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert!(
            out.is_empty(),
            "chained external cites must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_chain_breaks_on_comma_r380() {
        // R380 (c) over-skip guard: a comma is NOT a chain separator, so a
        // distinct internal cite after `, ` is still validated.
        let numeric = vec!["UAX".to_string()];
        let content = "// UAX #9 \u{00a7}3.3, \u{00a7}5.16 internal\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(
            out,
            vec![(1usize, "5.16".to_string())],
            "comma must break the chain; got: {:?}",
            out
        );
    }

    /// Round 801 — the chain set cannot derive its oracle the way Round 800's
    /// id-separator rule could, so this test states the CONTRAST instead: what
    /// joins one document's sections, and what ends the thought that named it.
    ///
    /// Both columns are asserted because the two failure directions are not
    /// symmetric. A char wrongly missing from the joining column costs a
    /// visible false positive; a char wrongly reaching it turns the axis OFF
    /// for every cite behind it, and nothing reports that. Only the second
    /// column can catch the second kind.
    #[test]
    fn chain_separator_joins_a_list_and_never_a_thought() {
        for gap in [" ", "  ", "/", " / ", "·", " · ", "・", "·・", " ·"] {
            assert!(
                gap_is_chain_only(gap),
                "gap {gap:?} joins sections of one document and must chain"
            );
        }
        for gap in [
            "", ",", ", ", ".", ". ", "、", "-", " - ", " and ", ";", "_",
        ] {
            assert!(
                !gap_is_chain_only(gap),
                "gap {gap:?} ends the thought and must break the chain"
            );
        }
    }

    /// Round 820 — the fact axis names what the store lacks and stays quiet
    /// otherwise, over every language the gate reads.
    ///
    /// The doc comment here used to describe the parenthetical-gloss test below
    /// it, which is a different axis; that was a paste, corrected in Round 856.
    ///
    /// The fixture is MIXED-LANGUAGE for the reason Round 854 learned the hard
    /// way: this axis filtered its walk to `.rs` and said nothing about it, and
    /// an all-Rust fixture cannot tell that apart from a language-agnostic one.
    /// The `.scxml` file carries both halves — a real id that must be counted
    /// and an invented one that must be named — because the consumer this axis
    /// was built for writes its fact citations in exactly such files, in prose.
    /// Every count below is exact, so the fixture cannot lose the non-Rust file
    /// without the test failing.
    #[test]
    fn the_fact_axis_names_what_the_store_lacks_and_stays_quiet_otherwise_r820() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        // A namespace needs TWO ids; `pro-` has one, so a token carrying it is
        // not on the axis at all — asserted below, since a rule that admitted it
        // would put every hyphenated word in a comment on this axis.
        let facts: BTreeSet<String> = ["f-bell-six", "f-tide-out", "pro-lonely"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let entities: BTreeSet<String> = ["ent-seward", "ent-bell"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        std::fs::write(
            root.join("src/a.rs"),
            "//! f-bell-six 은 종이 여섯을 친다는 선언이고, ent-seward 가 그걸 센다.\n\
             //! 이 줄은 f-bell-seven 을 인용하는데 스토어에 없다.\n\
             //! `ent-bell` 은 backtick 안에서도 한 자리다.\n\
             //! pro-lonely 는 접두사가 하나뿐이라 축에 오르지 않는다.\n\
             //! f-bell-six_tail 은 더 긴 이름이지 인용이 아니다.\n\
             fn f() { let _some_ident-f-bell-six = 0; }\n",
        )
        .unwrap();
        // NOT RUST, and the axis must read it (Round 856). `.scxml` has no known
        // comment syntax, so `comment_only` leaves the whole text on the axis —
        // which is what the consumer's scenario files rely on, since their fact
        // citations sit in prose rather than in a comment token.
        std::fs::write(
            root.join("src/scene.scxml"),
            "<!-- 물때가 f-tide-out 이다 (f-bell-eight 은 스토어에 없다) -->\n",
        )
        .unwrap();
        // Never scanned: the axis inherits walk_paths, which skips build trees —
        // the boundary that separates ten findings from a baked projection's
        // thousands of emitted ids.
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(
            root.join("target/debug/baked.rs"),
            "// f-invented-in-a-bake\n",
        )
        .unwrap();
        let paths = vec!["src".to_string()];
        let read_set = walk_paths(root, &paths).unwrap();
        let r = scan_id_citations(root, &read_set, true, &facts, &entities);
        assert_eq!(
            r.namespaces,
            [("ent-".to_string(), 2), ("f-".to_string(), 2)]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            "a prefix is derived from the store and needs two ids"
        );
        // The ACCEPT first: real ids are counted, so a report of zero findings
        // cannot be confused with an axis that read nothing. The second fact
        // site and the second citing file are the `.scxml` — under the Rust-only
        // universe these read (1, …) and (1, 1).
        assert_eq!((r.fact_sites, r.entity_sites), (2, 2));
        assert_eq!((r.facts_cited, r.facts_total), (2, 3));
        assert_eq!((r.entities_cited, r.entities_total), (2, 2));
        assert_eq!((r.files_citing, r.files_scanned), (2, 2));
        let unknown: Vec<(usize, &str)> = r
            .unknown
            .iter()
            .map(|f| (f.line, f.token.as_str()))
            .collect();
        assert_eq!(
            unknown,
            vec![(2, "f-bell-seven"), (1, "f-bell-eight")],
            "only the id-shaped tokens carrying a derived namespace that name \
             nothing — one per file, in walk order"
        );
        // With comment_only off the code line joins the scan, and the token
        // welded to an identifier is still not a citation.
        let whole = scan_id_citations(root, &read_set, false, &facts, &entities);
        assert_eq!(
            whole
                .unknown
                .iter()
                .map(|f| f.token.as_str())
                .collect::<Vec<_>>(),
            vec!["f-bell-seven", "f-bell-eight"],
            "`_some_ident-f-bell-six` is one name, never a cite"
        );
        // The boundary, asserted where it can fail: scanning the ROOT reaches the
        // build tree by path, and only `walk_paths`' skip rule keeps the baked
        // id out. Configuring `src` alone would have proved nothing here.
        let whole_root = scan_id_citations(
            root,
            &walk_paths(root, &[String::new()]).unwrap(),
            true,
            &facts,
            &entities,
        );
        assert_eq!(
            whole_root.files_scanned, 2,
            "a build tree is not authorship, and the two authored files are"
        );
        assert_eq!(
            whole_root
                .unknown
                .iter()
                .map(|f| f.token.as_str())
                .collect::<Vec<_>>(),
            vec!["f-bell-seven", "f-bell-eight"],
            "a baked id must never reach the advisory list"
        );
        // A store with no id namespaces reports the axis as inapplicable rather
        // than returning a clean run — this workspace is that store.
        let bare = scan_id_citations(root, &read_set, true, &BTreeSet::new(), &BTreeSet::new());
        assert!(bare.namespaces.is_empty() && bare.unknown.is_empty());
        assert_eq!(bare.files_scanned, 0, "no namespace means no walk at all");
    }

    #[test]
    fn a_parenthetical_gloss_chains_but_never_widens_what_follows_it() {
        for gap in [
            "(\u{ad6c}\u{c5ed} \u{ac1c}\u{d3d0})\u{b7}", // the reported gap, verbatim
            "(x) ",
            " (x) ",
            "(x)(y) ",      // two glosses in one gap
            "(a (b) c)/",   // nested groups
            "(a, b)\u{b7}", // a comma INSIDE the gloss does not end the outer thought
        ] {
            assert!(
                gap_is_chain_only(gap),
                "gap {gap:?} annotates the preceding cite and must chain"
            );
        }
        for gap in [
            "(x)",      // a gloss alone is not a separator
            "(x) and ", // a WORD outside the gloss still breaks
            "(x), ",    // a comma outside still breaks
            "(x",       // unbalanced open: no way to know where the gloss ends
            "\u{b7}(x", // separator THEN an unclosed gloss — the balance
            // check is what breaks this one (the outside run
            // is a legal separator, so nothing else would)
            "x)",         // unbalanced close
            ")(",         // closed before opened
            "(a) - (b) ", // a dash outside is still a dash
        ] {
            assert!(!gap_is_chain_only(gap), "gap {gap:?} must break the chain");
        }
    }

    /// Round 808 — the downstream workspace's remaining two violations, end to
    /// end and verbatim (`engine/src/places.rs:1`). Before this round the gloss
    /// broke the chain, so `\u{a7}2-4` and `\u{a7}4` fell to the bare axis and were
    /// returned as `SectionMissing` — the hallucination class, at
    /// `severity_missing = reject`, which blocks the commit. The un-glossed
    /// control on the line above is what Round 801 already closed: both must be
    /// empty, and only the gloss distinguishes them.
    #[test]
    fn extract_chains_across_a_glossed_cjk_list_r808() {
        let bare = vec!["\u{c124}\u{acc4}".to_string()];
        let control = "//! \u{c124}\u{acc4} \u{a7}2-1\u{b7}\u{a7}2-4\u{b7}\u{a7}4\n";
        assert!(
            extract_section_citations(control, &[], &bare).is_empty(),
            "the un-glossed chain was already closed by Round 801; got: {:?}",
            extract_section_citations(control, &[], &bare)
        );
        let glossed = "//! \u{c7a5}\u{c18c} \u{aca9}\u{c790} \u{2014} \u{c124}\u{acc4} \
                       \u{a7}2-1(\u{ad6c}\u{c5ed} \u{ac1c}\u{d3d0})\u{b7}\
                       \u{a7}2-4(\u{b9c8}\u{c744}=\u{b370}\u{c774}\u{d130})\u{b7}\u{a7}4\u{c758} \u{cd95}.\n";
        assert!(
            extract_section_citations(glossed, &[], &bare).is_empty(),
            "a glossed chain names one document too; got: {:?}",
            extract_section_citations(glossed, &[], &bare)
        );
        // The other half: a WORD between the gloss and the next cite still ends
        // the thought, so that cite stays under this ledger's jurisdiction.
        let broken =
            "//! \u{c124}\u{acc4} \u{a7}2-1(\u{ad6c}\u{c5ed}) \u{adf8}\u{b9ac}\u{ace0} \u{a7}2-4\n";
        assert_eq!(
            extract_section_citations(broken, &[], &bare),
            vec![(1usize, "2-4".to_string())],
            "a word outside the gloss must break the chain"
        );
    }

    /// Round 801 — the reported shape end to end: a Korean bare prefix names
    /// one document, then four sections joined by the CJK list dot. Before
    /// this round the ASCII-only set returned three of them as
    /// `SectionMissing` — the hallucination class — against a comment that
    /// had cited one document correctly.
    #[test]
    fn extract_chains_across_cjk_list_joiner_r801() {
        let bare = vec!["첫날".to_string()];
        let joined = "// 첫날 \u{a7}4\u{b7}\u{a7}6\u{b7}\u{a7}7\u{b7}\u{a7}13\n";
        assert!(
            extract_section_citations(joined, &[], &bare).is_empty(),
            "the list joiner must chain; got: {:?}",
            extract_section_citations(joined, &[], &bare)
        );
        let katakana = "// 첫날 \u{a7}4\u{30fb}\u{a7}6\n";
        assert!(
            extract_section_citations(katakana, &[], &bare).is_empty(),
            "the katakana joiner must chain too; got: {:?}",
            extract_section_citations(katakana, &[], &bare)
        );
        // The other half: the CJK comma is a comma, so the cite after it is a
        // distinct claim and stays under this ledger's jurisdiction.
        let comma = "// 첫날 \u{a7}4\u{3001}\u{a7}6\n";
        assert_eq!(
            extract_section_citations(comma, &[], &bare),
            vec![(1usize, "6".to_string())],
            "the CJK comma must break the chain"
        );
    }

    #[test]
    fn extract_carries_wrapped_prefix_across_comment_lines_r380() {
        // R380 (d): `/// WAI-ARIA 1.2` then `/// §6.6.6` — the sigil is the
        // first content on its line and inherits the prior line's prefix.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// WAI-ARIA 1.2\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert!(out.is_empty(), "wrapped prefix must carry; got: {:?}", out);
        // Composes with the chain: continuation line may itself chain.
        let chained = "/// WAI-ARIA 1.2\n/// \u{00a7}6.6.6 / \u{00a7}6.6.7\n";
        assert!(extract_section_citations(chained, &numeric, &[]).is_empty());
    }

    #[test]
    fn extract_wrap_carry_requires_prefix_at_line_tail_r380() {
        // R380 (d) over-skip guard #1: the previous line must *end with* the
        // prefix. Trailing prose after it ⇒ no carry, cite stays internal.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// implements WAI-ARIA 1.2 fully\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(out, vec![(2usize, "6.6.6".to_string())], "got: {:?}", out);
    }

    #[test]
    fn extract_wrap_carry_only_immediate_previous_line_r380() {
        // R380 (d) over-skip guard #2: only the immediately previous line
        // carries; an intervening prose line breaks it.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// WAI-ARIA 1.2\n/// unrelated note\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(out, vec![(3usize, "6.6.6".to_string())], "got: {:?}", out);
    }

    #[test]
    fn scan_bare_external_skips_section_missing() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// drives Nack (TR_SOMEIP \u{00a7}6.7.4.2.4) per spec\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let bare = vec!["TR_SOMEIP".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &bare,
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "bare-mode TR_SOMEIP cite must be skipped; got: {:?}",
            v
        );
    }

    #[test]
    fn extract_mixed_internal_and_external_on_same_line() {
        let prefixes = vec!["RFC".to_string()];
        let out =
            extract_section_citations("// see RFC 2131 §3.5 and §4.2.4 here\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn scan_external_rfc_cite_does_not_trigger_section_missing() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// RFC 2131 §3.5 external — should NOT fire SectionMissing\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let externals = vec!["RFC".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &externals,
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "RFC external cite must be skipped, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_internal_cite_still_fires_after_external_skip() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        // `\u{00a7}` avoids the literal sigil in this source file (self-
        // scan would otherwise see the fixture as an unrelated cite).
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// RFC 2131 \u{00a7}3.5 ok; \u{00a7}99 missing\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let externals = vec!["RFC".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &externals,
            &[],
            &[],
        )
        .unwrap();
        // Only the internal `\u{00a7}99` should surface.
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation, .. } => {
                assert!(matches!(kind, ViolationKind::SectionMissing));
                assert!(citation.entry_id.contains("99"));
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn scan_inventory_decay_surfaces_only_target_id() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/a.rs"),
            "// ARP_07 target\n// ARP_08 other\n",
        )
        .unwrap();
        let prefixes = vec!["ARP_".to_string()];
        let hits = scan_inventory_decay(
            tmp.path(),
            &["src/".to_string()],
            "ARP_07",
            &prefixes,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry_id, "ARP_07");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn scan_inventory_decay_empty_prefixes_yields_no_hits() {
        // Axis-disabled (empty prefixes) is a no-op regardless of file content.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/a.rs"), "// ARP_07 cite\n").unwrap();
        let hits =
            scan_inventory_decay(tmp.path(), &["src/".to_string()], "ARP_07", &[], &[], true)
                .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn scan_inventory_decay_respects_comment_only_flag() {
        // String literal cite must be suppressed under comment_only=true.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/a.rs"),
            "let s = \"ARP_07 inside string\";\n// ARP_07 in comment\n",
        )
        .unwrap();
        let prefixes = vec!["ARP_".to_string()];
        let hits = scan_inventory_decay(
            tmp.path(),
            &["src/".to_string()],
            "ARP_07",
            &prefixes,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn scan_empty_inventory_prefixes_disables_inventory_axis() {
        // An empty inventory_prefixes slice disables the inventory axis:
        // even when the store has Deprecated entries, no violation surfaces.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "ARP_07".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "empty inventory_prefixes must not scan inventory, got: {:?}",
            v
        );
    }

    // ============================================================================
    // Round 285 — inventory-axis orphan_ledger suppression tests.
    // ============================================================================

    #[test]
    fn inventory_orphan_ledger_suppresses_inventory_deprecated() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 hist\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_01".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical: V2->V3 deleted, dissector skips IP options".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "ledger must suppress Deprecated cite; got: {:?}",
            v
        );
    }

    #[test]
    fn inventory_orphan_ledger_suppresses_inventory_missing() {
        // Deleted-from-store case: id not registered at all, ledger still
        // suppresses (author's intentional historical reference).
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 hist\n").unwrap();
        let store = AtomicStore::new();
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical: id removed from inventory, comment retained".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "ledger must suppress Missing cite; got: {:?}",
            v
        );
    }

    #[test]
    fn inventory_orphan_ledger_unregistered_fires() {
        // (file, id) not in ledger → violation fires normally.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_02 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_02".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        // Ledger only covers _01, not _02.
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical _01 only".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "_02 must fire (not in ledger); got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, .. } => {
                assert!(matches!(kind, ViolationKind::InventoryDeprecated));
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn inventory_orphan_ledger_axis_filter_isolates_kinds() {
        // CodeCitation ledger rows must NOT suppress inventory violations,
        // and vice-versa. Axes are independent.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_01".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        // CodeCitation kind — should NOT suppress inventory cite.
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::CodeCitation,
            doc: "<code-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "wrong-axis row".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "CodeCitation row must not suppress inventory cite; got: {:?}",
            v
        );
    }

    // ============ Round 293 entry-id prefix-normalize ============

    #[test]
    fn long_form_entry_id_matches_short_form_citation() {
        // R293 trigger: entry-id stored as "Round 293 — <title>" must match
        // a code citation of the form "Round 293". Without the normalize step
        // the citation would be flagged Missing even though the round exists.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.rs"), "// Round 293 carry\n").unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 293 — R291 backfill entry append + commit↔ledger drift gate".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "long-form entry-id must match Round 293 cite; got: {:?}",
            v
        );
    }

    #[test]
    fn short_form_entry_id_still_matches_after_normalize() {
        // Regression guard: most ledger entries are short-form ("Round 292").
        // The normalize step must not break direct equality matches.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.rs"), "// Round 292 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 292".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "short-form entry-id must continue to match; got: {:?}",
            v
        );
    }

    #[test]
    fn unknown_round_still_flags_missing_after_normalize() {
        // Regression guard: normalize must not silence genuinely missing
        // citations. Cite a hallucinated round → Missing. The fixture content
        // is built via format!() rather than a string literal so the
        // production validate-code-refs scan over this very source file does
        // not pick up the synthetic round number as a real citation.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let cite = format!("// {} 9{} hallucinated\n", "Round", "99");
        std::fs::write(src.join("a.rs"), cite).unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 292".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "hallucinated round must still flag Missing; got: {:?}",
            v
        );
        match &v[0] {
            CodeRefViolation::Citation { citation, kind, .. } => {
                assert_eq!(*kind, ViolationKind::Missing);
                assert_eq!(citation.entry_id, "Round 999");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // ============ section_namespace scope tests ============

    #[test]
    fn citation_namespace_segments() {
        assert_eq!(citation_namespace("scxml-6.4"), "scxml");
        assert_eq!(citation_namespace("mesh-16.7"), "mesh");
        assert_eq!(citation_namespace("scxml-D-interpret"), "scxml");
        // no hyphen → whole id is its own namespace segment
        assert_eq!(citation_namespace("D"), "D");
        assert_eq!(citation_namespace("39"), "39");
    }

    #[test]
    fn namespace_scopes_out_foreign_cite() {
        // A `mesh-16.7` cite under section_namespace="scxml" belongs to a
        // different ledger — skip it, no SectionMissing despite empty store.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// dedup per \u{00a7}mesh-16.7 elsewhere\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "foreign-namespace cite must be skipped: {:?}",
            v
        );
    }

    #[test]
    fn namespace_keeps_matching_cite_in_scope() {
        // A `scxml-9.99` cite under section_namespace="scxml" is in scope, so
        // its absence from the (empty) store fires SectionMissing.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// see \u{00a7}scxml-9.99 hallucinated\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert_eq!(v.len(), 1, "in-namespace unknown id must fire: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation, .. } => {
                assert_eq!(*kind, ViolationKind::SectionMissing);
                assert!(citation.entry_id.contains("scxml-9.99"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn namespace_unset_checks_every_cite() {
        // Back-compat: with no section_namespace, a `mesh-16.7` cite is
        // treated as internal and fires SectionMissing against the store.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// dedup per \u{00a7}mesh-16.7 elsewhere\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(v.len(), 1, "unset namespace must check all cites: {:?}", v);
    }

    #[test]
    fn namespace_exact_segment_not_prefix() {
        // `scxmlfoo` is a different segment than `scxml`, so `scxmlfoo-1` is
        // foreign and skipped; `scxml-D-interpret` is in scope and fires.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// \u{00a7}scxmlfoo-1 foreign; \u{00a7}scxml-D-interpret in-scope\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "only the exact-segment cite is in scope: {:?}",
            v
        );
        match &v[0] {
            CodeRefViolation::Citation { citation, .. } => {
                assert!(citation.entry_id.contains("scxml-D-interpret"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn namespace_no_hyphen_id_is_foreign() {
        // A bare `D` cite (no hyphen) has namespace segment "D" ≠ "scxml" → skipped.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// appendix \u{00a7}D root reference\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "no-hyphen foreign id must be skipped: {:?}",
            v
        );
    }

    #[test]
    fn prose_fact_assertion_flags_verb_beside_section_ref() {
        // A relation/status verb adjacent to a section ref restates a
        // store-homed fact in prose -> flagged. A bare pointer or a "see"
        // pointer is read-side only -> ok; a ref inside backticks is a
        // documentation example -> skipped. The section sigil is built at
        // runtime (s) so this source file carries no literal section-citation
        // token for the code-ref gate to read.
        let s = "\u{a7}";
        let src = format!(
            "// the policy is decided in {s}5.37, which supersedes {s}5.36\n\
             // see {s}5.36 for the bridge\n\
             // plain {s}5.41 pointer\n\
             // example: `decided in {s}9.99` in a code span\n"
        );
        let hits = extract_prose_fact_assertions(&src);
        let lines: Vec<usize> = hits.iter().map(|(l, _, _)| *l).collect();
        assert!(lines.contains(&1), "verb+ref line must flag: {hits:?}");
        assert!(
            !lines.contains(&2),
            "a pointer line must not flag: {hits:?}"
        );
        assert!(
            !lines.contains(&3),
            "a bare pointer must not flag: {hits:?}"
        );
        assert!(
            !lines.contains(&4),
            "a backticked ref is an example: {hits:?}"
        );
    }

    #[test]
    fn prose_fact_assertion_rejects_the_r1013_1_comment_shape() {
        // The pinion R1013.1 failure shape: a comment RESTATED structured facts
        // (a decided-in claim and a supersedes claim) the store did not hold.
        // Both lines flag as prose sourcing a store-homed fact -- the design
        // acceptance test (claudedocs/structured-fact-ssot-design.md). The sigil
        // is built at runtime so the source has no literal citation token.
        let s = "\u{a7}";
        let src = format!(
            "// the grid font policy is decided in {s}5.37 (self-hosted),\n\
             // which supersedes this {s}5.36 bridge\n"
        );
        let hits = extract_prose_fact_assertions(&src);
        let lines: Vec<usize> = hits.iter().map(|(l, _, _)| *l).collect();
        assert!(
            lines.contains(&1),
            "decided-in assertion must flag: {hits:?}"
        );
        assert!(
            lines.contains(&2),
            "supersedes assertion must flag: {hits:?}"
        );
    }

    #[test]
    fn prose_fact_assertion_verb_set_is_homed_only() {
        // R579 — the verb set is curated to facts with a store home, so the
        // remedy (move it to the store, point) is always available. "deferred to"
        // (resolved_by) and "open question" (decision_status=Open) flag; "depends
        // on" / "refines" have no typed-relation home yet → must NOT flag; bare
        // "open" is noise → must NOT flag. Sigil built at runtime.
        let s = "\u{a7}";
        let src = format!(
            "// deferred to {s}5.37\n\
             // the open question is tracked in {s}5.37\n\
             // depends on {s}5.36\n\
             // refines {s}5.36\n\
             // open the file at {s}5.36\n"
        );
        let hits = extract_prose_fact_assertions(&src);
        let lines: Vec<usize> = hits.iter().map(|(l, _, _)| *l).collect();
        assert!(lines.contains(&1), "deferred-to must flag: {hits:?}");
        assert!(lines.contains(&2), "open-question must flag: {hits:?}");
        assert!(!lines.contains(&3), "depends-on has no home, must not flag");
        assert!(!lines.contains(&4), "refines has no home, must not flag");
        assert!(!lines.contains(&5), "bare 'open' is noise, must not flag");
    }

    #[test]
    fn section_prose_lints_settable_fields_but_exempts_append_only_caveat() {
        // R584 / sec 12b — store-side surface, scoped to SETTABLE fields. A
        // fact-assertion in `rationale` (replaceable → remediable) is flagged;
        // the same assertion in an append-only `caveat` (audit ledger, no
        // set/remove primitive → no remediation path) is EXEMPT; a bare pointer
        // in `intent` is not an assertion. Sigil built at runtime.
        use mnemosyne_atomic::{AtomicSection, AtomicStore};
        let s = "\u{a7}";
        let mut store = AtomicStore::new();
        let sec = AtomicSection {
            rationale_bullets: vec![format!("the canonical fix supersedes {s}5.36")],
            caveats_bullets: vec![format!("historical: superseded by {s}5.37 at R20")],
            intent: Some(format!("see {s}5.3 for the DSL")),
            ..AtomicSection::default()
        };
        store.sections.insert("5.41".into(), sec);
        let findings = scan_section_prose_fact_assertions(&store);
        // Only the rationale assertion flags — caveat exempt, intent is a pointer.
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].section_id, "5.41");
        assert_eq!(findings[0].field, "rationale");
        assert!(findings[0].verb.contains("supersede"), "{findings:?}");
    }

    #[test]
    fn prose_fact_assertion_skips_quoted_meta_mention() {
        // FP-1 (R582): a verb+ref inside double quotes is a meta-mention — e.g. a
        // corrective that QUOTES the old overclaim to refute it (must never be
        // flagged, the correction record is load-bearing). An unquoted assertion
        // on the next line still flags.
        let s = "\u{a7}";
        let src = format!(
            "// the corrective: the earlier note overclaimed \"decided in {s}5.37\"\n\
             // but it really supersedes {s}5.36\n"
        );
        let hits = extract_prose_fact_assertions(&src);
        let lines: Vec<usize> = hits.iter().map(|(l, _, _)| *l).collect();
        assert!(
            !lines.contains(&1),
            "quoted meta-mention must not flag: {hits:?}"
        );
        assert!(
            lines.contains(&2),
            "unquoted assertion still flags: {hits:?}"
        );
    }
    // ============ Round 867 — numbering origin + one resolver ============

    /// Stage a gitlink at `path`: the index entry git writes for a submodule.
    ///
    /// A real submodule checkout is not needed and would not be more faithful —
    /// what the derivation reads is the parent's INDEX, and mode `160000` there is
    /// what says "another repository". The object need not be a commit for git to
    /// accept the entry, and the fixture needs no commit and no user identity;
    /// both measured before this was written (Round 865's rule).
    fn git_gitlink_at(root: &Path, path: &str) {
        let run = |args: &[&str]| -> String {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .expect("git must be runnable to test the numbering-origin axis");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let object = run(&["hash-object", "-w", "--stdin"]);
        run(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{object},{path}"),
        ]);
    }

    /// The derivation names another repository, and only that.
    ///
    /// The `vendor/scenarios` sibling is the discriminating input: a string-prefix
    /// membership test calls it foreign because `vendor/sce` is a prefix of it, and
    /// then un-gates a directory nobody vendored. That is the collapse Round 866
    /// had to inject against on the sibling axis, so it is asserted here rather
    /// than trusted.
    #[test]
    fn the_vcs_names_another_repository_and_a_prefix_of_one_is_not_it() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        git_init_at(root);
        std::fs::create_dir_all(root.join("vendor/sce")).unwrap();
        std::fs::create_dir_all(root.join("vendor/scenarios")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("vendor/sce/doc.md"), "per §6.2\n").unwrap();
        std::fs::write(root.join("vendor/scenarios/one.rs"), "// §6.2\n").unwrap();
        std::fs::write(root.join("src/own.rs"), "// §6.2\n").unwrap();
        git_gitlink_at(root, "vendor/sce");

        let axis = NumberingOriginAxis::derive(root);
        assert_eq!(
            axis,
            NumberingOriginAxis::Measured {
                foreign_subtrees: vec![PathBuf::from("vendor/sce")],
            },
            "the index names one gitlink and nothing else: {axis:?}"
        );

        let cfg = decay_config(true);
        let attr = CitationAttribution::new(root, &cfg, axis);
        assert_eq!(
            attr.citations_in(&root.join("vendor/sce/doc.md"), "per §6.2\n")
                .cited,
            Vec::new(),
            "a file inside the other repository cites the other repository"
        );
        // The two files a string prefix would confuse, and the plain sibling.
        for still_ours in ["vendor/scenarios/one.rs", "src/own.rs"] {
            let got = attr.citations_in(&root.join(still_ours), "// §6.2\n").cited;
            assert_eq!(
                got,
                vec![(1, "6.2".to_string())],
                "{still_ours} is this tree's own file: {got:?}"
            );
        }
    }

    /// No VCS is a THIRD state, and it attributes nothing.
    ///
    /// This axis is the only one in the family that LOOSENS — it makes citations
    /// vanish — so an unanswerable tree must leave the gate exactly as tight as it
    /// was. Zero foreign subtrees and "nobody could be asked" therefore agree
    /// about filtering and must NOT agree in the report, which is the silence
    /// Round 856 removed and Round 864 typed.
    #[test]
    fn no_vcs_is_its_own_state_and_still_attributes_every_citation_here() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/own.rs"), "// §6.2\n").unwrap();

        let axis = NumberingOriginAxis::derive(root);
        let NumberingOriginAxis::NotDetermined { reason } = &axis else {
            panic!("a bare temp directory is not a repository: {axis:?}");
        };
        assert!(
            !reason.is_empty(),
            "the state carries the VCS's own words, or the reader cannot act"
        );
        assert!(
            axis.foreign_subtrees().is_empty(),
            "an unanswerable tree must not be able to un-gate anything"
        );

        let cfg = decay_config(true);
        let attr = CitationAttribution::new(root, &cfg, axis);
        let files: BTreeSet<PathBuf> = [root.join("src/own.rs")].into_iter().collect();
        let report = numbering_origin_coverage(&attr, &files);
        assert_eq!(
            (
                report.files_considered,
                report.files_foreign,
                report.citations_skipped
            ),
            (1, 0, 0),
            "nothing was removed: {report:?}"
        );
        assert!(
            matches!(report.axis, NumberingOriginAxis::NotDetermined { .. }),
            "the report keeps the state, so zero-removed does not read as measured"
        );
    }

    /// The count is the whole defence, so it is asserted non-vacuously.
    ///
    /// The line exists because this axis loosens: a wrong verdict silently
    /// un-gates real citations, and only the number and the subtree name make that
    /// visible. A report that said `0` while the filter dropped two would be worse
    /// than no report, so the fixture drops two and the assertion reads them back
    /// per subtree.
    #[test]
    fn the_numbering_origin_line_counts_what_it_removed_and_names_where() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        git_init_at(root);
        std::fs::create_dir_all(root.join("vendor/sce/docs")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        // Two citations in the other repository, one of them beside a token the
        // prefix registries would have skipped anyway — so the count is of
        // citations of THIS store, not of section-shaped tokens.
        std::fs::write(root.join("vendor/sce/doc.md"), "per §6.2 and §6.4\n").unwrap();
        std::fs::write(root.join("vendor/sce/docs/layout.md"), "UAX §9 only\n").unwrap();
        std::fs::write(root.join("src/own.rs"), "// §6.2 ours\n").unwrap();
        git_gitlink_at(root, "vendor/sce");

        let cfg = SetEqualityValidatorConfig {
            comment_only: false,
            external_section_prefixes_bare: vec!["UAX".to_string()],
            ..Default::default()
        };
        let attr = CitationAttribution::new(root, &cfg, NumberingOriginAxis::derive(root));
        let files: BTreeSet<PathBuf> = [
            root.join("vendor/sce/doc.md"),
            root.join("vendor/sce/docs/layout.md"),
            root.join("src/own.rs"),
        ]
        .into_iter()
        .collect();
        let report = numbering_origin_coverage(&attr, &files);
        assert_eq!(
            (
                report.files_considered,
                report.files_foreign,
                report.citations_skipped
            ),
            (3, 2, 2),
            "two files are inside the other repository and two citations went \
             with them; the registered `UAX §9` was never this store's to lose: \
             {report:?}"
        );
        assert_eq!(
            report.skipped_per_subtree,
            [("vendor/sce".to_string(), 2)]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            "the reader has to know WHICH tree went quiet: {report:?}"
        );
    }

    /// PARITY — five readers, one question, one answer (the Round 305 substrate).
    ///
    /// Round 867 collapsed three predicates into one: `scan`,
    /// `propose_implementations` and `citation_index` honoured the prefix
    /// registries and `section_namespace`, `swallowed_citations` honoured the
    /// registries and not the namespace, and `scan_section_decay` honoured
    /// neither. This test is what keeps them from drifting apart again: the same
    /// foreign citation is driven through all five, and each must also still see
    /// this tree's own citation, so neither half can pass vacuously.
    ///
    /// The swallowed axis reads the EXCLUDED set rather than the read set, so it
    /// gets its own config over the same tree — that is the reader's own idiom,
    /// not a second fixture. Its non-vacuity comes from a second excluded tree
    /// that is NOT foreign and must still be reported.
    #[test]
    fn five_readers_answer_alike_about_one_foreign_citation() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        git_init_at(root);
        std::fs::create_dir_all(root.join("vendor/sce")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("generated")).unwrap();
        std::fs::write(root.join("src/own.rs"), "// §7 ours\n").unwrap();
        std::fs::write(root.join("vendor/sce/doc.md"), "// §39 theirs\n").unwrap();
        std::fs::write(root.join("generated/out.rs"), "// §61 generated\n").unwrap();
        git_gitlink_at(root, "vendor/sce");

        let mut store = AtomicStore::new();
        for id in ["7", "39", "61"] {
            store
                .sections
                .insert(id.into(), mnemosyne_atomic::AtomicSection::default());
        }
        let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);

        // Four readers walk `paths`, so the foreign tree is INSIDE it — anything
        // narrower and their agreement would be about a file none of them read.
        let read_cfg = SetEqualityValidatorConfig {
            paths: vec!["src/".to_string(), "vendor/".to_string()],
            comment_only: true,
            severity_binding: mnemosyne_config::Severity::Reject,
            ..Default::default()
        };
        let validator = SetEqualityValidator {
            config: read_cfg.clone(),
            entry_id_prefix: "Round ".to_string(),
            orphan_ledger: vec![],
            symbol_resolvers: BTreeMap::new(),
            filter_id: None,
            path_scope: None,
        };
        let attr = CitationAttribution::new(root, &read_cfg, NumberingOriginAxis::derive(root));

        // 1. the gate. No bindings exist, so every counted citation surfaces as
        //    CitationUnbound — which makes "counted" observable.
        let unbound: Vec<String> = validator
            .scan(&attr, &snapshot)
            .unwrap()
            .iter()
            .filter_map(|v| match v {
                CodeRefViolation::Citation {
                    citation,
                    kind: ViolationKind::CitationUnbound,
                    ..
                } => Some(citation.entry_id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            unbound,
            vec!["§7".to_string()],
            "the gate counts ours and not theirs: {unbound:?}"
        );

        // 2. the density index.
        let index: Vec<String> = validator
            .citation_index(&attr, &snapshot)
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(index, vec!["7".to_string()], "index: {index:?}");

        // 3. the binding proposal.
        let proposed: Vec<String> = validator
            .propose_implementations(&attr, &snapshot)
            .unwrap()
            .iter()
            .map(|p| p.section_id.clone())
            .collect();
        assert_eq!(proposed, vec!["7".to_string()], "proposals: {proposed:?}");

        // 4. the decay trigger — the loosest of the five before this round.
        let theirs = scan_section_decay(&attr, &read_cfg.paths, "39").unwrap();
        assert!(
            theirs.is_empty(),
            "a superseded §39 here does not decay because of their §39: {theirs:?}"
        );
        let ours = scan_section_decay(&attr, &read_cfg.paths, "7").unwrap();
        assert_eq!(ours.len(), 1, "and ours still decays: {ours:?}");

        // 5. the exclusion-integrity axis, in its own idiom: it reads the
        //    EXCLUDED set, so the foreign tree is declared rather than scanned.
        let excl_cfg = SetEqualityValidatorConfig {
            paths: vec!["src/".to_string()],
            scan_exclusions: vec!["vendor/".to_string(), "generated/".to_string()],
            comment_only: true,
            ..Default::default()
        };
        let coverage = scan_coverage(root, &excl_cfg.paths, &excl_cfg.scan_exclusions).unwrap();
        let excl_attr =
            CitationAttribution::new(root, &excl_cfg, NumberingOriginAxis::derive(root));
        let known: BTreeSet<String> = ["7", "39", "61"].iter().map(|s| (*s).to_string()).collect();
        let swallowed: Vec<String> = swallowed_citations(&coverage, &known, &excl_attr)
            .into_iter()
            .map(|s| s.section_id)
            .collect();
        assert_eq!(
            swallowed,
            vec!["61".to_string()],
            "the generated tree really did swallow §61 and must still reject; \
             their §39 was never this store's coverage to lose: {swallowed:?}"
        );
    }

    /// ONE VALUE OF EVERY VIOLATION SHAPE THERE IS.
    ///
    /// The only population that reaches all thirteen axes — a scan fixture
    /// produces the handful its tree can provoke — so the laws that must hold
    /// for EVERY axis (the name space, the evidence table, the wire names) are
    /// the laws that read this list. Hand-built, and each value carries the
    /// evidence its axis declares; that consistency is not assumed, it is the
    /// first thing `every_violation_shape_and_every_axis_are_one_name_space`
    /// checks.
    fn every_violation_shape() -> Vec<CodeRefViolation> {
        vec![
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "Round 1".into(),
                },
                kind: ViolationKind::Missing,
                evidence: None,
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "Round 1".into(),
                },
                kind: ViolationKind::Decay,
                evidence: None,
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "§1".into(),
                },
                kind: ViolationKind::SectionMissing,
                evidence: None,
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "§1".into(),
                },
                kind: ViolationKind::CitationUnbound,
                evidence: Some(CitationEvidence::SectionBindings {
                    files: vec!["b.rs".into()],
                }),
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "§1".into(),
                },
                kind: ViolationKind::SymbolMismatch,
                evidence: Some(CitationEvidence::SymbolDrift(ReadSymbol {
                    found: "alpha".into(),
                    expected: vec!["beta".into()],
                })),
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "INV_1".into(),
                },
                kind: ViolationKind::InventoryMissing,
                evidence: None,
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "INV_1".into(),
                },
                kind: ViolationKind::InventoryDeprecated,
                evidence: None,
            },
            CodeRefViolation::Citation {
                citation: Citation {
                    file: PathBuf::from("a.rs"),
                    line: 1,
                    entry_id: "§1".into(),
                },
                kind: ViolationKind::ProseFactAssertion,
                evidence: Some(CitationEvidence::AssertionVerb {
                    verb: "supersede".into(),
                }),
            },
            CodeRefViolation::BindingUnbacked {
                section_id: "1".into(),
                file: PathBuf::from("a.rs"),
                symbol: None,
            },
            CodeRefViolation::ImplementationMissing {
                section_id: "1".into(),
                decision_status: None,
            },
            CodeRefViolation::VerificationMissing {
                section_id: "1".into(),
                decision_status: None,
            },
            CodeRefViolation::MisclassifiedCoverage {
                section_id: "1".into(),
                decision_status: None,
            },
            CodeRefViolation::BlanketVerifies {
                file: PathBuf::from("t.rs"),
                symbol: None,
                section_ids: vec!["1".into()],
            },
        ]
    }

    /// The axis table is the ONE name space: every violation shape names an
    /// axis, the enumeration reaches it, and the tag a violation carries is the
    /// tag its axis carries.
    ///
    /// Both directions. A tag the enumeration cannot produce would be a
    /// violation no `not_judged` list could ever name; an axis no violation
    /// produces would be a name in the report with nothing behind it.
    #[test]
    fn every_violation_shape_and_every_axis_are_one_name_space() {
        let shapes = every_violation_shape();
        let reachable: BTreeSet<&str> = AuditAxis::all().iter().map(|a| a.kind_tag()).collect();
        let from_shapes: BTreeSet<&str> = shapes.iter().map(CodeRefViolation::kind_tag).collect();
        assert_eq!(
            from_shapes, reachable,
            "the enumeration and the violation shapes must name the same axes"
        );
        assert_eq!(
            AuditAxis::all().len(),
            reachable.len(),
            "the enumeration must not visit an axis twice"
        );
        for v in &shapes {
            assert_eq!(
                v.kind_tag(),
                v.axis().kind_tag(),
                "a violation's tag is its axis's tag"
            );
            // Round 1167 — the axis table declares the EVIDENCE as well as the
            // tag, and this list is the only population that reaches all
            // thirteen axes: the scan law next door exercises the four its
            // fixture can produce, and the arms no fixture reaches are asserted
            // here rather than left to the day one of them does.
            if let CodeRefViolation::Citation { evidence, .. } = v {
                assert_eq!(
                    CitationEvidence::shape_of(evidence.as_ref()),
                    v.axis().evidence(),
                    "a violation carries the evidence its axis declares: {v:?}"
                );
            }
        }
        // The split the SCE lift request turns on, pinned so a later variant
        // cannot quietly join the half a path-scoped run judges.
        let spec: BTreeSet<&str> = AuditAxis::all()
            .iter()
            .filter(|a| a.side() == AuditSide::Spec)
            .map(|a| a.kind_tag())
            .collect();
        assert_eq!(
            spec,
            [
                "binding_unbacked",
                "impl_missing",
                "verification_missing",
                "misclassified_coverage",
                "blanket_verifies"
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
    }

    /// THE PUBLISHED WIRE NAMES ARE THE ONES THE SERIALIZER WRITES (Round 1167).
    ///
    /// [`EvidenceShape::wire_keys`] exists so the end-to-end law can ask what a
    /// payload is called on the wire instead of spelling it a second time. That
    /// only helps if the declaration cannot drift from
    /// [`CodeRefViolation::to_cli_json`], which is what this checks: over every
    /// violation shape there is, the keys the serializer adds BEYOND the four
    /// every citation carries are exactly the ones its axis's shape names.
    #[test]
    fn the_wire_names_are_the_ones_the_serializer_writes() {
        const COMMON: [&str; 4] = ["kind", "file", "line", "entry_id"];
        let mut checked = 0usize;
        for v in every_violation_shape() {
            let CodeRefViolation::Citation { .. } = v else {
                continue;
            };
            let json = v.to_cli_json();
            let obj = json.as_object().expect("an object");
            let extra: BTreeSet<&str> = obj
                .keys()
                .map(String::as_str)
                .filter(|k| !COMMON.contains(k))
                .collect();
            let declared: BTreeSet<&str> =
                v.axis().evidence().wire_keys().iter().copied().collect();
            assert_eq!(
                extra,
                declared,
                "the serializer and the declaration must name the same keys for \
                 `{}`: {json}",
                v.kind_tag()
            );
            checked += 1;
        }
        assert_eq!(
            checked,
            AuditAxis::all()
                .iter()
                .filter(|a| a.side() == AuditSide::Citation)
                .count(),
            "every citation-side axis must be checked, or a renamed key on the \
             axis this skipped goes unnoticed"
        );
    }

    /// A scope names files or directories, in whatever spelling a caller's hook
    /// hands over, and refuses the spellings that would silently mean
    /// "everything".
    #[test]
    fn a_path_scope_normalizes_what_a_caller_hands_it_and_refuses_the_rest() {
        let root = Path::new("/w");
        let scope = PathScope::new(
            root,
            &[
                "./src/a.rs".to_string(),
                "/w/src/b.rs".to_string(),
                "docs".to_string(),
            ],
        )
        .expect("three legal spellings");
        assert!(scope.selects(Path::new("src/a.rs")), "leading ./ stripped");
        assert!(
            scope.selects(Path::new("src/b.rs")),
            "an absolute path under the root is relativized"
        );
        assert!(
            scope.selects(Path::new("docs/deep/c.rs")),
            "a directory selects what is under it"
        );
        assert!(
            !scope.selects(Path::new("src/c.rs")),
            "and nothing else — a scope that widened would be a second gate"
        );
        // `src/ab.rs` starts with the STRING `src/a` and is not under it; the
        // check is over path components, not bytes.
        assert!(!scope.selects(Path::new("src/ab.rs")));

        for bad in [vec![], vec![String::new()], vec![".".to_string()]] {
            assert!(
                PathScope::new(root, &bad).is_err(),
                "an empty or root-wide scope must be refused, got {bad:?} accepted"
            );
        }
        assert!(
            PathScope::new(root, &["/elsewhere/x.rs".to_string()]).is_err(),
            "a path outside the workspace has no answer here"
        );
    }

    /// The three answers a requested path can get, told apart. A hook hands over
    /// every file a commit touched; "judged", "this gate never reads it" and
    /// "not on disk" are three different pieces of news and only the first one
    /// is about the file's contents.
    #[test]
    fn a_scope_tells_judged_from_unread_from_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "// Round 1\n").unwrap();
        std::fs::write(root.join("src/b.rs"), "// Round 1\n").unwrap();
        std::fs::write(root.join("README.md"), "prose\n").unwrap();

        let read_set = walk_paths(root, &["src".to_string()]).unwrap();
        let scope = PathScope::new(
            root,
            &[
                "src/a.rs".to_string(),
                "README.md".to_string(),
                "src/gone.rs".to_string(),
            ],
        )
        .unwrap();
        let cov = scope.coverage(root, &read_set);
        assert_eq!(cov.matched_files, vec!["src/a.rs".to_string()]);
        assert_eq!(cov.out_of_read_set, vec!["README.md".to_string()]);
        assert_eq!(cov.not_found, vec!["src/gone.rs".to_string()]);
        assert_eq!(cov.read_set_total, 2, "what the unscoped run would read");
        assert_eq!(
            scope.select(root, read_set).len(),
            1,
            "and the run reads exactly the matched file"
        );
    }
}
