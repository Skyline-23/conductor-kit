# Worker Protocol

## Goal

Workers need a small, fixed execution protocol.

The protocol should be runtime-enforced, not embedded in a huge prompt.

## Startup Contract

When a worker starts, it should receive:
- `run_id`
- `worker_id`
- `worker_kind`
- `session_ref`
- `task_id` or initial mailbox message
- working directory
- compact runtime instructions

The worker must reply with a startup acknowledgement.

Suggested ack payload:

```json
{
  "worker_id": "worker-2",
  "ack": true,
  "started_at": "2026-04-06T12:00:00Z"
}
```

## Worker Loop

Each worker loop is:
1. acknowledge startup
2. read current task or mailbox input
3. attempt task claim
4. execute bounded work
5. write progress or result
6. return to idle or request next work

## Task Claim

Before mutating task state or reporting completion, a worker must own the claim.

Worker-side sequence:
1. request claim for `task_id`
2. receive `claim_token`
3. begin work
4. renew or complete before lease expiry

## Progress Reporting

Workers may report progress by updating their worker state:
- `state`
- `current_task_id`
- `current_summary`
- `last_heartbeat_at`
- `reason`

This enables HUD without scraping stdout.

## Mailbox Polling

Workers may receive messages through mailbox delivery.

Polling rules for v1:
- poll mailbox between task completions
- poll mailbox when entering idle
- poll mailbox when explicitly nudged

Push delivery can be added later, but the protocol should not require it in v1.

## Completion Writeback

On success:
- task status -> `completed`
- result payload written
- worker state updated

On failure:
- task status -> `failed`
- error payload written
- worker state updated

On blocked:
- task status -> `blocked`
- block reason written
- worker state updated

## Worker States

Frozen worker states:
- `idle`
- `working`
- `blocked`
- `done`
- `failed`
- `stopped`
- `unknown`

## Restrictions

Workers may not:
- bypass claim checks
- mutate unrelated task files
- mark dispatch delivered without runtime confirmation
- invent authority ownership

## Orchestrator Handoff

Workers may hand work to another worker only through mailbox and dispatch.

They should not directly control another worker process.
