use crate::runtime::state_store::StateStore;
use crate::runtime::types::{
    EventEnvelope, EventKind, SCHEMA_VERSION, TaskClaim, TaskRecord, TaskStatus,
};
use chrono::{Duration, Utc};
use serde_json::Map;

pub fn acquire_claim(
    store: &StateStore,
    run_id: &str,
    task_id: &str,
    owner: &str,
    lease_minutes: i64,
) -> Result<TaskRecord, String> {
    let mut task = store.read_task(run_id, task_id)?;
    if matches!(task.status, TaskStatus::Completed | TaskStatus::Failed) {
        return Err("cannot claim terminal task".to_string());
    }
    if let Some(existing) = &task.claim {
        if existing.leased_until > Utc::now() && existing.owner != owner {
            return Err(format!("task already claimed by {}", existing.owner));
        }
    }
    let now = Utc::now();
    task.owner = Some(owner.to_string());
    task.claim = Some(TaskClaim {
        owner: owner.to_string(),
        token: format!("claim-{task_id}-{owner}"),
        leased_until: now + Duration::minutes(lease_minutes.max(1)),
    });
    if matches!(task.status, TaskStatus::Pending | TaskStatus::Blocked) {
        task.status = TaskStatus::InProgress;
    }
    task.updated_at = now;
    store.write_task(&task)?;
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::WorkerStateChanged,
            timestamp: now,
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "claims".to_string(),
            worker: Some(owner.to_string()),
            task_id: Some(task_id.to_string()),
            message_id: None,
            reason: Some("claim_acquired".to_string()),
            context: Map::new(),
        },
    )?;
    store.refresh_snapshot(run_id)?;
    Ok(task)
}

pub fn release_claim(
    store: &StateStore,
    run_id: &str,
    task_id: &str,
    owner: &str,
) -> Result<TaskRecord, String> {
    let mut task = store.read_task(run_id, task_id)?;
    match &task.claim {
        Some(claim) if claim.owner == owner => {}
        Some(claim) => return Err(format!("task claim owned by {}", claim.owner)),
        None => return Err("task has no active claim".to_string()),
    }
    let now = Utc::now();
    task.claim = None;
    task.owner = None;
    if task.status == TaskStatus::InProgress {
        task.status = TaskStatus::Pending;
    }
    task.updated_at = now;
    store.write_task(&task)?;
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::WorkerStateChanged,
            timestamp: now,
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "claims".to_string(),
            worker: Some(owner.to_string()),
            task_id: Some(task_id.to_string()),
            message_id: None,
            reason: Some("claim_released".to_string()),
            context: Map::new(),
        },
    )?;
    store.refresh_snapshot(run_id)?;
    Ok(task)
}
