//! Dogfood the generator against a fixture, at THIS crate's build time (Round 770).
//!
//! The round's claim is that a baked projection is checked by the compiler. A
//! generator whose own output nothing compiles would be the one place that claim
//! was not tested — so the output is compiled here, by the crate that produces
//! it, on every build. `src/render.rs` is `include!`d rather than imported (a
//! build script cannot depend on its own crate), which is also why the generator
//! lives in a module with no crate-internal dependencies: one text, two callers,
//! nothing to drift.
//!
//! The fixture deliberately exercises every emitted form — all three door kinds,
//! an anchored rung, both locator kinds, `Option` in Some and None, and strings
//! carrying quotes, newlines, backslashes and non-ASCII. The escaping claim in
//! `render`'s docs ("`{:?}` on a `&str` is exactly Rust literal escaping") is
//! therefore proven by rustc rather than asserted.
//!
//! Round 774 added the quest fixture on the same terms: all three `QuestState`
//! variants, a completion with an actor and one without, and a quest spanning two
//! roads.
//!
//! Round 780 added a SECOND family of fixtures on a different axis: the ones
//! above prove the emitted forms are right, and are far too small to say anything
//! about how much STACK the artifact costs to build. `stack_fixtures` emits
//! grown ones — each at a size and its quadruple, plus a deliberately unbounded
//! control — for `tests/projection_stack.rs`. See that file for what the pair
//! buys; here it is enough that a fixture nothing grows cannot measure growth.
//!
//! Round 782 added a THIRD family on a third axis. `alloc_fixtures` emits the
//! same data held three ways — owned as shipped, borrowed from `.rodata`, and
//! assembled entirely from `static`s — because the first consumer asked for
//! projection types that can borrow and nobody had measured what borrowing costs.
//! `tests/projection_alloc.rs` weighs them; the shipped types are untouched,
//! since what they should become is the decision those numbers are for.

use std::collections::{HashMap, HashSet};

use mnemosyne_engine::{DisclosureMode, Modality};

// The generator itself, spliced in: a build script cannot depend on its own
// crate, and a second copy would be free to drift from the one CI compiles.
include!("src/render.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/render.rs");
    println!("cargo:rerun-if-changed=build.rs");

    // A string that would break a naive quoter: an embedded quote, a newline, a
    // backslash, and non-ASCII prose.
    let nasty = "그는 \"셈\"이라 했다.\n뒤에 \\ 하나.";

    let parts = ProjectionParts {
        telling: "reader".to_string(),
        by_world: vec![(
            "main".to_string(),
            vec![(
                "sc-01".to_string(),
                vec![
                    LinePart {
                        fact_id: "f-a".into(),
                        text: nasty.into(),
                        mode: DisclosureMode::Hint,
                        frame: "ground-truth".into(),
                        entities: vec!["ent-a".into()].into(),
                        carrier: Some("ent-ledger".into()),
                        typed_predicate: Some("did".into()),
                        quote: Some(nasty.into()),
                        count: Some(3),
                    },
                    LinePart {
                        fact_id: "f-b".into(),
                        text: "plain".into(),
                        mode: DisclosureMode::State,
                        frame: String::new().into(),
                        entities: Vec::new().into(),
                        carrier: None,
                        typed_predicate: None,
                        quote: None,
                        count: None,
                    },
                ],
            )],
        )],
        walks: vec![("main".to_string(), vec!["sc-01".to_string()])],
        titles: vec![("sc-01".to_string(), nasty.to_string())],
        cast: vec![(
            "sc-01".to_string(),
            vec![CastPart {
                entity: "ent-jongdeuk".into(),
                modality: Modality::Observed,
                can_answer: true,
                quote: nasty.into(),
            }],
        )],
        forks: vec![ForkPart {
            at: "sc-01".to_string(),
            parent: "main".to_string(),
            world: "dark".to_string(),
            label: nasty.to_string(),
        }],
        divergent_endings: vec!["dark".to_string()],
        interactivity: Interactivity {
            ladders: HashMap::from([(
                "sc-01".to_string(),
                vec![
                    Rung {
                        question: nasty.to_string(),
                        question_anchor: Some(ContentAnchor {
                            source: "M.md".to_string(),
                            locator: Locator::Prefix("이름을".to_string()),
                        }),
                        reveals: "f-a".to_string(),
                        needs: vec!["f-b".to_string()],
                    },
                    Rung {
                        question: "free".to_string(),
                        question_anchor: Some(ContentAnchor {
                            source: "M.md".to_string(),
                            locator: Locator::Cfi("/6/4[c]!/4/2".to_string()),
                        }),
                        reveals: "f-b".to_string(),
                        needs: Vec::new(),
                    },
                ],
            )]),
            objects: HashSet::from(["ent-ledger".to_string()]),
            free_investigate: true,
        },
        choice_entity_refs: vec![mnemosyne_engine::ChoiceEntityRef {
            section: "sc-01".to_string(),
            entity: "ent-a".to_string(),
            choice: nasty.to_string(),
        }],
        ask_doors: vec![(
            "sc-01".to_string(),
            vec![
                DoorPart::Ask {
                    question: nasty.to_string(),
                    reveals: "f-a".to_string(),
                },
                DoorPart::Examine {
                    object: "ent-ledger".to_string(),
                    reveals: vec!["f-b".to_string()],
                },
                DoorPart::Fork {
                    world: "dark".to_string(),
                    label: nasty.to_string(),
                },
            ],
        )],
        // Round 787 — a journal-routed offer, so the knowledge axis is CARRIED by
        // the fixture the downstream-constructibility test compiles. An empty one
        // would emit `vec![]` and prove nothing about the field.
        journal_offers: vec![(
            "main".to_string(),
            vec![("sc-01".to_string(), vec!["f-leg".to_string()])],
        )],
    };

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out = std::path::Path::new(&out_dir).join("fixture_playable.rs");
    std::fs::write(&out, render(&parts)).expect("write the generated fixture");

    // The quest axis, dogfooded the same way (Round 774). Every emitted form
    // again: all three QuestState variants, an actor present and absent, two
    // roads on one quest, and the nasty string in the objective — a quest
    // objective is authored prose and quotes it just as freely as a line does.
    let quests = QuestProjectionParts {
        telling: "reader".to_string(),
        quests: vec![
            QuestPart {
                quest_id: "q-knot-1".to_string(),
                objective: nasty.to_string(),
                actors: vec!["ent-jiun".to_string()],
                prerequisites: vec!["q-salt".to_string()],
                per_world: vec![
                    (
                        "dark".to_string(),
                        QuestWorldPart {
                            state: mnemosyne_engine::QuestState::Unknown,
                            completions: Vec::new(),
                        },
                    ),
                    (
                        "main".to_string(),
                        QuestWorldPart {
                            state: mnemosyne_engine::QuestState::Done,
                            completions: vec![
                                QuestCompletionPart {
                                    fact: "f-confess".to_string(),
                                    scene: "sc-gut".to_string(),
                                    actor: Some("ent-eldest".to_string()),
                                },
                                QuestCompletionPart {
                                    fact: "f-second".to_string(),
                                    scene: "sc-gut".to_string(),
                                    actor: None,
                                },
                            ],
                        },
                    ),
                ],
                preconditions: vec!["f-clue".to_string()],
            },
            QuestPart {
                quest_id: "q-salt".to_string(),
                objective: "the salt debt".to_string(),
                actors: Vec::new(),
                prerequisites: Vec::new(),
                per_world: vec![(
                    "main".to_string(),
                    QuestWorldPart {
                        state: mnemosyne_engine::QuestState::Open,
                        completions: Vec::new(),
                    },
                )],
                preconditions: Vec::new(),
            },
        ],
    };
    std::fs::write(
        std::path::Path::new(&out_dir).join("fixture_quest.rs"),
        render_quest(&quests),
    )
    .expect("write the generated quest fixture");

    // The passage axis, dogfooded on the same terms (Round 791): both locator
    // kinds, and prose carrying the quote / newline / backslash / non-ASCII that
    // authored text carries — the escaping claim is proven by rustc here too.
    let passages = PassagesParts {
        passages: vec![
            (
                "sc-01".to_string(),
                mnemosyne_engine::PassagePart {
                    anchor: ContentAnchor {
                        source: "M.md".to_string(),
                        locator: Locator::Prefix("이름을".to_string()),
                    },
                    text: nasty.into(),
                },
            ),
            (
                "sc-02".to_string(),
                mnemosyne_engine::PassagePart {
                    anchor: ContentAnchor {
                        source: "book.epub".to_string(),
                        locator: Locator::Cfi("/6/4[c]!/4/2".to_string()),
                    },
                    text: "plain".into(),
                },
            ),
        ],
    };
    std::fs::write(
        std::path::Path::new(&out_dir).join("fixture_passages.rs"),
        render_passages(&passages),
    )
    .expect("write the generated passage fixture");

    stack_fixtures(&out_dir);
    alloc_fixtures(&out_dir);
    prose_fixtures(&out_dir);
}

/// A gate's small size, and its quadruple. Neither number means anything on its
/// own — every assertion built on them is that some cost is the SAME at both,
/// which is a claim only a pair can make (Round 780, the Round 775 scaling
/// assertion moved from the emitted text to the running artifact).
///
/// One pair for both gates rather than one apiece: the stack gate and the
/// allocation gate make the same shape of claim about the same emitter, and two
/// pairs would be two places for "four times the store" to drift apart.
///
/// Resolved rather than hardcoded since Round 788, so a consumer can run these
/// fixtures at ITS scale — see [`fixture_sizes`] for why the knob is one number
/// naming the big end. Defaults to the 200/800 pair Round 780 and Round 782
/// measured at, so an unset environment measures exactly what they did.
fn sizes() -> (usize, usize) {
    println!("cargo:rerun-if-env-changed={FIXTURE_LINES_ENV}");
    let raw = std::env::var(FIXTURE_LINES_ENV).ok();
    fixture_sizes(raw.as_deref()).unwrap_or_else(|e| panic!("{e}"))
}

/// Emit the grown fixtures `tests/projection_stack.rs` measures, plus the build
/// facts it needs to say what it measured.
///
/// Three families at two sizes each: the playable artifact as shipped, the quest
/// artifact as shipped (repeated rather than assumed from the playable one — the
/// R774 discipline, since `render_quest` is free to stop calling `chunked` while
/// `render` still does), and an unbounded CONTROL that is the shipped emitter
/// with one difference, its bound. The control is what makes the other two
/// meaningful: it grows, so a run that reports no growth is reporting that the
/// instrument works.
fn stack_fixtures(out_dir: &str) {
    let unbounded = NonZeroUsize::new(usize::MAX).expect("usize::MAX is not zero");
    let write = |name: &str, source: String| {
        std::fs::write(
            std::path::Path::new(out_dir).join(format!("stack_{name}.rs")),
            source,
        )
        .expect("write a stack fixture");
    };

    let (small, big) = sizes();
    for (tag, n) in [("small", small), ("big", big)] {
        let parts = lines_parts(n);
        write(&format!("playable_{tag}"), render(&parts));
        write(&format!("control_{tag}"), render_bounded(&parts, unbounded));
        write(&format!("quest_{tag}"), render_quest(&quest_parts(n)));
    }

    // What the gate measured, from the one place that knows it. `OPT_LEVEL` is a
    // build-script variable, and the difference matters: at any level above 0 the
    // optimizer folds the temporaries this gate weighs, so a green run would mean
    // "not measured here" rather than "flat". The test refuses to pass in that
    // case and needs this to say WHY.
    let facts = format!(
        "pub const OPT_LEVEL: &str = {:?};\n\
         pub const SMALL: usize = {small};\n\
         pub const BIG: usize = {big};\n",
        std::env::var("OPT_LEVEL").expect("OPT_LEVEL")
    );
    std::fs::write(
        std::path::Path::new(out_dir).join("stack_build_facts.rs"),
        facts,
    )
    .expect("write the stack build facts");
}

/// Emit the controlled trio `tests/projection_alloc.rs` weighs (Round 782).
///
/// # What is being decided
///
/// The first consumer's third field report measured 34,825 allocations and
/// 2.78 MB per startup — the baked artifact BUILDS values that already sit in
/// `.rodata` as literals — and asked for projection types that can borrow
/// `&'static` instead of owning `String`. Round 775's lesson is that the emitted
/// SHAPE is measured before it is chosen, and nobody has measured what borrowing
/// costs rustc. These fixtures are that measurement.
///
/// # Why a controlled trio rather than the shipped types
///
/// The shipped `LinePart` cannot borrow today; changing it is the decision these
/// numbers are FOR, so the pair holds every other thing equal instead — the same
/// literal content, the same field count and order, the same chunk bound, the
/// same two sizes — and varies only how the data is held:
///
/// - `owned` — `String` / `Vec<String>` / `Option<String>`, the shape shipped now
/// - `borrowed` — `&'static str` in the same `Vec`-of-chunks assembly, the
///   consumer's literal request: fields that can point at `.rodata`
/// - `static_slice` — the consumer's own sketch taken all the way, chunk
///   `static`s plus a `static` of slices over them. It keeps Round 775's bound
///   (no single body holds the store) while materializing nothing at all
///
/// The middle arm is the one worth having on its own: it says what stopping
/// halfway buys, because the `Vec` spine survives it and the strings do not.
///
/// Round 794 added two more, `cow` and `cow_list`, because none of the three
/// above is a shape the shipped type can HOLD — see [`render_cow_lines`].
fn alloc_fixtures(out_dir: &str) {
    let write = |name: &str, source: String| {
        std::fs::write(
            std::path::Path::new(out_dir).join(format!("alloc_{name}.rs")),
            source,
        )
        .expect("write an alloc fixture");
    };
    let (small, big) = sizes();
    for (tag, n) in [("small", small), ("big", big)] {
        write(&format!("owned_{tag}"), render_owned_lines(n));
        write(&format!("borrowed_{tag}"), render_borrowed_lines(n));
        write(&format!("static_slice_{tag}"), render_static_lines(n));
        write(
            &format!("cow_{tag}"),
            render_cow_lines(n, "CowLine", &|entity| format!("vec![{COW}({entity})]")),
        );
        write(
            &format!("cow_list_{tag}"),
            render_cow_lines(n, "CowListLine", &|entity| {
                format!("{COW}(&[{COW}({entity})])")
            }),
        );
    }
}

/// The literal content every arm carries, so the three differ in how the data is
/// HELD and in nothing else. Returned as already-quoted Rust literals because
/// that is the one form all three emitters need.
fn measured_line_literals(i: usize) -> (String, String, String, String) {
    (
        format!("{:?}", format!("f-{i:06}")),
        format!("{:?}", "그는 \"셈\"이라 했다."),
        format!("{:?}", "ground-truth"),
        format!("{:?}", "ent-a"),
    )
}

/// Arm one: the shape shipped today — every field owned, assembled through
/// `Vec`-returning chunk functions.
fn render_owned_lines(n: usize) -> String {
    let ty = "::std::vec::Vec<crate::OwnedLine>";
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..n).collect::<Vec<_>>().chunks(CHUNK.get()).enumerate() {
        let body: Vec<String> = group
            .iter()
            .map(|i| {
                let (fact, text, frame, entity) = measured_line_literals(*i);
                format!(
                    "crate::OwnedLine {{ fact_id: {fact}.to_string(), text: {text}.to_string(), \
                     mode: ::mnemosyne_engine::DisclosureMode::State, frame: {frame}.to_string(), \
                     entities: vec![{entity}.to_string()], carrier: None, typed_predicate: None, \
                     quote: None, count: None }}"
                )
            })
            .collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, ty, &names)
}

/// Arm two: the same assembly, with every string pointing at `.rodata`. The
/// `Vec` spine is deliberately kept — it is the residual this arm exists to
/// price.
fn render_borrowed_lines(n: usize) -> String {
    let ty = "::std::vec::Vec<crate::BorrowedLine>";
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..n).collect::<Vec<_>>().chunks(CHUNK.get()).enumerate() {
        let body: Vec<String> = group.iter().map(|i| borrowed_literal(*i)).collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, ty, &names)
}

/// Arm three: chunk `static`s and a `static` of slices over them. Round 775's
/// bound still holds — no single item carries the whole store — and nothing is
/// built at run time, so the answer this arm gives is a count, not a ratio.
fn render_static_lines(n: usize) -> String {
    let mut out = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..n).collect::<Vec<_>>().chunks(CHUNK.get()).enumerate() {
        let body: Vec<String> = group.iter().map(|i| borrowed_literal(*i)).collect();
        let name = format!("__MN_{c}");
        let _ = writeln!(
            out,
            "static {name}: &[crate::BorrowedLine] = &[{}];",
            body.join(", ")
        );
        names.push(name);
    }
    let _ = writeln!(
        out,
        "static __MN_ALL: &[&[crate::BorrowedLine]] = &[{}];",
        names.join(", ")
    );
    out.push_str("pub fn build() -> &'static [&'static [crate::BorrowedLine]] { __MN_ALL }\n");
    out
}

fn borrowed_literal(i: usize) -> String {
    let (fact, text, frame, entity) = measured_line_literals(i);
    format!(
        "crate::BorrowedLine {{ fact_id: {fact}, text: {text}, \
         mode: ::mnemosyne_engine::DisclosureMode::State, frame: {frame}, \
         entities: &[{entity}], carrier: None, typed_predicate: None, quote: None, \
         count: None }}"
    )
}

/// The path a generated `Cow::Borrowed` is written through, spelled absolutely
/// because generated code lands in a module whose imports it does not control.
const COW: &str = "::std::borrow::Cow::Borrowed";

/// Arms four and five: the shape a shipped `LinePart` could actually take
/// (Round 794).
///
/// Round 782's borrowed arm is `&'static str`, which the shipped type CANNOT be:
/// `from_workspace` derives at run time and must own, so one type holding both
/// modes means `Cow<'static, str>`. That is a different arm, and Round 792 is the
/// round that established what happens when a design reasons about an arm it did
/// not measure — the arithmetic was right on bytes and wrong on calls, and only
/// measuring the arm separated them.
///
/// The two arms differ in ONE fragment, how the entity list is held, which is
/// also the one decision Round 785 deliberately left to the build round:
///
/// - `cow` — `Vec<Cow<'static, str>>`. The strings point at `.rodata` and the
///   list spine is still built per line.
/// - `cow_list` — `Cow<'static, [Cow<'static, str>]>`. The list points at the
///   binary too, so a baked line materializes nothing at all.
///
/// `entities` is passed as a closure rather than duplicated into two emitters, so
/// "everything else is equal" is true by construction rather than by review.
///
/// # The inline slice is `'static` and that was checked, not assumed
///
/// `cow_list` writes `Cow::Borrowed(&[Cow::Borrowed("ent-a")])` inline rather
/// than naming a `static` per line. Rvalue static promotion covers it — verified
/// by compiling both halves of the discrimination: the literal form builds, and
/// the same shape holding a runtime `String` fails with `E0515`, so the probe can
/// tell the two apart. It matters for what is being priced: a named `static` per
/// line would add an ITEM per line, and Round 789 measured that rustc charges per
/// item rather than per byte.
fn render_cow_lines(n: usize, struct_name: &str, entities: &dyn Fn(&str) -> String) -> String {
    let ty = format!("::std::vec::Vec<crate::{struct_name}>");
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..n).collect::<Vec<_>>().chunks(CHUNK.get()).enumerate() {
        let body: Vec<String> = group
            .iter()
            .map(|i| {
                let (fact, text, frame, entity) = measured_line_literals(*i);
                format!(
                    "crate::{struct_name} {{ fact_id: {COW}({fact}), text: {COW}({text}), \
                     mode: ::mnemosyne_engine::DisclosureMode::State, frame: {COW}({frame}), \
                     entities: {}, carrier: None, typed_predicate: None, quote: None, \
                     count: None }}",
                    entities(&entity)
                )
            })
            .collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, &ty, &names)
}

/// The `Vec` reassembly both allocating arms share, so the only thing that
/// differs between them is the element type.
fn vec_assembly(fns: &str, ty: &str, names: &[String]) -> String {
    let mut out = String::from(fns);
    let (first, rest) = names.split_first().expect("a non-empty fixture chunks");
    let extends: String = rest
        .iter()
        .map(|name| format!(" v.extend({name}());"))
        .collect();
    let _ = writeln!(
        out,
        "pub fn build() -> {ty} {{ let mut v = {first}();{extends} v }}"
    );
    out
}

/// `n` lines in one section — parts that GROW, for the scaling assertion.
fn lines_parts(n: usize) -> ProjectionParts {
    ProjectionParts {
        telling: "reader".to_string(),
        by_world: vec![(
            "main".to_string(),
            vec![(
                "sc-01".to_string(),
                (0..n)
                    .map(|i| LinePart {
                        fact_id: format!("f-{i:06}").into(),
                        text: "그는 \"셈\"이라 했다.".into(),
                        mode: DisclosureMode::State,
                        frame: "ground-truth".into(),
                        entities: vec!["ent-a".into()].into(),
                        carrier: None,
                        typed_predicate: None,
                        quote: None,
                        count: None,
                    })
                    .collect(),
            )],
        )],
        walks: vec![("main".to_string(), vec!["sc-01".to_string()])],
        titles: Vec::new(),
        cast: Vec::new(),
        forks: Vec::new(),
        divergent_endings: Vec::new(),
        interactivity: Interactivity::default(),
        choice_entity_refs: Vec::new(),
        ask_doors: Vec::new(),
        // The scaling fixture prices LINE growth; the journal axis is not what it
        // varies, so it carries none (Round 787).
        journal_offers: Vec::new(),
    }
}

/// `n` quests, the journal-axis sibling of [`lines_parts`].
fn quest_parts(n: usize) -> QuestProjectionParts {
    QuestProjectionParts {
        telling: "reader".to_string(),
        quests: (0..n)
            .map(|i| QuestPart {
                quest_id: format!("q-{i:06}"),
                objective: "그는 \"셈\"이라 했다.".to_string(),
                actors: vec!["ent-a".to_string()],
                prerequisites: Vec::new(),
                per_world: vec![(
                    "main".to_string(),
                    QuestWorldPart {
                        state: mnemosyne_engine::QuestState::Done,
                        completions: vec![QuestCompletionPart {
                            fact: format!("f-{i:06}"),
                            scene: "sc-01".to_string(),
                            actor: None,
                        }],
                    },
                )],
                preconditions: Vec::new(),
            })
            .collect(),
    }
}

/// A gate's prose sizes: the first consumer's actual spot count, and its
/// quadruple (Round 788+). Chars per spot is their measured mean.
///
/// Separate from [`sizes`] on purpose. That pair counts LINES — many short
/// strings — and this one counts SPOTS, which are few and long. They are the two
/// opposite shapes the same emitter would have to hold, and one pair cannot
/// stand for both.
const PROSE_SMALL: usize = 56;
const PROSE_BIG: usize = 224;
const PROSE_CHARS: usize = 2655;

/// Emit the prose-shaped arms `tests/projection_alloc.rs` weighs (Round 788+).
///
/// # What is being decided
///
/// The first consumer's third lift request asks the kernel to BAKE its passages:
/// `store_passages` parses a 1.2 MB sidecar at every startup, 38,913 allocations
/// and 48% of what remains of their startup, and they cannot bake it themselves
/// because `Passage::from_excerpt` is crate-private as a forgery guard they say
/// they do not want opened. Before any shape is chosen, Round 775's rule applies:
/// measure.
///
/// # Why these arms and not Round 782's
///
/// Round 782 weighed 4,612 lines of a few dozen characters each and found the
/// strings were not the memory — the ELEMENTS were, so borrowing removed 99.5% of
/// the allocation calls and only 38% of the bytes. Prose is the opposite shape:
/// 56 spots averaging 2,655 characters. Whether the same conclusion holds when
/// the strings ARE most of the bytes is exactly what has not been measured, and
/// assuming it would carry over is how a measured round becomes a guessed one.
///
/// The fixture mirrors `Passage` field for field — the anchor's `source`, its
/// `Prefix` locator, and the text — because a passage is three strings, one of
/// them long, and a stand-in with one string would price a different type.
fn prose_fixtures(out_dir: &str) {
    let write = |name: &str, source: String| {
        std::fs::write(
            std::path::Path::new(out_dir).join(format!("prose_{name}.rs")),
            source,
        )
        .expect("write a prose fixture");
    };
    for (tag, spots) in [("small", PROSE_SMALL), ("big", PROSE_BIG)] {
        write(&format!("owned_{tag}"), render_owned_prose(spots));
        write(&format!("borrowed_{tag}"), render_borrowed_prose(spots));
        write(&format!("text_only_{tag}"), render_text_only_prose(spots));
    }
    let facts = format!(
        "pub const PROSE_SMALL: usize = {PROSE_SMALL};\n\
         pub const PROSE_BIG: usize = {PROSE_BIG};\n\
         pub const PROSE_CHARS: usize = {PROSE_CHARS};\n"
    );
    std::fs::write(
        std::path::Path::new(out_dir).join("prose_build_facts.rs"),
        facts,
    )
    .expect("write the prose build facts");
}

/// One spot's three literals, already quoted. The text is Korean so a character
/// is three bytes — the first consumer's prose is Korean, and a fixture in ASCII
/// would price a third of the bytes it claims to.
fn prose_literals(i: usize) -> (String, String, String) {
    let sentence = "물때가 셈한다. 그는 자리에 서서 물이 나가는 것을 보았다. ";
    let text: String = sentence.chars().cycle().take(PROSE_CHARS).collect();
    (
        format!("{:?}", format!("manuscript/ch-{i:03}.md")),
        format!("{:?}", format!("d{i:02}-여기서 시작한다")),
        format!("{:?}", text),
    )
}

/// Arm one: the shape `store_passages` returns today — every string owned.
fn render_owned_prose(spots: usize) -> String {
    let ty = "::std::vec::Vec<crate::OwnedPassage>";
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..spots)
        .collect::<Vec<_>>()
        .chunks(CHUNK.get())
        .enumerate()
    {
        let body: Vec<String> = group
            .iter()
            .map(|i| {
                let (source, prefix, text) = prose_literals(*i);
                format!(
                    "crate::OwnedPassage {{ source: {source}.to_string(), \
                     prefix: {prefix}.to_string(), text: {text}.to_string() }}"
                )
            })
            .collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, ty, &names)
}

/// Arm three: the text borrowed, the ANCHOR still owned (Round 792) — the shape
/// a real `Passage` can take without touching `mnemosyne-core`.
///
/// Arm two answers what borrowing EVERYTHING buys, and Round 790 argued from
/// arithmetic that nearly all of it survives borrowing the text alone, since the
/// text is ~99% of the bytes. Arithmetic is what this crate keeps being wrong
/// about, so it gets an arm. The distinction is not academic: `ContentAnchor` and
/// its `Locator` live in `mnemosyne-core` with serde derives and are shared with
/// the store, so the arm that leaves them owned is the one that costs nothing
/// outside the engine.
fn render_text_only_prose(spots: usize) -> String {
    let ty = "::std::vec::Vec<crate::TextBorrowedPassage>";
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..spots)
        .collect::<Vec<_>>()
        .chunks(CHUNK.get())
        .enumerate()
    {
        let body: Vec<String> = group
            .iter()
            .map(|i| {
                let (source, prefix, text) = prose_literals(*i);
                format!(
                    "crate::TextBorrowedPassage {{ source: {source}.to_string(), \
                     prefix: {prefix}.to_string(), text: {text} }}"
                )
            })
            .collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, ty, &names)
}

/// Arm two: the same three fields pointing at the literals instead of copying
/// them. `Vec` assembly is kept identical, so the arms differ in HOLDING only.
fn render_borrowed_prose(spots: usize) -> String {
    let ty = "::std::vec::Vec<crate::BorrowedPassage>";
    let mut fns = String::new();
    let mut names = Vec::new();
    for (c, group) in (0..spots)
        .collect::<Vec<_>>()
        .chunks(CHUNK.get())
        .enumerate()
    {
        let body: Vec<String> = group
            .iter()
            .map(|i| {
                let (source, prefix, text) = prose_literals(*i);
                format!(
                    "crate::BorrowedPassage {{ source: {source}, prefix: {prefix}, text: {text} }}"
                )
            })
            .collect();
        let name = format!("__mn_{c}");
        let _ = writeln!(fns, "fn {name}() -> {ty} {{ vec![{}] }}", body.join(", "));
        names.push(name);
    }
    vec_assembly(&fns, ty, &names)
}
