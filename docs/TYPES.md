# Types

## Goal

This document freezes the initial Rust naming contract for the runtime.

It exists to prevent implementation drift while the runtime is still being built.

## Module Layout

Suggested initial module layout:

```text
src/
  main.rs
  runtime/
    mod.rs
    types.rs
    state_store.rs
    event_log.rs
    snapshot.rs
    authority.rs
    claims.rs
    dispatch.rs
    mailbox.rs
    workers.rs
```

V1 should keep the module graph shallow.

## Core Identifiers

Use plain string ids with lightweight newtypes only if needed later.

Initial id fields:
- `run_id`
- `session_id`
- `worker_id`
- `task_id`
- `request_id`
- `message_id`
- `lease_id`
- `claim_token`

## Enums

### RunPhase

```rust
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
```

### WorkerState

```rust
pub enum WorkerState {
    Idle,
    Working,
    Blocked,
    Done,
    Failed,
    Stopped,
    Unknown,
}
```

### TaskStatus

```rust
pub enum TaskStatus {
    Pending,
    Blocked,
    InProgress,
    Completed,
    Failed,
}
```

### DispatchStatus

```rust
pub enum DispatchStatus {
    Pending,
    Notified,
    Delivered,
    Failed,
}
```

### WorkerKind

```rust
pub enum WorkerKind {
    Orchestrator,
    Worker,
    Verifier,
}
```

### EventKind

```rust
pub enum EventKind {
    AuthorityAcquired,
    AuthorityRenewed,
    WorkerSpawned,
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
    PhaseChanged,
    VerificationPassed,
    VerificationFailed,
    SnapshotCaptured,
}
```

## Structs

### AuthorityLease

```rust
pub struct AuthorityLease {
    pub owner: String,
    pub lease_id: String,
    pub leased_until: String,
    pub stale: bool,
}
```

### TaskClaim

```rust
pub struct TaskClaim {
    pub owner: String,
    pub token: String,
    pub leased_until: String,
}
```

### TaskRecord

```rust
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
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

### WorkerRecord

```rust
pub struct WorkerRecord {
    pub worker_id: String,
    pub run_id: String,
    pub worker_kind: WorkerKind,
    pub session_ref: Option<String>,
    pub state: WorkerState,
    pub current_task_id: Option<String>,
    pub current_summary: Option<String>,
    pub terminal_label: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub last_stdout_at: Option<String>,
    pub last_event_at: Option<String>,
    pub reason: Option<String>,
}
```

### DispatchRecord

```rust
pub struct DispatchRecord {
    pub request_id: String,
    pub run_id: String,
    pub target: String,
    pub status: DispatchStatus,
    pub attempt_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub notified_at: Option<String>,
    pub delivered_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_reason: Option<String>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

### MailboxMessage

```rust
pub struct MailboxMessage {
    pub message_id: String,
    pub run_id: String,
    pub from_worker: String,
    pub to_worker: String,
    pub body: String,
    pub created_at: String,
    pub notified_at: Option<String>,
    pub delivered_at: Option<String>,
}
```

### EventEnvelope

```rust
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event: EventKind,
    pub timestamp: String,
    pub run_id: Option<String>,
    pub session_id: Option<String>,
    pub source: String,
    pub worker: Option<String>,
    pub task_id: Option<String>,
    pub message_id: Option<String>,
    pub reason: Option<String>,
    pub context: serde_json::Map<String, serde_json::Value>,
}
```

### RuntimeSnapshot

```rust
pub struct RuntimeSnapshot {
    pub schema_version: u32,
    pub run: serde_json::Value,
    pub authority: serde_json::Value,
    pub workers: Vec<serde_json::Value>,
    pub tasks: serde_json::Value,
    pub dispatch: serde_json::Value,
    pub mailbox: serde_json::Value,
    pub replay: serde_json::Value,
    pub readiness: serde_json::Value,
}
```

## Serialization Rules

Rules:
- prefer `snake_case` field names in JSON
- prefer explicit enums over free-form strings in Rust
- derive `Serialize` and `Deserialize` on all persisted runtime records
- version every persisted top-level record family

## Mutation Boundaries

Initial ownership:
- `authority.rs` owns `AuthorityLease`
- `claims.rs` owns `TaskClaim` mutation rules
- `dispatch.rs` owns `DispatchRecord`
- `mailbox.rs` owns `MailboxMessage`
- `snapshot.rs` owns `RuntimeSnapshot`
- `state_store.rs` owns file layout and atomic persistence

## Non-Goals For V1

Do not introduce yet:
- generic actor frameworks
- async trait hierarchies
- transport abstraction layers with many implementations
- deeply nested generic event payload systems

Keep the first implementation explicit and concrete.
