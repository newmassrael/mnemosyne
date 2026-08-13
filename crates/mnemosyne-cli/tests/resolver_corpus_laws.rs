//! THE THREE BACKENDS WITH NO PREDECESSOR, PUT TO A CORPUS (Rounds 1157, 1161).
//!
//! Rounds 1151 to 1155 shipped five symbol-resolver backends. Two of them were
//! PORTS, and a port has an oracle that needs no invention: run the old
//! implementation beside the new one over every line of a real tree and require
//! them to agree. Rust's did (313 files, 221787 lines, 0 disagreements) and
//! C++'s did (1469 files, 235962 lines, 0 disagreements), and both harnesses were
//! deleted once they had answered.
//!
//! GO, PYTHON AND KOTLIN HAVE NO OLD ANSWER TO AGREE WITH. They shipped on unit
//! laws over hand-written shapes plus one two-site end-to-end fixture each — a
//! sample chosen by whoever wrote it, which is exactly the population a defect
//! hides outside of. Round 1153's grouped/ungrouped `type` and Round 1154's
//! class-body `block` were both found by a BATCH test, and both were shapes the
//! single-line cases could not see.
//!
//! ROUND 1157 PUT THEM TO THE CONSUMER'S TREE and Round 1160 found what that
//! cost: the tree is on ONE machine, so everywhere else — the build machine, and
//! CI — the laws printed `NOT MEASURED` and PASSED. A measurement that quietly
//! stops measuring is the shape this repository keeps paying for.
//!
//! ROUND 1161 SPLIT THE TWO JOBS THAT WERE TANGLED HERE.
//!
//! - DISCOVERY asks "what shapes exist that nobody imagined?". It needs a real
//!   tree at volume, and it is a ONE-SHOT instrument: Round 1157 ran it over
//!   174000 lines and got 0 disagreements, which is the same thing the two port
//!   harnesses did before being deleted. It stays here, but only when
//!   `MNEMOSYNE_RESOLVER_CORPUS` names a tree — and then it MUST measure.
//! - REGRESSION asks "do the engine's invariants still hold?". It must run
//!   everywhere, and for a metamorphic property a fixed sample is the weaker
//!   instrument. The corpus below is BUILT, from shapes composed in several
//!   adjacencies, and that buys the thing a vendored sample cannot give:
//!
//! THE POPULATION IS DERIVED, SO THE CORPUS CANNOT SILENTLY LAG THE BACKEND.
//! `LanguageSpec::pattern_count` is how many declaration patterns a backend's
//! query DECLARES and `patterns_exercised` is which of them a source reached —
//! both read off the compiled query rather than off a list beside it. Law 0
//! requires every pattern to be reached. Add a pattern to a backend and this
//! goes red naming the index nobody covers; a real tree could only ever report
//! what it happened to contain.
//!
//! THE LAWS
//!
//! 0. EVERY PATTERN THE BACKEND DECLARES IS EXERCISED. Non-vacuity, derived.
//! 1. BATCHING CHANGES NO ANSWER. Every line of a file in one call must equal
//!    the same lines split across two calls that INTERLEAVE them — odds in one,
//!    evens in the other, so every line's neighbours move to the other call.
//! 2. AN ANSWER IS TEXT FROM THE FILE. Every symbol returned must occur
//!    verbatim in the source it was given.
//! 3. THE ANSWER IS THE PLANTED ONE. Over the built corpus this is an ORACLE
//!    and not a floor: the composer knows which declaration covers each line, so
//!    every line is checked by name. Round 1157 could only assert
//!    `answered > lines/20` here, a constant of the kind this repository keeps
//!    catching in other people's code. Over a real tree, where nothing knows the
//!    right answer, that floor is still all there is and it stays.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use mnemosyne_core::SymbolResolver;
use mnemosyne_plugin_tree_sitter_core::LanguageSpec;

/// One declaration shape, as source that exhibits it.
///
/// `{N}` in `body` is replaced by an ordinal unique to each placement, so no two
/// copies of a shape share a name and an answer can never be right by accident.
/// `answers` is one entry per LINE of `body`: the name the resolver must give
/// for that line, or `None` where no declaration covers it.
struct Shape {
    label: &'static str,
    body: &'static str,
    answers: &'static [Option<&'static str>],
}

/// Where a subject's REAL-TREE pass gets its files.
///
/// Round 1163 split this, because "a real tree" had come to mean one machine's
/// consumer checkout and nothing else — so the two backends that were PORTS met
/// no real source anywhere once their port harnesses were deleted, and Rust's
/// new doc-comment binding shipped with only built witnesses behind it.
enum RealTree {
    /// The consumer checkout named by `MNEMOSYNE_RESOLVER_CORPUS`, when this
    /// machine has one. Opt-in, and it MUST measure when it is asked to.
    NamedByEnv {
        /// The fewest files that tree must hold for the pass to be about
        /// something. A floor and not an oracle: nothing here knows what a
        /// consumer's tree contains, only that a derivation returning almost
        /// nothing is broken.
        min_files: usize,
    },
    /// THIS REPOSITORY, which is itself a real corpus of several hundred tracked
    /// Rust files — so this pass needs no variable, and runs on every machine
    /// and in CI. Its floor is an EQUALITY rather than a number: every tracked
    /// file of the subject's extensions must be one the pass read.
    ThisRepository,
}

struct Subject {
    language: &'static str,
    spec: &'static LanguageSpec,
    resolver: fn() -> Box<dyn SymbolResolver>,
    /// Prepended to every built file. Go needs a package clause to parse as one.
    prelude: &'static str,
    shapes: &'static [Shape],
    /// Extensions this backend's language maps to, as `git ls-files` globs.
    globs: &'static [&'static str],
    /// Where this subject's real-tree pass reads from.
    real_tree: RealTree,
    /// One witness per entry of the spec's `documented_kinds`.
    witnesses: &'static [DocWitness],
    /// One witness per entry of the spec's `inward_markers`.
    inward: &'static [InwardWitness],
}

const CPP_SHAPES: &[Shape] = &[
    Shape {
        label: "function_definition",
        body: "int fn{N}() {\n    return 0;\n}\n",
        answers: &[Some("fn{N}"), Some("fn{N}"), Some("fn{N}")],
    },
    Shape {
        // `class_specifier` holding a `field_declaration` twice over: a member
        // variable and a method prototype are the same node kind here, and the
        // brace lines belong to the class.
        label: "class_specifier with field_declaration members",
        body: "class Cls{N} {\n    int field{N};\n    int meth{N}();\n};\n",
        answers: &[
            Some("Cls{N}"),
            Some("field{N}"),
            Some("meth{N}"),
            Some("Cls{N}"),
        ],
    },
    Shape {
        label: "struct_specifier",
        body: "struct Str{N} {\n    int s{N};\n};\n",
        answers: &[Some("Str{N}"), Some("s{N}"), Some("Str{N}")],
    },
    Shape {
        label: "union_specifier",
        body: "union Uni{N} {\n    int u{N};\n};\n",
        answers: &[Some("Uni{N}"), Some("u{N}"), Some("Uni{N}")],
    },
    Shape {
        label: "enum_specifier",
        body: "enum Enu{N} {\n    A{N}\n};\n",
        answers: &[Some("Enu{N}"), Some("Enu{N}"), Some("Enu{N}")],
    },
    Shape {
        // `namespace_definition` is captured by the query and is NOT a
        // documented kind — the two questions differ, and the corpus is where
        // that difference is visible.
        label: "namespace_definition holding an out-of-line definition",
        body: "namespace ns{N} {\nint free{N}() {\n    return 1;\n}\n}\n",
        answers: &[
            Some("ns{N}"),
            Some("free{N}"),
            Some("free{N}"),
            Some("free{N}"),
            Some("ns{N}"),
        ],
    },
];

const RUST_SHAPES: &[Shape] = &[
    Shape {
        label: "function_item",
        body: "fn fn{N}() -> u32 {\n    0\n}\n",
        answers: &[Some("fn{N}"), Some("fn{N}"), Some("fn{N}")],
    },
    Shape {
        label: "struct_item",
        body: "pub struct Str{N} {\n    field{N}: u32,\n}\n",
        answers: &[Some("Str{N}"), Some("Str{N}"), Some("Str{N}")],
    },
    Shape {
        label: "enum_item",
        body: "enum Enu{N} {\n    A{N},\n}\n",
        answers: &[Some("Enu{N}"), Some("Enu{N}"), Some("Enu{N}")],
    },
    Shape {
        // THE SHAPE THE BUILT CORPUS FOUND. A trait's required method has no
        // body and so is a `function_signature_item`, which this backend's
        // query did not capture until Round 1163 — a citation on that line
        // bound to the TRAIT, and every hand-written case used a method WITH a
        // body and passed.
        label: "trait_item with a required method and an associated type",
        body: "trait Tra{N} {\n    type Assoc{N};\n    fn req{N}(&self);\n}\n",
        answers: &[
            Some("Tra{N}"),
            Some("Assoc{N}"),
            Some("req{N}"),
            Some("Tra{N}"),
        ],
    },
    Shape {
        // The smallest covering declaration is the inner `fn`, and the impl's
        // own line is the impl — the nesting law, in the corpus rather than in
        // a single hand-written case.
        label: "impl_item holding a function_item",
        body: "impl Str{N} {\n    fn meth{N}(&self) {}\n}\n",
        answers: &[Some("Str{N}"), Some("meth{N}"), Some("Str{N}")],
    },
    Shape {
        label: "mod_item holding a const_item",
        body: "mod mod{N} {\n    const C{N}: u32 = 1;\n}\n",
        answers: &[Some("mod{N}"), Some("C{N}"), Some("mod{N}")],
    },
    Shape {
        label: "static_item",
        body: "static ST{N}: u32 = 2;\n",
        answers: &[Some("ST{N}")],
    },
    Shape {
        label: "type_item",
        body: "type Ali{N} = u32;\n",
        answers: &[Some("Ali{N}")],
    },
    Shape {
        label: "union_item",
        body: "union Uni{N} {\n    u{N}: u32,\n}\n",
        answers: &[Some("Uni{N}"), Some("Uni{N}"), Some("Uni{N}")],
    },
    Shape {
        label: "macro_definition",
        body: "macro_rules! mac{N} {\n    () => {};\n}\n",
        answers: &[Some("mac{N}"), Some("mac{N}"), Some("mac{N}")],
    },
];

const GO_SHAPES: &[Shape] = &[
    Shape {
        label: "function_declaration",
        body: "func Fn{N}() int {\n\treturn 0\n}\n",
        answers: &[Some("Fn{N}"), Some("Fn{N}"), Some("Fn{N}")],
    },
    Shape {
        label: "method_declaration on a named receiver",
        body: "type Holder{N} struct{}\n\nfunc (h Holder{N}) Meth{N}() int {\n\treturn 1\n}\n",
        answers: &[
            Some("Holder{N}"),
            None,
            Some("Meth{N}"),
            Some("Meth{N}"),
            Some("Meth{N}"),
        ],
    },
    Shape {
        // ROUND 1153's DEFECT LIVED HERE — a grouped `type` holds several specs
        // and an ungrouped one holds exactly one, and the single-line cases
        // could not see the difference.
        label: "type_spec, grouped",
        body: "type (\n\tAlpha{N} int\n\tBeta{N} string\n)\n",
        answers: &[None, Some("Alpha{N}"), Some("Beta{N}"), None],
    },
    Shape {
        label: "type_spec, ungrouped",
        body: "type Solo{N} float64\n",
        answers: &[Some("Solo{N}")],
    },
    Shape {
        label: "const_spec",
        body: "const C{N} = 1\n",
        answers: &[Some("C{N}")],
    },
    Shape {
        label: "var_spec, grouped",
        body: "var (\n\tV{N} int\n\tW{N} bool\n)\n",
        answers: &[None, Some("V{N}"), Some("W{N}"), None],
    },
];

const PYTHON_SHAPES: &[Shape] = &[
    Shape {
        label: "function_definition",
        body: "def fn_{N}():\n    return 0\n",
        answers: &[Some("fn_{N}"), Some("fn_{N}")],
    },
    Shape {
        // ROUND 1154's DEFECT LIVED HERE — a citation inside a class body used
        // to fall out to the enclosing scope.
        label: "class_definition holding a function_definition",
        body: "class Cls{N}:\n    def meth_{N}(self):\n        return 1\n",
        answers: &[Some("Cls{N}"), Some("meth_{N}"), Some("meth_{N}")],
    },
    Shape {
        label: "decorated_definition",
        body: "@wrapper_{N}\ndef deco_{N}():\n    return 2\n",
        answers: &[Some("deco_{N}"), Some("deco_{N}"), Some("deco_{N}")],
    },
];

const KOTLIN_SHAPES: &[Shape] = &[
    Shape {
        label: "class_declaration with a member property and function",
        body: "class Cls{N} {\n    val prop{N}: Int = 1\n    fun meth{N}(): Int {\n        return 2\n    }\n}\n",
        answers: &[
            Some("Cls{N}"),
            Some("prop{N}"),
            Some("meth{N}"),
            Some("meth{N}"),
            Some("meth{N}"),
            Some("Cls{N}"),
        ],
    },
    Shape {
        label: "object_declaration",
        body: "object Obj{N} {\n    fun inner{N}() {\n    }\n}\n",
        answers: &[
            Some("Obj{N}"),
            Some("inner{N}"),
            Some("inner{N}"),
            Some("Obj{N}"),
        ],
    },
    Shape {
        label: "property_declaration at the top level",
        body: "val top{N}: Int = 3\n",
        answers: &[Some("top{N}")],
    },
    Shape {
        label: "function_declaration at the top level",
        body: "fun free{N}(): Int {\n    return 4\n}\n",
        answers: &[Some("free{N}"), Some("free{N}"), Some("free{N}")],
    },
];

/// EVERY BACKEND THIS BUILD SHIPS, and the population law says so against
/// `mnemosyne_cli::backends` rather than against this file's idea of one.
///
/// Round 1161 covered the three that arrived without a predecessor and left the
/// two PORTS out, on the reasoning that a port already had an oracle: agreement
/// with the implementation it replaced. Round 1162 found what that reasoning
/// cannot see — a defect the predecessor ALSO had. Rust's port agreed line for
/// line over 313 files and 221787 lines while both bound a `///` citation to
/// the wrong item. So the ports are here now, on the same laws as the rest.
const SUBJECTS: &[Subject] = &[
    Subject {
        language: "cpp",
        spec: &mnemosyne_plugin_tree_sitter_cpp::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_cpp::resolver()),
        prelude: "",
        shapes: CPP_SHAPES,
        globs: &["*.cpp", "*.cc", "*.h", "*.hpp"],
        real_tree: RealTree::NamedByEnv { min_files: 500 },
        witnesses: CPP_WITNESSES,
        inward: &[],
    },
    Subject {
        language: "go",
        spec: &mnemosyne_plugin_tree_sitter_go::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_go::resolver()),
        prelude: "package corpus\n\n",
        shapes: GO_SHAPES,
        globs: &["*.go"],
        real_tree: RealTree::NamedByEnv { min_files: 100 },
        witnesses: GO_WITNESSES,
        inward: &[],
    },
    Subject {
        language: "kotlin",
        spec: &mnemosyne_plugin_tree_sitter_kotlin::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_kotlin::resolver()),
        prelude: "",
        shapes: KOTLIN_SHAPES,
        globs: &["*.kt", "*.kts"],
        real_tree: RealTree::NamedByEnv { min_files: 300 },
        witnesses: KOTLIN_WITNESSES,
        inward: &[],
    },
    Subject {
        language: "python",
        spec: &mnemosyne_plugin_tree_sitter_python::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_python::resolver()),
        prelude: "",
        shapes: PYTHON_SHAPES,
        globs: &["*.py"],
        real_tree: RealTree::NamedByEnv { min_files: 100 },
        witnesses: PYTHON_WITNESSES,
        inward: &[],
    },
    Subject {
        language: "rust",
        spec: &mnemosyne_plugin_tree_sitter_rust::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_rust::resolver()),
        prelude: "",
        shapes: RUST_SHAPES,
        globs: &["*.rs"],
        // THE ONE SUBJECT WHOSE REAL TREE TRAVELS WITH THE LAWS. Rust's new
        // doc-comment binding would otherwise meet no real source anywhere: the
        // enrolled consumer corpus is C++, and this repository's own citations
        // are all module-level, so its symbol axis has an empty population.
        real_tree: RealTree::ThisRepository,
        witnesses: RUST_WITNESSES,
        inward: RUST_INWARD,
    },
];

/// One entry of a backend's `documented_kinds`, and the source that witnesses
/// it.
///
/// The witness source is `before + doc + decl + after`, and every line of `doc`
/// must resolve to `binds_to`. The CONTROL is derived from the same parts with
/// one blank line inserted between `doc` and `decl`, where those lines must
/// resolve to `detached` instead — the enclosing declaration, or nothing. That
/// pair is what separates "pass 1 bound the comment to the declaration below"
/// from "pass 2 happened to cover the comment row anyway".
struct DocWitness {
    /// The `documented_kinds` entry this witnesses.
    kind: &'static str,
    /// Context above the comment — an enclosing class, a group opener.
    before: &'static str,
    /// The comment run holding the citation. Whole lines, one or more.
    doc: &'static str,
    /// The declaration the comment sits above.
    decl: &'static str,
    /// Whatever closes `before`.
    after: &'static str,
    /// The name every line of `doc` resolves to.
    binds_to: &'static str,
    /// What those same lines answer once a blank line separates them from the
    /// declaration: the enclosing declaration's name, or `None` for a comment
    /// no declaration covers.
    detached: Option<&'static str>,
}

/// One entry of a backend's `inward_markers`, and the pair of sources that
/// witnesses it.
///
/// The two sources differ in ONE thing: how the comment on `line` is spelled.
/// Both spellings are the same node kind, so nothing but the marker can
/// separate them — and they must resolve differently, or the marker is
/// decorative.
struct InwardWitness {
    /// The `inward_markers` entry this witnesses.
    marker: &'static str,
    /// A source whose comment on `line` carries that marker.
    inward: &'static str,
    /// The same source with the comment spelled the outward way.
    outward: &'static str,
    /// The 1-based line the two sources differ on.
    line: u32,
    /// What the inward spelling resolves to — the scope the comment is inside,
    /// or nothing at all at the top of a file.
    inward_binds_to: Option<&'static str>,
    /// What the outward spelling resolves to: the declaration below it.
    outward_binds_to: &'static str,
}

const CPP_WITNESSES: &[DocWitness] = &[
    DocWitness {
        kind: "function_definition",
        before: "",
        doc: "/// documents f\n",
        decl: "int f() { return 0; }\n",
        after: "",
        binds_to: "f",
        detached: None,
    },
    DocWitness {
        // NESTED, so the control answers a NAME rather than nothing: without
        // pass 1 this comment resolves to the class it sits in, which is a
        // different string and not merely an absent one.
        kind: "field_declaration",
        before: "class Holder {\n",
        doc: "  /// documents m\n",
        decl: "  int m;\n",
        after: "};\n",
        binds_to: "m",
        detached: Some("Holder"),
    },
    DocWitness {
        kind: "class_specifier",
        before: "",
        doc: "/// documents K\n",
        decl: "class K {};\n",
        after: "",
        binds_to: "K",
        detached: None,
    },
    DocWitness {
        kind: "struct_specifier",
        before: "",
        doc: "/// documents S\n",
        decl: "struct S {};\n",
        after: "",
        binds_to: "S",
        detached: None,
    },
    DocWitness {
        kind: "union_specifier",
        before: "",
        doc: "/// documents U\n",
        decl: "union U { int a; };\n",
        after: "",
        binds_to: "U",
        detached: None,
    },
    DocWitness {
        kind: "enum_specifier",
        before: "",
        doc: "/// documents E\n",
        decl: "enum E { A };\n",
        after: "",
        binds_to: "E",
        detached: None,
    },
];

const GO_WITNESSES: &[DocWitness] = &[
    DocWitness {
        // A MULTI-LINE RUN, which is the shape the sibling walk exists for: the
        // citation may be on any line of it and all of them document the same
        // declaration.
        kind: "function_declaration",
        before: "",
        doc: "// documents F\n// and keeps documenting it\n",
        decl: "func F() {}\n",
        after: "",
        binds_to: "F",
        detached: None,
    },
    DocWitness {
        kind: "method_declaration",
        before: "type Holder struct{}\n\n",
        doc: "// documents M\n",
        decl: "func (h Holder) M() {}\n",
        after: "",
        binds_to: "M",
        detached: None,
    },
    DocWitness {
        kind: "type_declaration",
        before: "",
        doc: "// documents T\n",
        decl: "type T int\n",
        after: "",
        binds_to: "T",
        detached: None,
    },
    DocWitness {
        kind: "const_declaration",
        before: "",
        doc: "// documents C\n",
        decl: "const C = 1\n",
        after: "",
        binds_to: "C",
        detached: None,
    },
    DocWitness {
        kind: "var_declaration",
        before: "",
        doc: "// documents V\n",
        decl: "var V int\n",
        after: "",
        binds_to: "V",
        detached: None,
    },
    DocWitness {
        kind: "type_spec",
        before: "type (\n",
        doc: "\t// documents Grouped\n",
        decl: "\tGrouped int\n",
        after: ")\n",
        binds_to: "Grouped",
        detached: None,
    },
    DocWitness {
        kind: "const_spec",
        before: "const (\n",
        doc: "\t// documents CG\n",
        decl: "\tCG = 2\n",
        after: ")\n",
        binds_to: "CG",
        detached: None,
    },
    DocWitness {
        kind: "var_spec",
        before: "var (\n",
        doc: "\t// documents VG\n",
        decl: "\tVG int\n",
        after: ")\n",
        binds_to: "VG",
        detached: None,
    },
];

const PYTHON_WITNESSES: &[DocWitness] = &[
    DocWitness {
        kind: "function_definition",
        before: "",
        doc: "# documents f\n",
        decl: "def f():\n    return 0\n",
        after: "",
        binds_to: "f",
        detached: None,
    },
    DocWitness {
        kind: "class_definition",
        before: "",
        doc: "# documents C\n",
        decl: "class C:\n    pass\n",
        after: "",
        binds_to: "C",
        detached: None,
    },
    DocWitness {
        kind: "decorated_definition",
        before: "class Outer:\n",
        doc: "    # documents d\n",
        decl: "    @wrapper\n    def d(self):\n        pass\n",
        after: "",
        binds_to: "d",
        detached: Some("Outer"),
    },
];

const KOTLIN_WITNESSES: &[DocWitness] = &[
    DocWitness {
        // KDOC, which is a `block_comment` here while `//` is a `line_comment`
        // — the pair is why `comment_kinds` is a list.
        kind: "class_declaration",
        before: "",
        doc: "/** documents K */\n",
        decl: "class K\n",
        after: "",
        binds_to: "K",
        detached: None,
    },
    DocWitness {
        kind: "function_declaration",
        before: "class Holder {\n",
        doc: "    // documents f\n",
        decl: "    fun f(): Int {\n        return 1\n    }\n",
        after: "}\n",
        binds_to: "f",
        detached: Some("Holder"),
    },
    DocWitness {
        kind: "object_declaration",
        before: "",
        doc: "// documents O\n",
        decl: "object O\n",
        after: "",
        binds_to: "O",
        detached: None,
    },
    DocWitness {
        kind: "property_declaration",
        before: "",
        doc: "// documents p\n",
        decl: "val p: Int = 1\n",
        after: "",
        binds_to: "p",
        detached: None,
    },
];

const RUST_WITNESSES: &[DocWitness] = &[
    DocWitness {
        kind: "function_item",
        before: "impl Holder {\n",
        doc: "    /// documents beta\n",
        decl: "    fn beta(&self) {}\n",
        after: "}\n",
        binds_to: "beta",
        detached: Some("Holder"),
    },
    DocWitness {
        kind: "struct_item",
        before: "",
        doc: "/// documents S\n",
        decl: "pub struct S;\n",
        after: "",
        binds_to: "S",
        detached: None,
    },
    DocWitness {
        kind: "enum_item",
        before: "",
        doc: "/// documents E\n",
        decl: "enum E { A }\n",
        after: "",
        binds_to: "E",
        detached: None,
    },
    DocWitness {
        kind: "trait_item",
        before: "",
        doc: "/// documents T\n",
        decl: "trait T {}\n",
        after: "",
        binds_to: "T",
        detached: None,
    },
    DocWitness {
        kind: "function_signature_item",
        before: "trait Holder2 {\n",
        doc: "    /// documents req\n",
        decl: "    fn req(&self);\n",
        after: "}\n",
        binds_to: "req",
        detached: Some("Holder2"),
    },
    DocWitness {
        kind: "associated_type",
        before: "trait Holder3 {\n",
        doc: "    /// documents Assoc\n",
        decl: "    type Assoc;\n",
        after: "}\n",
        binds_to: "Assoc",
        detached: Some("Holder3"),
    },
    DocWitness {
        kind: "impl_item",
        before: "",
        doc: "/// documents the impl\n",
        decl: "impl S {}\n",
        after: "",
        binds_to: "S",
        detached: None,
    },
    DocWitness {
        kind: "mod_item",
        before: "",
        doc: "/// documents m\n",
        decl: "mod m {}\n",
        after: "",
        binds_to: "m",
        detached: None,
    },
    DocWitness {
        kind: "const_item",
        before: "",
        doc: "/// documents C\n",
        decl: "const C: u32 = 1;\n",
        after: "",
        binds_to: "C",
        detached: None,
    },
    DocWitness {
        kind: "static_item",
        before: "",
        doc: "/// documents ST\n",
        decl: "static ST: u32 = 1;\n",
        after: "",
        binds_to: "ST",
        detached: None,
    },
    DocWitness {
        kind: "type_item",
        before: "",
        doc: "/// documents TA\n",
        decl: "type TA = u32;\n",
        after: "",
        binds_to: "TA",
        detached: None,
    },
    DocWitness {
        kind: "union_item",
        before: "",
        doc: "/// documents U\n",
        decl: "union U { a: u32 }\n",
        after: "",
        binds_to: "U",
        detached: None,
    },
    DocWitness {
        kind: "macro_definition",
        before: "",
        doc: "/// documents mac\n",
        decl: "macro_rules! mac { () => {} }\n",
        after: "",
        binds_to: "mac",
        detached: None,
    },
];

/// The one language here with a spelling for "documents the scope I am in".
const RUST_INWARD: &[InwardWitness] = &[InwardWitness {
    marker: "inner_doc_comment_marker",
    inward: "mod holder {\n    //! documents holder\n    pub struct Inner;\n}\n",
    outward: "mod holder {\n    /// documents Inner\n    pub struct Inner;\n}\n",
    line: 2,
    inward_binds_to: Some("holder"),
    outward_binds_to: "Inner",
}];

/// One built file: its source and the name expected at each 1-based line.
struct Built {
    label: String,
    source: String,
    expected: BTreeMap<u32, String>,
}

/// Place `shapes` one after another, in the order given, into one file.
///
/// EVERY PLACEMENT GETS ITS OWN ORDINAL, so the same shape appearing twice in a
/// file declares two different names — an answer that came from the wrong
/// placement is then a wrong NAME rather than a coincidence.
fn compose(subject: &Subject, order: &[usize], label: String) -> Built {
    let mut source = subject.prelude.to_string();
    let mut expected = BTreeMap::new();
    let mut line = source.lines().count() as u32;
    for (ordinal, &index) in order.iter().enumerate() {
        let shape = &subject.shapes[index];
        let n = ordinal.to_string();
        let body = shape.body.replace("{N}", &n);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(
            lines.len(),
            shape.answers.len(),
            "{}: shape `{}` has {} line(s) of source and {} expected answer(s) \
             — the composer would then be checking the wrong lines",
            subject.language,
            shape.label,
            lines.len(),
            shape.answers.len()
        );
        for (offset, text) in lines.iter().enumerate() {
            source.push_str(text);
            source.push('\n');
            line += 1;
            if let Some(name) = shape.answers[offset] {
                expected.insert(line, name.replace("{N}", &n));
            }
        }
        // A blank line between placements: covered by no declaration, so it is
        // also the check that a declaration's extent stops where it should.
        source.push('\n');
        line += 1;
    }
    Built {
        label,
        source,
        expected,
    }
}

/// The built corpus for one backend.
///
/// EACH SHAPE ALONE, then all of them in order, then all of them REVERSED. The
/// two whole-file orders are what put every shape next to a different neighbour,
/// which is the adjacency law 1 is about; the isolated files are what make a
/// failure name one shape instead of a file holding six.
fn built_corpus(subject: &Subject) -> Vec<Built> {
    let mut out = Vec::new();
    for (index, shape) in subject.shapes.iter().enumerate() {
        out.push(compose(subject, &[index], format!("{} alone", shape.label)));
    }
    let forward: Vec<usize> = (0..subject.shapes.len()).collect();
    let mut backward = forward.clone();
    backward.reverse();
    out.push(compose(subject, &forward, "every shape, in order".into()));
    out.push(compose(subject, &backward, "every shape, reversed".into()));
    // Every shape twice, so a placement's ordinal is what distinguishes two
    // instances of one form standing next to each other.
    let doubled: Vec<usize> = forward.iter().chain(forward.iter()).copied().collect();
    out.push(compose(subject, &doubled, "every shape, twice".into()));
    out
}

#[test]
fn every_pattern_a_backend_declares_is_one_the_built_corpus_reaches() {
    // LAW 0 — THE NON-VACUITY THIS ROUND EXISTS FOR, and the whole reason the
    // corpus is built rather than sampled. The denominator is the compiled
    // query's own pattern count; the numerator is the set of pattern indices the
    // matcher reports. Neither is a list anybody maintains, so a backend that
    // grows a pattern reddens this until the corpus grows a shape.
    for subject in SUBJECTS {
        let declared = subject
            .spec
            .pattern_count()
            .expect("the backend's query compiles");
        assert!(
            declared > 0,
            "{}: the query declares no pattern at all, so every law below is \
             about nothing",
            subject.language
        );
        let mut reached: BTreeSet<usize> = BTreeSet::new();
        for built in built_corpus(subject) {
            reached.extend(
                subject
                    .spec
                    .patterns_exercised(&built.source)
                    .expect("the backend parses its own corpus"),
            );
        }
        let missing: Vec<usize> = (0..declared).filter(|p| !reached.contains(p)).collect();
        assert!(
            missing.is_empty(),
            "{}: the built corpus reaches {} of {declared} declared pattern(s); \
             nothing exercises pattern index {:?}. Add a shape to this file that \
             exhibits it — a pattern no corpus reaches is a claim nothing checks.",
            subject.language,
            reached.len(),
            missing
        );
    }
}

#[test]
fn the_built_corpus_answers_exactly_what_was_planted_in_it() {
    // LAWS 1, 2 AND 3 OVER THE CORPUS THE REPOSITORY CARRIES — the pass that
    // runs on every machine and in CI.
    for subject in SUBJECTS {
        // A SHAPE THAT NAMES NOTHING IS A SHAPE WHOSE ORACLE SAYS NOTHING, and
        // it would sail through the comparison below by matching an empty
        // answer with an empty expectation. Law 0 cannot catch it either: a
        // shape can exercise a pattern the backend declines to NAME.
        for shape in subject.shapes {
            assert!(
                shape.answers.iter().any(Option::is_some),
                "{}: shape `{}` expects no name on any of its lines, so it \
                 contributes nothing an oracle can be wrong about",
                subject.language,
                shape.label
            );
        }
        let resolver = (subject.resolver)();
        for built in built_corpus(subject) {
            assert!(
                !built.expected.is_empty(),
                "{} / {}: nothing was planted in this file, so comparing its \
                 answers to the plan compares two empty maps",
                subject.language,
                built.label
            );
            let n = built.source.lines().count() as u32;
            let all: Vec<u32> = (1..=n).collect();
            let whole = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &all)
                .expect("the resolver answers");

            // LAW 1 — the same lines split across two calls that INTERLEAVE
            // them. Every line's neighbours move to the other call, so a batch
            // that let one line's resolution decide another's cannot agree.
            let odds: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 1).collect();
            let evens: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 0).collect();
            let mut split = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &odds)
                .expect("the resolver answers");
            split.extend(
                resolver
                    .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &evens)
                    .expect("the resolver answers"),
            );
            assert_eq!(
                whole, split,
                "{} / {}: batching changed the answer\n--- source ---\n{}",
                subject.language, built.label, built.source
            );

            // LAW 2 — an answer is text from the file it was given.
            for (line, name) in &whole {
                assert!(
                    built.source.contains(name.as_str()),
                    "{} / {}: line {line} answered `{name}`, which does not occur \
                     in the source it came from",
                    subject.language,
                    built.label
                );
            }

            // LAW 3 — AN ORACLE AND NOT A FLOOR. The composer planted every
            // declaration and knows which one covers each line, so both
            // directions are checked: a line that should answer and does not,
            // and a line that answers something the composer did not plant.
            let got: BTreeMap<u32, String> = whole.into_iter().collect();
            assert_eq!(
                got, built.expected,
                "{} / {}: the resolver's answers are not the planted ones\n\
                 --- source ---\n{}",
                subject.language, built.label, built.source
            );
        }
    }
}

/// A witness source and the 1-based lines its comment run occupies.
///
/// `detached` inserts the blank line that breaks the association, which is the
/// only difference between the law's source and its control.
fn witness_source(subject: &Subject, w: &DocWitness, detached: bool) -> (String, Vec<u32>) {
    let head = format!("{}{}", subject.prelude, w.before);
    let first = head.lines().count() as u32 + 1;
    let count = w.doc.lines().count() as u32;
    let gap = if detached { "\n" } else { "" };
    let source = format!("{head}{}{gap}{}{}", w.doc, w.decl, w.after);
    (source, (first..first + count).collect())
}

fn answers(subject: &Subject, source: &str, lines: &[u32]) -> BTreeMap<u32, String> {
    (subject.resolver)()
        .resolve_symbols_at(Path::new("/no/such/file"), source, lines)
        .expect("the resolver answers")
}

#[test]
fn every_backend_this_build_ships_answers_the_doc_comment_criterion() {
    // THE POPULATION IS THE BACKEND TABLE, not this file's idea of one. A row
    // added there without a subject here reddens this, which is the same
    // derivation Law 0 makes from the compiled query.
    let named: BTreeSet<&str> = SUBJECTS.iter().map(|s| s.spec.backend_key).collect();
    let shipped: BTreeSet<&str> = mnemosyne_cli::backends::keys().into_iter().collect();
    assert_eq!(
        named, shipped,
        "every shipped backend meets EVERY law in this file — there is no \
         longer a subset that meets only some of them"
    );

    for subject in SUBJECTS {
        // THE RULE IS NOT OPTIONAL. An empty list was how a backend used to
        // decline it without answering why, and that state no longer exists.
        let declared: BTreeSet<&str> = subject
            .spec
            .doc_comments
            .documented_kinds
            .iter()
            .copied()
            .collect();
        assert!(
            !declared.is_empty(),
            "{}: this backend documents no declaration kind at all, which is \
             the one answer the criterion does not take",
            subject.language
        );
        let witnessed: BTreeSet<&str> = subject.witnesses.iter().map(|w| w.kind).collect();
        assert_eq!(
            witnessed, declared,
            "{}: the kinds this backend CLAIMS and the kinds a witness below \
             actually binds are not the same set",
            subject.language
        );

        let markers: BTreeSet<&str> = subject
            .spec
            .doc_comments
            .inward_markers
            .iter()
            .copied()
            .collect();
        let marker_witnesses: BTreeSet<&str> = subject.inward.iter().map(|w| w.marker).collect();
        assert_eq!(
            marker_witnesses, markers,
            "{}: the inward markers this backend claims and the ones witnessed \
             below are not the same set",
            subject.language
        );
    }
}

#[test]
fn every_kind_a_backend_documents_is_one_a_comment_above_it_binds_to() {
    // THE OTHER HALF OF THE SPEC, PUT TO THE SAME TEST AS THE QUERY. Law 0
    // derives its population from the compiled query and requires the corpus to
    // reach every pattern. `documented_kinds` is a claim of exactly the same
    // shape — these declarations are the ones a comment above may be
    // documenting — and until this law nothing asked it anything: a kind that
    // could never bind, or a whole rule that never fired, read from outside
    // exactly like a language that chose not to have one.
    for subject in SUBJECTS {
        for w in subject.witnesses {
            let (source, lines) = witness_source(subject, w, false);
            let bound = answers(subject, &source, &lines);
            for line in &lines {
                assert_eq!(
                    bound.get(line).map(String::as_str),
                    Some(w.binds_to),
                    "{} / {}: line {line} of the comment above this declaration \
                     does not bind to it\n--- source ---\n{source}",
                    subject.language,
                    w.kind
                );
            }

            // THE CONTROL, AND IT IS THE HALF THAT SAYS PASS 1 DID THE WORK.
            // One blank line breaks the association, and the same lines must
            // fall through to whatever covers them — the enclosing declaration,
            // or nothing. Without this, a witness would also pass for a comment
            // that pass 2 happened to cover with the same name.
            let (control, control_lines) = witness_source(subject, w, true);
            let fell_through = answers(subject, &control, &control_lines);
            for line in &control_lines {
                assert_eq!(
                    fell_through.get(line).map(String::as_str),
                    w.detached,
                    "{} / {}: with a blank line between, line {line} must fall \
                     through to {:?}\n--- source ---\n{control}",
                    subject.language,
                    w.kind,
                    w.detached
                );
            }
        }
    }
}

#[test]
fn a_comment_marked_inward_documents_its_scope_and_not_what_follows() {
    // THE MARKER IS THE WHOLE DIFFERENCE. Both sources below spell the same
    // node kind in the same position, one row above the same declaration, and
    // they must answer differently — the inward one names the scope the comment
    // is inside, the outward one names the declaration below it. A marker that
    // is decorative fails here, and so does a spec that names a node kind its
    // grammar does not produce: the inward source would bind forward like any
    // other comment.
    for subject in SUBJECTS {
        for w in subject.inward {
            let lines = [w.line];
            assert_eq!(
                answers(subject, w.inward, &lines)
                    .get(&w.line)
                    .map(String::as_str),
                w.inward_binds_to,
                "{} / {}: an inward-marked comment must document its scope\n\
                 --- source ---\n{}",
                subject.language,
                w.marker,
                w.inward
            );
            assert_eq!(
                answers(subject, w.outward, &lines)
                    .get(&w.line)
                    .map(String::as_str),
                Some(w.outward_binds_to),
                "{} / {}: the same comment spelled outward must document what \
                 follows it — otherwise the two sources agree and the marker \
                 decides nothing\n--- source ---\n{}",
                subject.language,
                w.marker,
                w.outward
            );
            // The marker must be a node this grammar actually produces here,
            // so a failure above names a wrong spelling rather than a wrong
            // engine.
            let tree = subject.spec.parse(w.inward).expect("the corpus parses");
            assert!(
                tree.root_node().to_sexp().contains(w.marker),
                "{} / {}: the parse of the inward source holds no such node",
                subject.language,
                w.marker
            );
        }
    }
}

/// The discovery pass — a real tree, when this machine has one.
///
/// OPT-IN AND NEVER SILENT. Round 1157 defaulted this to a checkout that exists
/// on one machine and let its absence pass with a printed line, so the laws were
/// guarded there and nowhere else; Round 1160 registered that as a debt and this
/// is its repair. The regression job now belongs to the built corpus above,
/// which runs everywhere. What is left here is the job a built corpus cannot do:
/// meet shapes nobody thought to write. When the variable names a tree, that
/// tree must be there — an unset variable is a pass because the laws above
/// already ran, and a WRONG one is a failure because it means somebody asked for
/// a measurement and did not get it.
fn discovery_corpus() -> Option<PathBuf> {
    let named = std::env::var("MNEMOSYNE_RESOLVER_CORPUS").ok()?;
    let root = PathBuf::from(named);
    assert!(
        root.join(".git").exists(),
        "MNEMOSYNE_RESOLVER_CORPUS names {} and there is no checkout there — a \
         measurement was asked for and cannot be taken, which is not the same \
         as one nobody asked for",
        root.display()
    );
    Some(root)
}

/// This repository's root, from the test binary's own manifest.
///
/// NOT `git rev-parse`, which answers about the CURRENT DIRECTORY and would
/// make this pass read whatever tree the caller happened to stand in.
fn this_repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the workspace root")
        .to_path_buf()
}

fn tracked(root: &Path, globs: &[&str]) -> Vec<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).arg("ls-files");
    for g in globs {
        cmd.arg(g);
    }
    let out = cmd.output().expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed in {root:?}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_real_tree_meets_the_same_laws() {
    let named = discovery_corpus();
    let here = this_repository();
    let mut measured: Vec<&str> = Vec::new();
    for subject in SUBJECTS {
        // WHICH TREE, AND WHETHER THERE IS ONE, IS PART OF THE SUBJECT rather
        // than of this loop — so a subject whose tree is absent is a subject
        // that says so by name, not a silent iteration.
        let root = match subject.real_tree {
            RealTree::ThisRepository => here.clone(),
            RealTree::NamedByEnv { .. } => match &named {
                Some(root) => root.clone(),
                None => continue,
            },
        };
        let files = tracked(&root, subject.globs);
        match subject.real_tree {
            RealTree::NamedByEnv { min_files } => assert!(
                files.len() >= min_files,
                "{}: {} file(s) in the corpus, below the floor of {min_files} — \
                 the corpus derivation broke, not the resolver",
                subject.language,
                files.len(),
            ),
            // AN EQUALITY, NOT A NUMBER. Round 1150 was bitten by a hand-set
            // floor that outlived the population it was written for; this tree
            // knows exactly how many files it tracks, so the law is that the
            // pass read all of them.
            RealTree::ThisRepository => assert!(
                !files.is_empty(),
                "{}: this repository tracks no file matching {:?}, so the pass \
                 that is supposed to always run is about nothing",
                subject.language,
                subject.globs
            ),
        }
        measured.push(subject.language);
        let resolver = (subject.resolver)();
        // WHAT THE ALWAYS-ON PASS COSTS, PRINTED. It reads every tracked file
        // three times — whole, odds, evens — and each is a parse, so this is
        // the one law in this file that a reader might want to price. Round
        // 1163 took it from 129s to 77s by removing two quadratics it exposed;
        // hiding the remainder would make the next regression invisible.
        let started = std::time::Instant::now();

        let mut read = 0usize;
        let mut lines_total = 0usize;
        let mut answered = 0usize;
        let mut batch_disagreements: Vec<String> = Vec::new();
        let mut not_in_source: Vec<String> = Vec::new();

        for rel in &files {
            let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
                continue;
            };
            let n = text.lines().count() as u32;
            if n == 0 {
                continue;
            }
            read += 1;
            let all: Vec<u32> = (1..=n).collect();
            lines_total += all.len();

            let batched = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &text, &all)
                .expect("the resolver answers");
            let odds: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 1).collect();
            let evens: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 0).collect();
            let mut split: BTreeMap<u32, String> = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &text, &odds)
                .expect("the resolver answers");
            split.extend(
                resolver
                    .resolve_symbols_at(Path::new("/no/such/file"), &text, &evens)
                    .expect("the resolver answers"),
            );
            answered += batched.len();
            if batched != split {
                for line in &all {
                    let (a, b) = (batched.get(line), split.get(line));
                    if a != b {
                        batch_disagreements.push(format!("{rel}:{line} whole={a:?} split={b:?}"));
                    }
                }
            }

            for (line, name) in &batched {
                if !text.contains(name.as_str()) {
                    not_in_source.push(format!("{rel}:{line} answered `{name}`"));
                }
            }
        }

        println!(
            "{} @ {}: {read} file(s), {lines_total} line(s), {answered} answered, \
             {} batch disagreement(s), {} answer(s) absent from their source, \
             {:.1}s",
            subject.language,
            root.display(),
            batch_disagreements.len(),
            not_in_source.len(),
            started.elapsed().as_secs_f64()
        );

        // EVERY TRACKED FILE WAS READ, for the tree that travels with the laws.
        // `read` skips a file that is empty or not UTF-8, and this repository
        // has neither — so a drop here is the pass quietly shrinking rather
        // than the tree honestly changing.
        if matches!(subject.real_tree, RealTree::ThisRepository) {
            assert_eq!(
                read,
                files.len(),
                "{}: {read} of {} tracked file(s) were read",
                subject.language,
                files.len()
            );
        }

        // A FLOOR AND NOT AN ORACLE, and only here: nothing knows what a real
        // tree's right answers are, which is exactly why the built corpus exists.
        assert!(
            answered > lines_total / 20,
            "{}: only {answered} of {lines_total} line(s) answered — a resolver \
             that reaches almost nothing satisfies the other laws for free",
            subject.language
        );
        assert!(
            batch_disagreements.is_empty(),
            "{}: {} line(s) where batching changed the answer, first 20:\n  {}",
            subject.language,
            batch_disagreements.len(),
            batch_disagreements
                .iter()
                .take(20)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        );
        assert!(
            not_in_source.is_empty(),
            "{}: {} answer(s) do not occur in the source they came from, first \
             20:\n  {}",
            subject.language,
            not_in_source.len(),
            not_in_source
                .iter()
                .take(20)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        );
    }

    // AND THE PASS SAYS WHICH SUBJECTS IT REACHED. With no variable set this is
    // exactly the subjects whose tree travels with the laws — a list, so that
    // "nothing was measured" cannot be printed by an empty loop and read as
    // "everything was clean".
    println!("real-tree pass measured: {measured:?}");
    let always: Vec<&str> = SUBJECTS
        .iter()
        .filter(|s| matches!(s.real_tree, RealTree::ThisRepository))
        .map(|s| s.language)
        .collect();
    assert!(
        !always.is_empty(),
        "no subject reads a tree that travels with the laws, so this pass is \
         opt-in again and the two ports meet real source nowhere"
    );
    for language in &always {
        assert!(
            measured.contains(language),
            "{language} names this repository as its corpus and was not measured"
        );
    }
}
