use crate::runtime::state_store::StateStore;
use crate::runtime::types::{EventEnvelope, EventKind, RunPhase, RunRecord, SCHEMA_VERSION};
use chrono::Utc;
use serde_json::{Map, Value};

pub fn transition_phase(
    store: &StateStore,
    run_id: &str,
    next_phase: RunPhase,
    reason: Option<String>,
) -> Result<RunRecord, String> {
    let mut run = store.read_run(run_id)?;
    let prev_phase = run.current_phase.clone();
    validate_phase_transition(&prev_phase, &next_phase)?;
    let now = Utc::now();
    run.current_phase = next_phase.clone();
    run.updated_at = now;
    if matches!(
        next_phase,
        RunPhase::Complete | RunPhase::Failed | RunPhase::Cancelled
    ) {
        run.active = false;
        if run.completed_at.is_none() {
            run.completed_at = Some(now);
        }
        run.stop_reason = reason.clone();
    }
    store.write_run(&run)?;
    let mut context = Map::new();
    context.insert(
        "from".to_string(),
        Value::String(phase_name(&prev_phase).to_string()),
    );
    context.insert(
        "to".to_string(),
        Value::String(phase_name(&next_phase).to_string()),
    );
    store.append_runtime_event(
        run_id,
        EventEnvelope {
            schema_version: SCHEMA_VERSION,
            event: EventKind::PhaseChanged,
            timestamp: now,
            run_id: Some(run_id.to_string()),
            session_id: None,
            source: "phases".to_string(),
            worker: None,
            task_id: None,
            message_id: None,
            reason,
            context,
        },
    )?;
    store.refresh_snapshot(run_id)?;
    Ok(run)
}

fn validate_phase_transition(current: &RunPhase, next: &RunPhase) -> Result<(), String> {
    if current == next {
        return Ok(());
    }
    let valid = matches!(
        (current, next),
        (RunPhase::Starting, RunPhase::Discovering)
            | (RunPhase::Starting, RunPhase::Spawning)
            | (RunPhase::Discovering, RunPhase::Spawning)
            | (RunPhase::Spawning, RunPhase::Executing)
            | (RunPhase::Executing, RunPhase::Verifying)
            | (RunPhase::Executing, RunPhase::Fixing)
            | (RunPhase::Verifying, RunPhase::Fixing)
            | (RunPhase::Verifying, RunPhase::Complete)
            | (RunPhase::Fixing, RunPhase::Executing)
            | (_, RunPhase::Failed)
            | (_, RunPhase::Cancelled)
    );
    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid phase transition: {} -> {}",
            phase_name(current),
            phase_name(next)
        ))
    }
}

fn phase_name(phase: &RunPhase) -> &'static str {
    match phase {
        RunPhase::Starting => "starting",
        RunPhase::Discovering => "discovering",
        RunPhase::Spawning => "spawning",
        RunPhase::Executing => "executing",
        RunPhase::Verifying => "verifying",
        RunPhase::Fixing => "fixing",
        RunPhase::Complete => "complete",
        RunPhase::Failed => "failed",
        RunPhase::Cancelled => "cancelled",
    }
}
