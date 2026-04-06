use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::runtime::types::{DispatchStatus, RunPhase, WorkerState};

fn default_worker(
    cli: &str,
    model: &str,
    reasoning: Option<&str>,
    description: &str,
) -> WorkerConfig {
    WorkerConfig {
        cli: cli.to_string(),
        model: model.to_string(),
        reasoning: reasoning.map(ToOwned::to_owned),
        description: description.to_string(),
        delivery_mode: Some("session".to_string()),
        launch_mode: Some("stdin_text".to_string()),
        base_args: Some(vec![
            "-m".to_string(),
            "{model}".to_string(),
            "-c".to_string(),
            "model_reasoning_effort=\"{reasoning}\"".to_string(),
        ]),
        env: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub defaults: Defaults,
    pub surface: SurfaceConfig,
    pub runtime: RuntimeConfig,
    pub workers: BTreeMap<String, WorkerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    pub idle_timeout_ms: i64,
    pub summary_only: bool,
    pub max_parallel: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceConfig {
    pub cli: String,
    pub description: String,
    pub base_args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub transport: TransportConfig,
    #[serde(rename = "loop")]
    pub loop_config: LoopConfig,
    pub memory: MemoryConfig,
    pub workers: WorkerRuntimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub mode: String,
    pub preferred: Vec<String>,
    pub allow_tmux_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    pub persist_runs: bool,
    pub resume_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub ttl_hours: i64,
    pub invalidate_on_git_head_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRuntimeConfig {
    pub max_workers: i64,
    pub spawn_policy: String,
    pub continue_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub cli: String,
    pub model: String,
    pub reasoning: Option<String>,
    pub description: String,
    pub delivery_mode: Option<String>,
    pub launch_mode: Option<String>,
    pub base_args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct StatusPayload {
    pub config_path: String,
    pub transport: serde_json::Value,
    pub defaults: serde_json::Value,
    pub workers: serde_json::Value,
    pub worker_types: Vec<String>,
    pub ok: bool,
}

pub fn default_config() -> Config {
    let mut workers = BTreeMap::new();
    workers.insert(
        "explore".to_string(),
        default_worker(
            "codex",
            "gpt-5.3-codex-spark",
            Some("xhigh"),
            "Fast codebase exploration and triage lane",
        ),
    );
    workers.insert(
        "build".to_string(),
        default_worker(
            "codex",
            "gpt-5.4-mini",
            Some("high"),
            "Primary implementation lane",
        ),
    );
    workers.insert(
        "review".to_string(),
        default_worker(
            "codex",
            "gpt-5.4",
            Some("medium"),
            "Review and challenge lane",
        ),
    );
    workers.insert(
        "verify".to_string(),
        default_worker(
            "codex",
            "gpt-5.4-mini",
            Some("high"),
            "Verification and completion evidence lane",
        ),
    );

    Config {
        defaults: Defaults {
            idle_timeout_ms: 120000,
            summary_only: true,
            max_parallel: 4,
        },
        surface: SurfaceConfig {
            cli: "codex".to_string(),
            description: "Primary operator surface following the user's host defaults".to_string(),
            base_args: Some(Vec::new()),
            env: None,
        },
        runtime: RuntimeConfig {
            transport: TransportConfig {
                mode: "direct".to_string(),
                preferred: vec!["stdio".to_string(), "unix_socket".to_string()],
                allow_tmux_fallback: false,
            },
            loop_config: LoopConfig {
                persist_runs: true,
                resume_strategy: "ledger".to_string(),
            },
            memory: MemoryConfig {
                enabled: true,
                ttl_hours: 24,
                invalidate_on_git_head_change: true,
            },
            workers: WorkerRuntimeConfig {
                max_workers: 6,
                spawn_policy: "persistent".to_string(),
                continue_policy: "resume_when_possible".to_string(),
            },
        },
        workers,
    }
}

pub fn required_arg<'a>(args: &'a [String], index: usize, error: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error.to_string())
}

pub fn parse_worker_state(value: &str) -> Result<WorkerState, String> {
    match value {
        "idle" => Ok(WorkerState::Idle),
        "working" => Ok(WorkerState::Working),
        "blocked" => Ok(WorkerState::Blocked),
        "done" => Ok(WorkerState::Done),
        "failed" => Ok(WorkerState::Failed),
        "stopped" => Ok(WorkerState::Stopped),
        "unknown" => Ok(WorkerState::Unknown),
        _ => Err(
            "worker state must be idle, working, blocked, done, failed, stopped, or unknown"
                .to_string(),
        ),
    }
}

pub fn parse_dispatch_status(value: &str) -> Result<DispatchStatus, String> {
    match value {
        "pending" => Ok(DispatchStatus::Pending),
        "notified" => Ok(DispatchStatus::Notified),
        "delivered" => Ok(DispatchStatus::Delivered),
        "failed" => Ok(DispatchStatus::Failed),
        _ => Err("dispatch status must be pending, notified, delivered, or failed".to_string()),
    }
}

pub fn parse_run_phase(value: &str) -> Result<RunPhase, String> {
    match value {
        "starting" => Ok(RunPhase::Starting),
        "discovering" => Ok(RunPhase::Discovering),
        "spawning" => Ok(RunPhase::Spawning),
        "executing" => Ok(RunPhase::Executing),
        "verifying" => Ok(RunPhase::Verifying),
        "fixing" => Ok(RunPhase::Fixing),
        "complete" => Ok(RunPhase::Complete),
        "failed" => Ok(RunPhase::Failed),
        "cancelled" => Ok(RunPhase::Cancelled),
        _ => Err("phase must be starting, discovering, spawning, executing, verifying, fixing, complete, failed, or cancelled".to_string()),
    }
}

pub fn resolve_config_path() -> Result<PathBuf, String> {
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
    let repo_default = cwd.join("config").join("conductor.json");
    if repo_default.exists() {
        return Ok(repo_default);
    }

    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(Path::new(&home)
        .join(".conductor-kit")
        .join("conductor.json"))
}

pub fn resolve_state_root() -> Result<PathBuf, String> {
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

pub fn load_resolved_config() -> Result<(PathBuf, Config), String> {
    let path = resolve_config_path()?;
    if !path.exists() {
        return Ok((path, default_config()));
    }
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let cfg = serde_json::from_str::<Config>(&raw).map_err(|err| err.to_string())?;
    Ok((path, cfg))
}

pub fn save_config(path: &Path, cfg: &Config) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(cfg).map_err(|err| err.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, format!("{rendered}\n")).map_err(|err| err.to_string())
}

pub fn command_available(command: &str) -> bool {
    let path_var = match env::var_os("PATH") {
        Some(value) => value,
        None => return false,
    };
    env::split_paths(&path_var).any(|dir| {
        let candidate = dir.join(command);
        candidate.is_file()
    })
}
