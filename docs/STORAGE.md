# Storage

## Goal

Storage layout should be fixed before implementation so runtime code does not have to guess.

## Root Layout

Initial root layout:

```text
.conductor/
  runs/
    <run_id>/
      run.json
      snapshot.json
      events.jsonl
      workers/
        <worker_id>.json
      tasks/
        task-<id>.json
      dispatch/
        <request_id>.json
      mailbox/
        <worker_id>.json
      memory/
        project-memory.json
        notes.jsonl
  sessions/
    <session_id>/
      active-run.txt
```

## Scope Policy

Two scopes are allowed:
- root compatibility scope
- session-scoped authoritative scope

Session-scoped storage wins when present.

## File Ownership

Ownership by file family:
- `run.json`: runtime control plane
- `snapshot.json`: snapshot builder
- `events.jsonl`: event writer
- `workers/*.json`: worker registry/state
- `tasks/*.json`: task store
- `dispatch/*.json`: dispatch store
- `mailbox/*.json`: mailbox store
- `memory/project-memory.json`: structured memory
- `memory/notes.jsonl`: operator notes

## Atomic Writes

All state files should be written atomically:
1. write temp file
2. fsync if needed later
3. rename into place

## Locking

V1 locking policy:
- one lock per path family
- never hold a lock across process spawn
- dispatch and mailbox writes should be serialized independently

## Event Log

`events.jsonl` should be append-only.

Each line is a complete event envelope.

## Notes Split

Structured project memory and operator notes should stay separate:
- `project-memory.json`
- `notes.jsonl`

This preserves the OMX concept without forcing all memory into one blob.
