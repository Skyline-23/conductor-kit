# Recovery

## Goal

Recovery should be explicit, durable, and predictable.

The runtime must recover from process death, partial delivery, and stale authority without inventing missing history.

## Failure Classes

Initial failure classes:
- worker process crash
- orchestrator crash
- partial dispatch delivery
- stale claim
- stale authority
- snapshot drift
- event append failure

## Recovery Sources

Recovery may use:
- durable state files
- durable event log
- latest snapshot

Recovery may not rely on:
- terminal text
- pane contents
- unstructured prompt history

## Recovery Order

1. restore authoritative scope
2. restore run state
3. restore authority state
4. restore worker registry
5. restore task and claim state
6. restore dispatch and mailbox state
7. rebuild snapshot

## Replay Policy

Replay should be bounded:
- use event log to reconstruct when needed
- prefer snapshot + forward replay over full replay
- reject replay when schema versions are incompatible

## Operator Visibility

Recovery must emit events and snapshot fields that explain:
- what failed
- what was restored
- what remains unresolved
