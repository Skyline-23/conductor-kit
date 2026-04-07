---
name: ralph
description: Run the resumable Ralph-style conductor loop for larger work.
---

# Ralph

Use `$ralph` when the task needs the wider resumable orchestration loop.

## Role & Intent

`$ralph` switches the current conductor run into the wider resumable orchestration loop.
Use it when the task needs multi-step convergence, not just one fan-out.

## Operating Principles

- initialize or resume the current conductor run first
- use the wider loop only when the task is not trivial
- keep the current pane in the operator lane
- prefer visible worker progress and explicit evidence over implicit completion claims

## Execution Protocol

1. Run one of:
   - `conductor ralph`
   - `conductor ralph <run_id>`
   - `conductor ralph <run_id> <worker_count>`
2. Let conductor widen the team only as much as the task justifies.
3. Keep the operator lane focused on coordination, rerouting, and convergence.
4. Wait for worker reports, then decide whether to continue, verify, or shut down.

## Constraints & Safety

- do not call built-in sub-agent or delegation tools
- do not spawn agents directly from the host model
- do not treat `$ralph` as permission to recurse into ad hoc delegation
- do not close the loop until one verified outcome is ready

## Verification & Completion

- keep progress visible through tmux panes and the conductor HUD
- require worker evidence before declaring a branch complete
- converge back to one verified outcome before closing

## Recovery & Lifecycle

- if the loop stalls, inspect worker reports and reassign or shut down cleanly
- if the team shape is no longer justified, collapse back toward the main surface
- if the run must continue later, rely on conductor resume rather than redoing the same branch work
