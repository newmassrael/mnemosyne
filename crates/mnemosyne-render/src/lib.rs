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
    for section in projection.walk(world) {
        out.push_str(&render_scene(&projection.scene(world, section), theme));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{render_playthrough, render_scene, MarkerTheme, PlainTheme};

    use std::collections::BTreeMap;

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_engine::{DefaultOverrides, PlayableProjection};
    use mnemosyne_validate::continuity::{
        ForkTreeBranch, ForkTreeEdge, ForkTreeReport, ManuscriptFactEvent, ManuscriptScene,
        MapLocator, PlayableWorld, PlayableWorldReport, WorldManuscript,
    };

    fn begin(
        fact_id: &str,
        claim: &str,
        frame: &str,
        quote: Option<&str>,
        count: Option<i64>,
    ) -> ManuscriptFactEvent {
        ManuscriptFactEvent {
            fact_id: fact_id.into(),
            frame: frame.into(),
            claim: claim.into(),
            entities: Vec::new(),
            canon_from: "sc-01".into(),
            canon_to: None,
            evidence: Vec::new(),
            typed: None,
            quote: quote.map(str::to_string),
            count,
            disclosure: None,
        }
    }

    fn locator(fact_id: &str) -> MapLocator {
        MapLocator {
            world_line: "main".into(),
            fact_id: fact_id.into(),
            scene: "sc-01".into(),
            scene_ordinal: None,
            object: None,
            mode: DisclosureMode::State,
            first_at: None,
        }
    }

    fn demo() -> PlayableProjection {
        let scene = ManuscriptScene {
            section: "sc-01".into(),
            title: "Dawn".into(),
            epub_locator: None,
            begins: vec![
                begin("f-truth", "the tide pulls out", "ground-truth", None, None),
                begin(
                    "f-belief",
                    "Bunok guesses a name",
                    "frame-bunok",
                    None,
                    None,
                ),
                begin(
                    "f-quote",
                    "he said it plainly",
                    "ground-truth",
                    Some("I crossed at two"),
                    Some(3),
                ),
            ],
            ends: Vec::new(),
            holding_count: 0,
            scene_cast: Vec::new(),
        };
        let mut worlds = BTreeMap::new();
        worlds.insert(
            "main".to_string(),
            PlayableWorld {
                manuscript: WorldManuscript {
                    scenes: vec![scene],
                    ..Default::default()
                },
                locators: vec![locator("f-truth"), locator("f-belief"), locator("f-quote")],
            },
        );
        let fork_tree = ForkTreeReport {
            branches: vec![ForkTreeBranch {
                branch_id: "flee".into(),
                description: "run".into(),
                fork: Some(ForkTreeEdge {
                    parent: "main".into(),
                    at: "sc-01".into(),
                    at_placed: true,
                }),
                converges: Vec::new(),
            }],
            ..Default::default()
        };
        let report = PlayableWorldReport {
            telling: "reader".into(),
            fork_tree,
            worlds,
        };
        PlayableProjection::from_report(report, &DefaultOverrides::default()).unwrap()
    }

    #[test]
    fn plain_theme_renders_text_and_doors_unstyled() {
        let proj = demo();
        let out = render_scene(&proj.scene("main", "sc-01"), &PlainTheme);
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
        let out = render_scene(&proj.scene("main", "sc-01"), &MarkerTheme);
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
