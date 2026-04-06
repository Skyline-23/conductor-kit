# State

## Goal

State must be authoritative, scoped, and mutation-safe.

The runtime should not infer state from terminal text if a persisted record already exists.

## State Families

Conductor should separate these state families:
- run state
- worker state
- dispatch state
- mailbox state
- memory state
- operator note state

## Scope Precedence

Use the scope rule directly:

1. session-scoped state is authoritative when present
2. root state is fallback only
3. writes target one scope
4. unrelated sessions must never be mutated

## Run State

Suggested file content:
- `run_id`
- `active`
- `current_phase`
- `started_at`
- `updated_at`
- `completed_at`
- `stop_reason`
- `authority`
- `snapshot_ref`

## Worker State

Suggested file content:
- `worker_id`
- `state`
- `current_task_id`
- `claim`
- `last_heartbeat_at`
- `last_stdout_at`
- `last_event_at`
- `reason`

## Dispatch State

Dispatch records should include:
- `request_id`
- `target`
- `status`
- `attempt_count`
- `created_at`
- `updated_at`
- `notified_at`
- `delivered_at`
- `failed_at`
- `last_reason`

Status must stay inside:
- `pending`
- `notified`
- `delivered`
- `failed`

## Mailbox State

Mailbox messages should include:
- `message_id`
- `from_worker`
- `to_worker`
- `body`
- `created_at`
- `notified_at`
- `delivered_at`

The mailbox is durable coordination state, not just a convenience buffer.

## Claim Safety

Task ownership is lease-based.

A claim includes:
- `owner`
- `token`
- `leased_until`

State transitions must reject:
- completion without a valid claim
- conflicting claims
- stale writers trying to overwrite current truth

## Policy Versus Governance

Transport/runtime policy should stay separate from governance.

Policy examples:
- spawn policy
- continue policy
- dispatch mode
- transport mode

Governance examples:
- one orchestrator per session
- nested teams disallowed
- cleanup requires inactive workers
- approval required for specific mutations

These should not live in the same bucket.

## Memory Versus Notepad

Structured project memory should stay separate from operator notes.

Conductor should keep that distinction:

### Project memory
- structured
- mergeable
- durable across runs

### Operator notes
- temporary
- timestamped
- optionally prunable

This split keeps runtime memory useful without turning it into an append-only dump.

## Mutation Discipline

State mutation should happen through a narrow runtime API only.

Requirements:
- atomic writes
- per-path locking
- stable schema versions
- recovery-safe partial failure behavior
