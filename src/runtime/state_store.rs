use crate::runtime::types::{
    DispatchCounts, DispatchRecord, DispatchStatus, EventEnvelope, MailboxCounts, MailboxRecord,
    ReadinessState, ReplayState, RunPhase, RunRecord, RunSnapshot, RuntimeSnapshot, SCHEMA_VERSION,
    TaskCounts, TaskRecord, TaskStatus, WorkerProjection, WorkerRecord,
};
use chrono::{Duration, Utc};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn init_run(&self, run_id: &str, owner: &str) -> Result<RunRecord, String> {
        self.ensure_run_layout(run_id)?;

        let now = Utc::now();
        let authority = crate::runtime::types::AuthorityLease {
            owner: owner.to_string(),
            lease_id: format!("lease-{run_id}"),
            leased_until: now + Duration::minutes(5),
            stale: false,
        };
        let run = RunRecord {
            run_id: run_id.to_string(),
            active: true,
            current_phase: RunPhase::Starting,
            started_at: now,
            updated_at: now,
            completed_at: None,
            stop_reason: None,
            authority: Some(authority.clone()),
            snapshot_ref: Some("snapshot.json".to_string()),
        };

        self.write_json(&self.run_file(run_id), &run)?;
        self.write_json(
            &self.worker_file(run_id, owner),
            &WorkerRecord {
                worker_id: owner.to_string(),
                run_id: run_id.to_string(),
                worker_kind: crate::runtime::types::WorkerKind::Orchestrator,
                session_ref: None,
                state: crate::runtime::types::WorkerState::Idle,
                current_task_id: None,
                current_summary: Some("runtime initialized".to_string()),
                terminal_label: Some(owner.to_string()),
                last_heartbeat_at: Some(now),
                last_stdout_at: None,
                last_event_at: Some(now),
                reason: None,
            },
        )?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: crate::runtime::types::EventKind::AuthorityAcquired,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: Some(owner.to_string()),
                task_id: None,
                message_id: None,
                reason: None,
                context: serde_json::Map::new(),
            },
        )?;
        let snapshot = self.capture_snapshot(run_id)?;
        self.write_json(&self.snapshot_file(run_id), &snapshot)?;
        Ok(run)
    }

    pub fn read_snapshot(&self, run_id: &str) -> Result<RuntimeSnapshot, String> {
        self.read_json(&self.snapshot_file(run_id))
    }

    pub fn capture_snapshot(&self, run_id: &str) -> Result<RuntimeSnapshot, String> {
        let run: RunRecord = self.read_json(&self.run_file(run_id))?;
        let workers = self.read_workers(run_id)?;
        let tasks = self.read_tasks(run_id)?;
        let dispatch = self.read_dispatch_records(run_id)?;
        let mailbox = self.read_mailboxes(run_id)?;
        let pending_events = self.count_events(run_id)?;

        let mut task_counts = TaskCounts::zero();
        for task in &tasks {
            match task.status {
                TaskStatus::Pending => task_counts.pending += 1,
                TaskStatus::Blocked => task_counts.blocked += 1,
                TaskStatus::InProgress => task_counts.in_progress += 1,
                TaskStatus::Completed => task_counts.completed += 1,
                TaskStatus::Failed => task_counts.failed += 1,
            }
        }

        let mut dispatch_counts = DispatchCounts::zero();
        for record in &dispatch {
            match record.status {
                DispatchStatus::Pending => dispatch_counts.pending += 1,
                DispatchStatus::Notified => dispatch_counts.notified += 1,
                DispatchStatus::Delivered => dispatch_counts.delivered += 1,
                DispatchStatus::Failed => dispatch_counts.failed += 1,
            }
        }

        let unread = mailbox
            .iter()
            .flat_map(|entry| entry.records.iter())
            .filter(|message| message.delivered_at.is_none())
            .count();

        let worker_projections = workers
            .into_iter()
            .map(|worker| WorkerProjection {
                worker_id: worker.worker_id,
                worker_kind: worker.worker_kind,
                state: worker.state,
                current_task_id: worker.current_task_id,
                current_summary: worker.current_summary,
                last_heartbeat_at: worker.last_heartbeat_at,
                terminal_label: worker.terminal_label,
            })
            .collect::<Vec<_>>();

        let readiness = ReadinessState {
            ready: run.authority.is_some(),
            reasons: if run.authority.is_some() {
                Vec::new()
            } else {
                vec!["missing authority lease".to_string()]
            },
        };

        Ok(RuntimeSnapshot {
            schema_version: SCHEMA_VERSION,
            run: RunSnapshot {
                run_id: run.run_id,
                phase: run.current_phase,
                active: run.active,
                started_at: run.started_at,
                updated_at: run.updated_at,
            },
            authority: run.authority,
            workers: worker_projections,
            tasks: task_counts,
            dispatch: dispatch_counts,
            mailbox: MailboxCounts { unread },
            replay: ReplayState {
                cursor: None,
                pending_events,
            },
            readiness,
        })
    }

    fn ensure_run_layout(&self, run_id: &str) -> Result<(), String> {
        for path in [
            self.run_dir(run_id),
            self.run_dir(run_id).join("workers"),
            self.run_dir(run_id).join("tasks"),
            self.run_dir(run_id).join("dispatch"),
            self.run_dir(run_id).join("mailbox"),
            self.run_dir(run_id).join("memory"),
        ] {
            fs::create_dir_all(&path).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.root.join("runs").join(run_id)
    }

    fn run_file(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("run.json")
    }

    fn snapshot_file(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("snapshot.json")
    }

    fn event_log_file(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("events.jsonl")
    }

    fn worker_file(&self, run_id: &str, worker_id: &str) -> PathBuf {
        self.run_dir(run_id)
            .join("workers")
            .join(format!("{worker_id}.json"))
    }

    fn dispatch_dir(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("dispatch")
    }

    fn mailbox_dir(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("mailbox")
    }

    fn read_workers(&self, run_id: &str) -> Result<Vec<WorkerRecord>, String> {
        self.read_json_dir(&self.run_dir(run_id).join("workers"))
    }

    fn read_tasks(&self, run_id: &str) -> Result<Vec<TaskRecord>, String> {
        self.read_json_dir(&self.run_dir(run_id).join("tasks"))
    }

    fn read_dispatch_records(&self, run_id: &str) -> Result<Vec<DispatchRecord>, String> {
        self.read_json_dir(&self.dispatch_dir(run_id))
    }

    fn read_mailboxes(&self, run_id: &str) -> Result<Vec<MailboxRecord>, String> {
        self.read_json_dir(&self.mailbox_dir(run_id))
    }

    fn count_events(&self, run_id: &str) -> Result<usize, String> {
        let path = self.event_log_file(run_id);
        if !path.exists() {
            return Ok(0);
        }
        let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
        Ok(raw.lines().filter(|line| !line.trim().is_empty()).count())
    }

    fn append_event(&self, run_id: &str, event: EventEnvelope) -> Result<(), String> {
        let path = self.event_log_file(run_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| err.to_string())?;
        let line = serde_json::to_string(&event).map_err(|err| err.to_string())?;
        file.write_all(line.as_bytes())
            .map_err(|err| err.to_string())?;
        file.write_all(b"\n").map_err(|err| err.to_string())
    }

    fn read_json<T>(&self, path: &Path) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
        serde_json::from_str(&raw).map_err(|err| err.to_string())
    }

    fn read_json_dir<T>(&self, dir: &Path) -> Result<Vec<T>, String>
    where
        T: serde::de::DeserializeOwned,
    {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|err| err.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;
        entries.sort_by_key(|entry| entry.path());
        let mut items = Vec::new();
        for entry in entries {
            if entry.file_type().map_err(|err| err.to_string())?.is_file() {
                items.push(self.read_json(&entry.path())?);
            }
        }
        Ok(items)
    }

    fn write_json<T>(&self, path: &Path, value: &T) -> Result<(), String>
    where
        T: Serialize,
    {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let temp_path = path.with_extension(format!(
            "{}tmp",
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        let mut file = File::create(&temp_path).map_err(|err| err.to_string())?;
        serde_json::to_writer_pretty(&mut file, value).map_err(|err| err.to_string())?;
        file.write_all(b"\n").map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
        fs::rename(temp_path, path).map_err(|err| err.to_string())
    }
}
