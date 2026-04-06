mod runtime;

use crate::runtime::state_store::StateStore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug, Deserialize)]
struct Config {
    defaults: Defaults,
    runtime: RuntimeConfig,
    workers: BTreeMap<String, WorkerConfig>,
}

#[derive(Debug, Deserialize)]
struct Defaults {
    idle_timeout_ms: i64,
    summary_only: bool,
    max_parallel: i64,
}

#[derive(Debug, Deserialize)]
struct RuntimeConfig {
    transport: TransportConfig,
    #[serde(rename = "loop")]
    loop_config: LoopConfig,
    memory: MemoryConfig,
    workers: WorkerRuntimeConfig,
}

#[derive(Debug, Deserialize)]
struct TransportConfig {
    mode: String,
    preferred: Vec<String>,
    allow_tmux_fallback: bool,
}

#[derive(Debug, Deserialize)]
struct LoopConfig {
    persist_runs: bool,
    resume_strategy: String,
}

#[derive(Debug, Deserialize)]
struct MemoryConfig {
    enabled: bool,
    ttl_hours: i64,
    invalidate_on_git_head_change: bool,
}

#[derive(Debug, Deserialize)]
struct WorkerRuntimeConfig {
    max_workers: i64,
    spawn_policy: String,
    continue_policy: String,
}

#[derive(Debug, Deserialize)]
struct WorkerConfig {
    cli: String,
    model: String,
    reasoning: Option<String>,
    description: String,
}

#[derive(Debug, Serialize)]
struct StatusPayload {
    config_path: String,
    transport: serde_json::Value,
    defaults: serde_json::Value,
    workers: serde_json::Value,
    worker_types: Vec<String>,
    ok: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    let result = match cmd {
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "version" | "-v" | "--version" => {
            println!("conductor {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "config-path" => run_config_path(),
        "status" => run_status(),
        "doctor" => run_doctor(),
        "runtime-init" => run_runtime_init(&args[2..]),
        "runtime-snapshot" => run_runtime_snapshot(&args[2..]),
        _ => {
            print_help();
            Err("unknown command".to_string())
        }
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run_config_path() -> Result<(), String> {
    let path = resolve_config_path()?;
    println!("{}", path.display());
    Ok(())
}

fn run_status() -> Result<(), String> {
    let (path, cfg) = load_resolved_config()?;
    let payload = StatusPayload {
        config_path: path.display().to_string(),
        transport: json!({
            "mode": cfg.runtime.transport.mode,
            "preferred": cfg.runtime.transport.preferred,
            "allow_tmux_fallback": cfg.runtime.transport.allow_tmux_fallback
        }),
        defaults: json!({
            "idle_timeout_ms": cfg.defaults.idle_timeout_ms,
            "summary_only": cfg.defaults.summary_only,
            "max_parallel": cfg.defaults.max_parallel
        }),
        workers: json!({
            "max_workers": cfg.runtime.workers.max_workers,
            "spawn_policy": cfg.runtime.workers.spawn_policy,
            "continue_policy": cfg.runtime.workers.continue_policy
        }),
        worker_types: cfg.workers.keys().cloned().collect(),
        ok: true,
    };
    print_json(&payload)
}

fn run_doctor() -> Result<(), String> {
    let (path, cfg) = load_resolved_config()?;
    let issues = validate_config(&cfg);
    let payload = json!({
        "config_path": path.display().to_string(),
        "issues": issues,
        "ok": issues.is_empty()
    });
    print_json(&payload)?;
    if issues.is_empty() {
        Ok(())
    } else {
        Err("config validation failed".to_string())
    }
}

fn run_runtime_init(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("run-1");
    let owner = args
        .get(1)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("orchestrator-1");
    let store = StateStore::new(resolve_state_root()?);
    let run = store.init_run(run_id, owner)?;
    print_json(&json!({
        "ok": true,
        "state_dir": store.root().display().to_string(),
        "run": run
    }))
}

fn run_runtime_snapshot(args: &[String]) -> Result<(), String> {
    let run_id = args
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "runtime-snapshot requires <run_id>".to_string())?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = if store
        .root()
        .join("runs")
        .join(run_id)
        .join("snapshot.json")
        .exists()
    {
        store.read_snapshot(run_id)?
    } else {
        store.capture_snapshot(run_id)?
    };
    print_json(&snapshot)
}

fn resolve_config_path() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CONDUCTOR_CONFIG") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    if let Some(path) = find_project_config(&cwd) {
        return Ok(path);
    }

    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(Path::new(&home)
        .join(".conductor-kit")
        .join("conductor.json"))
}

fn resolve_state_root() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CONDUCTOR_STATE_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(env::current_dir()
        .map_err(|err| err.to_string())?
        .join(".conductor"))
}

fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        let candidate = current.join(".conductor-kit").join("conductor.json");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

fn load_resolved_config() -> Result<(PathBuf, Config), String> {
    let path = resolve_config_path()?;
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let cfg = serde_json::from_str::<Config>(&raw).map_err(|err| err.to_string())?;
    Ok((path, cfg))
}

fn validate_config(cfg: &Config) -> Vec<String> {
    let mut issues = Vec::new();

    if cfg.defaults.idle_timeout_ms < 0 {
        issues.push("defaults.idle_timeout_ms must be >= 0".to_string());
    }
    if cfg.defaults.max_parallel < 1 {
        issues.push("defaults.max_parallel must be >= 1".to_string());
    }
    if cfg.runtime.transport.mode != "direct" {
        issues.push("runtime.transport.mode must be direct".to_string());
    }
    if cfg.runtime.transport.preferred.is_empty() {
        issues.push("runtime.transport.preferred must not be empty".to_string());
    }
    if cfg.runtime.transport.allow_tmux_fallback {
        issues.push(
            "runtime.transport.allow_tmux_fallback must remain false in the new baseline"
                .to_string(),
        );
    }
    if !cfg.runtime.loop_config.persist_runs {
        issues.push("runtime.loop.persist_runs must be true".to_string());
    }
    if cfg.runtime.loop_config.resume_strategy != "ledger" {
        issues.push("runtime.loop.resume_strategy must be ledger".to_string());
    }
    if !cfg.runtime.memory.enabled {
        issues.push("runtime.memory.enabled must be true".to_string());
    }
    if cfg.runtime.memory.ttl_hours < 1 {
        issues.push("runtime.memory.ttl_hours must be >= 1".to_string());
    }
    if !cfg.runtime.memory.invalidate_on_git_head_change {
        issues.push("runtime.memory.invalidate_on_git_head_change must be true".to_string());
    }
    if cfg.runtime.workers.max_workers < 1 {
        issues.push("runtime.workers.max_workers must be >= 1".to_string());
    }
    if !matches!(
        cfg.runtime.workers.spawn_policy.as_str(),
        "ephemeral" | "persistent"
    ) {
        issues.push("runtime.workers.spawn_policy must be ephemeral or persistent".to_string());
    }
    if !matches!(
        cfg.runtime.workers.continue_policy.as_str(),
        "resume_when_possible" | "always_new"
    ) {
        issues.push(
            "runtime.workers.continue_policy must be resume_when_possible or always_new"
                .to_string(),
        );
    }
    if cfg.workers.is_empty() {
        issues.push("workers must not be empty".to_string());
    }

    for (name, worker) in &cfg.workers {
        if worker.cli.trim().is_empty() {
            issues.push(format!("workers.{name}.cli is required"));
        }
        if worker.model.trim().is_empty() {
            issues.push(format!("workers.{name}.model is required"));
        }
        if worker.description.trim().is_empty() {
            issues.push(format!("workers.{name}.description is required"));
        }
        if let Some(reasoning) = &worker.reasoning {
            if !matches!(reasoning.as_str(), "low" | "medium" | "high") {
                issues.push(format!(
                    "workers.{name}.reasoning must be low, medium, or high"
                ));
            }
        }
    }

    issues
}

fn print_help() {
    println!(
        "\
conductor <command>

Commands:
  help                Show this help
  version             Print version
  config-path         Print resolved config path
  status              Print config status payload
  doctor              Validate config
  runtime-init        Initialize runtime state for a run
  runtime-snapshot    Print runtime snapshot for a run
"
    );
}

fn print_json<T>(value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let rendered = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    println!("{rendered}");
    Ok(())
}
