# Team

Use this command when the current task needs OMX-style parallel orchestration.

What it should do:
- keep the current Codex session as the operator surface
- ensure a `conductor` run exists for the current project
- require an explicit team size and agent profile list from `config/conductor.json`
- open the tmux ops layout if it is not already open
- mix the requested agent profiles round-robin across the requested team width
- keep state visible through the HUD and worker panes

Default shape:
- `conductor team 4 worker verifier`

Optional:
- `conductor team <count> <profile> [profile...]`
- `conductor team <run_id> <count> <profile> [profile...]`
