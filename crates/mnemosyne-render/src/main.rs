//! `mnemosyne-render` — read a store as text (the default-engine "immediately
//! readable" driver). Projects the playable world under a telling and renders the
//! main-trunk playthrough to stdout. An optional third argument is a
//! `StaticOverrides` JSON (ladders / objects / journal policy).
//!
//! ```text
//! mnemosyne-render <workspace> <telling> [overrides.json]
//! ```

use std::path::Path;
use std::process::ExitCode;

use mnemosyne_engine::{
    DefaultOverrides, EngineOverrides, MapProjection, PlayableProjection, StaticOverrides,
    MAIN_BRANCH,
};
use mnemosyne_render::{render_playthrough, PlainTheme};

fn project(
    workspace: &str,
    telling: &str,
    overrides: &impl EngineOverrides,
) -> Result<PlayableProjection, String> {
    PlayableProjection::from_workspace(Path::new(workspace), telling, overrides)
        .map_err(|e| e.to_string())
}

fn run(workspace: &str, telling: &str, overrides_path: Option<&str>) -> Result<String, String> {
    let projection = match overrides_path {
        Some(path) => {
            let overrides = StaticOverrides::load(Path::new(path)).map_err(|e| e.to_string())?;
            project(workspace, telling, &overrides)?
        }
        None => project(workspace, telling, &DefaultOverrides::default())?,
    };
    // The place axis reads the same workspace. A store declaring no transition
    // rule yields a projection with no maps, and the renderer then prints no
    // place line — the inert case working, not the axis missing.
    let map =
        MapProjection::from_workspace(Path::new(workspace), None).map_err(|e| format!("{e:?}"))?;
    Ok(render_playthrough(
        &projection,
        &map,
        MAIN_BRANCH,
        &PlainTheme,
    ))
}

fn main() -> ExitCode {
    // Round 827 — this binary opens a workspace (its first argument is one), so
    // it owes the tool pin like every other. It was MISSED by Round 826 because
    // the test enumerated the binaries by hand; fail-closed meant it refused
    // every pinned workspace instead of doing something unsafe, which is the
    // design working and still a bug for this binary.
    mnemosyne_config::register_tool_stamp(env!("BUILD_GIT_HASH"));
    let args: Vec<String> = std::env::args().collect();
    // Round 861 — `--version` was declared a universal surface in Round 286 and
    // this binary never got one, which stopped being cosmetic when the tool pin
    // began ASKING a binary which revision it is before handing over to it. The
    // question has to be answerable by every shipped binary, including one too
    // old to open a config; here it was answerable by none.
    if matches!(args.get(1).map(String::as_str), Some("--version" | "-V")) {
        println!(
            "mnemosyne-render {} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("BUILD_GIT_HASH")
        );
        return ExitCode::SUCCESS;
    }
    let result = match args.as_slice() {
        [_, workspace, telling] => run(workspace, telling, None),
        [_, workspace, telling, overrides] => run(workspace, telling, Some(overrides)),
        _ => {
            eprintln!(
                "usage: mnemosyne-render <workspace> <telling> [overrides.json]\n       \
                 mnemosyne-render --version"
            );
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("mnemosyne-render: {err}");
            ExitCode::FAILURE
        }
    }
}
