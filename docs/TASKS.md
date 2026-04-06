# Tasks

## Goal

Tasks are the durable units of work owned by the runtime.

Workers do not own work implicitly. They own work only through claims recorded on tasks.

## Task Schema

Initial task schema:

```json
{
  "task_id": "7",
  "run_id": "run_123",
  "title": "verify the generated patch against tests",
  "description": "run the required checks and report failures",
  "status": "pending",
  "owner": null,
  "claim": null,
  "depends_on": [],
  "blocked_by": [],
  "created_at": "2026-04-06T12:00:00Z",
  "updated_at": "2026-04-06T12:00:00Z",
  "completed_at": null,
  "result": null,
  "error": null,
  "metadata": {}
}
```

Required fields:
- `task_id`
- `run_id`
- `title`
- `status`
- `created_at`
- `updated_at`

## Task Statuses

Frozen task statuses:
- `pending`
- `blocked`
- `in_progress`
- `completed`
- `failed`

Terminal statuses:
- `completed`
- `failed`

## Status Transition Rules

Allowed transitions:
- `pending -> in_progress`
- `pending -> blocked`
- `blocked -> pending`
- `blocked -> in_progress`
- `in_progress -> completed`
- `in_progress -> failed`
- `in_progress -> blocked`

Disallowed:
- any transition out of terminal state
- terminal completion without a valid claim

## Claim Schema

Claims are leases:

```json
{
  "owner": "worker-2",
  "token": "claim_abc123",
  "leased_until": "2026-04-06T12:05:00Z"
}
```

Rules:
- only one active claim may exist
- completion and failure require a valid current claim
- expired claims may be reclaimed
- claim conflicts must be explicit errors

## Dependencies

Tasks may depend on other tasks through `depends_on`.

Readiness rules:
- if any dependency is non-terminal, the task is not ready
- if any dependency failed, the task may remain blocked until orchestrator intervention
- readiness should be computable without reading worker logs

## Blocked State

Blocked tasks must include a reason in `metadata.block_reason` or `error`.

Blocked is not failure.

Blocked should be used for:
- waiting on another task
- waiting on user input
- waiting on external verification

## Result Contract

When a worker completes a task, it writes:
- `result.summary`
- `result.artifacts`
- `result.next_actions`
- optional `result.evidence`

The runtime does not need to interpret full natural language output to know task outcome.

## Task IDs

Use a simple stable id format:
- storage path may use `task-<id>.json`
- runtime API payload should expose bare `task_id`

This keeps the distinction between storage naming and API naming.
