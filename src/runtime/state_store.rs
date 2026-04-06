use crate::runtime::types::{
    DispatchCounts, DispatchRecord, DispatchStatus, EventEnvelope, EventKind, MailboxCounts,
    MailboxMessage, MailboxRecord, ReadinessState, ReplayState, RunPhase, RunRecord, RunSnapshot,
    RuntimeSnapshot, SCHEMA_VERSION, SessionRecord, TaskCounts, TaskRecord, TaskStatus,
    WorkerProjection, WorkerRecord,
};
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::{Map, Value};
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

    pub fn read_run(&self, run_id: &str) -> Result<RunRecord, String> {
        self.read_json(&self.run_file(run_id))
    }

    pub fn write_run(&self, run: &RunRecord) -> Result<(), String> {
        self.write_json(&self.run_file(&run.run_id), run)
    }

    pub fn read_worker(&self, run_id: &str, worker_id: &str) -> Result<WorkerRecord, String> {
        self.read_json(&self.worker_file(run_id, worker_id))
    }

    pub fn read_task(&self, run_id: &str, task_id: &str) -> Result<TaskRecord, String> {
        self.read_json(&self.task_file(run_id, task_id))
    }

    pub fn write_task(&self, task: &TaskRecord) -> Result<(), String> {
        self.write_json(&self.task_file(&task.run_id, &task.task_id), task)
    }

    pub fn complete_task(
        &self,
        run_id: &str,
        task_id: &str,
        summary: &str,
        evidence: Value,
    ) -> Result<TaskRecord, String> {
        let mut task = self.read_task(run_id, task_id)?;
        let now = Utc::now();
        task.status = TaskStatus::Completed;
        task.completed_at = Some(now);
        task.updated_at = now;
        task.owner = None;
        task.claim = None;
        task.error = None;
        let mut result = Map::new();
        result.insert("summary".to_string(), Value::String(summary.to_string()));
        result.insert("evidence".to_string(), evidence);
        task.result = Some(Value::Object(result));
        self.write_task(&task)?;
        self.refresh_snapshot(run_id)?;
        Ok(task)
    }

    pub fn fail_task(
        &self,
        run_id: &str,
        task_id: &str,
        error: &str,
    ) -> Result<TaskRecord, String> {
        let mut task = self.read_task(run_id, task_id)?;
        let now = Utc::now();
        task.status = TaskStatus::Failed;
        task.completed_at = Some(now);
        task.updated_at = now;
        task.owner = None;
        task.claim = None;
        task.error = Some(error.to_string());
        self.write_task(&task)?;
        self.refresh_snapshot(run_id)?;
        Ok(task)
    }

    pub fn append_runtime_event(&self, run_id: &str, event: EventEnvelope) -> Result<(), String> {
        self.append_event(run_id, event)
    }

    pub fn read_dispatch(&self, run_id: &str, request_id: &str) -> Result<DispatchRecord, String> {
        self.read_json(&self.dispatch_file(run_id, request_id))
    }

    pub fn read_events(&self, run_id: &str) -> Result<Vec<EventEnvelope>, String> {
        let path = self.event_log_file(run_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<EventEnvelope>(line).map_err(|err| err.to_string()))
            .collect()
    }

    pub fn refresh_snapshot(&self, run_id: &str) -> Result<RuntimeSnapshot, String> {
        let snapshot = self.capture_snapshot(run_id)?;
        self.write_json(&self.snapshot_file(run_id), &snapshot)?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::SnapshotCaptured,
                timestamp: Utc::now(),
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: None,
                task_id: None,
                message_id: None,
                reason: None,
                context: Map::new(),
            },
        )?;
        Ok(snapshot)
    }

    pub fn upsert_worker(&self, worker: WorkerRecord) -> Result<WorkerRecord, String> {
        let run_id = worker.run_id.clone();
        let worker_id = worker.worker_id.clone();
        self.write_json(&self.worker_file(&run_id, &worker_id), &worker)?;
        self.append_event(
            &run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::WorkerStateChanged,
                timestamp: Utc::now(),
                run_id: Some(run_id.clone()),
                session_id: None,
                source: "runtime".to_string(),
                worker: Some(worker_id),
                task_id: worker.current_task_id.clone(),
                message_id: None,
                reason: worker.reason.clone(),
                context: Map::new(),
            },
        )?;
        self.refresh_snapshot(&run_id)?;
        Ok(worker)
    }

    pub fn create_task(
        &self,
        run_id: &str,
        task_id: &str,
        title: &str,
        description: Option<String>,
    ) -> Result<TaskRecord, String> {
        let now = Utc::now();
        let task = TaskRecord {
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            title: title.to_string(),
            description,
            status: TaskStatus::Pending,
            owner: None,
            claim: None,
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            result: None,
            error: None,
            metadata: Map::new(),
        };
        self.write_json(&self.task_file(run_id, task_id), &task)?;
        self.refresh_snapshot(run_id)?;
        Ok(task)
    }

    pub fn queue_dispatch(
        &self,
        run_id: &str,
        request_id: &str,
        target: &str,
        metadata: Map<String, Value>,
    ) -> Result<DispatchRecord, String> {
        let now = Utc::now();
        let record = DispatchRecord {
            request_id: request_id.to_string(),
            run_id: run_id.to_string(),
            target: target.to_string(),
            status: DispatchStatus::Pending,
            attempt_count: 0,
            created_at: now,
            updated_at: now,
            notified_at: None,
            delivered_at: None,
            failed_at: None,
            last_reason: None,
            metadata,
        };
        self.write_json(&self.dispatch_file(run_id, request_id), &record)?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::DispatchQueued,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: None,
                task_id: None,
                message_id: None,
                reason: None,
                context: Map::new(),
            },
        )?;
        self.refresh_snapshot(run_id)?;
        Ok(record)
    }

    pub fn update_dispatch_status(
        &self,
        run_id: &str,
        request_id: &str,
        status: DispatchStatus,
        reason: Option<String>,
    ) -> Result<DispatchRecord, String> {
        let mut record: DispatchRecord = self.read_json(&self.dispatch_file(run_id, request_id))?;
        let now = Utc::now();
        record.status = status.clone();
        record.updated_at = now;
        match status {
            DispatchStatus::Pending => {}
            DispatchStatus::Notified => {
                record.attempt_count += 1;
                record.notified_at = Some(now);
            }
            DispatchStatus::Delivered => {
                record.delivered_at = Some(now);
            }
            DispatchStatus::Failed => {
                record.failed_at = Some(now);
            }
        }
        record.last_reason = reason.clone();
        self.write_json(&self.dispatch_file(run_id, request_id), &record)?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: match status {
                    DispatchStatus::Pending => EventKind::DispatchQueued,
                    DispatchStatus::Notified => EventKind::DispatchNotified,
                    DispatchStatus::Delivered => EventKind::DispatchDelivered,
                    DispatchStatus::Failed => EventKind::DispatchFailed,
                },
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: None,
                task_id: None,
                message_id: None,
                reason,
                context: Map::new(),
            },
        )?;
        self.refresh_snapshot(run_id)?;
        Ok(record)
    }

    pub fn create_mailbox_message(
        &self,
        run_id: &str,
        message_id: &str,
        from_worker: &str,
        to_worker: &str,
        body: &str,
    ) -> Result<MailboxMessage, String> {
        let mailbox_path = self.mailbox_file(run_id, to_worker);
        let mut mailbox = if mailbox_path.exists() {
            self.read_json::<MailboxRecord>(&mailbox_path)?
        } else {
            MailboxRecord {
                worker_id: to_worker.to_string(),
                records: Vec::new(),
            }
        };
        let now = Utc::now();
        let message = MailboxMessage {
            message_id: message_id.to_string(),
            run_id: run_id.to_string(),
            from_worker: from_worker.to_string(),
            to_worker: to_worker.to_string(),
            body: body.to_string(),
            created_at: now,
            notified_at: None,
            delivered_at: None,
        };
        mailbox.records.push(message.clone());
        self.write_json(&mailbox_path, &mailbox)?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: EventKind::MailboxMessageCreated,
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: Some(to_worker.to_string()),
                task_id: None,
                message_id: Some(message_id.to_string()),
                reason: None,
                context: Map::new(),
            },
        )?;
        self.refresh_snapshot(run_id)?;
        Ok(message)
    }

    pub fn update_mailbox_status(
        &self,
        run_id: &str,
        worker_id: &str,
        message_id: &str,
        delivered: bool,
    ) -> Result<MailboxMessage, String> {
        let mailbox_path = self.mailbox_file(run_id, worker_id);
        let mut mailbox: MailboxRecord = self.read_json(&mailbox_path)?;
        let now = Utc::now();
        let message = mailbox
            .records
            .iter_mut()
            .find(|record| record.message_id == message_id)
            .ok_or_else(|| format!("mailbox message not found: {message_id}"))?;
        if delivered {
            message.delivered_at = Some(now);
        } else {
            message.notified_at = Some(now);
        }
        let message = message.clone();
        self.write_json(&mailbox_path, &mailbox)?;
        self.append_event(
            run_id,
            EventEnvelope {
                schema_version: SCHEMA_VERSION,
                event: if delivered {
                    EventKind::MailboxMessageDelivered
                } else {
                    EventKind::MailboxMessageNotified
                },
                timestamp: now,
                run_id: Some(run_id.to_string()),
                session_id: None,
                source: "runtime".to_string(),
                worker: Some(worker_id.to_string()),
                task_id: None,
                message_id: Some(message_id.to_string()),
                reason: None,
                context: Map::new(),
            },
        )?;
        self.refresh_snapshot(run_id)?;
        Ok(message)
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
            self.run_dir(run_id).join("sessions"),
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

    pub fn session_dir(&self, run_id: &str, session_id: &str) -> PathBuf {
        self.run_dir(run_id).join("sessions").join(session_id)
    }

    pub fn session_file(&self, run_id: &str, session_id: &str) -> PathBuf {
        self.session_dir(run_id, session_id).join("session.json")
    }

    pub fn write_session(&self, session: &SessionRecord) -> Result<(), String> {
        self.write_json(
            &self.session_file(&session.run_id, &session.session_id),
            session,
        )
    }

    pub fn read_session(&self, run_id: &str, session_id: &str) -> Result<SessionRecord, String> {
        self.read_json(&self.session_file(run_id, session_id))
    }

    pub fn delete_worker(&self, run_id: &str, worker_id: &str) -> Result<(), String> {
        let path = self.worker_file(run_id, worker_id);
        if path.exists() {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn delete_session(&self, run_id: &str, session_id: &str) -> Result<(), String> {
        let dir = self.session_dir(run_id, session_id);
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn list_worker_ids(&self, run_id: &str) -> Result<Vec<String>, String> {
        self.list_json_stems(&self.run_dir(run_id).join("workers"))
    }

    pub fn list_session_ids(&self, run_id: &str) -> Result<Vec<String>, String> {
        let dir = self.run_dir(run_id).join("sessions");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|err| err.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;
        entries.sort_by_key(|entry| entry.path());
        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_dir())
                    .and_then(|_| entry.file_name().into_string().ok())
            })
            .collect())
    }

    fn task_file(&self, run_id: &str, task_id: &str) -> PathBuf {
        self.run_dir(run_id)
            .join("tasks")
            .join(format!("{task_id}.json"))
    }

    fn dispatch_dir(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("dispatch")
    }

    fn dispatch_file(&self, run_id: &str, request_id: &str) -> PathBuf {
        self.dispatch_dir(run_id).join(format!("{request_id}.json"))
    }

    fn mailbox_dir(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join("mailbox")
    }

    fn mailbox_file(&self, run_id: &str, worker_id: &str) -> PathBuf {
        self.mailbox_dir(run_id).join(format!("{worker_id}.json"))
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

    fn list_json_stems(&self, dir: &Path) -> Result<Vec<String>, String> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|err| err.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;
        entries.sort_by_key(|entry| entry.path());
        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_file())
                    .and_then(|_| {
                        entry
                            .path()
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .map(|stem| stem.to_string())
                    })
            })
            .collect())
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
