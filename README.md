# conductor-kit

`conductor-kit` is a clean restart built around the parts of
`/Users/skyline23/Downloads/oh-my-codex` that are actually worth keeping.

Those concepts are:
- orchestrator
- worker
- loop
- state
- memory
- resume

Everything else is intentionally stripped away. No giant prompt hierarchy. No tmux-first runtime. No extra role taxonomy layered on top.

## Principles

1. Keep the runtime vocabulary aligned with `oh-my-codex`.
2. Prefer direct process communication over terminal injection.
3. Make runtime state explicit and resumable.
4. Cache project context, but invalidate it on repository changes.
5. Keep the host-facing skill short enough to be usable.

## Initial Layout

- `skills/conductor/SKILL.md`
  Lean orchestration instructions for the host agent.
- `skills/team/SKILL.md`
  Expands the current run into a configured multi-worker team.
- `skills/ralph/SKILL.md`
  Starts the wider resumable orchestration loop.
- `skills/autoresearch/SKILL.md`
  Runs a lightweight experiment loop on top of the conductor surface.
- `skills/plan/SKILL.md`, `skills/implement/SKILL.md`,
  `skills/review/SKILL.md`, `skills/symphony/SKILL.md`
  Thin skill shims for the shared command surface.
- `commands/`
  Shared markdown commands for plan, implement, review, and symphony mode.
- `config/conductor.json`
  Runtime behavior and worker defaults.
- `src/main.rs`
  Small Rust helper for status, doctor, config discovery, and future runtime work.
- `docs/ARCHITECTURE.md`
  The new system model.
- `docs/BLUEPRINT.md`
  The full v2 redesign of the runtime and operator surface.

## Runtime Model

The runtime is built around direct transport:
- `stdio`
- `unix_socket`
- `tcp`

`tmux` is not part of the core runtime model.

The loop is:

1. discover
2. spawn
3. converge
4. verify
5. continue or close

The runtime nouns are:
- orchestrator
- worker
- loop
- state
- memory
- resume

## Configuration

The default config lives at `config/conductor.json`.

Worker adapters can now describe launch semantics per worker type:
- `delivery_mode`
- `launch_mode`
- `base_args`
- `env`

Supported launch modes:
- `stdin_json`
- `stdin_text`
- `argv_prompt`
- `argv_json`

Supported delivery modes:
- `session`

User overrides are expected at:
- `~/.conductor-kit/conductor.json`
- nearest `./.conductor-kit/conductor.json`

When running inside this repository, the CLI also falls back to `./config/conductor.json`.

The primary surface is:
- `conductor install`
- `conductor uninstall`
- `conductor`
- `conductor init`
- `conductor resume`
- `conductor team`
- `conductor ralph`
- `conductor autoresearch`
- `conductor attach`

`conductor` and `conductor init` bring up the primary surface session by default.
`conductor team` requires an explicit team size and agent profile list from
`config/conductor.json`, for example `conductor team 4 explore build review verify`, and
expands to the split view with the main pane on the left and the HUD
plus sub-workers stacked on the right.

Use `conductor settings` to edit each profile's `cli`, `model`, `reasoning`,
and description from the terminal.

Installed skill shortcuts:
- `$team`
- `$ralph`
- `$autoresearch`
- `$plan`
- `$implement`
- `$review`
- `$symphony`

Operator commands:
- `conductor install`
- `conductor uninstall`
- `conductor help`
- `conductor status`
- `conductor doctor`
- `conductor config-path`
- `conductor hud-view`
- `conductor hud-watch`
- `conductor worker-log`
- `conductor autoresearch`
- `conductor autoresearch continue "try a smaller code change"`
- `conductor autoresearch status`
- `conductor autoresearch stop`

Run `conductor install` once to link the managed skill shims into your active Codex home,
install the managed `~/.codex/hooks.json` entries, and enable `features.codex_hooks = true`
in `~/.codex/config.toml`.
Use `conductor uninstall` to remove the conductor-managed skill shims and only the
conductor-managed Codex hook commands from that Codex home.

The managed Codex hook bundle uses the official native `SessionStart` and `Stop`
events only. Conductor does not re-inject Ralph or autoresearch prompts through
tmux while a session is running.

The default team profiles now include:
- `explore`
- `build`
- `review`
- `verify`

The shipped defaults use:
- `surface` -> user-selected CLI with no forced model override
- `explore` -> `codex` + `gpt-5.3-codex-spark`
- `build` -> `codex` + `gpt-5.4-mini`
- `review` -> `codex` + `gpt-5.4`
- `verify` -> `codex` + `gpt-5.4-mini`

If you want Claude-oriented defaults, use:
- `CONDUCTOR_CONFIG=config/conductor.claude.json`

## Build

```bash
cargo build
```

## Status

This repository is now a fresh baseline. The next implementation work should focus on:
- project memory persistence
- worker session registry
- resumable state ledger
- thin MCP bridge adapters

The current source-of-truth design document is:
- `docs/BLUEPRINT.md`
