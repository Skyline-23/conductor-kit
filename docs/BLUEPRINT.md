# Blueprint

## Conductor V2 Goal

Keep the core ideas:
- orchestrator
- worker
- loop
- state
- memory
- resume
- authority
- claim safety
- dispatch
- mailbox
- event bus
- HUD
- hooks
- notifications

Implementation constraints:
- no tmux-owned control path
- no giant base prompt
- no text-scraping as source of truth
- no hidden mutation paths

## Design Rule

Conductor V2 should be:
- thin prompt
- thick runtime
- direct IPC
- explicit state
- observable by subscription

This is the central inversion in this runtime design.

## Runtime Planes

### 1. Control Plane

Owns:
- authority lease
- phase progression
- worker registry
- dispatch backlog
- claim validation
- replay/resume

The control plane is authoritative.

### 2. Data Plane

Owns:
- worker stdin/stdout transport
- mailbox message delivery
- structured worker results
- heartbeat and liveness updates

The data plane moves messages and results, but does not define policy.

### 3. Observer Plane

Owns:
- HUD
- notifications
- hooks
- status surfaces

The observer plane is read-only against runtime truth.

This is the key separation the runtime needs to keep.

## Worker Model

Workers are not prompt personas. They are runtime processes with stable metadata.

Each worker should have:
- `worker_id`
- `run_id`
- `kind`
- `session_ref`
- `state`
- `current_task_id`
- `current_summary`
- `last_heartbeat_at`
- `terminal_label`

Initial worker kinds:
- `orchestrator`
- `worker`
- `verifier`

This is not a heavy role taxonomy. It is a small runtime taxonomy.

## Message Model

There are three layers:

### Task

A durable unit of work with ownership and claim safety.

### Dispatch

A delivery request with lifecycle:

`pending -> notified -> delivered|failed`

### Mailbox Message

The actual handoff payload between orchestrator and workers or worker-to-worker.

This separation prevents transport concerns from corrupting work ownership.

## State Model

Conductor V2 keeps durable state in a small number of families:
- run state
- worker state
- task state
- dispatch state
- mailbox state
- memory state
- note state

Rules:
- session-scoped state wins over root fallback
- state writes are atomic
- per-path locking is required
- schema versions are explicit

## Event Model

Everything observable should flow from events and snapshots.

### Events

Examples:
- `authority_acquired`
- `worker_spawned`
- `worker_state_changed`
- `dispatch_queued`
- `mailbox_message_created`
- `phase_changed`
- `verification_failed`

### Snapshot

A snapshot is current truth:
- who owns authority
- what phase is active
- how many tasks are pending
- which workers are blocked
- whether replay is pending

HUD and hooks consume this.

## HUD Model

The HUD is mandatory, but it must be cheap.

It should:
- subscribe to the event stream
- maintain a local projection
- periodically repair from snapshots

It should not:
- scrape pane content
- infer runtime truth from logs
- send control commands

The HUD is a projection, not the runtime.

## Hook Model

Hooks are mandatory, but they must remain downstream.

Hooks receive:
- native runtime events
- derived events
- lifecycle events

Hooks may:
- notify
- log
- augment observer state

Hooks may not:
- mutate authoritative runtime state directly
- bypass claim safety
- invent dispatch completion

## Instruction Model

The base AGENTS file should be short.

It should say:
- when to invoke orchestration
- when not to
- verify before trust
- runtime state is authoritative

It should not include:
- worker protocol details
- mailbox mechanics
- phase transition tables
- dispatch lifecycle rules
- hook envelopes

Those belong in runtime contracts and code.

## Performance Model

To keep the system usable:

### Allowed in V1
- one runtime process
- stdio worker spawning
- file-backed state
- file-backed mailbox
- file-backed event log
- snapshot file

### Delayed to V2+
- background daemons
- socket fan-out brokers
- distributed runtimes
- tmux compatibility shell
- rich dashboard UI

This keeps the first implementation small while preserving the right architecture.

## Success Criteria

Conductor V2 is only worth building if it meets these constraints:

1. Startup overhead
   Small tasks must not pay a heavy orchestration cost.
2. Runtime truth
   State must come from authoritative records, not terminal inference.
3. Operator trust
   HUD, hooks, and notifications must agree because they read the same snapshot.
4. Prompt load
   The base instruction surface must stay short.
5. Recovery
   Resume must work from durable state, not fragile textual heuristics.

## Build Order

1. runtime types
2. state store
3. authority + claim + phase
4. dispatch + mailbox
5. event log + snapshot
6. HUD subscriber
7. hooks + notifications
8. worker spawn/continue adapters

That is the conductor-kit redesign.
