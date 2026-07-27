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
                        fact_id: "f-a".to_string(),
                        text: nasty.to_string(),
                        mode: DisclosureMode::Hint,
                        frame: "ground-truth".to_string(),
                        entities: vec!["ent-a".to_string()],
                        carrier: Some("ent-ledger".to_string()),
                        typed_predicate: Some("did".to_string()),
                        quote: Some(nasty.to_string()),
                        count: Some(3),
                    },
                    LinePart {
                        fact_id: "f-b".to_string(),
                        text: "plain".to_string(),
                        mode: DisclosureMode::State,
                        frame: String::new(),
                        entities: Vec::new(),
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
                entity: "ent-jongdeuk".to_string(),
                modality: Modality::Observed,
                can_answer: true,
                quote: nasty.to_string(),
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
    };

    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR"))
        .join("fixture_playable.rs");
    std::fs::write(&out, render(&parts)).expect("write the generated fixture");
}
