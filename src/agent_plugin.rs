use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PLUGIN_ID: &str = "munin-memory@munin-memory";
pub const MARKETPLACE_NAME: &str = "munin-memory";
pub const RECEIPT_SCHEMA: &str = "munin.agent-plugin-receipt.v1";

const BUNDLE_FILES: &[(&str, &str)] = &[
    (
        ".agents/plugins/marketplace.json",
        include_str!("../.agents/plugins/marketplace.json"),
    ),
    (
        ".claude-plugin/marketplace.json",
        include_str!("../.claude-plugin/marketplace.json"),
    ),
    (
        "plugins/munin-memory/.codex-plugin/plugin.json",
        include_str!("../plugins/munin-memory/.codex-plugin/plugin.json"),
    ),
    (
        "plugins/munin-memory/.claude-plugin/plugin.json",
        include_str!("../plugins/munin-memory/.claude-plugin/plugin.json"),
    ),
    (
        "plugins/munin-memory/skills/munin/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-brain/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-brain/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-doctor/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-doctor/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-friction/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-friction/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-hygiene/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-hygiene/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-nudge/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-nudge/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-prove/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-prove/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-recall/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-recall/SKILL.md"),
    ),
    (
        "plugins/munin-memory/skills/munin-resume/SKILL.md",
        include_str!("../plugins/munin-memory/skills/munin-resume/SKILL.md"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AgentPluginFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentHost {
    Codex,
    Claude,
}

impl AgentHost {
    fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn binary_env(self) -> &'static str {
        match self {
            Self::Codex => "MUNIN_CODEX_BIN",
            Self::Claude => "MUNIN_CLAUDE_BIN",
        }
    }

    fn default_binary(self) -> &'static str {
        self.label()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleAction {
    Install,
    Verify,
    Uninstall,
}

impl LifecycleAction {
    fn label(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Verify => "verify",
            Self::Uninstall => "uninstall",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleRequest {
    pub action: LifecycleAction,
    pub codex: bool,
    pub claude: bool,
    pub dry_run: bool,
}

impl LifecycleRequest {
    fn hosts(&self) -> Vec<AgentHost> {
        let both = !self.codex && !self.claude;
        let mut hosts = Vec::new();
        if self.codex || both {
            hosts.push(AgentHost::Codex);
        }
        if self.claude || both {
            hosts.push(AgentHost::Claude);
        }
        hosts
    }
}

#[derive(Debug, Serialize)]
pub struct LifecycleOutcome {
    #[serde(flatten)]
    pub receipt: LifecycleReceipt,
    #[serde(skip)]
    pub exit_code: i32,
}

#[derive(Debug, Serialize)]
pub struct LifecycleReceipt {
    pub schema: &'static str,
    pub status: &'static str,
    pub action: LifecycleAction,
    pub generated_at: String,
    pub plugin_id: &'static str,
    pub plugin_version: &'static str,
    pub bundle_path: String,
    pub bundle_created: bool,
    pub dry_run: bool,
    pub files: Vec<BundleFileReceipt>,
    pub hosts: Vec<HostReceipt>,
    pub rollback_actions: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BundleFileReceipt {
    pub path: &'static str,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NativeState {
    pub marketplace_present: bool,
    pub plugin_present: bool,
    pub installed_version: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct HostReceipt {
    pub host: AgentHost,
    pub binary: String,
    pub pre_existing: NativeState,
    pub final_state: NativeState,
    pub created_marketplace: bool,
    pub created_plugin: bool,
    pub commands: Vec<CommandReceipt>,
}

#[derive(Debug, Serialize)]
pub struct CommandReceipt {
    pub purpose: String,
    pub argv: Vec<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug)]
struct HostContext {
    host: AgentHost,
    binary: PathBuf,
    child_env: Vec<(&'static str, OsString)>,
}

#[derive(Debug, Default)]
struct OwnershipChange {
    marketplace: bool,
    plugin: bool,
}

struct NativeOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

pub fn run_lifecycle(request: LifecycleRequest) -> Result<LifecycleOutcome> {
    let install_home = install_home_dir()?;
    let bundle_path = install_home
        .join(".munin")
        .join("plugin-marketplaces")
        .join(env!("CARGO_PKG_VERSION"));
    let contexts = request
        .hosts()
        .into_iter()
        .map(|host| resolve_host(host, &install_home))
        .collect::<Result<Vec<_>>>()?;

    let mut receipt = LifecycleReceipt {
        schema: RECEIPT_SCHEMA,
        status: "pass",
        action: request.action,
        generated_at: Utc::now().to_rfc3339(),
        plugin_id: PLUGIN_ID,
        plugin_version: env!("CARGO_PKG_VERSION"),
        bundle_path: bundle_path.display().to_string(),
        bundle_created: false,
        dry_run: request.dry_run,
        files: bundle_receipts(),
        hosts: Vec::new(),
        rollback_actions: Vec::new(),
        warnings: Vec::new(),
    };

    if request.action == LifecycleAction::Install && !request.dry_run {
        receipt.bundle_created = materialize_bundle(&bundle_path)?;
    }

    let mut changes = Vec::<OwnershipChange>::new();
    let mut failed = false;
    for context in &contexts {
        let mut host_receipt = HostReceipt {
            host: context.host,
            binary: context.binary.display().to_string(),
            pre_existing: NativeState::default(),
            final_state: NativeState::default(),
            created_marketplace: false,
            created_plugin: false,
            commands: Vec::new(),
        };
        let mut change = OwnershipChange::default();
        let result = match request.action {
            LifecycleAction::Install => install_host(
                context,
                &bundle_path,
                request.dry_run,
                &mut change,
                &mut host_receipt,
            ),
            LifecycleAction::Verify => verify_host(context, &mut host_receipt),
            LifecycleAction::Uninstall => {
                uninstall_host(context, request.dry_run, &mut host_receipt)
            }
        };
        host_receipt.created_marketplace = change.marketplace;
        host_receipt.created_plugin = change.plugin;
        changes.push(change);
        if let Err(error) = result {
            receipt.status = "fail";
            receipt
                .warnings
                .push(format!("{}: {error:#}", context.host.label()));
            receipt.hosts.push(host_receipt);
            failed = true;
            break;
        }
        receipt.hosts.push(host_receipt);
    }

    if failed && request.action == LifecycleAction::Install && !request.dry_run {
        rollback_created(&contexts, &changes, &mut receipt);
        if receipt.bundle_created {
            match fs::remove_dir_all(&bundle_path) {
                Ok(()) => receipt
                    .rollback_actions
                    .push(format!("removed bundle {}", bundle_path.display())),
                Err(error) => receipt.warnings.push(format!(
                    "failed to remove transaction bundle {}: {error}",
                    bundle_path.display()
                )),
            }
        }
    }

    let exit_code = if failed { 1 } else { 0 };
    if !request.dry_run {
        let receipt_name = if failed {
            format!("plugin-{}-last-failure.json", request.action.label())
        } else {
            format!("plugin-{}-receipt.json", request.action.label())
        };
        write_receipt(&install_home.join(".munin").join(receipt_name), &receipt)?;
    }

    Ok(LifecycleOutcome { receipt, exit_code })
}

pub fn render_lifecycle(outcome: &LifecycleOutcome, format: AgentPluginFormat) -> Result<()> {
    match format {
        AgentPluginFormat::Json => println!("{}", serde_json::to_string_pretty(outcome)?),
        AgentPluginFormat::Text => {
            println!(
                "Munin plugin {}: {} ({} {})",
                outcome.receipt.action.label(),
                outcome.receipt.status,
                outcome.receipt.plugin_id,
                outcome.receipt.plugin_version
            );
            println!("Bundle: {}", outcome.receipt.bundle_path);
            for host in &outcome.receipt.hosts {
                println!(
                    "{}: installed={}, version={}, enabled={}",
                    host.host.label(),
                    host.final_state.plugin_present,
                    host.final_state
                        .installed_version
                        .as_deref()
                        .unwrap_or("none"),
                    host.final_state
                        .enabled
                        .map(|value| value.to_string())
                        .as_deref()
                        .unwrap_or("unknown")
                );
            }
            for warning in &outcome.receipt.warnings {
                eprintln!("warning: {warning}");
            }
        }
    }
    Ok(())
}

fn install_home_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("MUNIN_INSTALL_HOME") {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir().ok_or_else(|| anyhow!("could not determine the user home directory"))
}

fn resolve_host(host: AgentHost, install_home: &Path) -> Result<HostContext> {
    let requested =
        env::var_os(host.binary_env()).unwrap_or_else(|| OsString::from(host.default_binary()));
    let binary = which::which(&requested).with_context(|| {
        format!(
            "{} host CLI was not found (set {} to an explicit executable)",
            host.label(),
            host.binary_env()
        )
    })?;
    let mut child_env = Vec::new();
    if env::var_os("MUNIN_INSTALL_HOME").is_some() {
        match host {
            AgentHost::Codex => child_env.push(("CODEX_HOME", install_home.join(".codex").into())),
            AgentHost::Claude => {
                child_env.push(("CLAUDE_CONFIG_DIR", install_home.join(".claude").into()))
            }
        }
    }
    Ok(HostContext {
        host,
        binary,
        child_env,
    })
}

fn bundle_receipts() -> Vec<BundleFileReceipt> {
    BUNDLE_FILES
        .iter()
        .map(|(path, contents)| BundleFileReceipt {
            path,
            sha256: sha256(contents.as_bytes()),
        })
        .collect()
}

fn materialize_bundle(bundle_path: &Path) -> Result<bool> {
    if bundle_path.exists() {
        validate_bundle(bundle_path)?;
        return Ok(false);
    }
    let parent = bundle_path
        .parent()
        .ok_or_else(|| anyhow!("bundle path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let temp = parent.join(format!(
        ".{}.tmp-{}-{}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        Utc::now().timestamp_millis()
    ));
    fs::create_dir(&temp).with_context(|| format!("failed to create {}", temp.display()))?;
    let write_result = (|| -> Result<()> {
        for (relative, contents) in BUNDLE_FILES {
            let path = temp.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, contents)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        validate_bundle(&temp)?;
        fs::rename(&temp, bundle_path).with_context(|| {
            format!(
                "failed to promote {} to {}",
                temp.display(),
                bundle_path.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() && temp.exists() {
        let _ = fs::remove_dir_all(&temp);
    }
    write_result?;
    Ok(true)
}

fn validate_bundle(root: &Path) -> Result<()> {
    for (relative, expected) in BUNDLE_FILES {
        let path = root.join(relative);
        let actual = fs::read(&path)
            .with_context(|| format!("materialized bundle is missing {}", path.display()))?;
        let expected_hash = sha256(expected.as_bytes());
        let actual_hash = sha256(&actual);
        if actual_hash != expected_hash {
            bail!(
                "materialized bundle hash mismatch for {}: expected {}, found {}",
                relative,
                expected_hash,
                actual_hash
            );
        }
    }
    Ok(())
}

fn install_host(
    context: &HostContext,
    bundle_path: &Path,
    dry_run: bool,
    change: &mut OwnershipChange,
    receipt: &mut HostReceipt,
) -> Result<()> {
    let pre = probe_host(context, receipt)?;
    receipt.pre_existing = pre.clone();
    if pre.plugin_present
        && (pre.installed_version.as_deref() != Some(env!("CARGO_PKG_VERSION"))
            || pre.enabled != Some(true))
    {
        bail!(
            "pre-existing {} is not enabled at version {}",
            PLUGIN_ID,
            env!("CARGO_PKG_VERSION")
        );
    }
    if dry_run {
        receipt.final_state = pre;
        return Ok(());
    }

    if context.host == AgentHost::Claude {
        checked_command(
            context,
            &[
                OsStr::new("plugin"),
                OsStr::new("validate"),
                OsStr::new("--strict"),
                bundle_path.as_os_str(),
            ],
            "validate marketplace",
            receipt,
        )?;
    }
    if !pre.marketplace_present {
        let args = marketplace_add_args(context.host, bundle_path);
        checked_command(context, &args, "add marketplace", receipt)?;
        change.marketplace = true;
    }
    if !pre.plugin_present {
        let args = plugin_add_args(context.host);
        checked_command(context, &args, "install plugin", receipt)?;
        change.plugin = true;
    }
    let final_state = probe_host(context, receipt)?;
    require_healthy_plugin(context.host, &final_state)?;
    receipt.final_state = final_state;
    Ok(())
}

fn verify_host(context: &HostContext, receipt: &mut HostReceipt) -> Result<()> {
    let state = probe_host(context, receipt)?;
    receipt.pre_existing = state.clone();
    receipt.final_state = state.clone();
    require_healthy_plugin(context.host, &state)
}

fn uninstall_host(context: &HostContext, dry_run: bool, receipt: &mut HostReceipt) -> Result<()> {
    let pre = probe_host(context, receipt)?;
    receipt.pre_existing = pre.clone();
    if dry_run {
        receipt.final_state = pre;
        return Ok(());
    }
    if pre.plugin_present {
        let args = plugin_remove_args(context.host);
        checked_command(context, &args, "uninstall plugin", receipt)?;
    }
    if pre.marketplace_present {
        let args = marketplace_remove_args(context.host);
        checked_command(context, &args, "remove marketplace", receipt)?;
    }
    let final_state = probe_host(context, receipt)?;
    if final_state.plugin_present || final_state.marketplace_present {
        bail!("native uninstall left Munin host state behind");
    }
    receipt.final_state = final_state;
    Ok(())
}

fn rollback_created(
    contexts: &[HostContext],
    changes: &[OwnershipChange],
    receipt: &mut LifecycleReceipt,
) {
    for index in (0..changes.len()).rev() {
        let context = &contexts[index];
        let change = &changes[index];
        let Some(host_receipt) = receipt
            .hosts
            .iter_mut()
            .find(|item| item.host == context.host)
        else {
            continue;
        };
        if change.plugin {
            let args = plugin_remove_args(context.host);
            match checked_command(context, &args, "rollback plugin", host_receipt) {
                Ok(()) => receipt
                    .rollback_actions
                    .push(format!("removed {} plugin", context.host.label())),
                Err(error) => receipt.warnings.push(format!(
                    "{} plugin rollback failed: {error:#}",
                    context.host.label()
                )),
            }
        }
        if change.marketplace {
            let args = marketplace_remove_args(context.host);
            match checked_command(context, &args, "rollback marketplace", host_receipt) {
                Ok(()) => receipt
                    .rollback_actions
                    .push(format!("removed {} marketplace", context.host.label())),
                Err(error) => receipt.warnings.push(format!(
                    "{} marketplace rollback failed: {error:#}",
                    context.host.label()
                )),
            }
        }
        if let Ok(state) = probe_host(context, host_receipt) {
            host_receipt.final_state = state;
        }
    }
}

fn probe_host(context: &HostContext, receipt: &mut HostReceipt) -> Result<NativeState> {
    let marketplace = run_native(
        context,
        &marketplace_list_args(context.host),
        "list marketplaces",
        receipt,
    )?;
    ensure_success(context.host, "list marketplaces", &marketplace)?;
    let plugins = run_native(
        context,
        &plugin_list_args(context.host),
        "list plugins",
        receipt,
    )?;
    ensure_success(context.host, "list plugins", &plugins)?;
    let marketplace_json: Value = serde_json::from_str(&marketplace.stdout).with_context(|| {
        format!(
            "{} marketplace list returned invalid JSON",
            context.host.label()
        )
    })?;
    let plugin_json: Value = serde_json::from_str(&plugins.stdout)
        .with_context(|| format!("{} plugin list returned invalid JSON", context.host.label()))?;
    Ok(NativeState {
        marketplace_present: marketplace_present(context.host, &marketplace_json),
        ..plugin_state(context.host, &plugin_json)
    })
}

fn require_healthy_plugin(host: AgentHost, state: &NativeState) -> Result<()> {
    if !state.marketplace_present
        || !state.plugin_present
        || state.installed_version.as_deref() != Some(env!("CARGO_PKG_VERSION"))
        || state.enabled != Some(true)
    {
        bail!(
            "{} did not report {} enabled at version {}",
            host.label(),
            PLUGIN_ID,
            env!("CARGO_PKG_VERSION")
        );
    }
    Ok(())
}

fn marketplace_present(host: AgentHost, value: &Value) -> bool {
    let items = match host {
        AgentHost::Codex => value.get("marketplaces").and_then(Value::as_array),
        AgentHost::Claude => value.as_array(),
    };
    items.is_some_and(|items| {
        items
            .iter()
            .any(|item| item.get("name").and_then(Value::as_str) == Some(MARKETPLACE_NAME))
    })
}

fn plugin_state(host: AgentHost, value: &Value) -> NativeState {
    let items = match host {
        AgentHost::Codex => value.get("installed").and_then(Value::as_array),
        AgentHost::Claude => value.as_array(),
    };
    let item = items.and_then(|items| {
        items.iter().find(|item| {
            let id_key = match host {
                AgentHost::Codex => "pluginId",
                AgentHost::Claude => "id",
            };
            item.get(id_key).and_then(Value::as_str) == Some(PLUGIN_ID)
        })
    });
    NativeState {
        marketplace_present: false,
        plugin_present: item.is_some(),
        installed_version: item
            .and_then(|item| item.get("version"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        enabled: item
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool),
    }
}

fn checked_command(
    context: &HostContext,
    args: &[&OsStr],
    purpose: &str,
    receipt: &mut HostReceipt,
) -> Result<()> {
    let output = run_native(context, args, purpose, receipt)?;
    ensure_success(context.host, purpose, &output)
}

fn run_native(
    context: &HostContext,
    args: &[&OsStr],
    purpose: &str,
    receipt: &mut HostReceipt,
) -> Result<NativeOutput> {
    let mut command = Command::new(&context.binary);
    command.args(args);
    for (key, value) in &context.child_env {
        command.env(key, value);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to run {} for {}", context.binary.display(), purpose))?;
    receipt.commands.push(CommandReceipt {
        purpose: purpose.to_string(),
        argv: args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect(),
        exit_code: output.status.code(),
    });
    Ok(NativeOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn ensure_success(host: AgentHost, purpose: &str, output: &NativeOutput) -> Result<()> {
    if output.success {
        return Ok(());
    }
    bail!(
        "{} failed to {}: {}",
        host.label(),
        purpose,
        if output.stderr.is_empty() {
            "native command returned non-zero"
        } else {
            &output.stderr
        }
    )
}

fn marketplace_list_args(_host: AgentHost) -> Vec<&'static OsStr> {
    vec![os("plugin"), os("marketplace"), os("list"), os("--json")]
}

fn plugin_list_args(_host: AgentHost) -> Vec<&'static OsStr> {
    vec![os("plugin"), os("list"), os("--json")]
}

fn marketplace_add_args<'a>(host: AgentHost, bundle: &'a Path) -> Vec<&'a OsStr> {
    match host {
        AgentHost::Codex => vec![
            os("plugin"),
            os("marketplace"),
            os("add"),
            bundle.as_os_str(),
            os("--json"),
        ],
        AgentHost::Claude => vec![
            os("plugin"),
            os("marketplace"),
            os("add"),
            bundle.as_os_str(),
            os("--scope"),
            os("user"),
        ],
    }
}

fn plugin_add_args(host: AgentHost) -> Vec<&'static OsStr> {
    match host {
        AgentHost::Codex => vec![os("plugin"), os("add"), os(PLUGIN_ID), os("--json")],
        AgentHost::Claude => vec![
            os("plugin"),
            os("install"),
            os(PLUGIN_ID),
            os("--scope"),
            os("user"),
        ],
    }
}

fn plugin_remove_args(host: AgentHost) -> Vec<&'static OsStr> {
    match host {
        AgentHost::Codex => vec![os("plugin"), os("remove"), os(PLUGIN_ID), os("--json")],
        AgentHost::Claude => vec![
            os("plugin"),
            os("uninstall"),
            os(PLUGIN_ID),
            os("--scope"),
            os("user"),
        ],
    }
}

fn marketplace_remove_args(host: AgentHost) -> Vec<&'static OsStr> {
    match host {
        AgentHost::Codex => vec![
            os("plugin"),
            os("marketplace"),
            os("remove"),
            os(MARKETPLACE_NAME),
            os("--json"),
        ],
        AgentHost::Claude => vec![
            os("plugin"),
            os("marketplace"),
            os("remove"),
            os(MARKETPLACE_NAME),
            os("--scope"),
            os("user"),
        ],
    }
}

fn os(value: &'static str) -> &'static OsStr {
    OsStr::new(value)
}

fn write_receipt(path: &Path, receipt: &LifecycleReceipt) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&temp, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("failed to write {}", temp.display()))?;
    fs::rename(&temp, path)
        .with_context(|| format!("failed to promote lifecycle receipt to {}", path.display()))?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
