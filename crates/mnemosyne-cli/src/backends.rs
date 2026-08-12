//! THE SYMBOL-RESOLVER BACKENDS THIS BUILD SHIPS.
//!
//! One table, two readers. `build_symbol_resolver_map` in the binary consults
//! it to turn a `[plugins.symbol_resolver.<lang>]` entry into a live resolver,
//! and `describe-symbol-axis-reach` prints it. Before Round 1151 the same
//! knowledge was an `if / else if` chain inside `main.rs`: reachable only by
//! running the binary with a config that named each branch, so nothing could
//! ask the build what it contained, and the answer had to be read out of this
//! repository's source by anyone who needed it.
//!
//! SCE did read it, and wrote the result into their own tree as prose —
//! "Mnemosyne maps .go to the `go` language but ships no resolver plugin for
//! it". A restatement of another repository's internals has no reader on the
//! side that can invalidate it: the day a resolver lands, nothing they run
//! notices. The table exists so that sentence can be a query instead.
//!
//! EACH BACKEND DECLARES ITS OWN LANGUAGE. The pairing is not the wiring site's
//! to choose — `tree-sitter-cpp` answers in C++'s vocabulary wherever it is
//! registered — so the language id travels with the plugin crate
//! (`SYMBOL_AXIS_LANGUAGE`) and this table only collects. That is what lets the
//! config path refuse a backend registered under a language it does not
//! resolve, which parsed and ran until this round.

use mnemosyne_core::SymbolResolver;

/// One in-process `SymbolResolver` backend compiled into this build.
pub struct InProcessBackend {
    /// The `[plugins.symbol_resolver.<lang>] backend = "…"` value that selects
    /// it.
    pub key: &'static str,
    /// The symbol-axis language it resolves — the only `<lang>` key it may be
    /// registered under.
    pub language: &'static str,
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
/// `tests/symbol_axis_reach.rs` walks this table through the binary and fails
/// on a row it has no two-site fixture for, so a row cannot arrive without a
/// control that says which grammar answered.
pub static IN_PROCESS_BACKENDS: &[InProcessBackend] = &[
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_cpp::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_cpp::SYMBOL_AXIS_LANGUAGE,
        make: || Box::new(mnemosyne_plugin_tree_sitter_cpp::TreesitterCppResolver),
    },
    InProcessBackend {
        key: mnemosyne_plugin_tree_sitter_rust::BACKEND_KEY,
        language: mnemosyne_plugin_tree_sitter_rust::SYMBOL_AXIS_LANGUAGE,
        make: || Box::new(mnemosyne_plugin_tree_sitter_rust::TreesitterRustResolver),
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
