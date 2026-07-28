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

use mnemosyne_engine::{Door, Line, PlayableProjection, SceneView};

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
}

/// The default label for a door, ignoring style — the diegetic text a plain
/// theme shows. `Examine` supplies a default English verb (chrome a localized
/// theme would override); `Fork`/`Ask` are already authored labels.
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
}

/// Render one scene to display text: the heading, the disclosed lines, then the
/// numbered interactive doors — each element styled by `theme`. The layout is
/// the renderer's; the per-element look is the theme's (the style override).
#[must_use]
pub fn render_scene(scene: &SceneView, theme: &impl Theme) -> String {
    let mut out = String::new();
    if let Some(title) = &scene.title {
        out.push_str(&theme.heading(title));
        out.push('\n');
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
    world: &str,
    theme: &impl Theme,
) -> String {
    let mut out = String::new();
    // A playthrough rendering is a READING surface: it lays out the whole
    // declared walk at once, so there is no reader partway through it holding
    // anything. The empty set states that rather than leaving it unasked
    // (Round 779), and filters nothing, freshness being a set difference.
    let holds_nothing = std::collections::HashSet::new();
    for section in projection.walk(world) {
        out.push_str(&render_scene(
            &projection.scene(world, section, &holds_nothing),
            theme,
        ));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{render_playthrough, render_scene, MarkerTheme, PlainTheme};

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

    #[test]
    fn plain_theme_renders_text_and_doors_unstyled() {
        let proj = demo();
        let out = render_scene(
            &proj.scene("main", "sc-01", &std::collections::HashSet::new()),
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
            &MarkerTheme,
        );
        // ground truth stays plain; belief is set apart; quote wrapped; count shown.
        assert!(out.contains("\nthe tide pulls out\n")); // ground truth unmarked
        assert!(out.contains("~ Bunok guesses a name")); // is_belief -> "~ "
        assert!(out.contains("\"I crossed at two\"")); // verbatim quote wrapped
        assert!(out.contains("x3")); // count annotated
        assert!(out.contains("[1] > run")); // door -> "> label"
    }

    #[test]
    fn render_playthrough_walks_the_world() {
        let proj = demo();
        let out = render_playthrough(&proj, "main", &PlainTheme);
        assert!(out.contains("Dawn"));
        assert!(out.contains("the tide pulls out"));
    }
}
