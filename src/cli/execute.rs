use crate::cli::args::{
    Config, StatusPayload, command_available, load_resolved_config, parse_dispatch_status,
    parse_run_phase, parse_worker_state, required_arg, resolve_config_path, resolve_state_root,
};
use crate::cli::logging::{print_help, print_json};
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
    DispatchStatus, EventEnvelope, EventKind, RunPhase, SCHEMA_VERSION, SessionStatus, WorkerKind,
    WorkerRecord, WorkerState,
};
use crate::runtime::workers::{WorkerLaunchSpec, execute_worker};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

pub fn execute_command(args: &[String]) -> Result<(), String> {
    let cmd = args.get(0).map(String::as_str).unwrap_or("help");

    match cmd {
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
        "runtime-init" => run_runtime_init(&args[1..]),
        "runtime-snapshot" => run_runtime_snapshot(&args[1..]),
        "runtime-refresh" => run_runtime_refresh(&args[1..]),
        "run-orchestrate" => run_orchestrate(&args[1..]),
        "run-fanout" => run_fanout(&args[1..]),
        "authority-renew" => run_authority_renew(&args[1..]),
        "phase-set" => run_phase_set(&args[1..]),
        "task-claim" => run_task_claim(&args[1..]),
        "task-release" => run_task_release(&args[1..]),
        "worker-upsert" => run_worker_upsert(&args[1..]),
        "worker-exec" => run_worker_exec(&args[1..]),
        "worker-spawn-session" => run_worker_spawn_session(&args[1..]),
        "worker-adapter-exec" => run_worker_adapter_exec(&args[1..]),
        "worker-adapter-spawn-session" => run_worker_adapter_spawn_session(&args[1..]),
        "worker-send" => run_worker_send(&args[1..]),
        "worker-send-raw" => run_worker_send_raw(&args[1..]),
        "worker-attach" => run_worker_attach(&args[1..]),
        "worker-open-terminal" => run_worker_open_terminal(&args[1..]),
        "worker-log" => run_worker_log(&args[1..]),
        "worker-session-status" => run_worker_session_status(&args[1..]),
        "worker-stop-session" => run_worker_stop_session(&args[1..]),
        "worker-host" => run_worker_host_command(&args[1..]),
        "dispatch-route" => run_dispatch_route(&args[1..]),
        "hud-view" => run_hud_view(&args[1..]),
        "hud-watch" => run_hud_watch(&args[1..]),
        "events-list" => run_events_list(&args[1..]),
        "hook-run" => run_hook_run(&args[1..]),
        "task-create" => run_task_create(&args[1..]),
        "dispatch-queue" => run_dispatch_queue(&args[1..]),
        "dispatch-update" => run_dispatch_update(&args[1..]),
        "mailbox-send" => run_mailbox_send(&args[1..]),
        "mailbox-update" => run_mailbox_update(&args[1..]),
        _ => {
            print_help();
            Err("unknown command".to_string())
        }
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

    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;

    let (_, cfg) = load_resolved_config()?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let task_id = format!("task-{worker_id}");
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

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Executing,
        Some("dispatch_prompt".to_string()),
    )?;
    let dispatch_id = format!("dispatch-{worker_id}");
    let message_id = format!("message-{worker_id}");
    let routed = dispatch_prompt_to_adapter(
        &store,
        &adapter,
        run_id,
        &worker_id,
        &task.task_id,
        "orchestrator-main",
        &dispatch_id,
        &message_id,
        prompt,
    )?;
    let ok = routed.ok;
    let session_id = routed.session_id.clone();
    let response = routed.response;

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Verifying,
        Some("result_check".to_string()),
    )?;
    if adapter.delivery_mode == "session" && ok {
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
    } else if adapter.delivery_mode == "session" {
        let _ = store.fail_task(run_id, &task_id, "dispatch routing failed")?;
    }
    let _ = transition_phase(
        &store,
        run_id,
        if ok {
            RunPhase::Complete
        } else {
            RunPhase::Failed
        },
        Some("orchestration_end".to_string()),
    )?;
    let snapshot = store.read_snapshot(run_id)?;
    print_json(&json!({
        "ok": ok,
        "run_id": run_id,
        "worker_id": worker_id,
        "task_id": task_id,
        "session_id": session_id,
        "response": response,
        "snapshot": snapshot
    }))
}

fn run_fanout(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "run-fanout requires <run_id> <worker_type> <prompt> <worker_id> [worker_id...]"
                .to_string(),
        );
    }
    let run_id = &args[0];
    let worker_type = &args[1];
    let prompt = &args[2];
    let worker_ids = args[3..].to_vec();

    let (_, cfg) = load_resolved_config()?;
    let max_parallel = std::cmp::min(cfg.defaults.max_parallel, cfg.runtime.workers.max_workers);
    if worker_ids.len() as i64 > max_parallel {
        return Err(format!(
            "worker count exceeds configured max_parallel={max_parallel}"
        ));
    }

    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
    let adapter = worker_adapter_config(&cfg, worker_type)?;
    let invocation_id = Utc::now().timestamp_millis();

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Discovering,
        Some("fanout_start".to_string()),
    )?;

    let mut task_specs = Vec::new();
    for worker_id in &worker_ids {
        let task_id = format!("task-{worker_id}-{invocation_id}");
        let task = store.create_task(
            run_id,
            &task_id,
            &format!("fanout {worker_type} task for {worker_id}"),
            Some(prompt.clone()),
        )?;
        let _ = acquire_claim(&store, run_id, &task.task_id, worker_id, 10)?;
        task_specs.push((worker_id.clone(), task.task_id));
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Spawning,
        Some("fanout_worker_sessions".to_string()),
    )?;
    for (worker_id, task_id) in &task_specs {
        if adapter.delivery_mode == "session" {
            ensure_adapter_session(&store, &adapter, run_id, worker_id, Some(task_id))?;
        }
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Executing,
        Some("fanout_dispatch".to_string()),
    )?;
    let mut routed_results = Vec::new();
    for (worker_id, task_id) in &task_specs {
        let dispatch_id = format!("dispatch-{worker_id}-{invocation_id}");
        let message_id = format!("message-{worker_id}-{invocation_id}");
        routed_results.push((
            worker_id.clone(),
            task_id.clone(),
            dispatch_id.clone(),
            message_id.clone(),
            dispatch_prompt_to_adapter(
                &store,
                &adapter,
                run_id,
                worker_id,
                task_id,
                "orchestrator-main",
                &dispatch_id,
                &message_id,
                prompt,
            )?,
        ));
    }

    let _ = transition_phase(
        &store,
        run_id,
        RunPhase::Verifying,
        Some("fanout_verify".to_string()),
    )?;
    let mut failures = Vec::new();
    let mut results = Vec::new();
    for (worker_id, task_id, dispatch_id, message_id, routed) in routed_results {
        if adapter.delivery_mode == "session" && routed.ok {
            let _ = store.complete_task(
                run_id,
                &task_id,
                "fanout dispatch delivered",
                json!({
                    "worker_id": worker_id,
                    "session_id": routed.session_id,
                    "dispatch_id": dispatch_id,
                    "message_id": message_id
                }),
            )?;
        } else if adapter.delivery_mode == "session" {
            let reason = "dispatch routing failed".to_string();
            let _ = store.fail_task(run_id, &task_id, &reason)?;
            failures.push(json!({
                "worker_id": worker_id,
                "task_id": task_id,
                "reason": reason
            }));
        } else if !routed.ok {
            failures.push(json!({
                "worker_id": worker_id,
                "task_id": task_id,
                "reason": "worker execution failed"
            }));
        }
        results.push(json!({
            "worker_id": worker_id,
            "task_id": task_id,
            "session_id": routed.session_id,
            "dispatch_id": dispatch_id,
            "message_id": message_id,
            "response": routed.response
        }));
    }

    let verifier_result = if let Some(_verifier_cfg) = cfg.workers.get("verifier") {
        let verifier_adapter = worker_adapter_config(&cfg, "verifier")?;
        let verifier_worker_id = format!("verifier-{invocation_id}");
        let verifier_task_id = format!("task-{verifier_worker_id}");
        let verifier_prompt = serde_json::to_string_pretty(&json!({
            "run_id": run_id,
            "worker_type": worker_type,
            "prompt": prompt,
            "fanout_results": results,
            "fanout_failures": failures
        }))
        .map_err(|err| err.to_string())?;
        let task = store.create_task(
            run_id,
            &verifier_task_id,
            "verify fanout results",
            Some("Verifier pass over fan-out worker delivery results".to_string()),
        )?;
        let _ = acquire_claim(&store, run_id, &task.task_id, &verifier_worker_id, 10)?;
        if verifier_adapter.delivery_mode == "session" {
            ensure_adapter_session(
                &store,
                &verifier_adapter,
                run_id,
                &verifier_worker_id,
                Some(&task.task_id),
            )?;
        }
        let verifier_dispatch_id = format!("dispatch-{verifier_worker_id}-{invocation_id}");
        let verifier_message_id = format!("message-{verifier_worker_id}-{invocation_id}");
        let routed = dispatch_prompt_to_adapter(
            &store,
            &verifier_adapter,
            run_id,
            &verifier_worker_id,
            &verifier_task_id,
            "orchestrator-main",
            &verifier_dispatch_id,
            &verifier_message_id,
            &verifier_prompt,
        );
        match routed {
            Ok(routed) => {
                let verification_ok = failures.is_empty() && routed.ok;
                if verifier_adapter.delivery_mode == "session" && verification_ok {
                    let _ = store.complete_task(
                        run_id,
                        &verifier_task_id,
                        "verification dispatch delivered",
                        json!({
                            "worker_id": verifier_worker_id,
                            "session_id": routed.session_id,
                            "dispatch_id": verifier_dispatch_id,
                            "message_id": verifier_message_id
                        }),
                    )?;
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event: EventKind::VerificationPassed,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some("verifier_dispatch_delivered".to_string()),
                            context: serde_json::Map::new(),
                        },
                    )?;
                } else if verifier_adapter.delivery_mode == "session" {
                    let _ =
                        store.fail_task(run_id, &verifier_task_id, "verification gate failed")?;
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event: EventKind::VerificationFailed,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some("verification_gate_failed".to_string()),
                            context: serde_json::Map::new(),
                        },
                    )?;
                } else {
                    let event = if verification_ok {
                        EventKind::VerificationPassed
                    } else {
                        EventKind::VerificationFailed
                    };
                    let _ = store.append_runtime_event(
                        run_id,
                        EventEnvelope {
                            schema_version: SCHEMA_VERSION,
                            event,
                            timestamp: Utc::now(),
                            run_id: Some(run_id.to_string()),
                            session_id: routed.session_id.clone(),
                            source: "orchestrator".to_string(),
                            worker: Some(verifier_worker_id.clone()),
                            task_id: Some(verifier_task_id.clone()),
                            message_id: Some(verifier_message_id.clone()),
                            reason: Some(if verification_ok {
                                "verifier_execution_succeeded".to_string()
                            } else {
                                "verifier_execution_failed".to_string()
                            }),
                            context: serde_json::Map::new(),
                        },
                    )?;
                }
                Some(json!({
                    "configured": true,
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "dispatch_id": verifier_dispatch_id,
                    "message_id": verifier_message_id,
                    "response": routed.response,
                    "ok": verification_ok
                }))
            }
            Err(err) => {
                let _ = store.fail_task(run_id, &verifier_task_id, &err)?;
                let _ = store.append_runtime_event(
                    run_id,
                    EventEnvelope {
                        schema_version: SCHEMA_VERSION,
                        event: EventKind::VerificationFailed,
                        timestamp: Utc::now(),
                        run_id: Some(run_id.to_string()),
                        session_id: None,
                        source: "orchestrator".to_string(),
                        worker: Some(verifier_worker_id.clone()),
                        task_id: Some(verifier_task_id.clone()),
                        message_id: None,
                        reason: Some(err.clone()),
                        context: serde_json::Map::new(),
                    },
                )?;
                failures.push(json!({
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "reason": err
                }));
                Some(json!({
                    "configured": true,
                    "worker_id": verifier_worker_id,
                    "task_id": verifier_task_id,
                    "ok": false
                }))
            }
        }
    } else {
        None
    };

    let verifier_ok = verifier_result
        .as_ref()
        .and_then(|value| value.get("ok"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let final_phase = if failures.is_empty() && verifier_ok {
        RunPhase::Complete
    } else {
        RunPhase::Failed
    };
    let _ = transition_phase(&store, run_id, final_phase, Some("fanout_end".to_string()))?;
    let snapshot = store.read_snapshot(run_id)?;
    print_json(&json!({
        "ok": failures.is_empty(),
        "run_id": run_id,
        "worker_type": worker_type,
        "worker_count": worker_ids.len(),
        "results": results,
        "failures": failures,
        "verifier": verifier_result,
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
    ensure_run_exists(&store, run_id)?;
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id,
            worker_kind: WorkerKind::Worker,
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
    ensure_run_exists(&store, run_id)?;
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
    ensure_run_exists(&store, run_id)?;
    let result = execute_worker(
        WorkerLaunchSpec {
            run_id: run_id.to_string(),
            worker_id: worker_id.to_string(),
            task_id: task_id.map(str::to_string),
            worker_kind: WorkerKind::Worker,
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
    let launch = resolve_worker_adapter(&adapter, run_id, worker_id, None, None)?;
    let store = StateStore::new(resolve_state_root()?);
    ensure_run_exists(&store, run_id)?;
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
    if let Some(body) = prompt {
        let _ = send_session_command(
            Path::new(&result.session.socket_path),
            &SessionCommand::SendStdin {
                data: format!("{body}\n"),
            },
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
            data: format!(
                "{data}
"
            ),
        },
    )?;
    print_json(&response)
}

fn run_worker_send_raw(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let data = required_arg(
        args,
        2,
        "worker-send-raw requires <run_id> <session_id> <data>",
    )?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendRaw {
            data: data.to_string(),
        },
    )?;
    print_json(&response)
}

fn run_worker_attach(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, 0, "worker-attach requires <run_id> <session_id>")?;
    let session_id = required_arg(args, 1, "worker-attach requires <run_id> <session_id>")?;
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let socket_path = PathBuf::from(&session.socket_path);
    let stdout_path = PathBuf::from(&session.stdout_path);

    let running = Arc::new(AtomicBool::new(true));
    let follow_running = running.clone();
    let follow_path = stdout_path.clone();
    let follow_handle = thread::spawn(move || follow_log_file(&follow_path, follow_running));

    let raw_mode = TerminalRawMode::enable()?;
    let _ = std::io::stdout().write_all(
        b"\r\n[attached] press Ctrl-] to detach. input is forwarded to the worker PTY.\r\n",
    );
    let _ = std::io::stdout().flush();

    let mut stdin = std::io::stdin();
    let mut buf = [0_u8; 1];
    while running.load(Ordering::SeqCst) {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if buf[0] == 0x1d {
                    break;
                }
                let data = String::from_utf8_lossy(&buf[..1]).to_string();
                let response =
                    send_session_command(&socket_path, &SessionCommand::SendRaw { data })?;
                if response.status == "exited" || response.status == "stopped" {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.to_string()),
        }
    }

    running.store(false, Ordering::SeqCst);
    let _ = follow_handle.join();
    drop(raw_mode);
    let _ = std::io::stdout().write_all(b"\r\n[detached]\r\n");
    let _ = std::io::stdout().flush();
    Ok(())
}

fn run_worker_open_terminal(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-open-terminal requires <run_id> <session_id> [terminal_app]",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-open-terminal requires <run_id> <session_id> [terminal_app]",
    )?;
    let terminal_app = args
        .get(2)
        .cloned()
        .or_else(|| env::var("CONDUCTOR_TERMINAL_APP").ok())
        .unwrap_or_else(|| "Terminal".to_string());
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let state_root = resolve_state_root()?;
    let config_path = resolve_config_path().ok();
    let attach_cmd = build_attach_shell_command(
        &cwd,
        &conductor_bin,
        &state_root,
        config_path.as_deref(),
        run_id,
        session_id,
    );

    let script = format!(
        "tell application {} to activate\n\
         tell application {} to do script {}",
        apple_script_string(&terminal_app),
        apple_script_string(&terminal_app),
        apple_script_string(&attach_cmd)
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    print_json(&json!({
        "ok": true,
        "terminal_app": terminal_app,
        "run_id": run_id,
        "session_id": session_id
    }))
}

fn run_worker_log(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "worker-log requires <run_id> <session_id> [stdout|stderr|host_stdout|host_stderr] [lines]",
    )?;
    let session_id = required_arg(
        args,
        1,
        "worker-log requires <run_id> <session_id> [stdout|stderr|host_stdout|host_stderr] [lines]",
    )?;
    let stream = args.get(2).map(String::as_str).unwrap_or("stdout");
    let lines = args
        .get(3)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(40);
    let store = StateStore::new(resolve_state_root()?);
    let session = store.read_session(run_id, session_id)?;
    let path = match stream {
        "stdout" => PathBuf::from(&session.stdout_path),
        "stderr" => PathBuf::from(&session.stderr_path),
        "host_stdout" => store
            .session_dir(run_id, session_id)
            .join("host.stdout.log"),
        "host_stderr" => store
            .session_dir(run_id, session_id)
            .join("host.stderr.log"),
        _ => {
            return Err(
                "worker-log stream must be stdout, stderr, host_stdout, or host_stderr".to_string(),
            );
        }
    };
    let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let collected = raw.lines().collect::<Vec<_>>();
    let start = collected.len().saturating_sub(lines);
    println!(
        "{}",
        collected[start..].join(
            "
"
        )
    );
    Ok(())
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
            data: format!(
                "{body}
"
            ),
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

fn run_hud_watch(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(
        args,
        0,
        "hud-watch requires <run_id> [interval_ms] [iterations]",
    )?;
    let interval_ms = args
        .get(1)
        .map(|value| value.parse::<u64>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(1000);
    let iterations = args
        .get(2)
        .map(|value| value.parse::<usize>().map_err(|err| err.to_string()))
        .transpose()?
        .unwrap_or(0);
    let store = StateStore::new(resolve_state_root()?);
    let mut count = 0usize;
    loop {
        let snapshot = store.read_snapshot(run_id)?;
        print!("\x1B[2J\x1B[H");
        println!("run      {}", snapshot.run.run_id);
        println!("phase    {:?}", snapshot.run.phase);
        println!("active   {}", snapshot.run.active);
        println!(
            "owner    {}",
            snapshot
                .authority
                .as_ref()
                .map(|lease| lease.owner.clone())
                .unwrap_or_else(|| "none".to_string())
        );
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
                "  {}  kind={:?} state={:?} task={} summary={}",
                worker.worker_id,
                worker.worker_kind,
                worker.state,
                worker.current_task_id.unwrap_or_else(|| "-".to_string()),
                worker.current_summary.unwrap_or_else(|| "-".to_string())
            );
        }
        count += 1;
        if iterations > 0 && count >= iterations {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(interval_ms));
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

#[derive(Debug, Serialize)]
struct RoutedDispatch {
    session_id: Option<String>,
    ok: bool,
    response: serde_json::Value,
}

fn ensure_run_exists(store: &StateStore, run_id: &str) -> Result<(), String> {
    if !store
        .root()
        .join("runs")
        .join(run_id)
        .join("run.json")
        .exists()
    {
        let _ = store.init_run(run_id, "orchestrator-main")?;
    }
    Ok(())
}

fn ensure_adapter_session(
    store: &StateStore,
    adapter: &WorkerAdapterConfig,
    run_id: &str,
    worker_id: &str,
    task_id: Option<&str>,
) -> Result<String, String> {
    let session_id = format!("session-{worker_id}");
    let desired_kind = worker_kind_for_type(adapter.worker_type.as_str(), worker_id);
    if store.session_file(run_id, &session_id).exists() {
        let mut worker = store.read_worker(run_id, worker_id)?;
        if worker.worker_kind != desired_kind {
            worker.worker_kind = desired_kind;
            let _ = store.upsert_worker(worker)?;
        }
        return Ok(session_id);
    }
    let conductor_bin = env::current_exe().map_err(|err| err.to_string())?;
    let launch = resolve_worker_adapter(adapter, run_id, worker_id, task_id, None)?;
    let result = spawn_session(
        store,
        run_id,
        worker_id,
        &launch.program,
        &launch.args,
        &launch.env,
        &conductor_bin,
    )?;
    let mut worker = store.read_worker(run_id, worker_id)?;
    if worker.worker_kind != desired_kind {
        worker.worker_kind = desired_kind;
        let _ = store.upsert_worker(worker)?;
    }
    Ok(result.session.session_id)
}

fn worker_kind_for_type(worker_type: &str, worker_id: &str) -> WorkerKind {
    if worker_type == "orchestrator" {
        WorkerKind::Orchestrator
    } else if worker_type == "verifier" || worker_id.starts_with("verifier-") {
        WorkerKind::Verifier
    } else {
        WorkerKind::Worker
    }
}

fn dispatch_prompt_to_adapter(
    store: &StateStore,
    adapter: &WorkerAdapterConfig,
    run_id: &str,
    worker_id: &str,
    task_id: &str,
    source_worker: &str,
    dispatch_id: &str,
    message_id: &str,
    body: &str,
) -> Result<RoutedDispatch, String> {
    if adapter.delivery_mode != "session" {
        return Err(format!(
            "worker type {} is not allowed to use non-session delivery",
            adapter.worker_type
        ));
    }
    let _ = store.queue_dispatch(run_id, dispatch_id, worker_id, serde_json::Map::new())?;
    let dispatch = store.read_dispatch(run_id, dispatch_id)?;
    let _ =
        store.create_mailbox_message(run_id, message_id, source_worker, &dispatch.target, body)?;
    let _ = store.update_dispatch_status(run_id, dispatch_id, DispatchStatus::Notified, None)?;
    let _ = store.update_mailbox_status(run_id, &dispatch.target, message_id, false)?;
    let session_id = ensure_adapter_session(store, adapter, run_id, worker_id, Some(task_id))?;
    let session = store.read_session(run_id, &session_id)?;
    let response = send_session_command(
        Path::new(&session.socket_path),
        &SessionCommand::SendStdin {
            data: format!("{body}\n"),
        },
    )?;
    if response.ok {
        let _ = store.update_mailbox_status(run_id, &dispatch.target, message_id, true)?;
        let _ =
            store.update_dispatch_status(run_id, dispatch_id, DispatchStatus::Delivered, None)?;
    } else {
        let _ = store.update_dispatch_status(
            run_id,
            dispatch_id,
            DispatchStatus::Failed,
            response.message.clone(),
        )?;
    }
    Ok(RoutedDispatch {
        session_id: Some(session_id),
        ok: response.ok,
        response: serde_json::to_value(response).map_err(|err| err.to_string())?,
    })
}

fn follow_log_file(path: &Path, running: Arc<AtomicBool>) {
    let mut offset = 0_u64;
    while running.load(Ordering::SeqCst) {
        if let Ok(mut file) = File::open(path) {
            if let Ok(metadata) = file.metadata() {
                let len = metadata.len();
                if len < offset {
                    offset = 0;
                }
                if len > offset && file.seek(SeekFrom::Start(offset)).is_ok() {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() && !buffer.is_empty() {
                        let _ = std::io::stdout().write_all(&buffer);
                        let _ = std::io::stdout().flush();
                        offset = len;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn build_attach_shell_command(
    cwd: &Path,
    conductor_bin: &Path,
    state_root: &Path,
    config_path: Option<&Path>,
    run_id: &str,
    session_id: &str,
) -> String {
    let mut parts = vec![format!("cd {}", shell_quote(cwd))];
    parts.push(format!("CONDUCTOR_STATE_DIR={}", shell_quote(state_root)));
    if let Some(path) = config_path {
        parts.push(format!("CONDUCTOR_CONFIG={}", shell_quote(path)));
    }
    parts.push(shell_quote(conductor_bin));
    parts.push("worker-attach".to_string());
    parts.push(shell_quote_str(run_id));
    parts.push(shell_quote_str(session_id));
    parts.join(" ")
}

fn shell_quote(value: &Path) -> String {
    shell_quote_str(&value.display().to_string())
}

fn shell_quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn apple_script_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

struct TerminalRawMode {
    fd: i32,
    original: libc::termios,
}

impl TerminalRawMode {
    fn enable() -> Result<Self, String> {
        let fd = std::io::stdin().as_raw_fd();
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
        let rc = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let original = unsafe { termios.assume_init() };
        let mut raw = original;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        let rc = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self { fd, original })
    }
}

impl Drop for TerminalRawMode {
    fn drop(&mut self) {
        let _ = unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) };
    }
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
        if !command_available(&worker.cli) {
            issues.push(format!(
                "workers.{name}.cli binary not found in PATH: {}",
                worker.cli
            ));
        }
        if let Some(delivery_mode) = &worker.delivery_mode {
            if delivery_mode != "session" {
                issues.push(format!(
                    "workers.{name}.delivery_mode must remain session in the PTY baseline"
                ));
            }
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
        delivery_mode: worker
            .delivery_mode
            .clone()
            .unwrap_or_else(|| "session".to_string()),
        launch_mode: worker
            .launch_mode
            .clone()
            .unwrap_or_else(|| "stdin_json".to_string()),
        base_args: worker.base_args.clone().unwrap_or_default(),
        env: worker.env.clone().unwrap_or_default(),
    })
}
