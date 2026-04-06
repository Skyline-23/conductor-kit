# HUD

## Goal

The HUD is required.

But the HUD should be a subscriber, not a control surface.

It must render runtime state cheaply and accurately.

## Source Of Truth

The HUD must read:
- current runtime snapshot
- recent event stream
- optional project metadata

The HUD must not rely on:
- pane scraping
- prompt parsing
- terminal focus
- send-keys side effects

## Required HUD Views

### Global View

Show:
- active run id
- current phase
- authority owner
- worker count
- backlog counts
- verification status

### Worker View

For each worker show:
- worker id
- state
- current task id
- last update time
- blocked reason if any

### Dispatch View

Show:
- pending count
- notified count
- failed count
- recent mailbox activity

## Rendering Policy

HUD updates should be event-driven where possible.

Preferred model:
1. runtime emits events
2. HUD keeps a local projection
3. periodic snapshots repair drift

This is cheaper than polling many files or scraping terminals.

## Terminal Mapping

The user asked to know what each terminal is doing.

That should be represented as worker runtime metadata:
- `terminal_label`
- `worker_id`
- `state`
- `task_summary`

The terminal UI can map to those fields, but the runtime must not depend on a terminal existing.

## Presets

Start with three HUD densities:
- `minimal`
- `focused`
- `full`

The HUD contract should stay stable even if the visual format changes.

## Failure Mode

If HUD rendering fails:
- orchestration must continue
- state must remain correct
- snapshots must still be inspectable via CLI
