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
                    "mnemosyne-mcp — MCP server for Mnemosyne\n\n\
                     usage: mnemosyne-mcp [--workspace <path>]\n\n\
                     Communicates over stdio. Set MNEMOSYNE_CLI to override the\n\
                     mnemosyne-cli binary path (default: looked up on PATH).\n\
                     If --workspace is omitted, the current directory is used."
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
