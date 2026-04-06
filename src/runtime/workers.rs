use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    EventEnvelope, EventKind, SCHEMA_VERSION, TaskStatus, WorkerKind, WorkerRecord, WorkerState,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct WorkerLaunchSpec {
    pub run_id: String,
    pub worker_id: String,
    pub task_id: Option<String>,
    pub worker_kind: WorkerKind,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub stdin_payload: Option<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct WorkerExecutionResult {
    pub worker_id: String,
    pub task_id: Option<String>,
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub started_at: chrono::DateTime<Utc>,
    pub finished_at: chrono::DateTime<Utc>,
    pub duration_ms: u128,
}

pub fn execute_worker(
    spec: WorkerLaunchSpec,
    store: &StateStore,
) -> Result<WorkerExecutionResult, String> {
    let started_at = Utc::now();
    let start = Instant::now();
    let task_id = spec.task_id.clone();

    if let Some(task_id) = &task_id {
        let task = store.read_task(&spec.run_id, task_id)?;
        match &task.claim {
            Some(claim) if claim.owner == spec.worker_id && claim.leased_until > Utc::now() => {}
            Some(claim) => {
                return Err(format!("task claim is owned by {} or expired", claim.owner));
            }
            None => return Err("worker-exec requires an active task claim".to_string()),
        }
    }

    store.append_runtime_event(
        &spec.run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::WorkerSpawned,
            timestamp: started_at,
            run_id: Some(spec.run_id.clone()),
            session_id: None,
            source: "workers".to_string(),
            worker: Some(spec.worker_id.clone()),
            task_id: task_id.clone(),
            message_id: None,
            reason: None,
            context: Map::new(),
        },
    )?;

    let working = WorkerRecord {
        worker_id: spec.worker_id.clone(),
        run_id: spec.run_id.clone(),
        worker_kind: spec.worker_kind.clone(),
        session_ref: None,
        state: WorkerState::Working,
        current_task_id: task_id.clone(),
        current_summary: Some(format!("running {}", spec.program)),
        terminal_label: Some(spec.worker_id.clone()),
        last_heartbeat_at: Some(started_at),
        last_stdout_at: None,
        last_event_at: Some(started_at),
        reason: None,
    };
    store.upsert_worker(working)?;

    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    command.stdin(if spec.stdin_payload.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.envs(&spec.env);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn().map_err(|err| {
        let _ = store.append_runtime_event(
            &spec.run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::WorkerSpawnFailed,
                timestamp: Utc::now(),
                run_id: Some(spec.run_id.clone()),
                session_id: None,
                source: "workers".to_string(),
                worker: Some(spec.worker_id.clone()),
                task_id: task_id.clone(),
                message_id: None,
                reason: Some(err.to_string()),
                context: Map::new(),
            },
        );
        err.to_string()
    })?;

    if let Some(payload) = &spec.stdin_payload {
        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin
                .write_all(payload.as_bytes())
                .map_err(|err| err.to_string())?;
        }
    }

    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    let finished_at = Utc::now();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    let done_state = WorkerRecord {
        worker_id: spec.worker_id.clone(),
        run_id: spec.run_id.clone(),
        worker_kind: spec.worker_kind,
        session_ref: None,
        state: if success {
            WorkerState::Done
        } else {
            WorkerState::Failed
        },
        current_task_id: task_id.clone(),
        current_summary: Some(if success {
            "worker command completed".to_string()
        } else {
            "worker command failed".to_string()
        }),
        terminal_label: Some(spec.worker_id.clone()),
        last_heartbeat_at: Some(finished_at),
        last_stdout_at: Some(finished_at),
        last_event_at: Some(finished_at),
        reason: if success {
            None
        } else {
            Some(format!("exit_code={}", output.status.code().unwrap_or(-1)))
        },
    };
    store.upsert_worker(done_state)?;

    if let Some(task_id) = &task_id {
        let mut task = store.read_task(&spec.run_id, task_id)?;
        task.status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        task.completed_at = Some(finished_at);
        task.updated_at = finished_at;
        task.owner = None;
        task.claim = None;
        if success {
            let mut result = Map::new();
            result.insert(
                "summary".to_string(),
                Value::String("worker command completed".to_string()),
            );
            result.insert("stdout".to_string(), Value::String(stdout.clone()));
            result.insert("stderr".to_string(), Value::String(stderr.clone()));
            task.result = Some(Value::Object(result));
            task.error = None;
        } else {
            task.error = Some(stderr.clone());
        }
        store.write_task(&task)?;
        store.refresh_snapshot(&spec.run_id)?;
    }

    Ok(WorkerExecutionResult {
        worker_id: spec.worker_id,
        task_id,
        program: spec.program,
        args: spec.args,
        exit_code: output.status.code(),
        success,
        stdout,
        stderr,
        started_at,
        finished_at,
        duration_ms: start.elapsed().as_millis(),
    })
}
