# Conductor Kit - Agent Instructions

## Mission
Build a global skills pack for Codex CLI and Claude Code, inspired by oh-my-opencode. Keep the pack skills-first, and provide a small Go helper CLI for install, MCP orchestration (sync/async), and optional MCP tool bundles.

## Scope (In)
- Keep a single core `conductor` skill under `skills/` as the source of truth.
- Provide root-level docs (e.g., `README.md`) that describe install and usage.
- Provide global install via link/copy into `~/.codex` and `~/.claude`.
- Provide shared markdown commands under `commands/` (Codex + Claude).
- Provide a Go helper CLI in `cmd/conductor` for install + MCP server + delegation tools.
- Provide role/model routing config in `config/conductor.json`.

## Out of Scope
- Separate external runner repo or daemon.
- Auto-installing or authenticating 3rd-party CLIs.
- Hardcoding model selection; defer to host UX and per-role config.

## Constraints
- Support global install via copy or symlink into host dirs.
- Preserve host-provided model routing; skill may suggest when/why to switch.
- Keep skills lean: `SKILL.md` + only required `scripts/`, `references/`, `assets/`.
- Do not add README/INSTALL docs inside skill folders.
- Default to ASCII in new files.

## Current State
- `skills/conductor/SKILL.md` exists.
- `commands/` contains mode-switch commands (plan/search/implement/release/ultrawork).
- `cmd/conductor` provides install, diagnostics, and MCP tools (`run`, `run_batch`, history).
- `config/conductor.json` defines role -> CLI/model mapping with defaults.

## Work Plan
1) Add async MCP tools with status/wait/cancel + basic notifications via polling.
2) Provide optional MCP bundle templates and host-specific setup helpers.
3) Add project-local overrides (config discovery + optional local install).
4) Keep docs and skill instructions consistent with the Go helper CLI.
5) Validate Go CLI flags and JSON config behavior.

## Operating Notes
- Make small, surgical changes.
- Ask one blocking question at a time when requirements are unclear.
- Summarize changes with exact paths after edits.
