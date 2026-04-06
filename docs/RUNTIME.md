# Runtime

## Goal

The conductor runtime should preserve the orchestration concepts from `oh-my-codex`
without inheriting its tmux-heavy control path.

The runtime owns:
- authority
- worker lifecycle
- phase progression
- dispatch backlog
- claim safety
- snapshots
- resume

## Core Model

### Orchestrator

There is one active orchestrator authority per run.

The orchestrator:
- starts or continues workers
- decides phase transitions
- owns final convergence
- decides whether to continue or close

### Worker

A worker is a spawned agent process that performs a bounded task.

A worker:
- receives a task or mailbox message
- claims a unit of work before mutating it
- reports progress and completion
- can become idle, blocked, failed, or stopped

### Run

A run is the durable unit of orchestration.

Each run should have:
- `run_id`
- `created_at`
- `updated_at`
- `authority`
- `phase`
- `workers`
- `dispatch_backlog`
- `snapshot`

## Authority

Authority should be explicit and leased.

Suggested fields:
- `owner`
- `lease_id`
- `leased_until`
- `stale`

Invariants:
- exactly one semantic authority owner is active at a time
- authority can be renewed without changing ownership
- stale authority must be detectable from persisted state

## Phase Model

The runtime needs a frozen phase vocabulary rather than ad hoc strings.

Initial conductor phase vocabulary:
- `starting`
- `discovering`
- `spawning`
- `executing`
- `verifying`
- `fixing`
- `complete`
- `failed`
- `cancelled`

Transitions must be explicit and recorded.

## Worker Ownership And Claim Safety

Task ownership must be lease-safe.

Each claim should include:
- `owner`
- `token`
- `leased_until`

Rules:
- a task cannot be terminally completed without a valid claim
- active work cannot be stolen while a valid claim lease exists
- reclaim is allowed only for expired, released, or dead-owner claims

## Resume

Resume is not a convenience feature. It is a runtime contract.

Resume requires:
- persisted run state
- persisted phase
- persisted worker registry
- persisted dispatch backlog
- persisted continuation metadata

If the runtime restarts, it should reconstruct the current run from durable state rather than infer from terminal text.

## Scope Precedence

Root and session-scoped state should preserve scope precedence.

Policy:
1. session-scoped state is authoritative when present
2. root state is compatibility fallback only
3. writes target one authoritative scope
4. unrelated sessions must never be mutated as a side effect

## Transport

Preferred order:
1. `stdio`
2. `unix_socket`
3. `tcp`

`tmux` is not a runtime transport.

## Lightweight Rule

The runtime must stay lightweight by keeping the core small:
- no terminal injection in the core
- no HUD rendering logic in the core
- no notification transport logic in the core

The core emits events and snapshots. Subscribers render them.
