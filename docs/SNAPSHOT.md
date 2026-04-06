# Snapshot

## Goal

The snapshot is the current truth of the runtime.

HUD, hooks, notifications, and CLI status should all read from the same snapshot contract.

## Snapshot Schema

Initial snapshot shape:

```json
{
  "schema_version": 1,
  "run": {
    "run_id": "run_123",
    "phase": "executing",
    "active": true,
    "started_at": "2026-04-06T12:00:00Z",
    "updated_at": "2026-04-06T12:01:00Z"
  },
  "authority": {
    "owner": "orchestrator-1",
    "lease_id": "lease_1",
    "leased_until": "2026-04-06T12:05:00Z",
    "stale": false
  },
  "workers": [],
  "tasks": {
    "pending": 0,
    "blocked": 0,
    "in_progress": 0,
    "completed": 0,
    "failed": 0
  },
  "dispatch": {
    "pending": 0,
    "notified": 0,
    "delivered": 0,
    "failed": 0
  },
  "mailbox": {
    "unread": 0
  },
  "replay": {
    "cursor": null,
    "pending_events": 0
  },
  "readiness": {
    "ready": true,
    "reasons": []
  }
}
```

## Worker Projection

Each snapshot should include compact worker projections:
- `worker_id`
- `worker_kind`
- `state`
- `current_task_id`
- `current_summary`
- `last_heartbeat_at`
- `terminal_label`

This is the minimum HUD payload.

## Snapshot Update Policy

Rules:
- events are append-only
- snapshots are rewritten views of current truth
- snapshots may be regenerated from state
- snapshots must not require reading worker stdout

## Snapshot Consumers

Consumers:
- HUD
- notifications
- hook subscribers
- `conductor status`

All of them should be satisfied by the same schema.

## Repair Model

The observer plane may temporarily drift from the event stream.

Periodic snapshot refresh should repair drift without requiring runtime restarts.
