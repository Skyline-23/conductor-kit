---
name: ralph
description: Run the resumable Ralph-style conductor loop for larger work.
---

# Ralph

Use `$ralph` when the task needs the wider resumable orchestration loop.

Run:
- `conductor ralph`
- `conductor ralph <run_id>`
- `conductor ralph <run_id> <worker_count>`

Rules:
- do not call built-in sub-agent or delegation tools
- do not spawn agents directly from the host model
- initialize or resume the current conductor run first
- expand to a wider team only when the task is not trivial
- keep progress visible through tmux panes and the conductor HUD
- converge back to one verified outcome before closing
