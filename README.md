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
- `commands/`
  Shared markdown commands for plan, implement, review, and symphony mode.
- `config/conductor.json`
  Runtime behavior and worker defaults.
- `src/main.rs`
  Small Rust helper for status, doctor, config discovery, and future runtime work.
- `docs/ARCHITECTURE.md`
  The new system model.
- `docs/BLUEPRINT.md`
  The full v2 redesign that replaces the broken OMX operating model.

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
- `conductor`
- `conductor init`
- `conductor resume`
- `conductor team`
- `conductor ralph`
- `conductor attach`

`conductor` and `conductor init` bring up the lead Codex session by default.
`conductor team` requires an explicit team size and agent profile list from
`config/conductor.json`, for example `conductor team 4 worker verifier`, and
expands to the OMX-style split view with the lead pane on the left and the HUD
plus sub-workers stacked on the right.

Operator commands:
- `conductor help`
- `conductor status`
- `conductor doctor`
- `conductor config-path`
- `conductor hud-view`
- `conductor hud-watch`
- `conductor worker-log`

The default worker presets now include:
- `orchestrator`
- `worker`
- `gemini_worker`
- `verifier`
- `claude_worker`

The shipped presets are aimed at real local interactive CLIs:
- `orchestrator`, `worker`, and `verifier` use the user's existing `codex` settings
- `gemini_worker` uses interactive `gemini`
- `claude_worker` uses interactive `claude`

If you want Claude to handle both worker and verifier paths, use:
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
