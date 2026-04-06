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

User overrides are expected at:
- `~/.conductor-kit/conductor.json`
- nearest `./.conductor-kit/conductor.json`

The helper currently supports:
- `conductor help`
- `conductor status`
- `conductor doctor`
- `conductor config-path`

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
