---
name: team
description: Expand the current conductor run into an explicit multi-worker team.
---

# Team

Use `$team` when the current task needs parallel orchestration.

Run:
- `conductor team <count> <profile> [profile...]`
- `conductor team <count> <profile> [profile...] --prompt "<current task>"`

Rules:
- do not call built-in sub-agent or delegation tools
- do not spawn agents directly from the host model
- treat `$team` as a thin command shortcut, not an orchestration plan
- keep the current surface session as the operator pane
- require an explicit team width
- require configured profile names from conductor settings
- distribute the requested profiles across the requested width
- keep work visible through tmux panes and the conductor HUD
- pass the current task objective through `--prompt` whenever it is available

Examples:
- `conductor team 4 explore build review verify`
- `conductor team 6 explore explore build build review verify`
- `conductor team 4 explore build review verify --prompt "inspect the repository and find the likely bug surface"`
