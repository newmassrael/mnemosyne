//! THE SYMBOL-RESOLVER BACKENDS THIS BUILD SHIPS, and the one way a workspace's
//! `[plugins.symbol_resolver.<lang>]` becomes a live resolver map.
//!
//! # Why this is a crate rather than a module of the CLI
//!
//! Until Round 1335 the table and the wiring lived in `mnemosyne-cli`: the table
//! in its library, `build_symbol_resolver_map` in its binary. That was right
//! while one surface ran the citation gate. Round 1333 made the gate a library
//! call so a second surface could ask for the same answer, and the second
//! surface is `mnemosyne-mcp` — which cannot see the CLI's binary at all, and
//! should not depend on a crate of arg parsing and stdout handlers to learn
//! which grammars this build contains.
//!
//! A surface that shipped no backends would not be silently worse: the gate
//! reports the symbol axis as `not judged — no resolver`, BY NAME, which is the
//! distinction Round 1141 paid for. It would be worse in the way that matters
//! anyway — an agent that fixes everything its tool reports and is then refused
//! by the hook on an axis its tool never judged. So both binaries ship the same
//! table and answer alike.
//!
//! # Why not `mnemosyne-ops`
//!
//! Three crates depend on that one, and `mnemosyne-engine` consults no resolver.
//! Putting the grammars there would make it carry five parse tables it never
//! reads. MEASURED before the move: the five grammar rlibs are about 13.5 MB in
//! a debug build, the marginal COMPILE cost is zero for a workspace build
//! (the same build already produces them for the CLI), and `mnemosyne-mcp`
//! WITHOUT them was 67.5 MB against `mnemosyne-cli` WITH them at 57.1 MB — the
//! surface shipping none was already the larger binary. The cost is real and it
//! is not where the guess put it.
//!
//! # Each backend declares its own language
//!
//! The pairing is not the wiring site's to choose — `tree-sitter-cpp` answers in
//! C++'s vocabulary wherever it is registered — so the language id travels with
//! the plugin crate (`SYMBOL_AXIS_LANGUAGE`) and this table only collects. That
//! is what lets [`resolver_map`] refuse a backend registered under a language it
//! does not resolve, which parsed and ran until Round 1151.

use std::collections::BTreeMap;

use mnemosyne_config::{SymbolResolverConfig, WorkspaceConfig};
use mnemosyne_core::SymbolResolver;
use mnemosyne_plugin_tree_sitter_core::LanguageSpec;

/// One in-process `SymbolResolver` backend compiled into this build.
pub struct InProcessBackend {
    /// The `[plugins.symbol_resolver.<lang>] backend = "…"` value that selects
    /// it.
    pub key: &'static str,
    /// The symbol-axis language it resolves — the only `<lang>` key it may be
    /// registered under.
    pub language: &'static str,
    /// What this backend answers WITH — the grammar's declaration query and the
    /// doc-comment rule.
    ///
    /// EVERY BACKEND THIS BUILD SHIPS IS A TREE-SITTER BACKEND, so this is a
    /// field and not an `Option`: a nullable one would let a row arrive with no
    /// answer and publish that absence as a fact, which is the reading Round
    /// 1141 spent a round separating from "measured and empty". A backend of
    /// some other shape is a change to this type, made when there is one.
    pub spec: &'static LanguageSpec,
    make: fn() -> Box<dyn SymbolResolver>,
}

impl InProcessBackend {
    /// Instantiate the resolver. Backends are unit structs, so this is a
    /// pointer to a constructor rather than a stored instance: the table stays
    /// `static` and nothing is built for a backend no config names.
    #[must_use]
    pub fn make(&self) -> Box<dyn SymbolResolver> {
        (self.make)()
    }
}

/// Every in-process backend, in `key` order.
///
/// Adding a language is one row here plus its plugin crate. The contract in
/// `mnemosyne-cli`'s `tests/symbol_axis_reach.rs` walks this table through the
/// binary and fails on a row it has no two-site fixture for, so a row cannot
/// arrive without a control that says which grammar answered.
pub static IN_PROCESS_BACKENDS: &[InProcessBackend] = &[
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_cpp::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_cpp::SYMBOL_AXIS_LANGUAGE,
        spec: &mnemosyne_plugin_tree_sitter_cpp::SPEC,
        make: || Box::new(mnemosyne_plugin_tree_sitter_cpp::resolver()),
    },
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_go::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_go::SYMBOL_AXIS_LANGUAGE,
        spec: &mnemosyne_plugin_tree_sitter_go::SPEC,
        make: || Box::new(mnemosyne_plugin_tree_sitter_go::resolver()),
    },
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_kotlin::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_kotlin::SYMBOL_AXIS_LANGUAGE,
        spec: &mnemosyne_plugin_tree_sitter_kotlin::SPEC,
        make: || Box::new(mnemosyne_plugin_tree_sitter_kotlin::resolver()),
    },
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_python::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_python::SYMBOL_AXIS_LANGUAGE,
        spec: &mnemosyne_plugin_tree_sitter_python::SPEC,
        make: || Box::new(mnemosyne_plugin_tree_sitter_python::resolver()),
    },
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_rust::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_rust::SYMBOL_AXIS_LANGUAGE,
        spec: &mnemosyne_plugin_tree_sitter_rust::SPEC,
        make: || Box::new(mnemosyne_plugin_tree_sitter_rust::resolver()),
    },
];

/// The backend a `backend = "…"` value selects, or `None` when this build has
/// no plugin for it.
#[must_use]
pub fn find(key: &str) -> Option<&'static InProcessBackend> {
    IN_PROCESS_BACKENDS.iter().find(|b| b.key == key)
}

/// Every backend key this build accepts, for a refusal to name. Derived from
/// the table so the message cannot fall behind what the table holds — the
/// unknown-backend refusal used to say only that the plugin was absent, which
/// leaves a consumer with a typo and no list to compare it against.
#[must_use]
pub fn keys() -> Vec<&'static str> {
    IN_PROCESS_BACKENDS.iter().map(|b| b.key).collect()
}

/// The symbol-axis languages some shipped backend resolves.
#[must_use]
pub fn languages() -> std::collections::BTreeSet<&'static str> {
    IN_PROCESS_BACKENDS.iter().map(|b| b.language).collect()
}

/// Build the live resolver map a workspace's config declares.
///
/// THE ONE PLACE A CONFIG BECOMES RESOLVERS, so that every surface running the
/// citation gate refuses the same declarations and answers the symbol axis
/// alike. A second surface deriving its own map would be free to accept a
/// language this one refuses, and then two runs over one tree would disagree
/// about whether an axis was even judged.
///
/// Only `transport = "in-process"` lands a production backend. `mcp` and `cli`
/// parse and register placeholders that surface `NotImplemented` at resolve
/// time.
///
/// # Errors
///
/// A `lang` key no file extension maps to, an in-process `backend` name this
/// build has no plugin for, or a backend registered under a language it does not
/// resolve. All three are config errors rather than warnings (Rounds 855 and
/// 1151): each one leaves `severity_binding = reject` reading as symbol-level
/// enforcement while the run performs none, or performs it in the wrong
/// grammar and publishes the result as a judged count.
pub fn resolver_map(
    cfg: &WorkspaceConfig,
) -> anyhow::Result<BTreeMap<String, Box<dyn SymbolResolver>>> {
    let mut out: BTreeMap<String, Box<dyn SymbolResolver>> = BTreeMap::new();
    let Some(plugins) = cfg.plugins.as_ref() else {
        return Ok(out);
    };
    let known_langs = mnemosyne_validate::code_refs::symbol_axis_languages();
    for (lang, resolver_cfg) in &plugins.symbol_resolver {
        if !known_langs.contains(lang.as_str()) {
            anyhow::bail!(
                "[plugins.symbol_resolver.{lang}] names a language no file extension maps to, \
                 so nothing would ever consult it — the keys this build can reach are {known_langs:?}"
            );
        }
        match resolver_cfg {
            SymbolResolverConfig::InProcess { backend } => {
                let Some(entry) = find(backend) else {
                    anyhow::bail!(
                        "[plugins.symbol_resolver.{lang}] names in-process backend `{backend}`, \
                         which this build has no plugin for — the symbol axis would silently \
                         fall back to file-level binding for {lang}. This build ships {:?} \
                         (`mnemosyne-cli describe-symbol-axis-reach` prints them with the \
                         languages they resolve)",
                        keys()
                    );
                };
                if entry.language != lang.as_str() {
                    anyhow::bail!(
                        "[plugins.symbol_resolver.{lang}] names in-process backend \
                         `{backend}`, which resolves `{}` and not `{lang}` — it would answer \
                         in {}'s vocabulary for every {lang} file, and the axis would publish \
                         that as a judged count rather than say it had no instrument",
                        entry.language,
                        entry.language
                    );
                }
                out.insert(lang.clone(), entry.make());
            }
            SymbolResolverConfig::Mcp { command } => {
                out.insert(
                    lang.clone(),
                    Box::new(mnemosyne_core::McpResolver {
                        command: command.clone(),
                    }),
                );
            }
            SymbolResolverConfig::Cli {
                command,
                output_parser,
            } => {
                out.insert(
                    lang.clone(),
                    Box::new(mnemosyne_core::CliResolver {
                        command: command.clone(),
                        output_parser: output_parser.clone(),
                    }),
                );
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key names one backend. Two rows sharing a key would make `find`
    /// silently pick the first, and the report would print both.
    #[test]
    fn every_key_is_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for backend in IN_PROCESS_BACKENDS {
            assert!(
                seen.insert(backend.key),
                "duplicate backend key `{}`",
                backend.key
            );
        }
        assert_eq!(seen.len(), IN_PROCESS_BACKENDS.len());
    }

    /// A language is resolved by one backend. Two would make the config's
    /// choice between them invisible: both satisfy the language check, and
    /// which one runs depends on the order of a table nobody reads as ordered.
    #[test]
    fn no_language_has_two_backends() {
        let mut seen = std::collections::BTreeSet::new();
        for backend in IN_PROCESS_BACKENDS {
            assert!(
                seen.insert(backend.language),
                "language `{}` has more than one backend",
                backend.language
            );
        }
    }

    /// `find` answers with the row whose key was asked for, and with nothing
    /// for a key no row holds.
    #[test]
    fn find_selects_by_key() {
        for backend in IN_PROCESS_BACKENDS {
            let found = find(backend.key).expect("a listed key resolves");
            assert_eq!(found.key, backend.key);
            assert_eq!(found.language, backend.language);
        }
        assert!(find("tree-sitter-nothing").is_none());
    }

    /// The constructor pointer builds the backend it is filed under. A row
    /// whose `make` returned another crate's resolver would pass every test
    /// above — the version surface is the one thing the resolver itself says.
    #[test]
    fn each_row_builds_the_plugin_it_names() {
        for backend in IN_PROCESS_BACKENDS {
            let surface = backend.make().version_surface();
            assert!(
                surface.plugin_name.contains(backend.language),
                "backend `{}` is filed under `{}` but builds `{}`",
                backend.key,
                backend.language,
                surface.plugin_name
            );
        }
    }
}
