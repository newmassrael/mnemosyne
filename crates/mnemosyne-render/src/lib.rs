//! mnemosyne-render — the default text presentation for the playable projection.
//!
//! The presentation layer that consumes [`SceneView`]s from `mnemosyne-engine`
//! (the presentation-agnostic kernel) and produces displayable text. The kernel
//! supplies MEANING (a [`Line`]'s `mode` / `frame` / `quote` / `count` / …); a
//! [`Theme`] here supplies LOOKS. This is the STYLE override surface: a
//! downstream crate implements its own `Theme` (a terminal-ANSI theme, a
//! per-character-colour theme, a letter-spacing theme) WITHOUT touching the
//! kernel — colour and spacing live here, never in the store or the engine.
//!
//! A renderer can never surface a sentence no store fact backs: a [`Line`] is
//! only obtainable from the engine (its constructor is crate-private there), so
//! a `Theme` styles provenance-bound content and cannot fabricate narrative.

use mnemosyne_engine::{DisclosedPlace, Door, Line, MapProjection, PlayableProjection, SceneView};

/// The style-override surface: how the SEMANTIC axes of a [`Line`] / [`Door`]
/// map to a visual look. Implement it to restyle without touching the kernel;
/// the engine decides meaning, a `Theme` decides looks.
pub trait Theme {
    /// The display string for one narrative line, styled by its axes (`mode` /
    /// `frame` / `quote` / `count` / …).
    fn line(&self, line: &Line) -> String;

    /// The display label for one interactive door.
    fn door(&self, door: &Door) -> String;

    /// The display string for a scene heading (a section title). Default: the
    /// title unchanged.
    fn heading(&self, title: &str) -> String {
        title.to_string()
    }

    /// The display string for WHERE this scene is, and where it leads (Round
    /// 938) — the place a disclosed fact put someone at, plus the roads the
    /// store declares out of it.
    ///
    /// Default: the place, then the exits in declared order. A theme that wants
    /// no place line returns the empty string and the renderer prints nothing,
    /// which is how a reading surface opts out of the axis rather than the
    /// engine deciding for it.
    ///
    /// `exits` is what the DECLARATION allows, not what is passable now: a road
    /// whose guard does not hold is still here, because evaluating a guard is
    /// the consumer's (the kernel's R712 line, kept whole through the surface).
    /// It is also what the READER has been told (Round 946): a road the telling
    /// never disclosed is not among them, so a theme cannot print one.
    ///
    /// A BELIEVED place is named as believed. The kernel carries the frame and
    /// refuses to decide whether a rumour is worth showing; the default theme
    /// refuses to show it as a fact, which is the same refusal one layer up.
    /// Round 945 — before it, a town's false claim that a missing woman took a
    /// boat printed exactly like the room the scene was actually in.
    fn place(&self, place: &DisclosedPlace<'_>, exits: &[&str]) -> String {
        let named = if place.is_belief() {
            format!("{} [{}]", place.place, place.frame)
        } else {
            place.place.clone()
        };
        place_with_exits(&named, exits)
    }
}

/// A place and the roads out of it, ignoring style — the shape every theme
/// shares, so the exits cannot drift between two of them. The belief marking is
/// deliberately NOT here: that is the part a theme is expected to differ on,
/// and it is the axis a reader has to be able to see.
fn place_with_exits(named: &str, exits: &[&str]) -> String {
    if exits.is_empty() {
        return named.to_string();
    }
    format!("{named} -> {}", exits.join(", "))
}

/// The default label for a door, ignoring style — the diegetic text a plain
/// theme shows. `Examine` supplies a default English verb (chrome a localized
/// theme would override); `Fork`/`Ask` are already authored labels.
///
/// THE CATCH-ALL IS REQUIRED HERE AND R1282 LEARNED IT FROM THE COMPILER. It
/// reads like the shape that round's gate is about — every one of `Door`'s three
/// variants is named above it, so the arm cannot be taken — and removing it does
/// not build: `Door` is `#[non_exhaustive]`, so a crate other than the one
/// defining it must carry a wildcard whatever it names. That attribute is a
/// decision already written down, that adding a variant is not to be a compile
/// error for readers, and a gate demanding exhaustiveness over such an enum is
/// demanding the impossible. `unasked-variant` skips them for that reason.
fn door_label(door: &Door) -> String {
    match door {
        Door::Fork { label, .. } => label.clone(),
        Door::Examine { object, .. } => format!("examine {object}"),
        Door::Ask { question, .. } => question.clone(),
        _ => String::new(),
    }
}

/// The zero-styling theme: text as-is. The default look so a store reads
/// immediately with no theme authored.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlainTheme;

impl Theme for PlainTheme {
    fn line(&self, line: &Line) -> String {
        line.text().to_string()
    }

    fn door(&self, door: &Door) -> String {
        door_label(door)
    }
}

/// A demonstration style override: it sets truth apart from hearsay and marks
/// quotes and multiplicity — a representative subset of the kernel's semantic
/// axes made visible with plain-text markers (a real renderer would map every
/// axis to colour / weight instead). The override surface reaches every axis
/// structurally (a `Theme` receives the whole [`Line`]); this demo styles a few.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkerTheme;

impl Theme for MarkerTheme {
    fn line(&self, line: &Line) -> String {
        // A verbatim quote is shown as a quote; otherwise the paraphrase.
        let mut styled = match line.quote() {
            Some(quote) => format!("\"{quote}\""),
            None => line.text().to_string(),
        };
        // Hearsay is set apart from ground truth (the belief/truth axis).
        if line.is_belief() {
            styled = format!("~ {styled}");
        }
        // Asserted multiplicity is annotated.
        if let Some(count) = line.count() {
            styled.push_str(&format!(" x{count}"));
        }
        styled
    }

    fn door(&self, door: &Door) -> String {
        format!("> {}", door_label(door))
    }

    /// The place axis under the SAME belief convention this theme gives a line.
    ///
    /// Round 954. Round 945 made a rumoured place distinguishable from the
    /// world's own, and the default theme names the claiming frame in brackets;
    /// this theme marks hearsay with `~` and inherited that default, so the one
    /// axis a reader most has to see arrived two ways inside one theme. A
    /// consumer starting from this demo would have copied the split.
    ///
    /// The frame NAME is kept rather than dropped to match `line`, and the
    /// asymmetry is deliberate: on the place axis the default established that
    /// WHOSE claim it is belongs on the page, and a demo that quietly lost it
    /// would teach the loss. The `~` is what makes the two axes one convention.
    fn place(&self, place: &DisclosedPlace<'_>, exits: &[&str]) -> String {
        let named = if place.is_belief() {
            format!("~ {} [{}]", place.place, place.frame)
        } else {
            place.place.clone()
        };
        place_with_exits(&named, exits)
    }
}

/// Render one scene to display text: the heading, the disclosed lines, then the
/// numbered interactive doors — each element styled by `theme`. The layout is
/// the renderer's; the per-element look is the theme's (the style override).
#[must_use]
pub fn render_scene(
    scene: &SceneView,
    map: &MapProjection,
    told: &std::collections::HashSet<String>,
    theme: &impl Theme,
) -> String {
    let mut out = String::new();
    if let Some(title) = &scene.title {
        out.push_str(&theme.heading(title));
        out.push('\n');
    }
    // Where the scene is, before what happens in it — a reader orients, then
    // reads. Nothing prints when the telling disclosed no place, which is the
    // common and correct case for a store that declares no map at all: the
    // projection then holds no maps and this loop does not run.
    for here in map.places_disclosed_in(scene) {
        // THE ROADS A READER HAS BEEN TOLD ABOUT, not every road the town has
        // (Round 946). The map is ground truth because the gate needs it whole;
        // printing it here disclosed by the back door what the telling withheld,
        // which is the same leak `places_disclosed_in` refuses one line above.
        let exits: Vec<&str> = here
            .map
            .steps_disclosed_from(&here.place, told)
            .into_iter()
            .map(|(_, to)| to)
            .collect();
        let rendered = theme.place(&here, &exits);
        if !rendered.is_empty() {
            out.push_str(&rendered);
            out.push('\n');
        }
    }
    for line in &scene.lines {
        out.push_str(&theme.line(line));
        out.push('\n');
    }
    for (index, door) in scene.doors.iter().enumerate() {
        out.push_str(&format!("  [{}] {}\n", index + 1, theme.door(door)));
    }
    out
}

/// Render a whole world-line to display text — every scene of its declared walk,
/// in order, styled by `theme`. The "read a store immediately" surface.
#[must_use]
pub fn render_playthrough(
    projection: &PlayableProjection,
    map: &MapProjection,
    world: &str,
    theme: &impl Theme,
) -> String {
    let mut out = String::new();
    // A playthrough rendering is a READING surface: it lays out the whole
    // declared walk at once, so there is no reader partway through it holding
    // anything. The empty set states that rather than leaving it unasked
    // (Round 779), and filters nothing, freshness being a set difference.
    let holds_nothing = std::collections::HashSet::new();
    // What the reader has been TOLD, accumulated as the walk goes (Round 946).
    // This is not the same set as `holds_nothing` above: that one is freshness,
    // which a whole-playthrough layout has no reader partway through to compute.
    // This one is disclosure, and it only ever grows — a road named in scene two
    // is still known in scene twenty. Scenes are added BEFORE the scene renders,
    // so a road disclosed in this very scene is a road this scene may show.
    let mut told: std::collections::HashSet<String> = std::collections::HashSet::new();
    for section in projection.walk(world) {
        let scene = projection.scene(world, section, &holds_nothing);
        told.extend(scene.lines.iter().map(|l| l.fact_id().to_string()));
        out.push_str(&render_scene(&scene, map, &told, theme));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        render_playthrough, render_scene, Door, Line, MapProjection, MarkerTheme, PlainTheme, Theme,
    };

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_engine::{
        ForkPart, Interactivity, LinePart, PlayableProjection, ProjectionParts,
    };

    fn line(
        fact_id: &str,
        text: &str,
        frame: &str,
        quote: Option<&str>,
        count: Option<i64>,
    ) -> LinePart {
        LinePart {
            fact_id: fact_id.to_string().into(),
            text: text.to_string().into(),
            mode: DisclosureMode::State,
            frame: frame.to_string().into(),
            entities: Vec::new().into(),
            carrier: None,
            typed_predicate: None,
            typed_quantity: None,
            quote: quote.map(|q| q.to_string().into()),
            count,
        }
    }

    /// Round 771 — the render fixture is built from [`ProjectionParts`], the
    /// kernel's ONE ingestion door now that `from_report` is crate-private. This
    /// is also a second downstream crate exercising that door: if parts ever
    /// readmit a type a foreign crate cannot construct, this stops compiling.
    fn demo() -> PlayableProjection {
        PlayableProjection::from_parts(ProjectionParts {
            telling: "reader".into(),
            by_world: vec![(
                "main".into(),
                vec![(
                    "sc-01".into(),
                    vec![
                        line("f-truth", "the tide pulls out", "ground-truth", None, None),
                        line(
                            "f-belief",
                            "Bunok guesses a name",
                            "frame-bunok",
                            None,
                            None,
                        ),
                        line(
                            "f-quote",
                            "he said it plainly",
                            "ground-truth",
                            Some("I crossed at two"),
                            Some(3),
                        ),
                        // A DISCLOSED position: the `at` leg the place axis
                        // reads (Round 938). A fixture without one cannot tell a
                        // renderer that shows the place from one that does not.
                        LinePart {
                            typed_predicate: Some("at".to_string().into()),
                            entities: vec!["ent-bunok".into(), "loc-quay".into()].into(),
                            ..line("f-at", "Bunok is at the quay", "ground-truth", None, None)
                        },
                    ],
                )],
            )],
            walks: vec![("main".into(), vec!["sc-01".into()])],
            titles: vec![("sc-01".into(), "Dawn".into())],
            cast: Vec::new(),
            forks: vec![ForkPart {
                at: "sc-01".into(),
                parent: "main".into(),
                world: "flee".into(),
                label: "run".into(),
            }],
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: Vec::new(),
            // This renderer draws the PROSE stream, which is the half a journal
            // policy routes facts out of (Round 787). None here, and that is a
            // statement rather than a stub: a renderer showing a journal-routed
            // fact would be showing what the telling withheld from prose.
            journal_offers: Vec::new(),
        })
    }

    /// A projection whose ONE scene places the same person at the same place
    /// twice: once because the world says so, once because a character does.
    fn demo_with_a_rumour() -> PlayableProjection {
        let placed = |fact_id: &str, frame: &str| LinePart {
            typed_predicate: Some("at".to_string().into()),
            entities: vec!["ent-bunok".into(), "loc-quay".into()].into(),
            ..line(fact_id, "Bunok is at the quay", frame, None, None)
        };
        PlayableProjection::from_parts(ProjectionParts {
            telling: "reader".into(),
            by_world: vec![(
                "main".into(),
                vec![(
                    "sc-01".into(),
                    vec![
                        placed("f-at", "ground-truth"),
                        placed("f-at-said", "frame-bunok"),
                    ],
                )],
            )],
            walks: vec![("main".into(), vec!["sc-01".into()])],
            titles: vec![("sc-01".into(), "Dawn".into())],
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: Vec::new(),
            journal_offers: Vec::new(),
        })
    }

    /// A ROAD THE TELLING NEVER DISCLOSED IS NOT AN EXIT (Round 946).
    ///
    /// Round 943 measured a corpus that withheld all thirteen of its roads and
    /// printed every one of them anyway, byte-identically under the withholding
    /// telling and the open one — the place honoured the telling and the roads
    /// out of it did not.
    ///
    /// The pair is the guard: the SAME scene and the SAME map, rendered once with
    /// the road told and once without. Without the pair, "no exits" would also be
    /// what a renderer that lost the map entirely produces, and the assertion
    /// would be vacuous.
    #[test]
    fn an_undisclosed_road_is_not_offered_as_an_exit() {
        let proj = demo();
        let scene = proj.scene("main", "sc-01", &std::collections::HashSet::new());

        let told = render_scene(&scene, &town(), &every_road_told(), &PlainTheme);
        assert!(
            told.contains("loc-quay -> loc-ford"),
            "the told half must show the road, or the untold half proves nothing:\n{told}"
        );

        let untold = render_scene(
            &scene,
            &town(),
            &std::collections::HashSet::new(),
            &PlainTheme,
        );
        assert!(
            untold.contains("loc-quay"),
            "the PLACE is still disclosed — only the road was withheld:\n{untold}"
        );
        assert!(
            !untold.contains("loc-ford"),
            "a road the reader was never told about is not an exit:\n{untold}"
        );
        assert!(
            !untold.contains("->"),
            "and no empty arrow is left behind:\n{untold}"
        );
    }

    /// The walk accumulates: a road disclosed in an EARLIER scene is still an
    /// exit later. Round 946 — disclosure only ever grows, and a renderer that
    /// asked "was this road named in THIS scene" would blink the town's roads on
    /// and off as the reader moved through it.
    #[test]
    fn a_road_told_once_stays_told_for_the_rest_of_the_walk() {
        let placed = |fact_id: &str, place: &str| LinePart {
            typed_predicate: Some("at".to_string().into()),
            entities: vec!["ent-bunok".into(), place.to_string().into()].into(),
            ..line(fact_id, "Bunok stands there", "ground-truth", None, None)
        };
        let proj = PlayableProjection::from_parts(ProjectionParts {
            telling: "reader".into(),
            by_world: vec![(
                "main".into(),
                vec![
                    // Scene one names the road itself; scene two names nobody's
                    // road at all and must still be able to show it.
                    (
                        "sc-01".into(),
                        vec![
                            line("f-adj", "a lane joins the quay to the ford", "", None, None),
                            placed("f-at-1", "loc-quay"),
                        ],
                    ),
                    ("sc-02".into(), vec![placed("f-at-2", "loc-quay")]),
                ],
            )],
            walks: vec![("main".into(), vec!["sc-01".into(), "sc-02".into()])],
            titles: vec![
                ("sc-01".into(), "One".into()),
                ("sc-02".into(), "Two".into()),
            ],
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: Vec::new(),
            journal_offers: Vec::new(),
        });

        let out = render_playthrough(&proj, &town(), "main", &PlainTheme);
        assert_eq!(
            out.matches("loc-quay -> loc-ford").count(),
            2,
            "the road is an exit in both scenes, not only the one that named it:\n{out}"
        );
    }

    /// THE RENDERED PLACE SAYS WHOSE IT IS (Round 945).
    ///
    /// Round 943 watched a blind author's false rumour print exactly like the
    /// room the scene was in, and the author's sealed report said the engine
    /// could not do that. The kernel now carries the frame; the default theme
    /// declines to show hearsay as fact.
    ///
    /// The two rows at ONE place are the discriminating input. With the frame
    /// dropped anywhere along the path — struct, dedup key, or theme — the two
    /// lines become one line, or two identical ones, and this fails.
    #[test]
    fn a_believed_place_is_rendered_as_believed_and_the_worlds_is_not() {
        let proj = demo_with_a_rumour();
        let out = render_scene(
            &proj.scene("main", "sc-01", &std::collections::HashSet::new()),
            &town(),
            &every_road_told(),
            &PlainTheme,
        );

        assert!(
            out.contains("\nloc-quay -> loc-ford\n"),
            "the world's own place stays unmarked:\n{out}"
        );
        assert!(
            out.contains("\nloc-quay [frame-bunok] -> loc-ford\n"),
            "the believed place names the frame that claimed it:\n{out}"
        );
        assert_eq!(
            out.matches("loc-quay").count(),
            2,
            "two rows, not one collapsed and not one duplicated:\n{out}"
        );
    }

    #[test]
    fn plain_theme_renders_text_and_doors_unstyled() {
        let proj = demo();
        let out = render_scene(
            &proj.scene("main", "sc-01", &std::collections::HashSet::new()),
            &no_map(),
            &every_road_told(),
            &PlainTheme,
        );
        assert!(out.contains("Dawn"));
        assert!(out.contains("the tide pulls out"));
        assert!(out.contains("Bunok guesses a name")); // belief unmarked in plain
        assert!(out.contains("[1] run")); // the fork door label
        assert!(!out.contains('~')); // no styling markers
        assert!(!out.contains(" x3")); // count unshown in plain
    }

    #[test]
    fn marker_theme_styles_by_semantic_axis() {
        let proj = demo();
        let out = render_scene(
            &proj.scene("main", "sc-01", &std::collections::HashSet::new()),
            &no_map(),
            &every_road_told(),
            &MarkerTheme,
        );
        // ground truth stays plain; belief is set apart; quote wrapped; count shown.
        assert!(out.contains("\nthe tide pulls out\n")); // ground truth unmarked
        assert!(out.contains("~ Bunok guesses a name")); // is_belief -> "~ "
        assert!(out.contains("\"I crossed at two\"")); // verbatim quote wrapped
        assert!(out.contains("x3")); // count annotated
        assert!(out.contains("[1] > run")); // door -> "> label"
    }

    /// Every road `town()` declares, disclosed. The tests that assert EXITS are
    /// about exits, not about disclosure, so they hand the reader the road
    /// explicitly rather than inheriting a default (Round 946).
    fn every_road_told() -> std::collections::HashSet<String> {
        ["f-adj".to_string()].into_iter().collect()
    }

    /// A store that declares no transition rule — the inert place axis. Built
    /// through the public bake door, which is the only way a test outside the
    /// kernel can hold a projection at all.
    fn no_map() -> MapProjection {
        MapProjection::from_parts(mnemosyne_engine::MapProjectionParts {
            maps: Vec::new(),
            transition_rules: 0,
            unattached_costs: Vec::new(),
            unattached_guards: Vec::new(),
        })
    }

    /// A one-road town whose `at` predicate matches the demo's placed fact.
    fn town() -> MapProjection {
        MapProjection::from_parts(mnemosyne_engine::MapProjectionParts {
            maps: vec![mnemosyne_engine::DeclaredMapPart {
                rule: "town".into(),
                predicate: "at".into(),
                adjacency: "adjacent".into(),
                undirected: false,
                containment: None,
                nodes: vec!["loc-ford".into(), "loc-quay".into()],
                edges: vec![mnemosyne_engine::MapEdgePart {
                    fact_id: "f-adj".into(),
                    from: "loc-quay".into(),
                    to: "loc-ford".into(),
                    frame: "ground-truth".into(),
                    branch: "main".into(),
                    cost: None,
                    guard: None,
                }],
                self_loops: Vec::new(),
            }],
            transition_rules: 1,
            unattached_costs: Vec::new(),
            unattached_guards: Vec::new(),
        })
    }

    /// The place line reaches the page, above the prose, with its exits — and
    /// the SAME scene under a map-less projection prints none of it. The pair is
    /// the assertion: a bare `contains` would pass on a renderer that printed the
    /// place unconditionally from something else.
    #[test]
    fn a_scene_shows_where_it_is_and_where_it_leads_only_when_a_map_declares_it() {
        let proj = demo();
        let scene = proj.scene("main", "sc-01", &std::collections::HashSet::new());

        let with = render_scene(&scene, &town(), &every_road_told(), &PlainTheme);
        assert!(with.contains("loc-quay -> loc-ford"), "{with}");
        let place_at = with.find("loc-quay").expect("the place line");
        let prose_at = with.find("the tide pulls out").expect("the prose");
        assert!(place_at < prose_at, "a reader orients, then reads:\n{with}");

        let without = render_scene(&scene, &no_map(), &every_road_told(), &PlainTheme);
        assert!(
            !without.contains("loc-quay"),
            "a store with no declared map prints no place:\n{without}"
        );
    }

    /// A theme's OWN place rendering is what reaches the page, and this theme
    /// marks hearsay there the way it marks it everywhere else.
    ///
    /// Until Round 954 the only override this file exercised was the empty one
    /// (`a_theme_can_refuse_the_place_line`), which a renderer that honoured
    /// "empty means skip" and then printed the DEFAULT for every non-empty
    /// answer would satisfy completely. So the discriminator is the pair: the
    /// same scene and the same map under two themes, with the marked form
    /// asserted present in one and absent from the other. A whole-output
    /// `assert_ne!` would not do it — this fixture's second line is belief-
    /// framed too, so the two renderings differ on the LINE axis whether or not
    /// the place override was ever called.
    #[test]
    fn marker_theme_marks_a_believed_place_and_its_override_reaches_the_page() {
        let proj = demo_with_a_rumour();
        let scene = proj.scene("main", "sc-01", &std::collections::HashSet::new());
        let marked = render_scene(&scene, &town(), &every_road_told(), &MarkerTheme);
        let plain = render_scene(&scene, &town(), &every_road_told(), &PlainTheme);

        assert!(
            marked.contains("\n~ loc-quay [frame-bunok] -> loc-ford\n"),
            "the rumoured place carries this theme's own belief marker:\n{marked}"
        );
        assert!(
            marked.contains("\nloc-quay -> loc-ford\n"),
            "the world's own place stays unmarked:\n{marked}"
        );
        assert!(
            !plain.contains("~ loc-quay"),
            "the default theme does not use this theme's marker, so the \
             assertion above is about the override and not about the \
             default:\n{plain}"
        );
    }

    /// A theme opts the axis out by returning the empty string, and the renderer
    /// prints nothing rather than a blank line.
    #[test]
    fn a_theme_can_refuse_the_place_line() {
        struct Placeless;
        impl Theme for Placeless {
            fn line(&self, line: &Line) -> String {
                line.text().to_string()
            }
            fn door(&self, door: &Door) -> String {
                super::door_label(door)
            }
            fn place(&self, _: &mnemosyne_engine::DisclosedPlace<'_>, _: &[&str]) -> String {
                String::new()
            }
        }
        let proj = demo();
        let scene = proj.scene("main", "sc-01", &std::collections::HashSet::new());
        let out = render_scene(&scene, &town(), &every_road_told(), &Placeless);
        assert!(!out.contains("loc-quay"), "{out}");
        assert!(
            out.contains("the tide pulls out"),
            "the prose still renders"
        );

        // The refusal must leave NO trace, and where a trace would be is BETWEEN
        // the heading and the first prose line — not at the start of the output,
        // which is what an earlier draft of this assertion checked and is why an
        // injection that printed the blank line stayed green. Comparing against
        // the same scene with no map at all pins the position too: an empty place
        // string must cost exactly one nothing.
        assert_eq!(
            out,
            render_scene(&scene, &no_map(), &every_road_told(), &Placeless),
            "a refused place line left a blank line behind:\n{out}"
        );
    }

    #[test]
    fn render_playthrough_walks_the_world() {
        let proj = demo();
        let out = render_playthrough(&proj, &no_map(), "main", &PlainTheme);
        assert!(out.contains("Dawn"));
        assert!(out.contains("the tide pulls out"));
    }
}
