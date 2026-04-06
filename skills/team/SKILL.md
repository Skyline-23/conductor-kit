---
name: team
description: Expand the current conductor run into an explicit multi-worker team.
---

# Team

Use `$team` when the current task needs parallel orchestration.

Run:
- `conductor team <count> <profile> [profile...]`

Rules:
- keep the current surface session as the operator pane
- require an explicit team width
- require configured profile names from conductor settings
- distribute the requested profiles across the requested width
- keep work visible through tmux panes and the conductor HUD

Examples:
- `conductor team 4 explore build review verify`
- `conductor team 6 explore explore build build review verify`
