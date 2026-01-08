# Conductor Kit - Agent Instructions

## Mission
Build a global skills pack for Codex CLI and Claude Code, inspired by oh-my-opencode. Keep the pack skills-first, and provide a small Go helper CLI for install, background orchestration, and optional MCP tools.

## Scope (In)
- Keep a single core `conductor` skill under `skills/` as the source of truth.
- Provide root-level docs (e.g., `README.md`) that describe install and usage.
- Provide global install via link/copy into `~/.codex` and `~/.claude`.
- Provide shared markdown commands under `commands/` (Codex + Claude).
- Provide a Go helper CLI in `cmd/conductor` for install + background tasks + MCP server.
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
- `cmd/conductor/main.go` provides install + background tasks + MCP server.
- `config/conductor.json` defines role -> CLI/model mapping.

## Work Plan
1) Keep docs and skill instructions consistent with the Go helper CLI.
2) Validate Go CLI flags and JSON config behavior.
3) Ensure install flow supports link/copy, commands, bins, and config.
4) Update docs after behavior changes.

## Operating Notes
- Make small, surgical changes.
- Ask one blocking question at a time when requirements are unclear.
- Summarize changes with exact paths after edits.
