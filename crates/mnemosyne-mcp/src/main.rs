//! mnemosyne-mcp — Model Context Protocol server for Mnemosyne.
//!
//! Exposes the production design-doc lifecycle CLI as MCP tools, plus a
//! curated set of concept resources under `mnemosyne://concepts/*` so
//! AI clients can internalize Mnemosyne's semantics before mutating.
//!
//! Transport: stdio. Configure your MCP client with:
//!
//! ```jsonc
//! {
//! "mcpServers": {
//! "mnemosyne": {
//! "command": "mnemosyne-mcp",
//! "args": ["--workspace", "."]
//! }
//! }
//! }
//! ```

mod cli;
mod resources;

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        Annotated, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, RawResource, ResourceContents, ServerCapabilities, ServerInfo,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QuerySectionArgs {
 /// Section ID without the leading `§` (e.g. `"39"`, `"39.1"`,
 /// `"changelog"`). Pass `--list-sections` form via `list_sections`
 /// instead.
    pub section_id: String,
 /// Include 1-hop CrossRef neighborhood (outbound + inbound).
    #[serde(default)]
    pub include_related: bool,
 /// Include §N citations from changelog entries.
    #[serde(default)]
    pub include_changelog: bool,
}

// Round 292 — query_term read primitive (literal/regex search across the
// atomic store). Pure read; preview substrate for the deferred redact_term
// mutate primitive but useful standalone for verifying a term's footprint
// before mutating.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryTermArgs {
 /// Pattern to search. Literal by default; set `regex = true` to
 /// interpret as a regex (`regex` crate syntax).
    pub pattern: String,
 /// Interpret `pattern` as a regex. Default = literal substring.
    #[serde(default)]
    pub regex: bool,
 /// Case-insensitive match. Default = case-sensitive.
    #[serde(default)]
    pub case_insensitive: bool,
 /// Scope. One of `"all"` (default), `"sections"`, `"changelog"`,
 /// `"inventory"`.
    #[serde(default)]
    pub scope: Option<String>,
 /// Optional field-name whitelist. When non-empty, only hits in the
 /// listed fields are returned. Use base field names: `"intent"`,
 /// `"rationale_bullets"`, `"decision_summary"`,
 /// `"changes_bullets"`, `"alternatives_rejected"`, `"examples"`,
 /// `"implementations"`, `"source"`, `"reason"`, etc.
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StyleCheckArgs {
 /// Optional doc path relative to workspace root. Omit to check
 /// every doc listed in `mnemosyne.toml`.
    #[serde(default)]
    pub doc: Option<String>,
 /// Severity filter — `"t3"`, `"t4"`, or `"all"` (default).
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionTextArgs {
 /// Section ID to mutate. Pass `"39"`, not `""`.
    pub section_id: String,
 /// New value. For intent: a single sentence, max ~200 chars.
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionBulletsArgs {
    pub section_id: String,
 /// Ordered list of bullets. Each ≤ 100 chars per T3 default.
    pub bullets: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionCaveatArgs {
    pub section_id: String,
 /// Single caveat bullet to append.
    pub bullet: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetImpactScopeArgs {
    pub section_id: String,
 /// Cross-ref targets without the `§` prefix, e.g. `["39", "61.1"]`.
    pub refs: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionExampleArgs {
    pub section_id: String,
 /// Code-fence language tag (e.g. `"rust"`, `"toml"`).
    pub language: String,
 /// Code body — embedded inside a fenced block. No leading fence.
    pub code: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionImplementationArgs {
 /// Section ID without the `§` prefix.
    pub section_id: String,
 /// Workspace-relative POSIX file path. No leading `/`, no leading
 /// `./`, no `..` segment, no backslash. The file does not need to
 /// exist at write time — schema records intent.
    pub file: String,
 /// Optional opaque language-agnostic identifier (function / type /
 /// qualified path). Stored as-is; no language-grammar regex applied.
 /// Omit for file-level binding.
    #[serde(default)]
    pub symbol: Option<String>,
}

// Round 287/289 — Section creation + outline setter MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionArgs {
 /// Section ID to create. No `§` prefix in the value; use the bare slug
 /// or numbered id (e.g. `"39"`, `"39.1"`, `"my-section"`).
    pub section_id: String,
 /// Owning doc identifier (workspace-relative path or doc id).
    pub parent_doc: String,
 /// Heading title (non-empty).
    pub title: String,
 /// Optional parent section id. Omit for top-level; pass a bare id
 /// (no `§`) to nest under an existing section. The parent must exist
 /// in the atomic store at write time.
    #[serde(default)]
    pub parent_section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionParentSectionArgs {
 /// Section being re-parented.
    pub section_id: String,
 /// New parent. Pass `Some("<id>")` to nest under that section, or
 /// `None` (omit) to promote to top-level. Self-loop rejected.
    #[serde(default)]
    pub parent_section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSectionImplementationArgs {
 /// Section ID without the `§` prefix.
    pub section_id: String,
 /// Workspace-relative POSIX file path to remove from the binding set.
    pub file: String,
 /// Optional symbol — must exact-match the row to remove. Omit to
 /// target a file-only binding (a row with `symbol = None`).
    #[serde(default)]
    pub symbol: Option<String>,
 /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

// Round 278 — Phase 1A inventory MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InventoryIdArgs {
 /// Inventory id (e.g. `"ARP_07"`, `"TCP_RETRANSMISSION_TO_04"`).
    pub inventory_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddInventoryEntryArgs {
 /// Stable inventory id. Must be non-empty, no whitespace.
    pub inventory_id: String,
 /// Lifecycle status: `"active"` / `"deprecated"` / `"reserved"`.
    pub status: String,
 /// Optional section binding without leading `§` (e.g. `"4.2.4"`).
    #[serde(default)]
    pub section_ref: Option<String>,
 /// Optional traceability pointer (PDF page ref, JSON row id, etc.).
    #[serde(default)]
    pub source: Option<String>,
 /// Optional rationale (typically used when status starts as
 /// `"deprecated"` — explains the deprecation cause).
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetInventoryStatusArgs {
    pub inventory_id: String,
 /// New status: `"active"` / `"deprecated"` / `"reserved"`.
    pub status: String,
 /// Optional reason. Omit to preserve existing; empty string clears.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetInventorySectionRefArgs {
    pub inventory_id: String,
 /// New section_ref without `§`. Omit (or pass `null`) AND set
 /// `clear: true` to unset the binding.
    #[serde(default)]
    pub section_ref: Option<String>,
 /// Set to `true` to explicitly unset the section_ref. Exactly one
 /// of `section_ref` or `clear` must be present.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveInventoryEntryArgs {
    pub inventory_id: String,
 /// Mandatory rationale recorded in the receipt (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AppendChangelogEntryArgs {
 /// Entry id matching `[schema] entry_id_prefix`. Must be strictly
 /// monotonic (greater than the last entry's id).
    pub entry_id: String,
 /// One-sentence headline of the decision.
    pub decision_summary: String,
 /// What concretely changed. File paths, primitives, etc.
    pub changes_bullets: Vec<String>,
 /// How the change was validated (tests, measurements).
    pub verification_bullets: Vec<String>,
 /// Section ids affected (without `§`), e.g. `["39", "66"]`.
    #[serde(default)]
    pub impact_refs: Vec<String>,
 /// Carry-forward items for next round.
    #[serde(default)]
    pub carry_forward_bullets: Vec<String>,
}

#[derive(Clone)]
pub struct MnemosyneServer {
    workspace: Arc<PathBuf>,
    #[allow(dead_code)] // populated by #[tool_router] expansion
    tool_router: ToolRouter<Self>,
}

impl MnemosyneServer {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace: Arc::new(workspace),
            tool_router: Self::tool_router(),
        }
    }

    fn tool_text(s: String) -> rmcp::model::CallToolResult {
        rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(s)])
    }

    fn tool_error(s: String) -> rmcp::model::CallToolResult {
        rmcp::model::CallToolResult::error(vec![rmcp::model::Content::text(s)])
    }

    async fn run_cli(&self, args: &[&str]) -> rmcp::model::CallToolResult {
        match cli::run(&self.workspace, args).await {
            Ok(out) if out.ok() => Self::tool_text(out.combined()),
            Ok(out) => Self::tool_error(format!(
                "exit={} workspace={}\n{}",
                out.status,
                self.workspace.display(),
                out.combined()
            )),
            Err(e) => Self::tool_error(format!("subprocess error: {}", e)),
        }
    }

    async fn run_cli_with_files(
        &self,
        args_template: Vec<String>,
        temp_files: Vec<PathBuf>,
    ) -> rmcp::model::CallToolResult {
        let args: Vec<&str> = args_template.iter().map(|s| s.as_str()).collect();
        let result = self.run_cli(&args).await;
        for path in &temp_files {
            let _ = std::fs::remove_file(path);
        }
        result
    }
}

#[tool_router]
impl MnemosyneServer {
    #[tool(
        description = "Run T1 (cross-ref orphan) + T2 (frozen ledger) + round-trip validation across the entire workspace. Returns the metric summary (orphan total / round-trip mandatory / T3 warn / T4 info). Run this at session start to surface the baseline, and after every mutation to confirm no new violations."
    )]
    async fn validate_workspace(
        &self,
        _args: Parameters<EmptyArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["validate-workspace"]).await
    }

    #[tool(
        description = "List every section_id in the workspace (one per line, BTreeMap order). Use this to discover the section topology before authoring §N references."
    )]
    async fn list_sections(
        &self,
        _args: Parameters<EmptyArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["query", "--list-sections"]).await
    }

    #[tool(
        description = "Look up a single section. Returns the SectionView (atomic fields rendered as JSON). Optionally include 1-hop CrossRef neighborhood and §N citations from changelog entries. Always call this BEFORE mutating a section to verify decision_status and avoid editing strong-carry / Superseded sections."
    )]
    async fn query_section(
        &self,
        args: Parameters<QuerySectionArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec!["query".to_string(), format!("§{}", args.0.section_id), "--json".to_string()];
        if args.0.include_related {
            argv.push("--include-related".to_string());
        }
        if args.0.include_changelog {
            argv.push("--include-changelog".to_string());
        }
        let argv_ref: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        self.run_cli(&argv_ref).await
    }

    #[tool(
        description = "Round 292 — literal/regex search across atomic Section + ChangelogEntry + Inventory text fields. Returns hits as JSON: target_kind (section|changelog_entry|inventory), target_id, field_path (e.g. `rationale_bullets[2]`, `alternatives_rejected[0].reason`), line_context (full field/bullet text). Pure read. Use this before redact_term (deferred) or before mutating prose, to know which entries cite a term."
    )]
    async fn query_term(
        &self,
        args: Parameters<QueryTermArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "query".to_string(),
            "--term".to_string(),
            args.0.pattern.clone(),
            "--json".to_string(),
        ];
        if args.0.regex {
            argv.push("--regex".to_string());
        }
        if args.0.case_insensitive {
            argv.push("--case-insensitive".to_string());
        }
        if let Some(scope) = &args.0.scope {
            argv.push("--scope".to_string());
            argv.push(scope.clone());
        }
        if !args.0.fields.is_empty() {
            argv.push("--field".to_string());
            argv.push(args.0.fields.join(","));
        }
        let argv_ref: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        self.run_cli(&argv_ref).await
    }

    #[tool(
        description = "Render the atomic store to docs/GENERATED.md (template render → atomic write temp + rename). Cascade auto-update normally invokes this after every successful mutate primitive; call directly only when you need to force-refresh after a manual JSON edit (which you should not do)."
    )]
    async fn generate_docs(
        &self,
        _args: Parameters<EmptyArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["generate-docs"]).await
    }

    #[tool(
        description = "Verify that docs/GENERATED.md byte-equals what would be rendered fresh from the atomic store. Exit code 0 = synced, 1 = stale. Wire into pre-commit hooks to catch drift."
    )]
    async fn verify_generated(
        &self,
        _args: Parameters<EmptyArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["verify-generated"]).await
    }

    #[tool(
        description = "Run T3/T4 style checks. T3 = warning surface (max_paragraph_length, sentence length, terminology); T4 = info. Reject power is configurable; default = warn-only so existing prose stays valid on day 1."
    )]
    async fn style_check(
        &self,
        args: Parameters<StyleCheckArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec!["style-check".to_string()];
        if let Some(doc) = &args.0.doc {
            argv.push("--doc".to_string());
            argv.push(doc.clone());
        }
        if let Some(sev) = &args.0.severity {
            argv.push("--severity".to_string());
            argv.push(sev.clone());
        }
        let argv_ref: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        self.run_cli(&argv_ref).await
    }

    #[tool(
        description = "Create a new Section in the atomic store (Round 287/289). Outline fields only — `section_id` (no `§` prefix), `parent_doc`, `title`, and optional `parent_section`. Content fields (intent, rationale, etc.) populate via subsequent set_section_* / add_section_* calls. Rejects duplicate `section_id` and missing `parent_section`. Pairs with remove_section."
    )]
    async fn add_section(
        &self,
        args: Parameters<AddSectionArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "add-section".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--parent-doc".to_string(),
            args.0.parent_doc.clone(),
            "--title".to_string(),
            args.0.title.clone(),
        ];
        if let Some(parent) = &args.0.parent_section {
            argv.push("--parent".to_string());
            argv.push(format!("§{}", parent));
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Set Section.title (heading text). Section must exist (use add_section to create first)."
    )]
    async fn set_section_title(
        &self,
        args: Parameters<SetSectionTextArgs>,
    ) -> rmcp::model::CallToolResult {
        let argv = vec![
            "set-section-title".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--title".to_string(),
            args.0.text.clone(),
        ];
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Set Section.parent_doc (re-bind section to a different owning doc). Section must exist."
    )]
    async fn set_section_parent_doc(
        &self,
        args: Parameters<SetSectionTextArgs>,
    ) -> rmcp::model::CallToolResult {
        let argv = vec![
            "set-section-parent-doc".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--parent-doc".to_string(),
            args.0.text.clone(),
        ];
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Set Section.parent_section (re-parent in hierarchy). Pass `parent_section: Some(\"<id>\")` to nest under another section, or omit / pass null to promote to top-level. Self-loop rejected; missing parent rejected."
    )]
    async fn set_section_parent_section(
        &self,
        args: Parameters<SetSectionParentSectionArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "set-section-parent-section".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
        ];
        match &args.0.parent_section {
            Some(p) => {
                argv.push("--parent".to_string());
                argv.push(format!("§{}", p));
            }
            None => argv.push("--no-parent".to_string()),
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Set Section.intent atomic field. The intent is a one-sentence statement of what the section is for. Replaces any previous intent. T1+T2 run pre-write."
    )]
    async fn set_section_intent(
        &self,
        args: Parameters<SetSectionTextArgs>,
    ) -> rmcp::model::CallToolResult {
        let argv = vec![
            "set-section-intent".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--intent".to_string(),
            args.0.text.clone(),
        ];
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Set Section.rationale_bullets. Replaces existing. Each bullet ≤ 100 chars (T3 default)."
    )]
    async fn set_section_rationale(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> rmcp::model::CallToolResult {
        self.set_section_bullets("set-section-rationale", &args.0).await
    }

    #[tool(description = "Set Section.inputs_bullets. Replaces existing.")]
    async fn set_section_inputs(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> rmcp::model::CallToolResult {
        self.set_section_bullets("set-section-inputs", &args.0).await
    }

    #[tool(description = "Set Section.outputs_bullets. Replaces existing.")]
    async fn set_section_outputs(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> rmcp::model::CallToolResult {
        self.set_section_bullets("set-section-outputs", &args.0).await
    }

    #[tool(
        description = "Append a single caveat bullet to Section.caveats_bullets. Append-only — does not replace existing caveats."
    )]
    async fn add_section_caveat(
        &self,
        args: Parameters<AddSectionCaveatArgs>,
    ) -> rmcp::model::CallToolResult {
        let argv = vec![
            "add-section-caveat".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--bullet".to_string(),
            args.0.bullet.clone(),
        ];
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(description = "Set Section.alternatives_rejected. Replaces existing.")]
    async fn set_section_alternatives(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> rmcp::model::CallToolResult {
        let payload = args.0.bullets.join("\n");
        let path = match cli::write_temp(&self.workspace, "alternatives", &payload) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write: {}", e)),
        };
        let argv = vec![
            "set-section-alternatives".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--alternatives-file".to_string(),
            path.to_string_lossy().into_owned(),
        ];
        self.run_cli_with_files(argv, vec![path]).await
    }

    #[tool(
        description = "Set Section.impact_scope. Each ref is a section_id without the `§` prefix; T1 cross-ref orphan reject runs pre-write so non-existent §N targets fail cleanly."
    )]
    async fn set_section_impact_scope(
        &self,
        args: Parameters<SetImpactScopeArgs>,
    ) -> rmcp::model::CallToolResult {
        let refs_arg: String = args
            .0
            .refs
            .iter()
            .map(|r| format!("§{}", r))
            .collect::<Vec<_>>()
            .join(",");
        let argv = vec![
            "set-section-impact-scope".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--refs".to_string(),
            refs_arg,
        ];
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Append a code-fenced example to Section.examples. The code block is rendered with the supplied language tag."
    )]
    async fn add_section_example(
        &self,
        args: Parameters<AddSectionExampleArgs>,
    ) -> rmcp::model::CallToolResult {
        let path = match cli::write_temp(&self.workspace, "example", &args.0.code) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write: {}", e)),
        };
        let argv = vec![
            "add-section-example".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--language".to_string(),
            args.0.language.clone(),
            "--code-file".to_string(),
            path.to_string_lossy().into_owned(),
        ];
        self.run_cli_with_files(argv, vec![path]).await
    }

    #[tool(
        description = "Round 259 — Path B (Spec ↔ Code bidirectional binding) substrate. Append a (file, symbol?) implementation binding to Section.implementations. file = workspace-relative POSIX path (no leading `/`, no `..`, no `\\`); symbol = optional opaque identifier (function/type/qualified path; language-agnostic, no grammar regex). Set semantics — duplicate (file, symbol) rejected at write time. The schema records intent; file existence is not checked here (Round 260+ cross-checks code citations against this set)."
    )]
    async fn add_section_implementation(
        &self,
        args: Parameters<AddSectionImplementationArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "add-section-implementation".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--file".to_string(),
            args.0.file.clone(),
        ];
        if let Some(symbol) = &args.0.symbol {
            argv.push("--symbol".to_string());
            argv.push(symbol.clone());
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Round 283 — remove one `(file, symbol?)` binding from `Section.implementations`. Exact set-element match: pass `symbol` to target a symbol-narrowed row, omit it to target a file-only row. NotFound when the section or the binding is absent (no silent no-op). `reason` mandatory — recorded on the receipt for audit symmetry with remove-section / remove-inventory-entry. Use when code refactor (or citation hygiene cleanup) leaves stale bindings: validate-code-refs surfaces them as impl_unbacked, and this primitive is the typed-API cleanup path (don't edit the sidecar JSON directly)."
    )]
    async fn remove_section_implementation(
        &self,
        args: Parameters<RemoveSectionImplementationArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "remove-section-implementation".to_string(),
            "--section".to_string(),
            format!("§{}", args.0.section_id),
            "--file".to_string(),
            args.0.file.clone(),
            "--reason".to_string(),
            args.0.reason.clone(),
        ];
        if let Some(symbol) = &args.0.symbol {
            argv.push("--symbol".to_string());
            argv.push(symbol.clone());
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Append a new ChangelogEntry to the atomic store. entry_id must be strictly monotonic (greater than the last entry's id under the configured schema.entry_id_prefix). All five atomic fields are required for proper audit shape."
    )]
    async fn append_changelog_entry_v2(
        &self,
        args: Parameters<AppendChangelogEntryArgs>,
    ) -> rmcp::model::CallToolResult {
        let changes = args.0.changes_bullets.join("\n");
        let verify = args.0.verification_bullets.join("\n");
        let carry = args.0.carry_forward_bullets.join("\n");

        let changes_path = match cli::write_temp(&self.workspace, "changes", &changes) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write changes: {}", e)),
        };
        let verify_path = match cli::write_temp(&self.workspace, "verify", &verify) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write verify: {}", e)),
        };
        let carry_path = match cli::write_temp(&self.workspace, "carry", &carry) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write carry: {}", e)),
        };

        let impact_arg: String = args
            .0
            .impact_refs
            .iter()
            .map(|r| format!("§{}", r))
            .collect::<Vec<_>>()
            .join(",");

        let mut argv = vec![
            "append-changelog-entry-v2".to_string(),
            "--entry-id".to_string(),
            args.0.entry_id.clone(),
            "--decision".to_string(),
            args.0.decision_summary.clone(),
            "--changes-file".to_string(),
            changes_path.to_string_lossy().into_owned(),
            "--verification-file".to_string(),
            verify_path.to_string_lossy().into_owned(),
            "--carry-file".to_string(),
            carry_path.to_string_lossy().into_owned(),
        ];
        if !impact_arg.is_empty() {
            argv.push("--impact".to_string());
            argv.push(impact_arg);
        }

        self.run_cli_with_files(argv, vec![changes_path, verify_path, carry_path])
            .await
    }

    // Round 278 — Phase 1A inventory tool surface.

    #[tool(
        description = "List every inventory entry in the atomic store (id, status, section_ref). Phase 1A 5th-entity surface (Round 273). Returns one entry per line by default — pass nothing, the CLI walks AtomicStore.inventory_entries in BTreeMap order."
    )]
    async fn list_inventory(
        &self,
        _args: Parameters<EmptyArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["query", "--list-inventory", "--json"]).await
    }

    #[tool(
        description = "Look up a single inventory entry (status / section_ref / source / reason). Phase 1A 5th-entity (Round 273). Call this BEFORE writing an inventory citation in code to verify status (Deprecated → don't cite; cite-time reject is the validator's job, but author-time check is cheap)."
    )]
    async fn query_inventory(
        &self,
        args: Parameters<InventoryIdArgs>,
    ) -> rmcp::model::CallToolResult {
        self.run_cli(&["query", "--inventory", &args.0.inventory_id, "--json"])
            .await
    }

    #[tool(
        description = "Register a new inventory entry (Phase 1A, Round 274). Duplicate inventory_id rejects. status = active|deprecated|reserved. Pass status=deprecated to register an already-retired upstream id; the mutate-time cascade (Round 276) then surfaces any pre-existing cite-sites. section_ref omits the leading §."
    )]
    async fn add_inventory_entry(
        &self,
        args: Parameters<AddInventoryEntryArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "add-inventory-entry".to_string(),
            "--id".to_string(),
            args.0.inventory_id.clone(),
            "--status".to_string(),
            args.0.status.clone(),
        ];
        if let Some(s) = &args.0.section_ref {
            argv.push("--section".to_string());
            argv.push(format!("§{}", s));
        }
        if let Some(s) = &args.0.source {
            argv.push("--source".to_string());
            argv.push(s.clone());
        }
        if let Some(s) = &args.0.reason {
            argv.push("--reason".to_string());
            argv.push(s.clone());
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Update an inventory entry's status (Round 274). Returns NotFound if the id is not registered. reason: omit to preserve existing; pass empty string to clear; pass non-empty to overwrite. Active→Deprecated transitions invoke the cascade scan (Round 276)."
    )]
    async fn set_inventory_status(
        &self,
        args: Parameters<SetInventoryStatusArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "set-inventory-status".to_string(),
            "--id".to_string(),
            args.0.inventory_id.clone(),
            "--status".to_string(),
            args.0.status.clone(),
        ];
        if let Some(s) = &args.0.reason {
            argv.push("--reason".to_string());
            argv.push(s.clone());
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Update an inventory entry's section_ref binding (Round 274). Exactly one of section_ref or clear must be supplied. section_ref omits the leading §. NotFound on unregistered ids."
    )]
    async fn set_inventory_section_ref(
        &self,
        args: Parameters<SetInventorySectionRefArgs>,
    ) -> rmcp::model::CallToolResult {
        let mut argv = vec![
            "set-inventory-section-ref".to_string(),
            "--id".to_string(),
            args.0.inventory_id.clone(),
        ];
        match (&args.0.section_ref, args.0.clear) {
            (Some(s), false) => {
                argv.push("--section".to_string());
                argv.push(format!("§{}", s));
            }
            (None, true) => {
                argv.push("--clear".to_string());
            }
            _ => {
                return Self::tool_error(
                    "exactly one of section_ref or clear must be supplied".to_string(),
                );
            }
        }
        self.run_cli_with_files(argv, vec![]).await
    }

    #[tool(
        description = "Remove an inventory entry (Round 274). reason is mandatory (audit safeguard recorded in the receipt). Triggers the cascade scan (Round 276) so any pre-existing cite-sites surface mutate-time as `removed` cascade lines."
    )]
    async fn remove_inventory_entry(
        &self,
        args: Parameters<RemoveInventoryEntryArgs>,
    ) -> rmcp::model::CallToolResult {
        let argv = vec![
            "remove-inventory-entry".to_string(),
            "--id".to_string(),
            args.0.inventory_id.clone(),
            "--reason".to_string(),
            args.0.reason.clone(),
        ];
        self.run_cli_with_files(argv, vec![]).await
    }
}

impl MnemosyneServer {
    async fn set_section_bullets(
        &self,
        cmd: &str,
        args: &SetSectionBulletsArgs,
    ) -> rmcp::model::CallToolResult {
        let payload = args.bullets.join("\n");
        let path = match cli::write_temp(&self.workspace, cmd, &payload) {
            Ok(p) => p,
            Err(e) => return Self::tool_error(format!("temp write: {}", e)),
        };
        let argv = vec![
            cmd.to_string(),
            "--section".to_string(),
            format!("§{}", args.section_id),
            "--bullets-file".to_string(),
            path.to_string_lossy().into_owned(),
        ];
        self.run_cli_with_files(argv, vec![path]).await
    }
}

#[tool_handler]
impl ServerHandler for MnemosyneServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(concat!(
            "Mnemosyne MCP server. Read mnemosyne://concepts/overview first, ",
            "then anti-patterns + atomic-store + frozen-ledger before any mutation. ",
            "Run validate_workspace to surface the baseline, mutate via typed primitives, ",
            "validate_workspace again to confirm no new T1/T2 violations. ",
            "NEVER edit docs/GENERATED.md or the atomic JSON directly."
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = resources::RESOURCES
            .iter()
            .map(|r| {
                let raw = RawResource::new(r.uri, r.name)
                    .with_title(r.title)
                    .with_description(r.description)
                    .with_mime_type("text/markdown");
                Annotated::new(raw, None)
            })
            .collect();
        let mut result = ListResourcesResult::default();
        result.resources = resources;
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match resources::lookup(&request.uri) {
            Some(r) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                r.body, r.uri,
            )
            .with_mime_type("text/markdown")])),
            None => Err(McpError::resource_not_found(
                "unknown resource uri",
                Some(serde_json::json!({"uri": request.uri})),
            )),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let workspace = parse_workspace_arg()?;
    if !workspace.exists() {
        anyhow::bail!("workspace path does not exist: {}", workspace.display());
    }

    let server = MnemosyneServer::new(workspace);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn parse_workspace_arg() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let mut workspace: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workspace" | "-w" => {
                workspace = Some(PathBuf::from(args.next().ok_or_else(|| {
                    anyhow::anyhow!("--workspace requires a path argument")
                })?));
            }
            "--help" | "-h" => {
                eprintln!(
                    "mnemosyne-mcp {} ({}) — MCP server for Mnemosyne\n\n\
                     usage: mnemosyne-mcp [--workspace <path>]\n\n\
                     Communicates over stdio. Set MNEMOSYNE_CLI to override the\n\
                     mnemosyne-cli binary path (default: looked up on PATH).\n\
                     If --workspace is omitted, the current directory is used.",
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_GIT_HASH"),
                );
                std::process::exit(0);
            }
            "--version" | "-V" => {
                // Round 286 — universal CLI surface. Mirror mnemosyne-cli
                // format. stdout (not stderr) so wrapper scripts can pipe.
                println!(
                    "mnemosyne-mcp {} ({})",
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_GIT_HASH")
                );
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown argument: {}", other);
            }
        }
    }
    Ok(workspace.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))))
}
