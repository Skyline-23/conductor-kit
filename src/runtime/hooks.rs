use crate::runtime::state_store::StateStore;
use crate::runtime::types::EventEnvelope;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn is_wakeable_event(event: &EventEnvelope) -> bool {
    matches!(
        event.event,
        crate::runtime::types::EventKind::WorkerStateChanged
            | crate::runtime::types::EventKind::MailboxMessageCreated
            | crate::runtime::types::EventKind::MailboxMessageDelivered
            | crate::runtime::types::EventKind::LeaderNotificationDeferred
            | crate::runtime::types::EventKind::VerificationPassed
            | crate::runtime::types::EventKind::VerificationFailed
    ) || matches!(
        event.reason.as_deref(),
        Some("all_workers_idle_prompted")
            | Some("blocked_reported_to_operator")
            | Some("worker_reported_to_main")
    )
}

pub fn filter_events(
    events: Vec<EventEnvelope>,
    event_name: Option<&str>,
    wakeable_only: bool,
) -> Vec<EventEnvelope> {
    events
        .into_iter()
        .filter(|event| {
            (!wakeable_only || is_wakeable_event(event))
                && event_name
                .map(|name| name == "*" || event_name_of(event) == name)
                .unwrap_or(true)
        })
        .collect()
}

pub fn event_name_of(event: &EventEnvelope) -> &'static str {
    match event.event {
        crate::runtime::types::EventKind::AuthorityAcquired => "authority_acquired",
        crate::runtime::types::EventKind::AuthorityRenewed => "authority_renewed",
        crate::runtime::types::EventKind::WorkerSpawned => "worker_spawned",
        crate::runtime::types::EventKind::WorkerSessionStarted => "worker_session_started",
        crate::runtime::types::EventKind::WorkerSessionStopped => "worker_session_stopped",
        crate::runtime::types::EventKind::WorkerStateChanged => "worker_state_changed",
        crate::runtime::types::EventKind::WorkerSpawnFailed => "worker_spawn_failed",
        crate::runtime::types::EventKind::WorkerHeartbeatStale => "worker_heartbeat_stale",
        crate::runtime::types::EventKind::WorkerStdoutStale => "worker_stdout_stale",
        crate::runtime::types::EventKind::DispatchQueued => "dispatch_queued",
        crate::runtime::types::EventKind::DispatchNotified => "dispatch_notified",
        crate::runtime::types::EventKind::DispatchDelivered => "dispatch_delivered",
        crate::runtime::types::EventKind::DispatchFailed => "dispatch_failed",
        crate::runtime::types::EventKind::MailboxMessageCreated => "mailbox_message_created",
        crate::runtime::types::EventKind::MailboxMessageNotified => "mailbox_message_notified",
        crate::runtime::types::EventKind::MailboxMessageDelivered => "mailbox_message_delivered",
        crate::runtime::types::EventKind::LeaderNotificationDeferred => "leader_notification_deferred",
        crate::runtime::types::EventKind::PhaseChanged => "phase_changed",
        crate::runtime::types::EventKind::VerificationPassed => "verification_passed",
        crate::runtime::types::EventKind::VerificationFailed => "verification_failed",
        crate::runtime::types::EventKind::SnapshotCaptured => "snapshot_captured",
    }
}

pub fn run_hook_command(
    event: &EventEnvelope,
    program: &str,
    args: &[String],
    cwd: Option<PathBuf>,
) -> Result<i32, String> {
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.env("CONDUCTOR_EVENT", event_name_of(event));
    if let Some(run_id) = &event.run_id {
        command.env("CONDUCTOR_RUN_ID", run_id);
    }
    if let Some(worker) = &event.worker {
        command.env("CONDUCTOR_WORKER_ID", worker);
    }

    let mut child = command.spawn().map_err(|err| err.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        let payload = serde_json::to_vec(event).map_err(|err| err.to_string())?;
        stdin.write_all(&payload).map_err(|err| err.to_string())?;
        stdin.write_all(b"\n").map_err(|err| err.to_string())?;
    }
    let status = child.wait().map_err(|err| err.to_string())?;
    Ok(status.code().unwrap_or(-1))
}

pub fn watch_and_run_hooks(
    store: &StateStore,
    run_id: &str,
    event_name: Option<&str>,
    program: &str,
    args: &[String],
    timeout_secs: u64,
    cwd: Option<PathBuf>,
) -> Result<usize, String> {
    let start = Instant::now();
    let mut cursor = 0usize;
    let mut handled = 0usize;
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        let events = store.read_events(run_id)?;
        if cursor < events.len() {
            let slice = events[cursor..].to_vec();
            for event in filter_events(slice, event_name, false) {
                let _ = run_hook_command(&event, program, args, cwd.clone())?;
                handled += 1;
            }
            cursor = events.len();
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(handled)
}
