# Conductor Kit - Agent Instructions

## Mission
Build a global skills pack for Codex CLI and Claude Code, inspired by oh-my-opencode but not dependent on OpenCode. Keep the pack skills-first, and provide a small Go helper CLI for install, CLI MCP bridges, and optional MCP tool bundles.

## Scope (In)
- Keep a single core `conductor` skill under `skills/` as the source of truth.
- Provide root-level docs (e.g., `README.md`) that describe install and usage.
- Provide global install via link/copy into `~/.codex`, `~/.claude`, and `~/.config/opencode`.
- Provide shared markdown commands under `commands/` (Codex + Claude).
- Provide a Go helper CLI in `cmd/conductor` for install + CLI MCP bridge servers.
- Provide role/model routing config in `config/conductor.json`.

## Out of Scope
- Separate external runner repo. (Daemon components must live inside this repo and remain optional.)
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
- `commands/` contains mode-switch commands (plan/search/implement/release/symphony).
- `cmd/conductor` provides install/uninstall, settings/login helpers, diagnostics, and CLI MCP bridge servers.
- Release uses Homebrew cask (with uninstall hook) instead of formula.
- `config/conductor.json` defines role -> CLI/model mapping with CLI-native defaults (no provider prefix) and sage reasoning.

## Work Plan
1) Provide optional MCP bundle templates and host-specific setup helpers.
2) Add project-local overrides (config discovery + optional local install).
3) Expand CLI MCP bridge coverage and behavior.
4) Keep docs and skill instructions consistent with the Go helper CLI.
5) Validate Go CLI flags and JSON config behavior.
6) Keep model defaults aligned to CLI-native naming (no provider prefixes) and validate in doctor output.

## Operating Notes
- Make small, surgical changes.
- Ask one blocking question at a time when requirements are unclear.
- Summarize changes with exact paths after edits.
- Orchestration policy should follow oh-my-opencode patterns (parallel background fan-out, continuation via status checks, approval gating when required).
