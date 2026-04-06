mod runtime;

use crate::runtime::adapters::{WorkerAdapterConfig, resolve_worker_adapter};
use crate::runtime::authority::renew_authority;
use crate::runtime::claims::{acquire_claim, release_claim};
use crate::runtime::hooks::{event_name_of, filter_events, watch_and_run_hooks};
use crate::runtime::phases::transition_phase;
use crate::runtime::sessions::{
    SessionCommand, run_worker_host, send_session_command, spawn_session,
};
use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    DispatchStatus, RunPhase, SessionStatus, WorkerKind, WorkerRecord, WorkerState,
};
use crate::runtime::workers::{WorkerLaunchSpec, execute_worker};
use chrono::Utc;
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
    launch_mode: Option<String>,
    base_args: Option<Vec<String>>,
    env: Option<BTreeMap<String, String>>,
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
        "runtime-refresh" => run_runtime_refresh(&args[2..]),
        "run-orchestrate" => run_orchestrate(&args[2..]),
        "authority-renew" => run_authority_renew(&args[2..]),
        "phase-set" => run_phase_set(&args[2..]),
        "task-claim" => run_task_claim(&args[2..]),
        "task-release" => run_task_release(&args[2..]),
        "worker-upsert" => run_worker_upsert(&args[2..]),
        "worker-exec" => run_worker_exec(&args[2..]),
        "worker-spawn-session" => run_worker_spawn_session(&args[2..]),
        "worker-adapter-exec" => run_worker_adapter_exec(&args[2..]),
        "worker-adapter-spawn-session" => run_worker_adapter_spawn_session(&args[2..]),
        "worker-send" => run_worker_send(&args[2..]),
        "worker-session-status" => run_worker_session_status(&args[2..]),
        "worker-stop-session" => run_worker_stop_session(&args[2..]),
        "worker-host" => run_worker_host_command(&args[2..]),
        "dispatch-route" => run_dispatch_route(&args[2..]),
        "hud-view" => run_hud_view(&args[2..]),
        "events-list" => run_events_list(&args[2..]),
        "hook-run" => run_hook_run(&args[2..]),
        "task-create" => run_task_create(&args[2..]),
        "dispatch-queue" => run_dispatch_queue(&args[2..]),
        "dispatch-update" => run_dispatch_update(&args[2..]),
        "mailbox-send" => run_mailbox_send(&args[2..]),
        "mailbox-update" => run_mailbox_update(&args[2..]),
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

fn run_runtime_refresh(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "runtime-refresh requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.refresh_snapshot(run_id)?;
    print_json(&snapshot)
}

fn run_orchestrate(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "run-orchestrate requires <run_id> <worker_type> <prompt> [worker_id]".to_string(),
        );
    }
    let run_id = &args[0];
    let worker_type = &args[1];
    let prompt = &args[2];
    let worker_id = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| format!("{worker_type}-1"));

    let state_root = resolve_state_root()?;
    let store = StateStore::new(state_root);
    if !store
        .root()
        .join("runs")
        .join(run_id)
        .join("run.json")
        .exists()
    {
        let _ = store.init_run(run_id, "orchestrator-main")?;
    }

    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let task_id = format!("task-{worker_id}");
    let session_id = format!("session-{worker_id}");

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Discovering,
        Some("orchestration_start".to_string()),
    )?;
    let task = match store.read_task(run_id, &task_id) {
        Ok(existing) => existing,
        Err(_) => store.create_task(run_id, &task_id, prompt, Some(prompt.clone()))?,
    };
    let _ = acquire_claim(&store, run_id, &task.task_id, &worker_id, 10)?;
    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Spawning,
        Some("worker_session_start".to_string()),
    )?;

    let session_exists = store.session_file(run_id, &session_id).exists();
    if !session_exists {
        let launch = resolve_worker_adapter(
            &adapter,
            run_id,
            &worker_id,
            Some(&task.task_id),
            Some(prompt),
        )?;
        let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
        let result = spawn_session(
            &store,
            run_id,
            &worker_id,
            &launch.program,
            &launch.args,
            &launch.env,
            &conductor_bin,
        )?;
        if let Some(payload) = launch.stdin_payload {
            let _ = send_session_command(
                Path::new(&result.session.socket_path),
                &SessionCommand::SendStdin { data: payload },
            )?;
        }
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Executing,
        Some("dispatch_prompt".to_string()),
    )?;
    let dispatch_id = format!("dispatch-{worker_id}");
    let message_id = format!("message-{worker_id}");
    let _ = store.queue_dispatch(run_id, &dispatch_id, &worker_id, serde_json::Map::new())?;
    let response = {
        let dispatch = store.read_dispatch(run_id, &dispatch_id)?;
        let session = store.read_session(run_id, &session_id)?;
        let _ = store.create_mailbox_message(
            run_id,
            &message_id,
            "orchestrator-main",
            &dispatch.target,
            prompt,
        )?;
        let _ =
            store.update_dispatch_status(run_id, &dispatch_id, DispatchStatus::Notified, None)?;
        let _ = store.update_mailbox_status(run_id, &dispatch.target, &message_id, false)?;
        let response = send_session_command(
            Path::new(&session.socket_path),
            &SessionCommand::SendStdin {
                data: format!("{prompt}\n"),
            },
        )?;
        if response.ok {
            let _ = store.update_mailbox_status(run_id, &dispatch.target, &message_id, true)?;
            let _ = store.update_dispatch_status(
                run_id,
                &dispatch_id,
                DispatchStatus::Delivered,
                None,
            )?;
        } else {
            let _ = store.update_dispatch_status(
                run_id,
                &dispatch_id,
                DispatchStatus::Failed,
                response.message.clone(),
            )?;
        }
        response
    };

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Verifying,
        Some("result_check".to_string()),
    )?;
    if response.ok {
        let _ = store.complete_task(
            run_id,
            &task_id,
            "orchestration dispatch delivered",
            json!({
                "session_id": session_id,
                "dispatch_id": dispatch_id,
                "message_id": message_id
            }),
        )?;
    } else {
        let _ = store.fail_task(
            run_id,
            &task_id,
            response
                .message
                .as_deref()
                .unwrap_or("dispatch routing failed"),
        )?;
    }
    let _ = transition_phase(
        &store,
        run_id,
        if response.ok {
            RunPhase::Complete
        } else {
            RunPhase::Failed
        },
        Some("orchestration_end".to_string()),
    )?;
    let snapshot = store.read_snapshot(run_id)?;
    print_json(&json!({
        "ok": response.ok,
        "run_id": run_id,
        "worker_id": worker_id,
        "task_id": task_id,
        "session_id": session_id,
        "response": response,
        "snapshot": snapshot
    }))
}

fn run_authority_renew(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "authority-renew requires <run_id> <owner> [lease_minutes]",
    )?;
    let owner = required_arg(
        args,
        1,
        "authority-renew requires <run_id> <owner> [lease_minutes]",
    )?;
    let lease_minutes = args
        .get(2)
        .map(|value| value.parse::<i64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(5);
    let store = StateStore::new(resolve_state_root()?);
    let run = renew_authority(&store, run_id, owner, lease_minutes)?;
    print_json(&run)
}

fn run_phase_set(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "phase-set requires <run_id> <phase> [reason]")?;
    let phase = parse_run_phase(required_arg(
        args,
        1,
        "phase-set requires <run_id> <phase> [reason]",
    )?)?;
    let reason = args.get(2).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let run = transition_phase(&store, run_id, phase, reason)?;
    print_json(&run)
}

fn run_task_claim(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let task_id = required_arg(
        args,
        1,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let owner = required_arg(
        args,
        2,
        "task-claim requires <run_id> <task_id> <owner> [lease_minutes]",
    )?;
    let lease_minutes = args
        .get(3)
        .map(|value| value.parse::<i64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(5);
    let store = StateStore::new(resolve_state_root()?);
    let task = acquire_claim(&store, run_id, task_id, owner, lease_minutes)?;
    print_json(&task)
}

fn run_task_release(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "task-release requires <run_id> <task_id> <owner>")?;
    let task_id = required_arg(args, 1, "task-release requires <run_id> <task_id> <owner>")?;
    let owner = required_arg(args, 2, "task-release requires <run_id> <task_id> <owner>")?;
    let store = StateStore::new(resolve_state_root()?);
    let task = release_claim(&store, run_id, task_id, owner)?;
    print_json(&task)
}

fn run_worker_upsert(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?;
    let worker_id = required_arg(
        args,
        1,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?;
    let state = parse_worker_state(required_arg(
        args,
        2,
        "worker-upsert requires <run_id> <worker_id> <state>",
    )?)?;
    let store = StateStore::new(resolve_state_root()?);
    let now = Utc::now();
    let worker = WorkerRecord {
        worker_id: worker_id.to_string(),
        run_id: run_id.to_string(),
        worker_kind: WorkerKind::Worker,
        session_ref: None,
        state,
        current_task_id: args.get(3).cloned(),
        current_summary: args.get(4).cloned(),
        terminal_label: Some(worker_id.to_string()),
        last_heartbeat_at: Some(now),
        last_stdout_at: None,
        last_event_at: Some(now),
        reason: None,
    };
    let worker = store.upsert_worker(worker)?;
    print_json(&worker)
}

fn run_worker_exec(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "worker-exec requires <run_id> <worker_id> <task_id|-> <program> [args...]".to_string(),
        );
    }
    let run_id = args[0].as_str();
    let worker_id = args[1].as_str();
    let task_id = if args[2] == "-" {
        None
    } else {
        Some(args[2].clone())
    };
    let program = args[3].clone();
    let program_args = args[4..].to_vec();
    let stdin_payload = env::var("CONDUCTOR_WORKER_STDIN").ok();
    let cwd = env::var("CONDUCTOR_WORKER_CWD").ok().map(PathBuf::from);
    let store = StateStore::new(resolve_state_root()?);
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id,
            program,
            args: program_args,
            cwd,
            stdin_payload,
            env: BTreeMap::new(),
        },
        &store,
    )?;
    print_json(&result)
}

fn run_worker_spawn_session(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "worker-spawn-session requires <run_id> <worker_id> <program> [args...]".to_string(),
        );
    }
    let run_id = &args[0];
    let worker_id = &args[1];
    let program = &args[2];
    let program_args = args[3..].to_vec();
    let store = StateStore::new(resolve_state_root()?);
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let result = spawn_session(
        &store,
        run_id,
        worker_id,
        program,
        &program_args,
        &BTreeMap::new(),
        &conductor_bin,
    )?;
    print_json(&result.session)
}

fn run_worker_adapter_exec(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "worker-adapter-exec requires <worker_type> <run_id> <worker_id> <task_id|-> [prompt]"
                .to_string(),
        );
    }
    let worker_type = &args[0];
    let run_id = &args[1];
    let worker_id = &args[2];
    let task_id = if args[3] == "-" {
        None
    } else {
        Some(args[3].as_str())
    };
    let prompt = args.get(4).map(String::as_str);
    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, task_id, prompt)?;
    let store = StateStore::new(resolve_state_root()?);
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id: task_id.map(str::to_string),
            program: launch.program,
            args: launch.args,
            cwd: launch.cwd,
            stdin_payload: launch.stdin_payload,
            env: launch.env,
        },
        &store,
    )?;
    print_json(&result)
}

fn run_worker_adapter_spawn_session(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "worker-adapter-spawn-session requires <worker_type> <run_id> <worker_id> [prompt]"
                .to_string(),
        );
    }
    let worker_type = &args[0];
    let run_id = &args[1];
    let worker_id = &args[2];
    let prompt = args.get(3).map(String::as_str);
    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, None, prompt)?;
    let store = StateStore::new(resolve_state_root()?);
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let result = spawn_session(
        &store,
        run_id,
        worker_id,
        &launch.program,
        &launch.args,
        &launch.env,
        &conductor_bin,
    )?;
    if let Some(payload) = launch.stdin_payload {
        let _ = send_session_command(
            Path::new(&result.session.socket_path),
            &SessionCommand::SendStdin { data: payload },
        )?;
    }
    print_json(&result.session)
}

fn run_worker_send(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "worker-send requires <run_id> <session_id> <data>")?;
    let session_id = required_arg(args, 1, "worker-send requires <run_id> <session_id> <data>")?;
    let data = required_arg(args, 2, "worker-send requires <run_id> <session_id> <data>")?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!("{data}\n"),
        },
    )?;
    print_json(&response)
}

fn run_worker_session_status(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-session-status requires <run_id> <session_id>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-session-status requires <run_id> <session_id>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(Path::new(&session.socket_path), &SessionCommand::Status)?;
    let mut next = session;
    next.updated_at = Utc::now();
    next.status = match response.status.as_str() {
        "running" => SessionStatus::Running,
        "stopped" => SessionStatus::Stopped,
        "exited" => SessionStatus::Exited,
        _ => SessionStatus::Failed,
    };
    if let Some(message) = &response.message {
        if let Some(code) = message.strip_prefix("exit_code=") {
            next.exit_code = code.parse::<i32>().ok();
        }
    }
    if matches!(
        next.status,
        SessionStatus::Exited | SessionStatus::Stopped | SessionStatus::Failed
    ) {
        next.exited_at = Some(Utc::now());
    }
    store.write_session(&next)?;
    print_json(&response)
}

fn run_worker_stop_session(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-stop-session requires <run_id> <session_id>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-stop-session requires <run_id> <session_id>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(Path::new(&session.socket_path), &SessionCommand::Stop)?;
    let mut next = session;
    next.updated_at = Utc::now();
    next.status = SessionStatus::Stopped;
    next.exited_at = Some(Utc::now());
    next.exit_code = Some(-1);
    store.write_session(&next)?;
    let mut worker = store.read_worker(run_id, &next.worker_id)?;
    worker.state = WorkerState::Stopped;
    worker.last_event_at = Some(Utc::now());
    worker.reason = Some("session_stopped".to_string());
    store.upsert_worker(worker)?;
    print_json(&response)
}

fn run_worker_host_command(args: &[String]) -> Result<(), String> {
    if args.len() < 7 {
        return Err("worker-host requires <run_id> <worker_id> <session_id> <socket_path> <stdout_path> <stderr_path> <program> [args...]".to_string());
    }
    let run_id = &args[0];
    let worker_id = &args[1];
    let session_id = &args[2];
    let socket_path = PathBuf::from(&args[3]);
    let stdout_path = PathBuf::from(&args[4]);
    let stderr_path = PathBuf::from(&args[5]);
    let program = &args[6];
    let program_args = args[7..].to_vec();
    run_worker_host(
        run_id,
        worker_id,
        session_id,
        &socket_path,
        &stdout_path,
        &stderr_path,
        program,
        &program_args,
    )
}

fn run_dispatch_route(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let message_id = required_arg(
        args,
        2,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let body = required_arg(
        args,
        3,
        "dispatch-route requires <run_id> <request_id> <message_id> <body>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let dispatch = store.read_dispatch(run_id, request_id)?;
    let session = store.read_session(run_id, &format!("session-{}", dispatch.target))?;
    let mailbox =
        store.create_mailbox_message(run_id, message_id, "orchestrator", &dispatch.target, body)?;
    store.update_dispatch_status(run_id, request_id, DispatchStatus::Notified, None)?;
    store.update_mailbox_status(run_id, &dispatch.target, message_id, false)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!("{body}\n"),
        },
    )?;
    if response.ok {
        store.update_mailbox_status(run_id, &dispatch.target, message_id, true)?;
        store.update_dispatch_status(run_id, request_id, DispatchStatus::Delivered, None)?;
    } else {
        store.update_dispatch_status(
            run_id,
            request_id,
            DispatchStatus::Failed,
            response.message.clone(),
        )?;
    }
    print_json(&json!({
        "dispatch": dispatch.request_id,
        "target": dispatch.target,
        "mailbox": mailbox.message_id,
        "response": response
    }))
}

fn run_hud_view(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "hud-view requires <run_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let snapshot = store.read_snapshot(run_id)?;
    let run = &snapshot.run;
    let authority = snapshot
        .authority
        .as_ref()
        .map(|lease| lease.owner.clone())
        .unwrap_or_else(|| "none".to_string());
    println!("run      {}", run.run_id);
    println!("phase    {:?}", run.phase);
    println!("active   {}", run.active);
    println!("owner    {}", authority);
    println!(
        "tasks    pending={} blocked={} in_progress={} completed={} failed={}",
        snapshot.tasks.pending,
        snapshot.tasks.blocked,
        snapshot.tasks.in_progress,
        snapshot.tasks.completed,
        snapshot.tasks.failed
    );
    println!(
        "dispatch pending={} notified={} delivered={} failed={}",
        snapshot.dispatch.pending,
        snapshot.dispatch.notified,
        snapshot.dispatch.delivered,
        snapshot.dispatch.failed
    );
    println!("mailbox  unread={}", snapshot.mailbox.unread);
    println!("workers");
    for worker in snapshot.workers {
        println!(
            "  {}  state={:?} task={} summary={}",
            worker.worker_id,
            worker.state,
            worker.current_task_id.unwrap_or_else(|| "-".to_string()),
            worker.current_summary.unwrap_or_else(|| "-".to_string())
        );
    }
    Ok(())
}

fn run_events_list(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "events-list requires <run_id> [event_name]")?;
    let event_name = args.get(1).map(String::as_str);
    let store = StateStore::new(resolve_state_root()?);
    let events = filter_events(store.read_events(run_id)?, event_name);
    let payload = events
        .into_iter()
        .map(|event| {
            json!({
                "event": event_name_of(&event),
                "timestamp": event.timestamp,
                "source": event.source,
                "run_id": event.run_id,
                "worker": event.worker,
                "task_id": event.task_id,
                "message_id": event.message_id,
                "reason": event.reason,
                "context": event.context
            })
        })
        .collect::<Vec<_>>();
    print_json(&payload)
}

fn run_hook_run(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err("hook-run requires <run_id> <event_name|*> <program> [args...]".to_string());
    }
    let run_id = &args[0];
    let event_name = &args[1];
    let program = &args[2];
    let program_args = args[3..].to_vec();
    let timeout_secs = env::var("CONDUCTOR_HOOK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(2);
    let cwd = env::var("CONDUCTOR_HOOK_CWD").ok().map(PathBuf::from);
    let store = StateStore::new(resolve_state_root()?);
    let handled = watch_and_run_hooks(
        &store,
        run_id,
        Some(event_name),
        program,
        &program_args,
        timeout_secs,
        cwd,
    )?;
    print_json(&json!({
        "ok": true,
        "handled": handled,
        "event": event_name
    }))
}

fn run_task_create(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "task-create requires <run_id> <task_id> <title>")?;
    let task_id = required_arg(args, 1, "task-create requires <run_id> <task_id> <title>")?;
    let title = required_arg(args, 2, "task-create requires <run_id> <task_id> <title>")?;
    let description = args.get(3).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let task = store.create_task(run_id, task_id, title, description)?;
    print_json(&task)
}

fn run_dispatch_queue(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let target = required_arg(
        args,
        2,
        "dispatch-queue requires <run_id> <request_id> <target>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let record = store.queue_dispatch(run_id, request_id, target, serde_json::Map::new())?;
    print_json(&record)
}

fn run_dispatch_update(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?;
    let request_id = required_arg(
        args,
        1,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?;
    let status = parse_dispatch_status(required_arg(
        args,
        2,
        "dispatch-update requires <run_id> <request_id> <status>",
    )?)?;
    let reason = args.get(3).cloned();
    let store = StateStore::new(resolve_state_root()?);
    let record = store.update_dispatch_status(run_id, request_id, status, reason)?;
    print_json(&record)
}

fn run_mailbox_send(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let message_id = required_arg(
        args,
        1,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let from_worker = required_arg(
        args,
        2,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let to_worker = required_arg(
        args,
        3,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let body = required_arg(
        args,
        4,
        "mailbox-send requires <run_id> <message_id> <from_worker> <to_worker> <body>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let message = store.create_mailbox_message(run_id, message_id, from_worker, to_worker, body)?;
    print_json(&message)
}

fn run_mailbox_update(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let worker_id = required_arg(
        args,
        1,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let message_id = required_arg(
        args,
        2,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let mode = required_arg(
        args,
        3,
        "mailbox-update requires <run_id> <worker_id> <message_id> <notified|delivered>",
    )?;
    let delivered = match mode {
        "notified" => false,
        "delivered" => true,
        _ => return Err("mailbox-update status must be notified or delivered".to_string()),
    };
    let store = StateStore::new(resolve_state_root()?);
    let message = store.update_mailbox_status(run_id, worker_id, message_id, delivered)?;
    print_json(&message)
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
        if let Some(launch_mode) = &worker.launch_mode {
            if !matches!(
                launch_mode.as_str(),
                "stdin_json" | "stdin_text" | "argv_prompt" | "argv_json"
            ) {
                issues.push(format!(
                    "workers.{name}.launch_mode must be stdin_json, stdin_text, argv_prompt, or argv_json"
                ));
            }
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

fn worker_adapter_config(cfg: &Config, worker_type: &str) -> Result<WorkerAdapterConfig, String> {
    let worker = cfg
        .workers
        .get(worker_type)
        .ok_or_else(|| format!("unknown worker type: {worker_type}"))?;
    Ok(WorkerAdapterConfig {
        worker_type: worker_type.to_string(),
        cli: worker.cli.clone(),
        model: worker.model.clone(),
        reasoning: worker.reasoning.clone(),
        description: worker.description.clone(),
        launch_mode: worker
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdin_json".to_string()),
        base_args: worker.base_args.clone().unwrap_or_default(),
        env: worker.env.clone().unwrap_or_default(),
    })
}

fn required_arg<'a>(args: &'a [String], index: usize, error: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error.to_string())
}

fn parse_worker_state(value: &str) -> Result<WorkerState, String> {
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

fn parse_dispatch_status(value: &str) -> Result<DispatchStatus, String> {
    match value {
        "pending" => Ok(DispatchStatus::Pending),
        "notified" => Ok(DispatchStatus::Notified),
        "delivered" => Ok(DispatchStatus::Delivered),
        "failed" => Ok(DispatchStatus::Failed),
        _ => Err("dispatch status must be pending, notified, delivered, or failed".to_string()),
    }
}

fn parse_run_phase(value: &str) -> Result<RunPhase, String> {
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
  runtime-refresh     Rebuild and persist snapshot for a run
  run-orchestrate     Run a minimal orchestration loop
  authority-renew     Renew authority lease for a run
  phase-set           Transition run phase
  task-claim          Acquire a task claim
  task-release        Release a task claim
  worker-upsert       Upsert worker state for a run
  worker-exec         Execute a worker command over stdio
  worker-spawn-session Start a long-lived worker session host
  worker-adapter-exec Execute a configured worker adapter once
  worker-adapter-spawn-session Start a configured worker adapter session
  worker-send         Send stdin to a worker session
  worker-session-status Query a worker session
  worker-stop-session Stop a worker session
  dispatch-route      Deliver a queued dispatch to a worker session
  hud-view            Print a compact runtime HUD view
  events-list         Print runtime events
  hook-run            Run a hook command against matching events
  task-create         Create a task record
  dispatch-queue      Create a dispatch record
  dispatch-update     Update dispatch status
  mailbox-send        Append a mailbox message
  mailbox-update      Mark mailbox message notified or delivered
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
