# Hooks

## Goal

Hooks are required, but they must remain subscribers to runtime events.

They should not become a second runtime.

## Hook Sources

Hooks may be triggered by:
- native runtime events
- derived runtime events
- user lifecycle events

Examples:
- `session-start`
- `session-idle`
- `ask-user-question`
- `stop`
- `session-end`
- `needs-input`
- `pre-tool-use`
- `post-tool-use`

## Hook Envelope

Hooks should receive a stable event envelope with:
- `schema_version`
- `event`
- `timestamp`
- `source`
- `context`
- optional `session_id`
- optional `run_id`
- optional `worker`

Derived events should include:
- `confidence`
- `parser_reason`

## Hook Responsibilities

Hooks may:
- send notifications
- update HUD side channels
- write debug logs
- request observer-side actions

Hooks may not:
- mutate authoritative runtime state directly
- bypass claim safety
- bypass dispatch lifecycle
- invent truth by parsing terminal text alone

## Notifications

Notification delivery belongs here, not in the runtime core.

The runtime emits:
- `dispatch_failed`
- `worker_blocked`
- `needs_input`
- `verification_failed`
- `session_end`

Hook handlers decide whether to send:
- Discord
- Telegram
- Slack
- webhooks

## Task-Size Gate

One concept worth keeping is the heavy-orchestration gate.

Not every request should trigger multi-worker orchestration.

The hook layer may classify tasks as:
- `small`
- `medium`
- `large`

And should support lightweight escape hatches such as:
- `quick:`
- `simple:`
- `minor:`

This keeps the system responsive for trivial work without removing orchestration for large work.

## Guidance Surface

The runtime should also keep a unified guidance schema across:
- root AGENTS
- runtime overlays
- worker overlays
- worker protocol guidance

Conductor should keep the same idea, but thinner:
- one short root guidance surface
- one runtime overlay contract
- one worker protocol contract

Do not put runtime protocol details into the main AGENTS file.
