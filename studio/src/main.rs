//! `mnemosyne-studio` — the Layer-3 pinion native viewer (ARCHITECTURE.md §3).
//!
//! First screen: the **changelog timeline**. Reads the whole ledger via
//! [`mnemosyne_query::list_changelog`] (round-number order) and renders it as a
//! windowed virtual list (the pinion `view_virtual_list` substrate), so a large
//! ledger paints only the visible window.
//!
//! Native path: this binary links the GUI-free core crates
//! (`mnemosyne-query` / `mnemosyne-atomic`) over the JSON `AtomicStore`
//! in-process — no daemon, no network. The store path is argv[1] (default
//! `docs/.atomic/workspace.atomic.json`, i.e. run from the repo root).
//!
//! The view is the hello-virtual-list shape adapted to real data: the dataset
//! is the loaded changelog (`changelog()`), `N = changelog().len()`, and
//! `build_row` reads the entry at each visible index. Scroll via wheel or the
//! scrollbar peer; only the viewport window + overscan ever materialize.

use std::sync::OnceLock;

use mnemosyne_query::ChangelogEntryView;

use pinion_a11y::{windowed_list_nodes, AccessNode, WidgetA11y};
use pinion_core::external::{External, StubExternal};
use pinion_core::scene::{ContainerNode, Rect, TextNode};
use pinion_core::style::{
    AlignItems, BoxStyle, FlexDirection, JustifyContent, LayoutStyle, Size, TextStyle,
};
use pinion_core::theme::{use_theme, ColorRole, Theme};
use pinion_core::widget_core::ExtraExternal;
use pinion_core::widgets::scroll::use_scroll_state;
use pinion_core::widgets::scrollbar::{scrollbar_extra_external, use_scrollbar_interaction};
use pinion_core::widgets::virtual_list::compute_visible_range;
use pinion_core::{Frame, Scene, WidgetCore};
use pinion_shell::{vello_renderer_impl, WidgetView};
use pinion_widget_paint::scrollbar::{view_vertical_scrollbar, VerticalScrollbarStyle};
use pinion_widget_paint::virtual_list::view_virtual_list;

include!(concat!(env!("OUT_DIR"), "/app.rs"));
vello_renderer_impl!(StudioRenderer, StudioRendererError);

const WIN_W: u32 = 720;
const WIN_H: u32 = 560;
/// Shared `ThemeProvider` cache key (the `"app"` convention from the pinion
/// catalogue).
const THEME_TAG: &str = "app";
/// Uniform per-row vertical slot (logical px). Uniform pitch → exact integer
/// windowing math (variable-height rows are a later round).
const ROW_PITCH: u32 = 28;
/// Extra rows above + below the strict window so a fast wheel-flick never
/// exposes a blank gap.
const OVERSCAN: usize = 3;
const VIEWPORT_W: u32 = 680;
/// Viewport height — 18 rows tall.
const VIEWPORT_H: u32 = 18 * ROW_PITCH;
/// Paint-root + a11y `list` container tag (also the `StubExternal` anchor tag).
const LIST_TAG: &str = "changelog";
/// Cache key for the scroll container's reactive `ScrollState`.
const SCROLL_KEY: &str = "changelog_scroll";
/// Paint + state tag for the interactive scrollbar peer.
const SCROLLBAR_TAG: &str = "changelog_scrollbar";

/// The whole changelog ledger, loaded once at startup (round-number order).
/// The view fn reads it by index; it never mutates after `main` sets it.
///
/// v1 read-only choice: a `OnceLock` is correct while the Studio only VIEWS.
/// The Phase-2 editor (a mutation must re-project the timeline) will replace
/// this with a reactive `Signal` — do NOT mistake the `OnceLock` for the
/// final state model; it does not extend to writes.
static CHANGELOG: OnceLock<Vec<ChangelogEntryView>> = OnceLock::new();

fn changelog() -> &'static [ChangelogEntryView] {
    CHANGELOG.get().map(Vec::as_slice).unwrap_or(&[])
}

/// One timeline row: a zebra-striped strip carrying the entry id
/// (`Round <n> — …`). Tagged `"changelog#<i>"` so the a11y `listitem`
/// bounds + name attach to the row at that absolute index.
fn build_row(index: usize, theme: &Theme) -> Scene {
    let fill = if index.is_multiple_of(2) {
        theme.resolve(ColorRole::SurfaceContainerLow)
    } else {
        theme.resolve(ColorRole::SurfaceContainer)
    };
    let text = changelog()
        .get(index)
        .map(|e| e.entry_id.clone())
        .unwrap_or_default();
    let label = Scene::Text(TextNode::styled(
        text,
        Rect::default(),
        TextStyle::new()
            .with_size_px(13)
            .with_fg(theme.resolve(ColorRole::OnSurface)),
    ));
    Scene::Container(
        ContainerNode::new(vec![label])
            .with_tag(format!("{LIST_TAG}#{index}"))
            .with_style(BoxStyle::filled(fill))
            .with_layout(
                LayoutStyle::new()
                    .flex(FlexDirection::Row)
                    .with_align_items(AlignItems::Center)
                    .with_size(Size::px(VIEWPORT_W, ROW_PITCH))
                    .with_padding(Rect::new(12, 0, 12, 0)),
            ),
    )
}

/// view-fn (§6.3): pure sync `() -> Scene`. The dataset is virtual —
/// `view_virtual_list` invokes [`build_row`] only for the indices in the
/// current scroll window.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn view(_state: (), _frame: &Frame) -> Scene {
    let n = changelog().len();
    let scroll_state = use_scroll_state(SCROLL_KEY);
    let theme = use_theme(THEME_TAG).theme_animated();

    let list = view_virtual_list(
        &scroll_state,
        Rect::new(0, 0, VIEWPORT_W, VIEWPORT_H),
        n,
        ROW_PITCH,
        OVERSCAN,
        |index| build_row(index, &theme),
    );

    let scrollbar_style = VerticalScrollbarStyle::material(VIEWPORT_H, SCROLLBAR_TAG);
    let scrollbar_interaction = use_scrollbar_interaction(SCROLLBAR_TAG);
    let scrollbar_visual = view_vertical_scrollbar(
        &scroll_state,
        &theme,
        &scrollbar_style,
        scrollbar_interaction.get(),
    );

    let list_root = Scene::Container(
        ContainerNode::new(vec![list, scrollbar_visual])
            .with_tag(LIST_TAG)
            .with_layout(LayoutStyle::new().flex(FlexDirection::Row)),
    );

    Scene::Container(
        ContainerNode::new(vec![list_root])
            .with_style(BoxStyle::filled(theme.resolve(ColorRole::Surface)))
            .with_layout(
                LayoutStyle::new()
                    .flex(FlexDirection::Column)
                    .with_justify(JustifyContent::Center)
                    .with_align_items(AlignItems::Center),
            ),
    )
}

struct StudioView;

impl WidgetCore for StudioView {
    type State = ();
    type Event = ();

    fn create_external() -> Box<dyn External> {
        Box::new(StubExternal::new())
    }

    fn create_extra_externals() -> Vec<ExtraExternal> {
        vec![scrollbar_extra_external(
            use_scroll_state(SCROLL_KEY),
            SCROLLBAR_TAG,
        )]
    }

    fn tag() -> &'static str {
        LIST_TAG
    }

    fn read_state(_scene: &Scene) {}

    fn view(state: (), frame: &Frame) -> Scene {
        view(state, frame)
    }

    fn event_name(_event: ()) -> &'static str {
        "__internal__"
    }

    fn focusable_tags() -> Vec<&'static str> {
        Vec::new()
    }

    fn title() -> &'static str {
        "Mnemosyne Studio — changelog timeline"
    }

    fn fmt_state_log(_state: &()) -> String {
        "display-only (no widget state)".to_string()
    }
}

impl WidgetA11y for StudioView {
    fn access_node(_state: &(), _focused: Option<&str>) -> Vec<AccessNode> {
        let n = changelog().len();
        let scroll_state = use_scroll_state(SCROLL_KEY);
        let window =
            compute_visible_range(scroll_state.offset_y(), VIEWPORT_H, n, ROW_PITCH, OVERSCAN);
        windowed_list_nodes(
            LIST_TAG,
            "Changelog timeline",
            u32::try_from(n).unwrap_or(u32::MAX),
            &window,
        )
    }
}

impl WidgetView for StudioView {
    type Renderer = StudioRenderer;

    fn initial_size_strategy() -> pinion_shell::SizeStrategy {
        pinion_shell::SizeStrategy::Fixed {
            width: WIN_W,
            height: WIN_H,
        }
    }
}

/// Load the atomic store at `path` and populate the changelog timeline once.
fn load_changelog(path: &str) {
    match mnemosyne_atomic::AtomicStore::load(std::path::Path::new(path)) {
        Ok(store) => {
            let _ = CHANGELOG.set(mnemosyne_query::list_changelog(&store));
        }
        Err(e) => {
            eprintln!("mnemosyne-studio: failed to load atomic store at {path}: {e}");
            std::process::exit(1);
        }
    }
}

/// Headless render of the first screen to a PNG via pinion's offscreen
/// wgpu + vello path (`HeadlessScreenshot`). No display required — this is
/// the same scene the live window paints, proving the GUI rasterizes (not
/// just that the view fn builds a `Scene`). Exits 2 if no wgpu adapter is
/// available (run on a display instead).
fn screenshot(out_path: &str, store_path: &str) {
    use pinion_core::Owner;
    use pinion_runtime::compute_layout;
    use pinion_runtime::image_cache::ImageCache;
    use pinion_runtime::paint_adapter::{root_background, to_vello_cached, FragmentCache};
    use pinion_text::LayoutCache;
    use vello::Scene as VelloScene;

    load_changelog(store_path);

    let mut scene = Owner::new().run(|| view((), &Frame::default()));
    let mut text_cache = LayoutCache::new();
    compute_layout(&mut scene, &mut text_cache, WIN_W, WIN_H);

    let base = root_background(&scene);
    let mut frag = FragmentCache::new();
    let mut image_cache = ImageCache::new();
    let mut vello = VelloScene::new();
    to_vello_cached(
        &scene,
        &|_| None,
        &mut text_cache,
        &mut image_cache,
        &mut frag,
        &mut vello,
    );

    let mut shot = match pinion_shell::HeadlessScreenshot::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mnemosyne-studio: no headless wgpu adapter ({e}); run on a display");
            std::process::exit(2);
        }
    };
    let file = match std::fs::File::create(out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("mnemosyne-studio: cannot create {out_path}: {e}");
            std::process::exit(1);
        }
    };
    match shot.render_to_png(&vello, WIN_W, WIN_H, base, std::io::BufWriter::new(file)) {
        Ok(()) => println!(
            "mnemosyne-studio: wrote {out_path} ({WIN_W}x{WIN_H}, {} entries)",
            changelog().len()
        ),
        Err(e) => {
            eprintln!("mnemosyne-studio: render_to_png failed: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next();
    match cmd.as_deref() {
        Some("screenshot") => {
            let out = args
                .next()
                .unwrap_or_else(|| "studio-timeline.png".to_string());
            let store = args
                .next()
                .unwrap_or_else(|| "docs/.atomic/workspace.atomic.json".to_string());
            screenshot(&out, &store);
        }
        other => {
            let store = other
                .map(str::to_string)
                .unwrap_or_else(|| "docs/.atomic/workspace.atomic.json".to_string());
            load_changelog(&store);
            pinion_shell::run::<StudioView>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinion_core::Owner;

    /// Count `changelog#<i>` row containers anywhere in the scene.
    fn count_row_tags(scene: &Scene) -> usize {
        fn walk(scene: &Scene, n: &mut usize) {
            match scene {
                Scene::Container(c) => {
                    if c.tag.as_deref().is_some_and(|t| t.starts_with("changelog#")) {
                        *n += 1;
                    }
                    for child in &c.children {
                        walk(child, n);
                    }
                }
                Scene::Scroll(s) => walk(s.content.as_ref(), n),
                _ => {}
            }
        }
        let mut n = 0;
        walk(scene, &mut n);
        n
    }

    /// Headless seam proof: a real ledger (AtomicStore + `list_changelog`)
    /// flows through `view` and renders a *windowed* subset of rows, not the
    /// whole ledger. No display needed — the view fn is pure `() -> Scene`.
    #[test]
    fn view_windows_the_changelog_not_the_whole_ledger() {
        let mut store = mnemosyne_atomic::AtomicStore::default();
        for i in 1..=40 {
            store.changelog_entries.insert(
                format!("Round {i} — entry {i}"),
                mnemosyne_atomic::AtomicChangelogEntry::default(),
            );
        }
        let _ = CHANGELOG.set(mnemosyne_query::list_changelog(&store));
        assert_eq!(changelog().len(), 40);

        let scene = Owner::new().run(|| view((), &Frame::default()));
        let rendered = count_row_tags(&scene);
        assert!(
            rendered >= 18,
            "must cover the 18-row viewport, got {rendered}"
        );
        assert!(
            rendered < 40,
            "must window the ledger, not render all 40: got {rendered}"
        );
    }
}
