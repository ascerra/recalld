//! clap derive structs for the Recalld CLI.
//!
//! Defines the top-level [`Cli`] parser, all subcommand argument structs,
//! and the [`OutputFormat`] / [`ExportFormat`] enums.

use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

/// Recalld — AI memory with human-like forgetting.
///
/// A CLI for managing memories stored in a Recalld server.
/// Output is JSON by default (for LLM tool-use). Use --format human
/// for colored tables.
#[derive(Debug, Parser)]
#[command(name = "recalld", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Output format. Overrides config file default.
    #[arg(long, short = 'F', global = true, value_enum)]
    pub format: Option<OutputFormat>,

    /// API server URL. Overrides config file and $RECALLD_URL.
    #[arg(long, global = true, env = "RECALLD_URL")]
    pub server: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// Output format for CLI responses.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// JSON output (default) — machine-readable for LLM tool-use
    Json,
    /// Human-readable tables with colors — for debugging
    Human,
}

/// Top-level CLI commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Explicit offline vector re-embedding into a fresh destination (no daemon).
    ReembedVectors(ReembedVectorsArgs),
    /// Explicit offline inherited-policy preview/adoption (no daemon).
    Policy(PolicyArgs),
    /// Store a new memory
    Store(StoreArgs),

    /// Search memories by natural language query
    Recall(RecallArgs),

    /// Get a specific memory by ID
    Get(GetArgs),

    /// Delete a memory (permanent)
    Forget(ForgetArgs),

    /// Manually reinforce a memory (increase stability)
    Reinforce(ReinforceArgs),

    /// Full debug view of a memory (stability, phase, connections, access history)
    Inspect(InspectArgs),

    /// Namespace management (list, create, stats)
    Namespaces(NamespacesArgs),

    /// Trigger a manual decay sweep
    Sweep(SweepArgs),

    /// System health: counts per phase, cache stats, uptime
    Status,

    /// Bulk export memories
    Export(ExportArgs),

    /// List all memories with filtering, sorting, and pagination
    List(ListArgs),

    /// Bulk import memories from file
    Import(ImportArgs),

    /// Comprehensive health report: decay forecast, at-risk memories, storage breakdown
    Health(HealthArgs),
}

#[derive(Debug, Args)]
pub struct ReembedVectorsArgs {
    #[arg(long)]
    pub source: std::path::PathBuf,
    #[arg(long)]
    pub destination: std::path::PathBuf,
    /// JSON ReembedVectors request with original namespace/profile and full CAS.
    #[arg(long)]
    pub request: std::path::PathBuf,
    /// Explicit JSON provider configuration and document/query prefixes.
    #[arg(long)]
    pub provider: std::path::PathBuf,
}

// ── Store ──────────────────────────────────────────────────────────

/// Arguments for the `store` command.
#[derive(Debug, Args)]
pub struct StoreArgs {
    /// The memory content to store. Used as both summary and full_text.
    /// If the text exceeds 2,000 bytes, it becomes the full_text and
    /// the server generates a summary via the embedding provider.
    pub text: String,

    /// Tags to attach (comma-separated).
    /// Example: --tags "topic/rust,project/recalld"
    #[arg(long, short, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Target namespace. Defaults to the config file's default_namespace.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Parent memory ID — creates a parent-child edge from this
    /// memory to the new one.
    #[arg(long)]
    pub parent_id: Option<Uuid>,
}

// ── Recall ─────────────────────────────────────────────────────────

/// Arguments for the `recall` (search) command.
#[derive(Debug, Args)]
pub struct RecallArgs {
    /// Natural language search query. Embedded by the server and used
    /// for similarity search.
    pub query: String,

    /// Maximum number of results. Default: 10.
    #[arg(long, short, default_value_t = 10)]
    pub limit: u32,

    /// Restrict search to a specific namespace.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Include ghost memories (phase 3) in results.
    /// By default, ghosts are excluded because they lack summary text.
    #[arg(long)]
    pub include_ghosts: bool,

    /// Filter results by tag (comma-separated). Results must have ALL
    /// specified tags.
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Number of graph hops to include related memories. Default: 0.
    #[arg(long, default_value_t = 0)]
    pub depth: u32,

    /// Minimum strength threshold (0.0-1.0). Results below this are excluded.
    #[arg(long)]
    pub min_strength: Option<f32>,
}

// ── Get ────────────────────────────────────────────────────────────

/// Arguments for the `get` command.
#[derive(Debug, Args)]
pub struct GetArgs {
    /// Memory UUID to retrieve.
    pub id: Uuid,
}

// ── Forget ─────────────────────────────────────────────────────────

/// Arguments for the `forget` (delete) command.
#[derive(Debug, Args)]
pub struct ForgetArgs {
    /// Memory UUID to delete.
    pub id: Uuid,

    /// Skip confirmation prompt (for scripting).
    #[arg(long, short = 'y')]
    pub yes: bool,
}

// ── Reinforce ──────────────────────────────────────────────────────

/// Arguments for the `reinforce` command.
#[derive(Debug, Args)]
pub struct ReinforceArgs {
    /// Memory UUID to reinforce.
    pub id: Uuid,
}

// ── Inspect ────────────────────────────────────────────────────────

/// Arguments for the `inspect` command.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Memory UUID to inspect.
    pub id: Uuid,
}

// ── Namespaces ─────────────────────────────────────────────────────

/// Arguments for the `namespaces` command group.
#[derive(Debug, Args)]
pub struct NamespacesArgs {
    #[command(subcommand)]
    pub action: NamespaceAction,
}

/// Namespace subcommands.
#[derive(Debug, Subcommand)]
pub enum NamespaceAction {
    /// List all namespaces
    List,

    /// Create a new namespace
    Create(CreateNamespaceArgs),

    /// Show statistics for a namespace (memory count, phase distribution, disk usage)
    Stats(NamespaceStatsArgs),

    /// Change how fast memories decay in an existing namespace. Flags not
    /// given keep their current values; prints the policy before and after.
    Policy(NamespacePolicyArgs),
}

/// Arguments for `namespaces policy`.
#[derive(Debug, Args)]
pub struct NamespacePolicyArgs {
    /// Namespace name (`default` included).
    pub name: String,

    /// Time scale for decay: 1 normal, 2 twice as slow, 0 no decay.
    #[arg(long, conflicts_with = "inherit_decay_rate")]
    pub decay_rate_multiplier: Option<f32>,

    /// Drop the namespace's own multiplier and use the `[decay]` setting.
    #[arg(long)]
    pub inherit_decay_rate: bool,

    /// Stability, in days, a new memory starts with.
    #[arg(long)]
    pub initial_stability: Option<f32>,

    /// Retrievability below which a memory drops from Full to Summary.
    #[arg(long)]
    pub full_to_summary: Option<f32>,

    /// Retrievability below which a memory drops from Summary to Ghost.
    #[arg(long)]
    pub summary_to_ghost: Option<f32>,

    /// Retrievability below which a Ghost memory is deleted.
    #[arg(long)]
    pub ghost_to_delete: Option<f32>,

    /// Stability, in days, at which a memory becomes permanent.
    #[arg(long)]
    pub permastore_days: Option<f32>,
}

impl NamespacePolicyArgs {
    /// The partial policy change these flags ask for, in the
    /// `update_namespace_policy` field names.
    pub fn patch(&self) -> serde_json::Map<String, serde_json::Value> {
        use serde_json::{Map, Value, json};
        let mut patch = Map::new();
        if let Some(m) = self.decay_rate_multiplier {
            patch.insert("decayRateMultiplier".into(), json!(m));
        }
        if self.inherit_decay_rate {
            patch.insert("decayRateMultiplier".into(), Value::Null);
        }
        if let Some(s) = self.initial_stability {
            patch.insert("initialStability".into(), json!(s));
        }
        if let Some(p) = self.permastore_days {
            patch.insert("permastoreThreshold".into(), json!(p));
        }
        let mut thresholds = Map::new();
        for (name, value) in [
            ("fullToSummary", self.full_to_summary),
            ("summaryToGhost", self.summary_to_ghost),
            ("ghostToDelete", self.ghost_to_delete),
        ] {
            if let Some(v) = value {
                thresholds.insert(name.into(), json!(v));
            }
        }
        if !thresholds.is_empty() {
            patch.insert("phaseThresholds".into(), Value::Object(thresholds));
        }
        patch
    }
}

/// Arguments for `namespaces create`.
#[derive(Debug, Args)]
pub struct CreateNamespaceArgs {
    /// Namespace name (alphanumeric + hyphens + underscores, 1-64 chars).
    pub name: String,

    /// Embedding dimensionality (fixed after creation).
    #[arg(long, default_value_t = 1536)]
    pub dim: u32,

    /// Initial stability for new memories (days).
    #[arg(long, default_value_t = 3.7145)]
    pub initial_stability: f32,
}

/// Arguments for `namespaces stats`.
#[derive(Debug, Args)]
pub struct NamespaceStatsArgs {
    /// Namespace name. If omitted, shows stats for all namespaces.
    pub name: Option<String>,
}

// ── Sweep ──────────────────────────────────────────────────────────

/// Arguments for the `sweep` command.
#[derive(Debug, Args)]
pub struct SweepArgs {
    /// Show what would change without applying. Reports phase
    /// transitions and deletions that would occur.
    #[arg(long)]
    pub dry_run: bool,

    /// Restrict sweep to a specific namespace.
    #[arg(long, short)]
    pub namespace: Option<String>,
}

// ── Export ──────────────────────────────────────────────────────────

/// Arguments for the `export` command.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Restrict export to a specific namespace.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Export format.
    #[arg(long, default_value = "json", value_enum)]
    pub export_format: ExportFormat,

    /// Include full_text in export (increases size significantly).
    #[arg(long)]
    pub include_text: bool,

    /// Include embeddings in export (very large).
    #[arg(long)]
    pub include_embeddings: bool,
}

/// Export file format.
#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat {
    /// Single JSON array
    Json,
    /// One JSON object per line (streaming-friendly)
    Jsonl,
}

// ── List ──────────────────────────────────────────────────────────

/// Arguments for the `list` command.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Filter by namespace name.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Filter by decay phase: full, summary, ghost.
    #[arg(long, short, value_parser = parse_phase)]
    pub phase: Option<String>,

    /// Filter by tag (can be repeated, comma-separated). Memories must have ALL tags.
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Sort field: created, accessed, strength, stability.
    #[arg(long, default_value = "created", value_enum)]
    pub sort: SortField,

    /// Sort direction: asc, desc.
    #[arg(long, default_value = "desc", value_enum)]
    pub order: SortOrder,

    /// Maximum number of results.
    #[arg(long, short, default_value_t = 50)]
    pub limit: u32,

    /// Pagination offset.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
}

/// Sort field for list command.
#[derive(Debug, Clone, ValueEnum)]
pub enum SortField {
    /// Sort by creation timestamp
    Created,
    /// Sort by last access timestamp
    Accessed,
    /// Sort by retrievability/strength
    Strength,
    /// Sort by stability (days)
    Stability,
}

/// Sort order.
#[derive(Debug, Clone, ValueEnum)]
pub enum SortOrder {
    /// Ascending (oldest/weakest first)
    Asc,
    /// Descending (newest/strongest first)
    Desc,
}

/// Parse phase string, validating it's one of: full, summary, ghost.
fn parse_phase(s: &str) -> Result<String, String> {
    match s.to_lowercase().as_str() {
        "full" | "summary" | "ghost" => Ok(s.to_lowercase()),
        _ => Err(format!(
            "phase must be one of: full, summary, ghost (got '{s}')"
        )),
    }
}

// ── Import ──────────────────────────────────────────────────────────

/// Arguments for the `import` command.
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Path to JSON or JSONL file (or '-' for stdin).
    pub file: String,

    /// Input format (auto-detected from extension if omitted).
    #[arg(long, value_enum)]
    pub import_format: Option<ImportFormat>,

    /// Override namespace for all imported memories.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Show what would be imported without actually importing.
    #[arg(long)]
    pub dry_run: bool,

    /// Skip memories whose summary already exists (fuzzy match).
    #[arg(long)]
    pub skip_duplicates: bool,

    /// Number of memories per batch (reserved for future use).
    #[arg(long, default_value_t = 50)]
    pub batch_size: u32,

    /// Continue importing even if some memories fail.
    #[arg(long)]
    pub continue_on_error: bool,
}

/// Import file format.
#[derive(Debug, Clone, ValueEnum)]
pub enum ImportFormat {
    /// Single JSON array
    Json,
    /// One JSON object per line (streaming-friendly)
    Jsonl,
}

// ── Health ─────────────────────────────────────────────────────────

/// Arguments for the `health` command.
#[derive(Debug, Args)]
pub struct HealthArgs {
    /// Filter to a specific namespace.
    #[arg(long, short)]
    pub namespace: Option<String>,

    /// Output format (overrides global --format).
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
}

#[derive(Debug, Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub action: PolicyAction,
}
#[derive(Debug, Subcommand)]
pub enum PolicyAction {
    /// Offline, read-only complete preview; unknown old inputs remain unknown.
    PreviewInherited {
        #[arg(long)]
        store: std::path::PathBuf,
        #[arg(long)]
        namespace_id: u32,
        #[arg(long)]
        multiplier: f64,
        #[arg(long)]
        expected_store_id: Uuid,
        #[arg(long, value_parser = parse_policy_generation)]
        expected_generation: crate::storage::control::Generation,
    },
    /// Explicitly accept a complete preview under exclusive store ownership.
    AdoptInherited {
        #[arg(long)]
        store: std::path::PathBuf,
        #[arg(long)]
        request: std::path::PathBuf,
    },
}

fn parse_policy_generation(
    value: &str,
) -> std::result::Result<crate::storage::control::Generation, String> {
    value
        .to_owned()
        .try_into()
        .map_err(|e: crate::storage::StorageError| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn policy_args(flags: &[&str]) -> NamespacePolicyArgs {
        let argv = ["recalld-cli", "namespaces", "policy", "default"];
        let cli = Cli::try_parse_from(argv.iter().chain(flags)).unwrap();
        let Command::Namespaces(NamespacesArgs {
            action: NamespaceAction::Policy(args),
        }) = cli.command
        else {
            panic!("not namespaces policy")
        };
        args
    }

    #[test]
    fn namespaces_policy_flags_change_only_what_they_name() {
        let args = policy_args(&[
            "--decay-rate-multiplier",
            "2",
            "--full-to-summary",
            "0.6",
            "--permastore-days",
            "900",
        ]);
        let current = crate::serialization::policy::PolicyValues::current(
            &crate::model::NamespaceConfig::default_namespace(0),
        );
        let merged = current.merge(&args.patch()).unwrap();
        let num = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        assert_eq!(num(&merged["decayRateMultiplier"]), 2.0);
        assert_eq!(num(&merged["phaseThresholds"]["fullToSummary"]), 0.6);
        assert_eq!(num(&merged["phaseThresholds"]["summaryToGhost"]), 0.3);
        assert_eq!(num(&merged["permastoreThreshold"]), 900.0);
        assert_eq!(num(&merged["initialStability"]), 3.7145);

        let inherit = policy_args(&["--inherit-decay-rate"]).patch();
        assert!(inherit["decayRateMultiplier"].is_null());
        assert!(policy_args(&[]).patch().is_empty());
        assert!(current.merge(&policy_args(&[]).patch()).is_err());
        let argv = [
            "recalld-cli",
            "namespaces",
            "policy",
            "default",
            "--inherit-decay-rate",
            "--decay-rate-multiplier",
            "2",
        ];
        assert!(Cli::try_parse_from(argv).is_err());
    }
}
