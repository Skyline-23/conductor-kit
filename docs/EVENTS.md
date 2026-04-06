# Events

## Goal

HUD, notifications, hooks, and external observers should subscribe to runtime truth.

They should not reconstruct truth by scraping terminal output.

## Event Bus

Conductor needs a first-class event bus.

The event bus should carry:
- lifecycle events
- dispatch events
- mailbox events
- worker state events
- verification events
- snapshot events

## Event Envelope

Each event should use a stable envelope.

Suggested envelope:

```json
{
  "schema_version": 1,
  "event": "dispatch_queued",
  "timestamp": "2026-04-06T12:00:00Z",
  "run_id": "run_123",
  "session_id": "sess_123",
  "source": "runtime",
  "worker": "worker-2",
  "context": {}
}
```

Required fields:
- `schema_version`
- `event`
- `timestamp`
- `source`
- `context`

Optional fields:
- `run_id`
- `session_id`
- `worker`
- `task_id`
- `message_id`
- `reason`

## Runtime Commands

The runtime should distinguish commands from events.

Suggested command family:
- `acquire-authority`
- `renew-authority`
- `spawn-worker`
- `queue-dispatch`
- `mark-notified`
- `mark-delivered`
- `mark-failed`
- `capture-snapshot`
- `request-replay`

## Runtime Events

Suggested event family:
- `authority_acquired`
- `authority_renewed`
- `worker_spawned`
- `worker_state_changed`
- `dispatch_queued`
- `dispatch_notified`
- `dispatch_delivered`
- `dispatch_failed`
- `mailbox_message_created`
- `mailbox_message_notified`
- `mailbox_message_delivered`
- `phase_changed`
- `verification_passed`
- `verification_failed`
- `snapshot_captured`

## Snapshots

Snapshots are current truth, not an append-only log.

Each snapshot should expose:
- authority
- backlog counts
- worker states
- current phase
- readiness
- replay metadata

This is the data HUD and status surfaces should render.

## Dispatch Lifecycle

The dispatch lifecycle should stay frozen:

`pending -> notified -> delivered|failed`

This keeps the event model simple enough to maintain.

## Mailbox Lifecycle

Mailbox messages should carry:
- `message_id`
- `from_worker`
- `to_worker`
- `body`
- `created_at`
- `notified_at`
- `delivered_at`

The runtime owns delivery status. Observers only read it.
