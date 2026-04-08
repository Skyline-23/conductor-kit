use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Starting,
    Discovering,
    Spawning,
    Executing,
    Verifying,
    Fixing,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerState {
    Idle,
    Working,
    AwaitingReport,
    Blocked,
    Done,
    DonePendingVerification,
    VerifiedComplete,
    Failed,
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Blocked,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Notified,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerKind {
    Orchestrator,
    Worker,
    Verifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    AuthorityAcquired,
    AuthorityRenewed,
    ClaimReclaimed,
    WorkerBootstrapStarted,
    WorkerSpawned,
    WorkerSessionStarted,
    WorkerSessionStopped,
    WorkerStateChanged,
    WorkerSpawnFailed,
    WorkerHeartbeatStale,
    WorkerStdoutStale,
    DispatchQueued,
    DispatchNotified,
    DispatchDelivered,
    DispatchFailed,
    MailboxMessageCreated,
    MailboxMessageNotified,
    MailboxMessageDelivered,
    HandoffNeeded,
    LeaderNotificationDeferred,
    PhaseChanged,
    VerificationPassed,
    VerificationFailed,
    SnapshotCaptured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityLease {
    pub owner: String,
    pub lease_id: String,
    pub leased_until: DateTime<Utc>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClaim {
    pub owner: String,
    pub token: String,
    pub leased_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub active: bool,
    pub current_phase: RunPhase,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub stop_reason: Option<String>,
    pub authority: Option<AuthorityLease>,
    pub snapshot_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub run_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub claim: Option<TaskClaim>,
    pub depends_on: Vec<String>,
    pub blocked_by: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub approval_status: Option<ApprovalStatus>,
    pub approval_reason: Option<String>,
    pub approval_reviewer: Option<String>,
    pub approval_updated_at: Option<DateTime<Utc>>,
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub worker_id: String,
    pub run_id: String,
    pub worker_kind: WorkerKind,
    pub session_ref: Option<String>,
    pub state: WorkerState,
    pub current_task_id: Option<String>,
    pub current_summary: Option<String>,
    pub terminal_label: Option<String>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub last_stdout_at: Option<DateTime<Utc>>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRecord {
    pub request_id: String,
    pub run_id: String,
    pub target: String,
    pub status: DispatchStatus,
    pub attempt_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub last_reason: Option<String>,
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxMessage {
    pub message_id: String,
    pub run_id: String,
    pub from_worker: String,
    pub to_worker: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxRecord {
    pub worker_id: String,
    pub records: Vec<MailboxMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event: EventKind,
    pub timestamp: DateTime<Utc>,
    pub run_id: Option<String>,
    pub session_id: Option<String>,
    pub source: String,
    pub worker: Option<String>,
    pub task_id: Option<String>,
    pub message_id: Option<String>,
    pub reason: Option<String>,
    pub context: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerProjection {
    pub worker_id: String,
    pub worker_kind: WorkerKind,
    pub state: WorkerState,
    pub current_task_id: Option<String>,
    pub current_summary: Option<String>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub terminal_label: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Running,
    Exited,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub run_id: String,
    pub worker_id: String,
    pub session_id: String,
    pub socket_path: String,
    pub stdout_path: String,
    pub stderr_path: String,
    pub pid: u32,
    pub child_pid: Option<u32>,
    pub program: String,
    pub args: Vec<String>,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub exited_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCounts {
    pub pending: usize,
    pub blocked: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchCounts {
    pub pending: usize,
    pub notified: usize,
    pub delivered: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxCounts {
    pub unread: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayState {
    pub cursor: Option<String>,
    pub pending_events: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessState {
    pub ready: bool,
    pub reasons: Vec<String>,
    pub pending_approvals: usize,
    pub stale_operator: bool,
    pub silent_workers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub leader_stale: bool,
    pub all_workers_idle: bool,
    pub bootstrapping_workers: Vec<String>,
    pub verification_gaps: usize,
    pub non_reporting_workers: Vec<String>,
    pub reclaimed_claims: usize,
    pub pending_handoffs: usize,
    pub active_handoff: Option<String>,
    pub pending_leader_notifications: usize,
    pub leader_nudge_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorDecision {
    pub next_action: String,
    pub focus_worker: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub run_id: String,
    pub phase: RunPhase,
    pub active: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub schema_version: u32,
    pub run: RunSnapshot,
    pub authority: Option<AuthorityLease>,
    pub workers: Vec<WorkerProjection>,
    pub tasks: TaskCounts,
    pub dispatch: DispatchCounts,
    pub mailbox: MailboxCounts,
    pub replay: ReplayState,
    pub readiness: ReadinessState,
    pub monitor: MonitorState,
    pub decision: OperatorDecision,
}

impl TaskCounts {
    pub fn zero() -> Self {
        Self {
            pending: 0,
            blocked: 0,
            in_progress: 0,
            completed: 0,
            failed: 0,
        }
    }
}

impl DispatchCounts {
    pub fn zero() -> Self {
        Self {
            pending: 0,
            notified: 0,
            delivered: 0,
            failed: 0,
        }
    }
}
