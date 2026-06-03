//! Static concept resources exposed under `mnemosyne://concepts/*`.
//!
//! Each entry is a (uri, name, title, description, body) record. Bodies
//! are embedded at compile time via `include_str!` so the server is a
//! single self-contained binary.

pub struct ConceptResource {
    pub uri: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

pub static RESOURCES: &[ConceptResource] = &[
    ConceptResource {
        uri: "mnemosyne://concepts/overview",
        name: "overview",
        title: "Mnemosyne — Overview for AI Agents",
        description: "What Mnemosyne is, why it exists, and the canonical concept reading order.",
        body: include_str!("../resources/overview.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/atomic-store",
        name: "atomic-store",
        title: "The Atomic Store",
        description: "Shape, location, and read/mutate contract of docs/.atomic/workspace.atomic.json.",
        body: include_str!("../resources/atomic-store.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/frozen-ledger",
        name: "frozen-ledger",
        title: "Frozen Ledger Semantics",
        description: "Append-only invariant on ChangelogEntry bodies + T2 jaccard rule.",
        body: include_str!("../resources/frozen-ledger.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/tier-rules",
        name: "tier-rules",
        title: "Tier Rules — T1 / T2 / T3 / T4",
        description: "The four severity tiers, what they reject, and when each runs.",
        body: include_str!("../resources/tier-rules.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/anti-patterns",
        name: "anti-patterns",
        title: "Anti-Patterns — Things You MUST NOT Do",
        description: "Category violations to refuse: cleanup of frozen entries, direct atomic-store JSON edits, schema bloat.",
        body: include_str!("../resources/anti-patterns.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/schema-guide",
        name: "schema-guide",
        title: "mnemosyne.toml Schema Guide",
        description: "Complete schema for the workspace config file with example presets.",
        body: include_str!("../resources/schema-guide.md"),
    },
    ConceptResource {
        uri: "mnemosyne://concepts/workflow",
        name: "workflow",
        title: "Workflow — How a Typical Session Looks",
        description: "Canonical session pattern: read concepts → validate baseline → mutate via tools → re-validate.",
        body: include_str!("../resources/workflow.md"),
    },
];

pub fn lookup(uri: &str) -> Option<&'static ConceptResource> {
    RESOURCES.iter().find(|r| r.uri == uri)
}
