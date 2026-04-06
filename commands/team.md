# Team

Use this command when the current task needs parallel orchestration.

What it should do:
- call `conductor team ...` directly and immediately
- pass the current task summary with `--prompt "..."` when available
- not use model-native sub-agents or delegation
- not inspect code, read files, or explain the plan before running the command
- keep the current Codex session as the operator surface
- ensure a `conductor` run exists for the current project
- infer the team size and profile mix from the current task when the operator does not provide an explicit shape
- open the tmux ops layout if it is not already open
- mix the requested agent profiles round-robin across the requested team width
- keep state visible through the HUD and worker panes around the main surface

Default shape:
- `conductor team`
- `conductor team --prompt "inspect the repository and find the likely bug surface"`
- `conductor team 4 explore build review verify`
- `conductor team 4 explore build review verify --prompt "inspect the repository and find the likely bug surface"`

Optional:
- `conductor team <count> <profile> [profile...]`
- `conductor team <run_id> <count> <profile> [profile...]`
