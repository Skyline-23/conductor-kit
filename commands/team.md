# Team

Use this command when the current task needs OMX-style parallel orchestration.

What it should do:
- keep the current Codex session as the operator surface
- ensure a `conductor` run exists for the current project
- open the tmux ops layout if it is not already open
- prefer spawning additional Codex workers before bringing in optional workers
- keep state visible through the HUD and worker panes

Default shape:
- `conductor team`

Optional:
- `conductor team <run_id>`
- `conductor team <run_id> <worker_count>`
