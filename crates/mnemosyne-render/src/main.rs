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
