---
name: ralph
description: Run the resumable Ralph-style conductor loop for larger work.
---

# Ralph

Use `$ralph` when the task needs the wider resumable orchestration loop.

## Role & Intent

`$ralph` switches the current conductor run into the persistent resumable orchestration loop.
Use it when the task should keep iterating until one verified outcome is accepted or the run is explicitly cancelled.

## Operating Principles

- initialize or resume the current conductor run first
- keep Ralph persistence-first; do not stop at partial completion
- keep the current pane in the operator lane
- do not widen into a team unless the user explicitly asked for workers
- prefer visible worker progress and explicit evidence over implicit completion claims

## Execution Protocol

1. Run one of:
   - `conductor ralph`
   - `conductor ralph <run_id>`
   - `conductor ralph <run_id> <worker_count>`
2. Treat `conductor ralph` and `conductor ralph <run_id>` as surface-only operator loops.
3. Only use `conductor ralph <run_id> <worker_count>` when you explicitly want Ralph to widen into a team.
4. Expect Ralph to immediately re-enter the operator lane with the current focus, reason, and next command.
5. Keep the operator lane focused on coordination, rerouting, verification, and closure.
6. Continue iterating until one verified outcome is accepted or the run is explicitly cancelled.

## Constraints & Safety

- do not call built-in sub-agent or delegation tools
- do not spawn agents directly from the host model
- do not treat `$ralph` as permission to recurse into ad hoc delegation
- do not widen into a team unless the worker count was explicitly requested
- do not close the loop until one verified outcome is ready

## Verification & Completion

- keep progress visible through tmux panes and the conductor HUD
- require worker evidence before declaring a branch complete
- converge back to one verified outcome before closing
- if verification rejects completion, keep Ralph running and re-enter fix/verify rather than exiting

## Recovery & Lifecycle

- if the loop stalls, inspect worker reports and reassign or shut down cleanly
- if the team shape is no longer justified, collapse back toward the main surface
- if the run must continue later, rely on conductor resume rather than redoing the same branch work
- if the loop is not done, keep iterating instead of treating partial progress as completion
