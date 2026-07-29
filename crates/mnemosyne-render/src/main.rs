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
    DefaultOverrides, EngineOverrides, PlayableProjection, StaticOverrides, MAIN_BRANCH,
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
    Ok(render_playthrough(&projection, MAIN_BRANCH, &PlainTheme))
}

fn main() -> ExitCode {
    // Round 827 — this binary opens a workspace (its first argument is one), so
    // it owes the tool pin like every other. It was MISSED by Round 826 because
    // the test enumerated the binaries by hand; fail-closed meant it refused
    // every pinned workspace instead of doing something unsafe, which is the
    // design working and still a bug for this binary.
    mnemosyne_config::register_tool_stamp(env!("BUILD_GIT_HASH"));
    let args: Vec<String> = std::env::args().collect();
    let result = match args.as_slice() {
        [_, workspace, telling] => run(workspace, telling, None),
        [_, workspace, telling, overrides] => run(workspace, telling, Some(overrides)),
        _ => {
            eprintln!("usage: mnemosyne-render <workspace> <telling> [overrides.json]");
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
