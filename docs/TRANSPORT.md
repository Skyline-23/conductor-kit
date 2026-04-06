# Transport

## Goal

Transport should move data with low overhead.

Transport should not define runtime truth.

## Preferred Order

V1 preference:
1. `stdio`
2. `unix_socket`
3. `tcp`

`tmux` is not a runtime transport.

## Transport Responsibilities

Transport is responsible for:
- process spawn
- stdin/stdout exchange
- mailbox delivery transport
- liveness detection

Transport is not responsible for:
- authority
- claim safety
- phase transitions
- dispatch truth

## V1 Choice

V1 should implement:
- spawned child processes
- stdio request/response streams
- file-backed mailbox and event log

This gives a low-overhead baseline without introducing a daemon too early.

## Failure Handling

Transport failure should map into explicit runtime events:
- `worker_spawn_failed`
- `worker_stdout_stale`
- `worker_heartbeat_stale`
- `dispatch_failed`

Failures should not be inferred indirectly from terminal UI.

## Recovery

If transport fails:
- task state remains durable
- claims remain durable
- dispatch state remains durable
- snapshots continue to reflect failure truth

The orchestrator then decides whether to retry, requeue, or fail the run.
