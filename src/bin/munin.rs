use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use munin_memory::{
    agent_hook, agent_plugin, analytics, core, mcp, privacy, proactivity_cmd, runtime_context,
    session_brain, strategy_cmd,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "munin",
    version,
    about = "Munin - Local memory for agent-driven development",
    long_about = "Munin reads local agent sessions, compiles startup memory, surfaces friction, and keeps noisy shell output out of agent context."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Startup memory brief for the current project
    Resume {
        /// Inspection scope
        #[arg(long, default_value = "user")]
        scope: String,
        /// Project directory to read from another working directory
        #[arg(long)]
        project: Option<String>,
        /// Output format
        #[arg(short, long, default_value = "prompt")]
        format: String,
    },

    /// Startup brain for the current session and project
    Brain {
        /// Output format
        #[arg(short, long, default_value = "prompt")]
        format: String,
    },

    /// Unified runtime context packet for Claude/Codex integration assets
    Runtime {
        /// Which continuity surface to build
        #[arg(long, default_value = "auto")]
        surface: String,
        /// Inspection scope for resume-style packets
        #[arg(long, default_value = "user")]
        scope: String,
        /// Output format
        #[arg(short, long, default_value = "prompt")]
        format: String,
    },

    /// Show the next strategy-backed nudge
    Nudge {
        /// Strategy scope (defaults to configured default scope)
        #[arg(long)]
        scope: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: strategy_cmd::StrategyFormat,
    },

    /// Show replay proof for the promoted Memory OS read path
    Prove {
        /// Accepted for product-copy compatibility; proof currently reports Memory OS promotion state
        #[arg(long = "last-resume", default_value_t = false)]
        last_resume: bool,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show repeated correction and friction patterns
    Friction {
        /// Filter evidence to an agent label when available
        #[arg(long)]
        agent: Option<String>,
        /// Limit to a recent window label such as 30d when available
        #[arg(long)]
        last: Option<String>,
        /// Inspection scope
        #[arg(long, default_value = "user")]
        scope: String,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Record an observe-only rule candidate
    Promote {
        /// Rule text to record
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        rule: Vec<String>,
    },

    /// Read compiled memory for a query
    Recall {
        /// Topic to answer from compiled Memory OS evidence
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show a stored Munin artifact by id
    Show {
        /// Artifact id such as @munin/a_deadbeefdeadbeef
        artifact_id: String,
        /// Optional line slice such as 1:80
        #[arg(long)]
        lines: Option<String>,
    },

    /// Show a simple diff between two stored Munin artifacts
    Diff {
        /// Left artifact id
        left_artifact_id: String,
        /// Right artifact id
        right_artifact_id: String,
    },

    /// Install Munin skills/plugin assets for Claude and Codex
    Install {
        /// Install Claude skills into ~/.claude/skills
        #[arg(long, default_value_t = false)]
        claude: bool,
        /// Install Codex skills into ~/.codex/skills and plugin assets into ~/.codex/plugins
        #[arg(long, default_value_t = false)]
        codex: bool,
        /// Print planned writes without changing files
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Replace existing Munin skill/plugin files
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Keep old Munin/context-wrapper skill names instead of archiving them
        #[arg(long, default_value_t = false)]
        keep_legacy: bool,
        /// Validate generated skills and resolver targets without writing files
        #[arg(long, default_value_t = false)]
        check_resolvable: bool,
    },

    /// Install, verify, or uninstall Munin through native host plugin managers
    Plugin {
        #[command(subcommand)]
        command: AgentPluginCommands,
    },

    /// Run bounded agent lifecycle hooks
    Hook {
        #[command(subcommand)]
        command: HookCommands,
    },

    /// Serve Munin's read-only local memory tools over MCP
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Inspect, export, retain, or forget Munin-owned local data
    Privacy {
        #[command(subcommand)]
        command: PrivacyCommands,
    },

    /// Resolve a natural-language request to the Munin read surface that should answer it
    Resolve {
        #[arg(short, long, default_value = "text")]
        format: String,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,
    },

    /// Read or update strategy KPI metrics
    Metrics {
        #[command(subcommand)]
        command: MetricsCommands,
    },

    /// Audit and prune duplicated agent memory guidance files
    Hygiene {
        /// Root directory to scan
        #[arg(long, default_value = ".")]
        root: String,
        /// Apply exact duplicate removals with .munin-bak backups
        #[arg(long, default_value_t = false)]
        write: bool,
        /// Include .codex memory files in the scan
        #[arg(long, default_value_t = false)]
        include_codex: bool,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Fast Memory OS health check, with optional release checks
    Doctor {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(short, long, default_value = "text")]
        format: String,
        #[arg(long, default_value_t = false)]
        release: bool,
        #[arg(long)]
        repo_root: Option<String>,
        #[arg(long)]
        site_root: Option<String>,
    },

    /// Read and inspect Memory OS state
    #[command(name = "memory-os")]
    MemoryOs {
        #[command(subcommand)]
        command: MemoryOsCommands,
    },

    /// Strategy kernel setup and read surfaces
    Strategy {
        #[command(subcommand)]
        command: StrategyCommands,
    },
    /// Morning strategic proactivity runner, queue, and scheduling surface
    Proactivity {
        #[command(subcommand)]
        command: ProactivityCommands,
    },
}

#[derive(Subcommand)]
enum AgentPluginCommands {
    /// Install the native Munin plugin
    Install {
        /// Manage only the Codex plugin
        #[arg(long, default_value_t = false)]
        codex: bool,
        /// Manage only the Claude plugin
        #[arg(long, default_value_t = false)]
        claude: bool,
        /// Inspect and report without changing host state
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: agent_plugin::AgentPluginFormat,
    },
    /// Verify the native Munin plugin identity, version, and enabled state
    Verify {
        /// Verify only the Codex plugin
        #[arg(long, default_value_t = false)]
        codex: bool,
        /// Verify only the Claude plugin
        #[arg(long, default_value_t = false)]
        claude: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: agent_plugin::AgentPluginFormat,
    },
    /// Uninstall the native Munin plugin and marketplace
    Uninstall {
        /// Manage only the Codex plugin
        #[arg(long, default_value_t = false)]
        codex: bool,
        /// Manage only the Claude plugin
        #[arg(long, default_value_t = false)]
        claude: bool,
        /// Inspect and report without changing host state
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: agent_plugin::AgentPluginFormat,
    },
}

#[derive(Subcommand)]
enum HookCommands {
    /// Return memory freshness and queue a bounded refresh when needed
    SessionStart {
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: agent_hook::HookFormat,
    },
    /// Run the detached incremental refresh worker
    #[command(hide = true)]
    RefreshWorker,
}

#[derive(Subcommand)]
enum McpCommands {
    /// Serve newline-delimited JSON-RPC over standard input/output
    Serve,
}

#[derive(Subcommand)]
enum PrivacyCommands {
    /// Inventory Munin-owned local data without reading source transcripts
    Inspect {
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Export a consistent snapshot of Munin-owned local data
    Export {
        /// New directory to create for the export
        #[arg(long)]
        output: PathBuf,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Preview or apply compiled-row retention
    Retention {
        /// Number of days of compiled history to retain
        #[arg(long)]
        days: u32,
        /// Save the policy and delete eligible compiled rows
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Delete exact-project rows or all Munin-owned state
    Forget {
        /// Exact project_path value to delete from compiled state
        #[arg(long, conflicts_with = "all")]
        project: Option<String>,
        /// Delete all Munin-owned data and configuration
        #[arg(long, default_value_t = false, conflicts_with = "project")]
        all: bool,
        /// Required exact acknowledgement: FORGET-MUNIN
        #[arg(long)]
        confirm: String,
        /// Output format
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum MemoryOsCommands {
    /// Ingest local agent sessions into Memory OS and show timing
    Ingest {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        check_now: bool,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Compact startup briefing from compiled Memory OS state
    Brief {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Summarize compiled Memory OS state
    Overview {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show full Memory OS inspection report
    Inspect {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Diagnose Memory OS pipeline health
    Doctor {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Summarize operating style and preferences
    Profile {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Derive memory-to-action defaults and guardrails
    #[command(name = "action-policy")]
    ActionPolicy {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Summarize friction and behavior-change signals
    Friction {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show current project snapshot
    Snapshot {
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Rebuild narrow Memory OS kernel from journal events
    Kernel {
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Derive observe-only action-memory candidates
    Actions {
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Inspect trust observations
    Trust {
        #[arg(long, default_value = "user")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show whether replay proof promotes Memory OS cutover
    Promotion {
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum MetricsCommands {
    /// Record the current value for a configured KPI metric
    Set {
        metric_key: String,
        value: f64,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        unit: Option<String>,
        #[arg(long)]
        at: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Read recorded KPI metrics
    Get {
        metric_key: Option<String>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Import a metrics snapshot into the configured metrics path
    Sync {
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        from: String,
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum StrategyCommands {
    /// Configure or import a strategy kernel source
    Setup {
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long = "import")]
        import_path: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = false)]
        bootstrap_claude: bool,
        #[arg(long, default_value_t = false)]
        template: bool,
        #[arg(short, long, value_enum, default_value = "text")]
        format: strategy_cmd::StrategyFormat,
    },
    /// Inspect the imported strategy kernel and source registry
    Inspect {
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(short, long, value_enum, default_value = "text")]
        format: strategy_cmd::StrategyFormat,
    },
    /// Show deterministic strategy status
    Status {
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(short, long, value_enum, default_value = "text")]
        format: strategy_cmd::StrategyFormat,
    },
    /// Synthesize bounded strategic nudges
    Recommend {
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(short, long, value_enum, default_value = "text")]
        format: strategy_cmd::StrategyFormat,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ProactivityStatusArg {
    Complete,
    Failed,
    Deferred,
    Suppressed,
}

impl From<ProactivityStatusArg> for core::proactivity::ProactivityTerminalStatus {
    fn from(value: ProactivityStatusArg) -> Self {
        match value {
            ProactivityStatusArg::Complete => {
                core::proactivity::ProactivityTerminalStatus::Complete
            }
            ProactivityStatusArg::Failed => core::proactivity::ProactivityTerminalStatus::Failed,
            ProactivityStatusArg::Deferred => {
                core::proactivity::ProactivityTerminalStatus::Deferred
            }
            ProactivityStatusArg::Suppressed => {
                core::proactivity::ProactivityTerminalStatus::Suppressed
            }
        }
    }
}

#[derive(Subcommand)]
enum ProactivityCommands {
    /// Run the morning proactivity evaluation and optionally spawn a session
    Run {
        /// Optional scope override
        #[arg(long)]
        scope: Option<String>,
        /// Optional provider override
        #[arg(long, value_enum)]
        provider: Option<core::config::ProactivityProvider>,
        /// Compute artifacts but do not launch a visible session
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Immediately launch the queued intervention after writing approval artifacts
        #[arg(long, default_value_t = false)]
        auto_spawn: bool,
        /// Skip the actual spawn but still write queue/brief/decision artifacts
        #[arg(long, default_value_t = false)]
        no_spawn: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Sweep stale queue claims/results and finalize daemon state
    Sweep {
        /// Optional scope override
        #[arg(long)]
        scope: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Show current proactivity status, schedules, and today's artifact state
    Status {
        /// Optional scope override
        #[arg(long)]
        scope: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Install the 8am morning task and maintenance sweep task
    #[command(name = "schedule-install")]
    ScheduleInstall {
        /// Optional scope override
        #[arg(long)]
        scope: Option<String>,
        /// Optional provider override
        #[arg(long, value_enum)]
        provider: Option<core::config::ProactivityProvider>,
        /// Optional project path override
        #[arg(long)]
        project_path: Option<PathBuf>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Remove installed proactivity scheduled tasks for the scope
    #[command(name = "schedule-remove")]
    ScheduleRemove {
        /// Optional scope override
        #[arg(long)]
        scope: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Claim a queued morning proactivity job by atomic rename
    Claim {
        /// Job id to claim
        #[arg(long)]
        job_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Approve a queued morning proactivity job and optionally launch it
    Approve {
        /// Job id to approve
        #[arg(long)]
        job_id: String,
        /// Claim the job without launching a visible session
        #[arg(long, default_value_t = false)]
        no_spawn: bool,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Reject a queued morning proactivity job with a terminal result
    Reject {
        /// Job id to reject
        #[arg(long)]
        job_id: String,
        /// Summary line for the rejection
        #[arg(long)]
        summary: String,
        /// Optional rejection notes (repeatable)
        #[arg(long = "note")]
        notes: Vec<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
    /// Write a terminal result for a claimed morning proactivity job
    Complete {
        /// Job id to complete
        #[arg(long)]
        job_id: String,
        /// Final status to write
        #[arg(long, value_enum)]
        status: ProactivityStatusArg,
        /// Summary line for the result
        #[arg(long)]
        summary: String,
        /// Optional error detail
        #[arg(long)]
        error: Option<String>,
        /// Optional notes (repeatable)
        #[arg(long = "note")]
        notes: Vec<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: proactivity_cmd::ProactivityFormat,
    },
}

fn main() {
    let code = match run_cli() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("munin: {error:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run_cli() -> Result<i32> {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Resume {
            scope,
            project,
            format,
        } => {
            let scope = if project.is_some() && scope == "user" {
                "project"
            } else {
                &scope
            };
            analytics::memory_os_cmd::run_startup_brief(
                scope,
                project.as_deref(),
                &format,
                cli.verbose,
            )?;
            0
        }
        Commands::Brain { format } => {
            session_brain::run_inspect_current(&format, cli.verbose)?;
            0
        }
        Commands::Runtime {
            surface,
            scope,
            format,
        } => {
            let mode = runtime_context::RuntimeContextRenderMode::parse(&format)?;
            let packet = match surface.as_str() {
                "auto" => {
                    let brain = runtime_context::build_current_brain_packet()?;
                    if brain.meta.source_mode == runtime_context::RuntimeContextSourceMode::Live {
                        brain
                    } else {
                        runtime_context::build_current_resume_packet(&scope)?
                    }
                }
                "brain" => runtime_context::build_current_brain_packet()?,
                "resume" => runtime_context::build_current_resume_packet(&scope)?,
                other => {
                    anyhow::bail!(
                        "unsupported surface '{}' (expected auto, brain, or resume)",
                        other
                    )
                }
            };
            println!("{}", runtime_context::render_packet(&packet, mode)?);
            0
        }
        Commands::Nudge { scope, format } => {
            strategy_cmd::run_recommend(strategy_cmd::StrategyReadRequest {
                scope: scope.unwrap_or_else(configured_strategy_scope_or_default),
                format,
            })?;
            0
        }
        Commands::Prove {
            last_resume: _,
            format,
        } => {
            analytics::memory_os_cmd::run_promotion(&format, cli.verbose)?;
            0
        }
        Commands::Friction {
            agent,
            last,
            scope,
            format,
        } => {
            analytics::memory_os_cmd::run_friction_filtered(
                &scope,
                None,
                &format,
                agent.as_deref(),
                last.as_deref(),
            )?;
            0
        }
        Commands::Promote { rule } => {
            run_munin_promote(&rule)?;
            0
        }
        Commands::Recall { query, format } => {
            analytics::memory_os_cmd::run_recall(
                "user",
                None,
                query.join(" ").as_str(),
                &format,
                cli.verbose,
            )?;
            0
        }
        Commands::Show { artifact_id, lines } => {
            println!(
                "{}",
                core::artifacts::show_artifact(&artifact_id, lines.as_deref())?
            );
            0
        }
        Commands::Diff {
            left_artifact_id,
            right_artifact_id,
        } => {
            let left = core::artifacts::load_artifact_text(&left_artifact_id)?;
            let right = core::artifacts::load_artifact_text(&right_artifact_id)?;
            println!(
                "{}",
                render_artifact_diff(&left_artifact_id, &right_artifact_id, &left, &right)
            );
            0
        }
        Commands::Install {
            claude,
            codex,
            dry_run,
            force,
            keep_legacy,
            check_resolvable,
        } => run_install(InstallOptions {
            claude,
            codex,
            dry_run,
            force,
            keep_legacy,
            check_resolvable,
        })?,
        Commands::Plugin { command } => run_agent_plugin(command)?,
        Commands::Hook { command } => match command {
            HookCommands::SessionStart { format } => {
                let receipt = agent_hook::session_start()?;
                agent_hook::render_session_start(&receipt, format)?;
                0
            }
            HookCommands::RefreshWorker => {
                agent_hook::refresh_worker()?;
                0
            }
        },
        Commands::Mcp { command } => match command {
            McpCommands::Serve => {
                mcp::serve_stdio()?;
                0
            }
        },
        Commands::Privacy { command } => {
            match command {
                PrivacyCommands::Inspect { format } => {
                    let receipt = privacy::inspect()?;
                    render_json_or_text(&receipt, &format, || privacy::inspect_text(&receipt))?;
                }
                PrivacyCommands::Export { output, format } => {
                    let receipt = privacy::export(&output)?;
                    render_json_or_text(&receipt, &format, || privacy::export_text(&receipt))?;
                }
                PrivacyCommands::Retention {
                    days,
                    apply,
                    format,
                } => {
                    let receipt = privacy::retention(days, apply)?;
                    render_json_or_text(&receipt, &format, || privacy::retention_text(&receipt))?;
                }
                PrivacyCommands::Forget {
                    project,
                    all,
                    confirm,
                    format,
                } => {
                    let receipt = privacy::forget(project.as_deref(), all, &confirm)?;
                    render_json_or_text(&receipt, &format, || privacy::forget_text(&receipt))?;
                }
            }
            0
        }
        Commands::Resolve { format, query } => {
            let source_status = session_brain::current_source_status();
            let report = core::resolver::resolve_with_source_status(
                query.join(" ").as_str(),
                source_status.as_deref(),
            );
            render_resolve_report(&report, &format)?;
            0
        }
        Commands::Metrics { command } => {
            run_metrics(command)?;
            0
        }
        Commands::Hygiene {
            root,
            write,
            include_codex,
            format,
        } => {
            let report = core::memory_hygiene::run(&core::memory_hygiene::MemoryHygieneOptions {
                root: PathBuf::from(root),
                write,
                include_codex,
            })?;
            render_hygiene_report(&report, &format)?;
            0
        }
        Commands::Doctor {
            scope,
            format,
            release,
            repo_root,
            site_root,
        } => {
            let doctor_status =
                analytics::memory_os_cmd::run_doctor(&scope, None, &format, cli.verbose)?;
            if release {
                run_release_doctor_checks(repo_root.as_deref(), site_root.as_deref())?;
            }
            doctor_status.exit_code()
        }
        Commands::MemoryOs { command } => run_memory_os(command, cli.verbose)?,
        Commands::Strategy { command } => run_strategy(command)?,
        Commands::Proactivity { command } => run_proactivity(command)?,
    };
    Ok(code)
}

fn run_metrics(command: MetricsCommands) -> Result<()> {
    match command {
        MetricsCommands::Set {
            metric_key,
            value,
            scope,
            unit,
            at,
            format,
        } => {
            let report = core::strategy::metrics_set(core::strategy::StrategyMetricSetOptions {
                scope,
                metric_key,
                value,
                unit,
                updated_at: at,
            })?;
            render_metrics_report(&report, &format)
        }
        MetricsCommands::Get {
            metric_key,
            scope,
            format,
        } => {
            let report = core::strategy::metrics_get(core::strategy::StrategyMetricGetOptions {
                scope,
                metric_key,
            })?;
            render_metrics_report(&report, &format)
        }
        MetricsCommands::Sync {
            scope,
            from,
            format,
        } => {
            let report = core::strategy::metrics_sync(core::strategy::StrategyMetricSyncOptions {
                scope,
                from_path: PathBuf::from(from),
            })?;
            render_metrics_report(&report, &format)
        }
    }
}

fn render_json_or_text<T: Serialize>(
    report: &T,
    format: &str,
    text: impl FnOnce() -> String,
) -> Result<()> {
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(report)?),
        "text" => println!("{}", text()),
        other => anyhow::bail!("unsupported format `{other}`; expected text or json"),
    }
    Ok(())
}

fn render_artifact_diff(left_id: &str, right_id: &str, left: &str, right: &str) -> String {
    let left_lines = left.lines().collect::<Vec<_>>();
    let right_lines = right.lines().collect::<Vec<_>>();
    let removed = left_lines
        .iter()
        .filter(|line| !right_lines.contains(line))
        .take(40)
        .map(|line| format!("- {}", line))
        .collect::<Vec<_>>();
    let added = right_lines
        .iter()
        .filter(|line| !left_lines.contains(line))
        .take(40)
        .map(|line| format!("+ {}", line))
        .collect::<Vec<_>>();
    let mut lines = vec![
        format!("Munin artifact diff"),
        format!("left: {left_id}"),
        format!("right: {right_id}"),
    ];
    if removed.is_empty() && added.is_empty() {
        lines.push("No line-level differences found.".to_string());
    } else {
        lines.extend(removed);
        lines.extend(added);
    }
    lines.join("\n")
}

fn render_resolve_report(report: &core::resolver::ResolveReport, format: &str) -> Result<()> {
    render_json_or_text(report, format, || {
        format!(
            "Munin Resolve\n-------------\nRoute: {}\nCommand: {}\nWhy: {}",
            report.route, report.command, report.reason
        )
    })
}

fn render_metrics_report(
    report: &core::strategy::StrategyMetricsReport,
    format: &str,
) -> Result<()> {
    render_json_or_text(report, format, || {
        let mut lines = vec![
            "Munin Metrics".to_string(),
            "-------------".to_string(),
            format!("Scope: {}", report.scope_id),
            format!("Path: {}", report.metrics_path),
        ];
        if report.kpis.is_empty() {
            lines.push("No metric values recorded.".to_string());
        } else {
            for (key, record) in &report.kpis {
                let value = record
                    .current
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let unit = record.unit.as_deref().unwrap_or("unitless");
                let updated = record.updated_at.as_deref().unwrap_or("unknown time");
                lines.push(format!("- {key}: {value} {unit} ({updated})"));
            }
        }
        for warning in &report.warnings {
            lines.push(format!("warning: {warning}"));
        }
        lines.join("\n")
    })
}

fn render_hygiene_report(
    report: &core::memory_hygiene::MemoryHygieneReport,
    format: &str,
) -> Result<()> {
    render_json_or_text(report, format, || {
        let mut lines = vec![
            "Munin Memory Hygiene".to_string(),
            "--------------------".to_string(),
            format!("Root: {}", report.root),
            format!("Files scanned: {}", report.files_scanned.len()),
            format!("Duplicate groups: {}", report.duplicate_groups.len()),
            format!("Planned removals: {}", report.planned_removals.len()),
        ];
        if report.write_applied {
            lines.push(format!("Backups written: {}", report.backups.len()));
        }
        if !report.skipped_dirs.is_empty() {
            lines.push(format!(
                "Skipped directories: {} (worktree/runtime/cache exclusions)",
                report.skipped_dirs.len()
            ));
        }
        if !report.files_scanned.is_empty() {
            lines.push("Memory files:".to_string());
            for file in &report.files_scanned {
                lines.push(format!(
                    "- [{}] {} ({} guidance units)",
                    file.store_kind, file.path, file.guidance_units
                ));
            }
        }
        if !report.duplicate_groups.is_empty() {
            lines.push("Duplicates:".to_string());
            for group in report.duplicate_groups.iter().take(8) {
                let mode = if group.auto_prunable {
                    "auto-prunable"
                } else {
                    "report-only"
                };
                lines.push(format!("- {}: {}", mode, group.reason));
                for occurrence in group.occurrences.iter().take(4) {
                    lines.push(format!(
                        "  - {}:{} [{}] {}",
                        occurrence.path,
                        occurrence.line_number,
                        occurrence.store_kind,
                        occurrence.text
                    ));
                }
            }
        }
        for warning in &report.warnings {
            lines.push(format!("warning: {warning}"));
        }
        lines.join("\n")
    })
}

fn run_release_doctor_checks(repo_root: Option<&str>, site_root: Option<&str>) -> Result<()> {
    let repo_root = repo_root.unwrap_or(".");
    let cargo_toml = Path::new(repo_root).join("Cargo.toml");
    let cargo = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    if !cargo
        .lines()
        .any(|line| line.trim() == r#"license = "Apache-2.0""#)
    {
        anyhow::bail!("release doctor failed: Cargo.toml must declare Apache-2.0");
    }
    let brain = session_brain::build_current_session_brain()
        .context("release doctor failed: could not inspect Session Brain")?;
    if !session_brain_source_is_release_safe(&brain.meta.source_status) {
        anyhow::bail!("release doctor failed: Session Brain is reading stale fallback context");
    }
    let recall_source = fs::read_to_string(Path::new(repo_root).join("src/bin/munin.rs"))
        .context("release doctor failed: could not inspect CLI source")?;
    let stale_recall_warning = ["munin recall", ": query search is not active yet"].concat();
    if recall_source.contains(stale_recall_warning.as_str()) {
        anyhow::bail!("release doctor failed: recall still contains overview fallback wiring");
    }
    let site_root = site_root
        .map(PathBuf::from)
        .or_else(|| std::env::var("MUNIN_SITE_ROOT").ok().map(PathBuf::from));
    if let Some(path) = site_root.as_deref() {
        if !path.exists() {
            anyhow::bail!(
                "release doctor failed: site root {} does not exist",
                path.display()
            );
        }
        assert_public_docs_parity(Path::new(repo_root), Some(path))?;
    } else {
        assert_public_docs_parity(Path::new(repo_root), None)?;
    }
    println!("release checks: package guard, freshness, recall, and docs parity checks passed");
    Ok(())
}

fn assert_public_docs_parity(repo_root: &Path, site_root: Option<&Path>) -> Result<()> {
    let mut docs = vec![repo_root.join("README.md")];
    if let Some(site_root) = site_root {
        collect_public_html_docs(site_root, &mut docs)?;
    }
    let mut failures = Vec::new();
    for path in docs {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let lowered = content.to_lowercase();
        for banned in BANNED_PUBLIC_DOC_SNIPPETS {
            if lowered.contains(banned) {
                failures.push(format!(
                    "{} contains unsupported `{}`",
                    path.display(),
                    banned
                ));
            }
        }
        let is_html = path.extension().and_then(|ext| ext.to_str()) == Some("html");
        for command in extract_public_munin_commands(&content, is_html) {
            let validation_command = public_doc_command_for_validation(&command);
            if let Err(error) = validate_munin_command(&validation_command) {
                failures.push(format!(
                    "{} command `{}` is not resolvable: {:#}",
                    path.display(),
                    command,
                    error
                ));
            }
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "release doctor failed: public docs contain unsupported Munin promises:\n{}",
            failures.join("\n")
        );
    }
    Ok(())
}

fn collect_public_html_docs(root: &Path, docs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read site root {}", root.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_public_html_docs(&path, docs)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("html") {
            docs.push(path);
        }
    }
    Ok(())
}

const BANNED_PUBLIC_DOC_SNIPPETS: &[&str] = &[
    "munin init",
    "munin gain",
    "munin pack",
    "munin vitest",
    "munin cargo test",
    "munin git diff",
    "munin replay-eval",
];

fn extract_public_munin_commands(content: &str, is_html: bool) -> Vec<String> {
    let normalized = if is_html {
        decode_public_doc_entities(&strip_html_tags(content))
    } else {
        decode_public_doc_entities(content)
    };
    let mut commands = Vec::new();
    for raw_line in normalized.lines() {
        let line = raw_line.trim();
        if let Some(command) = extract_prompted_munin_command(line, !is_html) {
            push_unique_command(&mut commands, command);
        }
        if !is_html {
            for code_span in markdown_code_spans(line) {
                if let Some(command) = extract_prompted_munin_command(code_span.trim(), true) {
                    push_unique_command(&mut commands, command);
                }
            }
        }
    }
    commands
}

fn push_unique_command(commands: &mut Vec<String>, command: String) {
    if !commands.iter().any(|existing| existing == &command) {
        commands.push(command);
    }
}

fn extract_prompted_munin_command(line: &str, allow_bare_command: bool) -> Option<String> {
    let mut candidate = line.trim();
    let mut saw_prompt = false;
    for prompt in ["$", ">", "#"] {
        if let Some(stripped) = candidate.strip_prefix(prompt) {
            candidate = stripped.trim_start();
            saw_prompt = true;
        }
    }
    if !saw_prompt && !allow_bare_command {
        return None;
    }
    if !candidate.starts_with("munin ") {
        return None;
    }
    Some(clean_public_munin_command(candidate))
}

fn clean_public_munin_command(command: &str) -> String {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = command.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '`' if !in_single_quote && !in_double_quote => {
                return command[..index].trim().to_string()
            }
            '#' if !in_single_quote && !in_double_quote => {
                let previous_is_space = command[..index]
                    .chars()
                    .next_back()
                    .map(|previous| previous.is_whitespace())
                    .unwrap_or(false);
                if previous_is_space {
                    return command[..index].trim().to_string();
                }
            }
            _ if !in_single_quote && !in_double_quote => {
                if let Some((_, next)) = chars.peek() {
                    if ch.is_whitespace() && *next == '$' {
                        return command[..index].trim().to_string();
                    }
                }
            }
            _ => {}
        }
    }
    command
        .trim()
        .trim_end_matches([',', ';', '.'])
        .trim()
        .to_string()
}

fn public_doc_command_for_validation(command: &str) -> String {
    command
        .replace("<metric_key>", "metric_key")
        .replace("<value>", "1")
        .replace("<scope>", "default")
        .replace("<topic>", "resolver")
        .replace("<query>", "resolver")
        .replace(
            "<strategic-plan.context.json>",
            "strategic-plan.context.json",
        )
        .replace("<published-site-root>", ".")
}

fn markdown_code_spans(line: &str) -> Vec<&str> {
    line.split('`')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
        .collect()
}

fn strip_html_tags(content: &str) -> String {
    let mut stripped = String::with_capacity(content.len());
    let mut in_tag = false;
    for ch in content.chars() {
        match ch {
            '<' => {
                in_tag = true;
                stripped.push(' ');
            }
            '>' => {
                in_tag = false;
                stripped.push(' ');
            }
            _ if !in_tag => stripped.push(ch),
            _ => {}
        }
    }
    stripped
}

fn decode_public_doc_entities(content: &str) -> String {
    content
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

fn session_brain_source_is_release_safe(source_status: &str) -> bool {
    !matches!(source_status, "stale" | "stale-fallback")
}

fn run_memory_os(command: MemoryOsCommands, verbose: u8) -> Result<i32> {
    match command {
        MemoryOsCommands::Ingest {
            force,
            check_now,
            format,
        } => run_memory_os_ingest(force, check_now, &format)?,
        MemoryOsCommands::Brief {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_brief(
            &scope,
            project.as_deref(),
            &format,
            false,
            verbose,
        )?,
        MemoryOsCommands::Overview {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_overview(&scope, project.as_deref(), &format, verbose)?,
        MemoryOsCommands::Inspect {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_inspect(&scope, project.as_deref(), &format, verbose)?,
        MemoryOsCommands::Doctor {
            scope,
            project,
            format,
        } => {
            let doctor_status =
                analytics::memory_os_cmd::run_doctor(&scope, project.as_deref(), &format, verbose)?;
            return Ok(doctor_status.exit_code());
        }
        MemoryOsCommands::Profile {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_profile(&scope, project.as_deref(), &format, verbose)?,
        MemoryOsCommands::ActionPolicy {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_action_policy(
            &scope,
            project.as_deref(),
            &format,
            verbose,
        )?,
        MemoryOsCommands::Friction {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_friction(&scope, project.as_deref(), &format, verbose)?,
        MemoryOsCommands::Snapshot { project, format } => {
            analytics::memory_os_cmd::run_snapshot(project.as_deref(), &format, verbose)?
        }
        MemoryOsCommands::Kernel { project, format } => {
            analytics::memory_os_cmd::run_kernel(project.as_deref(), &format, verbose)?
        }
        MemoryOsCommands::Actions { project, format } => {
            analytics::memory_os_cmd::run_actions(project.as_deref(), &format, verbose)?
        }
        MemoryOsCommands::Trust {
            scope,
            project,
            format,
        } => analytics::memory_os_cmd::run_trust(&scope, project.as_deref(), &format, verbose)?,
        MemoryOsCommands::Promotion { format } => {
            analytics::memory_os_cmd::run_promotion(&format, verbose)?
        }
    }
    Ok(0)
}

fn run_memory_os_ingest(force: bool, check_now: bool, format: &str) -> Result<()> {
    if !matches!(format, "text" | "json") {
        anyhow::bail!("unsupported format `{format}`; expected text or json");
    }
    let started = std::time::Instant::now();
    let report = if check_now && !force {
        analytics::session_backfill::ensure_memory_os_session_backfill_check_now()?
    } else {
        analytics::session_backfill::ensure_memory_os_session_backfill_with_force(force)?
    };
    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_millis();
    match format {
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status": if report.is_some() { "imported" } else { "current" },
                    "force": force,
                    "check_now": check_now,
                    "elapsed_ms": elapsed_ms,
                    "sessions_processed": report.as_ref().map(|report| report.sessions_processed).unwrap_or(0),
                    "shells_ingested": report.as_ref().map(|report| report.shells_ingested).unwrap_or(0),
                    "corrections_ingested": report.as_ref().map(|report| report.corrections_ingested).unwrap_or(0),
                    "completed_at": report.as_ref().map(|report| report.completed_at.as_str()),
                }))?
            );
        }
        "text" => {
            println!("Memory OS Ingest");
            println!("----------------");
            println!("Elapsed: {}", format_elapsed(elapsed));
            println!(
                "Mode: {}",
                if force {
                    "forced replay"
                } else if check_now {
                    "incremental check now"
                } else {
                    "incremental"
                }
            );
            if let Some(report) = report {
                println!("Status: imported");
                println!("Sessions processed: {}", report.sessions_processed);
                println!("Shell executions ingested: {}", report.shells_ingested);
                println!("Corrections ingested: {}", report.corrections_ingested);
                println!("Completed: {}", report.completed_at);
            } else {
                println!("Status: current");
                println!("No new sessions needed ingestion.");
            }
        }
        _ => unreachable!("format is validated before rendering"),
    }
    Ok(())
}

fn format_elapsed(elapsed: std::time::Duration) -> String {
    if elapsed.as_secs() > 0 {
        format!("{:.2}s", elapsed.as_secs_f64())
    } else {
        format!("{}ms", elapsed.as_millis())
    }
}

fn run_strategy(command: StrategyCommands) -> Result<i32> {
    match command {
        StrategyCommands::Setup {
            scope,
            import_path,
            bootstrap_claude,
            template,
            format,
        } => strategy_cmd::run_setup(strategy_cmd::StrategySetupRequest {
            scope,
            import_path,
            bootstrap_claude,
            template,
            format,
        })?,
        StrategyCommands::Inspect { scope, format } => {
            strategy_cmd::run_inspect(strategy_cmd::StrategyReadRequest { scope, format })?
        }
        StrategyCommands::Status { scope, format } => {
            strategy_cmd::run_status(strategy_cmd::StrategyReadRequest { scope, format })?
        }
        StrategyCommands::Recommend { scope, format } => {
            strategy_cmd::run_recommend(strategy_cmd::StrategyReadRequest { scope, format })?
        }
    }
    Ok(0)
}

fn run_proactivity(command: ProactivityCommands) -> Result<i32> {
    match command {
        ProactivityCommands::Run {
            scope,
            provider,
            dry_run,
            auto_spawn,
            no_spawn,
            format,
        } => {
            proactivity_cmd::run(proactivity_cmd::ProactivityRunRequest {
                scope,
                provider,
                dry_run,
                auto_spawn,
                no_spawn,
                format,
            })?;
        }
        ProactivityCommands::Sweep { scope, format } => {
            proactivity_cmd::sweep(proactivity_cmd::ProactivityScopeRequest { scope, format })?;
        }
        ProactivityCommands::Status { scope, format } => {
            proactivity_cmd::status(proactivity_cmd::ProactivityScopeRequest { scope, format })?;
        }
        ProactivityCommands::ScheduleInstall {
            scope,
            provider,
            project_path,
            format,
        } => {
            proactivity_cmd::schedule_install(
                proactivity_cmd::ProactivityScheduleInstallRequest {
                    scope,
                    provider,
                    project_path,
                    format,
                },
            )?;
        }
        ProactivityCommands::ScheduleRemove { scope, format } => {
            proactivity_cmd::schedule_remove(proactivity_cmd::ProactivityScopeRequest {
                scope,
                format,
            })?;
        }
        ProactivityCommands::Claim { job_id, format } => {
            proactivity_cmd::claim(proactivity_cmd::ProactivityClaimRequest { job_id, format })?;
        }
        ProactivityCommands::Approve {
            job_id,
            no_spawn,
            format,
        } => {
            proactivity_cmd::approve(proactivity_cmd::ProactivityApproveRequest {
                job_id,
                no_spawn,
                format,
            })?;
        }
        ProactivityCommands::Reject {
            job_id,
            summary,
            notes,
            format,
        } => {
            proactivity_cmd::complete(proactivity_cmd::ProactivityCompleteRequest {
                job_id,
                status: core::proactivity::ProactivityTerminalStatus::Suppressed,
                summary,
                error: None,
                notes,
                format,
            })?;
        }
        ProactivityCommands::Complete {
            job_id,
            status,
            summary,
            error,
            notes,
            format,
        } => {
            proactivity_cmd::complete(proactivity_cmd::ProactivityCompleteRequest {
                job_id,
                status: status.into(),
                summary,
                error,
                notes,
                format,
            })?;
        }
    }
    Ok(0)
}

fn configured_strategy_scope_or_default() -> String {
    core::config::Config::load()
        .ok()
        .and_then(|config| config.strategy.configured_scope_name(None))
        .unwrap_or_else(|| "default".to_string())
}

fn run_munin_promote(rule: &[String]) -> Result<()> {
    let rule_text = rule.join(" ").trim().to_string();
    if rule_text.is_empty() {
        anyhow::bail!(
            "munin promote needs rule text, for example: munin promote \"use bun, not npm\""
        );
    }
    let key = format!("munin-promoted-rule:{}", short_sha256(&rule_text));
    analytics::claims_cmd::set_user_decision(&key, &rule_text)?;
    println!(
        "Recorded observe-only Munin rule candidate. Enforcement is not active yet; `munin promote` only records the candidate today."
    );
    Ok(())
}

fn short_sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..12].to_string()
}

struct InstallOptions {
    claude: bool,
    codex: bool,
    dry_run: bool,
    force: bool,
    keep_legacy: bool,
    check_resolvable: bool,
}

#[derive(Debug, Clone, Copy)]
struct InstallQuickSkill {
    name: &'static str,
    description: &'static str,
    when_to_use: &'static str,
    primary_command: &'static str,
    how_to_read_output: &'static str,
    done: &'static str,
}

const INSTALL_QUICK_SKILLS: &[InstallQuickSkill] = &[
    InstallQuickSkill {
        name: "munin-install-check",
        description: "Validate Munin's generated skills and resolver targets before installing.",
        when_to_use: "Use when demonstrating or verifying that a fresh Munin binary can generate parseable Codex and Claude skills before writing files.",
        primary_command: "munin install --check-resolvable",
        how_to_read_output: "Success prints the number of resolver, skill, and fixture checks that passed. Any failure means the installer contract is broken and should be fixed before recording or shipping.",
        done: "The install contract check passes, and the output names resolver, skill, and fixture checks.",
    },
    InstallQuickSkill {
        name: "munin-install-preview",
        description: "Preview the Codex skill and plugin files Munin will install.",
        when_to_use: "Use when you want a recording-friendly dry run that shows the planned Codex skill/plugin writes without changing the target profile.",
        primary_command: "munin install --codex --dry-run",
        how_to_read_output: "The output should list `would write` paths, `Codex skill` paths, and `Codex plugin skill` paths, then finish with the dry-run write count.",
        done: "The dry run shows the planned Codex skills/plugin files and reports planned writes without modifying files.",
    },
    InstallQuickSkill {
        name: "munin-install-codex",
        description: "Install or refresh Munin's Codex skills and plugin assets.",
        when_to_use: "Use after previewing the install when you are ready to place Munin's Codex skills and plugin files into the active user profile.",
        primary_command: "munin install --codex --force",
        how_to_read_output: "The output should list written Codex skill/plugin paths and end with the number of files written or refreshed.",
        done: "The Codex skill/plugin files are written, and a new Codex session can invoke the installed Munin skills by name.",
    },
    InstallQuickSkill {
        name: "munin-install-claude-preview",
        description: "Preview the Claude skills and slash commands Munin will install.",
        when_to_use: "Use when you want a recording-friendly dry run that shows the planned Claude skill and slash-command writes without changing the target profile.",
        primary_command: "munin install --claude --dry-run",
        how_to_read_output: "The output should list `would write` paths, `Claude skill` paths, and `Claude command` paths, then finish with the dry-run write count.",
        done: "The dry run shows the planned Claude skills and slash commands without modifying files.",
    },
    InstallQuickSkill {
        name: "munin-install-claude",
        description: "Install or refresh Munin's Claude skills and slash commands.",
        when_to_use: "Use after previewing the Claude install when you are ready to place Munin's Claude skills and slash commands into the active user profile.",
        primary_command: "munin install --claude --force",
        how_to_read_output: "The output should list written Claude skill and command paths and end with the number of files written or refreshed.",
        done: "The Claude skill and slash-command files are written, and a new Claude session can invoke Munin via `/munin-*` commands.",
    },
    InstallQuickSkill {
        name: "munin-memory-os-ingest",
        description: "Ingest local agent sessions into Memory OS and show timing.",
        when_to_use: "Use in demos or fresh-install verification when you want to show Munin importing session data into the local Memory OS store.",
        primary_command: "munin memory-os ingest --force --format text",
        how_to_read_output: "The output shows elapsed time plus sessions, shell executions, and corrections imported. The recording command uses `--force` so repeated takes still show timing and corpus counts.",
        done: "The run reports ingestion timing and either imported session counts or a clear already-current result.",
    },
    InstallQuickSkill {
        name: "munin-proactive",
        description: "Run Munin's morning strategic proactivity evaluation on demand.",
        when_to_use: "Use when the user asks to invoke proactivity manually, kick off a morning proactivity cycle now, or see the latest strategic next-move recommendations without waiting for the scheduled 8am task.",
        primary_command: "munin proactivity run --no-spawn --format text",
        how_to_read_output: "The output is the recommendation report: strategic nudges with confidence, continuity tasks, any queued job id, and warnings. Because `--no-spawn` is set, no follow-up session is launched; the queued brief uses the configured provider unless the command includes `--provider codex` or `--provider claude`. The user can approve the queued brief later with `munin proactivity approve <job-id>`.",
        done: "The run reports one or more nudges (or a clean no-action state), names any queued job id, and confirms no session was spawned.",
    },
];

fn run_install(options: InstallOptions) -> Result<i32> {
    if options.check_resolvable {
        run_check_resolvable()?;
        return Ok(0);
    }
    if options.force || options.keep_legacy {
        eprintln!(
            "warning: --force and --keep-legacy are compatibility flags; native host managers own plugin replacement"
        );
    }
    run_agent_plugin_request(
        agent_plugin::LifecycleAction::Install,
        options.codex,
        options.claude,
        options.dry_run,
        agent_plugin::AgentPluginFormat::Text,
    )
}

fn run_agent_plugin(command: AgentPluginCommands) -> Result<i32> {
    match command {
        AgentPluginCommands::Install {
            codex,
            claude,
            dry_run,
            format,
        } => run_agent_plugin_request(
            agent_plugin::LifecycleAction::Install,
            codex,
            claude,
            dry_run,
            format,
        ),
        AgentPluginCommands::Verify {
            codex,
            claude,
            format,
        } => run_agent_plugin_request(
            agent_plugin::LifecycleAction::Verify,
            codex,
            claude,
            false,
            format,
        ),
        AgentPluginCommands::Uninstall {
            codex,
            claude,
            dry_run,
            format,
        } => run_agent_plugin_request(
            agent_plugin::LifecycleAction::Uninstall,
            codex,
            claude,
            dry_run,
            format,
        ),
    }
}

fn run_agent_plugin_request(
    action: agent_plugin::LifecycleAction,
    codex: bool,
    claude: bool,
    dry_run: bool,
    format: agent_plugin::AgentPluginFormat,
) -> Result<i32> {
    let outcome = agent_plugin::run_lifecycle(agent_plugin::LifecycleRequest {
        action,
        codex,
        claude,
        dry_run,
    })?;
    agent_plugin::render_lifecycle(&outcome, format)?;
    Ok(outcome.exit_code)
}

fn run_check_resolvable() -> Result<()> {
    let mut checked = 0usize;
    checked += agent_plugin::validate_embedded_bundle()?;
    for skill in INSTALL_QUICK_SKILLS {
        validate_munin_command(skill.primary_command).with_context(|| {
            format!(
                "install quick skill `{}` command is not resolvable",
                skill.name
            )
        })?;
        let rendered = render_install_quick_skill(skill);
        assert_install_quick_skill_contract(skill.name, &rendered, skill)?;
        checked += 2;
    }
    for rule in core::access_layer::intent_rules::INTENT_RULES {
        let command = rule
            .primary_command
            .replace("<query>", "resolver decisions");
        validate_munin_command(command.as_str())
            .with_context(|| format!("skill `{}` command is not resolvable", rule.skill_name))?;
        checked += 1;
        if let Some(fallback) = rule.fallback_command {
            if fallback.command.starts_with("munin ") {
                validate_munin_command(
                    fallback
                        .command
                        .replace("<query>", "resolver decisions")
                        .as_str(),
                )
                .with_context(|| format!("fallback for `{}` is not resolvable", rule.skill_name))?;
                checked += 1;
            } else if fallback.command != "qmd \"<query>\"" {
                anyhow::bail!(
                    "external fallback for `{}` must be an explicit raw archive fallback",
                    rule.skill_name
                );
            }
        }
        let rendered = render_narrow_skill(rule);
        assert_skill_contract(rule.skill_name, &rendered, rule)?;
        if core::access_layer::intent_rules::intent_by_skill_name(rule.skill_name).is_none() {
            anyhow::bail!("intent registry cannot look up skill `{}`", rule.skill_name);
        }
        checked += 1;
    }
    let umbrella = render_umbrella_skill();
    if !umbrella.contains("munin resolve") || !umbrella.contains("narrow skill") {
        anyhow::bail!("umbrella skill is missing resolver flow");
    }
    checked += 1;
    for command in core::resolver::known_resolver_commands() {
        if command.starts_with("munin ") {
            validate_munin_command(command.replace("<query>", "resolver decisions").as_str())
                .with_context(|| format!("resolver target `{command}` is not resolvable"))?;
        }
        checked += 1;
    }
    checked += validate_resolver_trigger_fixtures()?;
    for invalid in [
        "munin nudge --format xml",
        "munin recall --format prompt resolver",
        "munin resolve --format yaml resolver",
    ] {
        if validate_munin_command(invalid).is_ok() {
            anyhow::bail!("invalid command unexpectedly parsed: {invalid}");
        }
        checked += 1;
    }
    println!("install check-resolvable: {checked} resolver, skill, and fixture checks passed");
    Ok(())
}

fn assert_install_quick_skill_contract(
    skill_name: &str,
    rendered: &str,
    skill: &InstallQuickSkill,
) -> Result<()> {
    for section in [
        "## When to use",
        "## Primary command",
        "## How to read output",
        "## Trust",
        "## Done",
    ] {
        if !rendered.contains(section) {
            anyhow::bail!(
                "generated install quick skill `{skill_name}` is missing section `{section}`"
            );
        }
    }
    if !rendered.contains(skill.primary_command) {
        anyhow::bail!("generated install quick skill `{skill_name}` is missing its command");
    }
    Ok(())
}

fn assert_skill_contract(
    skill_name: &str,
    rendered: &str,
    rule: &core::access_layer::intent_rules::IntentRule,
) -> Result<()> {
    for section in [
        "## When to use",
        "## Primary command",
        "## How to read output",
        "## Trust",
        "## Fallback",
        "## What not to do",
        "## Done",
    ] {
        if !rendered.contains(section) {
            anyhow::bail!("generated skill `{skill_name}` is missing section `{section}`");
        }
    }
    for required in rule
        .output_contract
        .iter()
        .chain(rule.trust_rules)
        .chain(rule.fallback_rules)
        .chain(rule.done_criteria)
        .chain(rule.what_not_to_do)
    {
        if !rendered.contains(required) {
            anyhow::bail!("generated skill `{skill_name}` is missing rule `{required}`");
        }
    }
    Ok(())
}

fn validate_munin_command(command: &str) -> Result<()> {
    if command.contains("--format xml")
        || command.contains("--format yaml")
        || command.contains("recall --format prompt")
    {
        anyhow::bail!("unsupported runtime format in `{command}`");
    }
    let mut args = command_line_words(command);
    if args.first().map(|value| value.as_str()) != Some("munin") {
        anyhow::bail!("command must start with munin: {command}");
    }
    args.remove(0);
    let mut argv = vec!["munin".to_string()];
    argv.extend(args);
    match Cli::try_parse_from(argv) {
        Ok(_) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("failed to parse `{command}`")),
    }
}

#[derive(Debug, Deserialize)]
struct ResolverTriggerFixture {
    route: String,
    source_status: Option<String>,
    triggers: Vec<String>,
    negative_triggers: Vec<String>,
}

const RESOLVER_TRIGGER_FIXTURES: &[(&str, &str)] = &[
    (
        "brain.json",
        include_str!("../../tests/fixtures/resolver_triggers/brain.json"),
    ),
    (
        "doctor.json",
        include_str!("../../tests/fixtures/resolver_triggers/doctor.json"),
    ),
    (
        "friction.json",
        include_str!("../../tests/fixtures/resolver_triggers/friction.json"),
    ),
    (
        "hygiene.json",
        include_str!("../../tests/fixtures/resolver_triggers/hygiene.json"),
    ),
    (
        "nudge.json",
        include_str!("../../tests/fixtures/resolver_triggers/nudge.json"),
    ),
    (
        "prove.json",
        include_str!("../../tests/fixtures/resolver_triggers/prove.json"),
    ),
    (
        "recall.json",
        include_str!("../../tests/fixtures/resolver_triggers/recall.json"),
    ),
    (
        "resume.json",
        include_str!("../../tests/fixtures/resolver_triggers/resume.json"),
    ),
];

fn validate_resolver_trigger_fixtures() -> Result<usize> {
    let mut checked = 0usize;
    let mut fixture_routes = std::collections::BTreeSet::new();
    for (fixture_name, content) in RESOLVER_TRIGGER_FIXTURES {
        let fixture: ResolverTriggerFixture = serde_json::from_str(content)
            .with_context(|| format!("failed to parse fixture {}", fixture_name))?;
        let rule = core::access_layer::intent_rules::INTENT_RULES
            .iter()
            .find(|rule| rule.route == fixture.route)
            .with_context(|| format!("fixture {} does not match an intent rule", fixture_name))?;
        fixture_routes.insert(fixture.route.clone());
        if fixture.triggers.len() < 5 || fixture.negative_triggers.len() < 2 {
            anyhow::bail!(
                "fixture {} needs at least five positive and two negative triggers",
                fixture_name
            );
        }
        for query in fixture.triggers {
            let report = core::resolver::resolve_with_source_status(
                query.as_str(),
                fixture.source_status.as_deref(),
            );
            if report.route != fixture.route {
                anyhow::bail!(
                    "fixture {} trigger `{}` routed to `{}` instead of `{}`",
                    fixture_name,
                    query,
                    report.route,
                    fixture.route
                );
            }
            if report.command.starts_with("munin ") {
                validate_munin_command(&report.command).with_context(|| {
                    format!("fixture {} command is not resolvable", fixture_name)
                })?;
            }
            checked += 1;
        }
        for query in fixture.negative_triggers {
            let report = core::resolver::resolve_with_source_status(
                query.as_str(),
                fixture.source_status.as_deref(),
            );
            if report.route == fixture.route {
                anyhow::bail!(
                    "fixture {} negative trigger `{}` unexpectedly routed to `{}`",
                    fixture_name,
                    query,
                    fixture.route
                );
            }
            checked += 1;
        }
        checked += rule.trigger_phrases.len().min(1);
    }
    for rule in core::access_layer::intent_rules::INTENT_RULES {
        if !fixture_routes.contains(rule.route) {
            anyhow::bail!("resolver trigger fixture missing route `{}`", rule.route);
        }
    }
    if checked < 50 {
        anyhow::bail!("resolver trigger coverage too low: {checked} assertions");
    }
    Ok(checked)
}

fn command_line_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in command.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            ' ' | '\t' if !in_quote => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn render_umbrella_skill() -> String {
    let umbrella = core::access_layer::intent_rules::UMBRELLA_SKILL;
    let routes = core::access_layer::intent_rules::INTENT_RULES
        .iter()
        .map(|rule| rule.route)
        .collect::<Vec<_>>()
        .join(", ");
    render_template(
        include_str!("../assets/integrations/v1/templates/umbrella-skill.md.tmpl"),
        &[
            ("__NAME__", umbrella.name.to_string()),
            ("__DESCRIPTION__", umbrella.description.to_string()),
            (
                "__DESCRIPTION_EXPANDED__",
                umbrella.description_expanded.to_string(),
            ),
            ("__ROUTES__", routes),
        ],
    )
}

fn render_install_quick_skill(skill: &InstallQuickSkill) -> String {
    render_template(
        include_str!("../assets/integrations/v1/templates/install-quick-skill.md.tmpl"),
        &[
            ("__NAME__", skill.name.to_string()),
            ("__DESCRIPTION__", skill.description.to_string()),
            ("__WHEN_TO_USE__", skill.when_to_use.to_string()),
            ("__PRIMARY_COMMAND__", skill.primary_command.to_string()),
            (
                "__HOW_TO_READ_OUTPUT__",
                skill.how_to_read_output.to_string(),
            ),
            ("__DONE__", skill.done.to_string()),
        ],
    )
}

fn render_narrow_skill(rule: &core::access_layer::intent_rules::IntentRule) -> String {
    let fallback_command = rule
        .fallback_command
        .map(|fallback| {
            format!(
                "\nFallback command:\n\n```powershell\n{}\n```",
                fallback.command
            )
        })
        .unwrap_or_default();
    render_template(
        include_str!("../assets/integrations/v1/templates/narrow-skill.md.tmpl"),
        &[
            ("__SKILL_NAME__", rule.skill_name.to_string()),
            ("__DESCRIPTION__", rule.description.to_string()),
            (
                "__DESCRIPTION_EXPANDED__",
                rule.description_expanded.to_string(),
            ),
            ("__PRIMARY_COMMAND__", rule.primary_command.to_string()),
            ("__FALLBACK_BLOCK__", fallback_command),
            ("__OUTPUT_CONTRACT__", render_bullets(rule.output_contract)),
            ("__TRUST_RULES__", render_bullets(rule.trust_rules)),
            ("__FALLBACK_RULES__", render_bullets(rule.fallback_rules)),
            ("__WHAT_NOT_TO_DO__", render_bullets(rule.what_not_to_do)),
            ("__DONE_CRITERIA__", render_bullets(rule.done_criteria)),
        ],
    )
}

fn render_bullets(items: &[&str]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_template(template: &str, replacements: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (needle, value) in replacements {
        rendered = rendered.replace(needle, value);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(args: &[&str]) {
        Cli::try_parse_from(args).unwrap_or_else(|err| panic!("{args:?} failed: {err}"));
    }

    #[test]
    fn public_munin_commands_parse() {
        parse_ok(&["munin", "resume", "--format", "prompt"]);
        parse_ok(&["munin", "resume", "--project", "C:/repo"]);
        parse_ok(&["munin", "brain", "--format", "prompt"]);
        parse_ok(&[
            "munin",
            "runtime",
            "--surface",
            "brain",
            "--format",
            "prompt",
        ]);
        parse_ok(&[
            "munin",
            "runtime",
            "--surface",
            "auto",
            "--format",
            "prompt",
        ]);
        parse_ok(&[
            "munin",
            "runtime",
            "--surface",
            "resume",
            "--scope",
            "user",
            "--format",
            "json",
        ]);
        parse_ok(&["munin", "nudge", "--scope", "example-business"]);
        parse_ok(&["munin", "prove", "--last-resume"]);
        parse_ok(&["munin", "friction", "--agent", "codex", "--last", "30d"]);
        parse_ok(&["munin", "promote", "use bun, not npm"]);
        parse_ok(&["munin", "recall", "refund SLA"]);
        parse_ok(&["munin", "show", "@munin/a_deadbeefdeadbeef"]);
        parse_ok(&[
            "munin",
            "show",
            "@context/a_deadbeefdeadbeef",
            "--lines",
            "1:20",
        ]);
        parse_ok(&[
            "munin",
            "diff",
            "@munin/a_deadbeefdeadbeef",
            "@munin/a_feedfacefeedface",
        ]);
        parse_ok(&["munin", "install", "--dry-run"]);
        parse_ok(&["munin", "install", "--claude", "--force"]);
        parse_ok(&["munin", "install", "--keep-legacy"]);
        parse_ok(&["munin", "install", "--codex", "--dry-run"]);
        parse_ok(&["munin", "install", "--check-resolvable"]);
        parse_ok(&["munin", "resolve", "what", "keeps", "going", "wrong"]);
        parse_ok(&["munin", "metrics", "get", "--scope", "example-business"]);
        parse_ok(&["munin", "hygiene"]);
        parse_ok(&["munin", "hygiene", "--root", ".", "--include-codex"]);
        parse_ok(&[
            "munin",
            "proactivity",
            "run",
            "--scope",
            "example-business",
            "--provider",
            "claude",
            "--auto-spawn",
            "--dry-run",
        ]);
        parse_ok(&["munin", "proactivity", "sweep"]);
        parse_ok(&["munin", "proactivity", "status"]);
        parse_ok(&[
            "munin",
            "proactivity",
            "schedule-install",
            "--scope",
            "example-business",
            "--provider",
            "codex",
            "--project-path",
            ".",
        ]);
        parse_ok(&["munin", "proactivity", "schedule-remove"]);
        parse_ok(&[
            "munin",
            "proactivity",
            "claim",
            "--job-id",
            "morning-example-business-2026-04-19",
        ]);
        parse_ok(&[
            "munin",
            "proactivity",
            "approve",
            "--job-id",
            "morning-example-business-2026-04-19",
            "--no-spawn",
        ]);
        parse_ok(&[
            "munin",
            "proactivity",
            "reject",
            "--job-id",
            "morning-example-business-2026-04-19",
            "--summary",
            "not today",
        ]);
        parse_ok(&[
            "munin",
            "proactivity",
            "complete",
            "--job-id",
            "morning-example-business-2026-04-19",
            "--status",
            "complete",
            "--summary",
            "done",
        ]);
        parse_ok(&[
            "munin",
            "metrics",
            "set",
            "sales.pipeline",
            "12",
            "--scope",
            "example-business",
            "--unit",
            "leads",
        ]);
        parse_ok(&["munin", "doctor", "--scope", "user"]);
        parse_ok(&["munin", "doctor", "--release", "--repo-root", "."]);
    }

    #[test]
    fn munin_friction_skill_matches_golden_contract() {
        let rule = core::access_layer::intent_rules::intent_by_skill_name("munin-friction")
            .expect("friction rule");
        let rendered = render_narrow_skill(rule);
        assert_eq!(
            rendered.replace("\r\n", "\n").trim_end(),
            include_str!("../../tests/golden/munin_friction_skill.md")
                .replace("\r\n", "\n")
                .trim_end()
        );
    }

    #[test]
    fn install_quick_skills_render_parseable_commands() {
        let mut names = std::collections::BTreeSet::new();
        for skill in INSTALL_QUICK_SKILLS {
            assert!(
                names.insert(skill.name),
                "duplicate install quick skill {}",
                skill.name
            );
            validate_munin_command(skill.primary_command)
                .unwrap_or_else(|err| panic!("{} did not parse: {err}", skill.primary_command));
            let rendered = render_install_quick_skill(skill);
            assert_install_quick_skill_contract(skill.name, &rendered, skill)
                .unwrap_or_else(|err| panic!("{} contract failed: {err}", skill.name));
        }
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn access_layer_registry_has_unique_routes_and_skill_names() {
        let mut routes = std::collections::BTreeSet::new();
        let mut skills = std::collections::BTreeSet::new();
        for rule in core::access_layer::intent_rules::INTENT_RULES {
            assert!(routes.insert(rule.route), "duplicate route {}", rule.route);
            assert!(
                skills.insert(rule.skill_name),
                "duplicate skill {}",
                rule.skill_name
            );
            assert!(!rule.trigger_phrases.is_empty());
            assert!(!rule.output_contract.is_empty());
            assert!(!rule.trust_rules.is_empty());
            assert!(!rule.fallback_rules.is_empty());
            assert!(!rule.done_criteria.is_empty());
            assert!(!rule.what_not_to_do.is_empty());
        }
        assert_eq!(routes.len(), 8);
    }

    #[test]
    fn memory_os_subcommands_parse() {
        parse_ok(&["munin", "memory-os", "brief"]);
        parse_ok(&["munin", "memory-os", "ingest"]);
        parse_ok(&["munin", "memory-os", "ingest", "--force"]);
        parse_ok(&["munin", "memory-os", "ingest", "--check-now"]);
        parse_ok(&["munin", "memory-os", "ingest", "--format", "json"]);
        parse_ok(&["munin", "memory-os", "overview"]);
        parse_ok(&["munin", "memory-os", "doctor"]);
        parse_ok(&["munin", "memory-os", "friction"]);
        parse_ok(&["munin", "memory-os", "promotion"]);
    }

    #[test]
    fn umbrella_skill_does_not_auto_run_for_bare_invocation() {
        let content = render_umbrella_skill();
        assert!(content.contains("only invoked Munin"));
        assert!(content.contains("Do not run recall, doctor, proactivity"));
    }

    #[test]
    fn release_doctor_rejects_stale_session_brain_statuses() {
        assert!(!session_brain_source_is_release_safe("stale"));
        assert!(!session_brain_source_is_release_safe("stale-fallback"));
        assert!(session_brain_source_is_release_safe("fallback-latest"));
        assert!(session_brain_source_is_release_safe("live"));
    }

    #[test]
    fn public_doc_command_extractor_handles_markdown_and_html() {
        let markdown = [
            "Run `munin doctor --scope user --format text` before shipping.",
            "$ munin recall --format text \"resolver decisions\" # explain recall",
        ]
        .join("\n");
        let commands = extract_public_munin_commands(&markdown, false);
        assert_eq!(
            commands,
            vec![
                "munin doctor --scope user --format text".to_string(),
                "munin recall --format text \"resolver decisions\"".to_string(),
            ]
        );

        let html = r#"
          <pre><code><span class="c-prompt">$</span> <span class="c-cmd">munin promote &quot;use bun&quot;</span>
          <span class="c-prompt">$</span> <span class="c-cmd">munin memory-os ingest --format text</span></code></pre>
        "#;
        let commands = extract_public_munin_commands(html, true);
        assert_eq!(
            commands,
            vec![
                "munin promote \"use bun\"".to_string(),
                "munin memory-os ingest --format text".to_string(),
            ]
        );
    }

    #[test]
    fn public_docs_parity_aggregates_banned_and_unresolvable_commands() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        fs::write(
            temp.path().join("README.md"),
            [
                "# Test",
                "```",
                "munin init",
                "munin recall --format prompt resolver",
                "```",
            ]
            .join("\n"),
        )
        .expect("write README");

        let error = assert_public_docs_parity(temp.path(), None)
            .expect_err("unsupported public docs should fail")
            .to_string();
        assert!(error.contains("munin init"));
        assert!(error.contains("munin recall --format prompt resolver"));
    }
}
