---
name: conductor
description: (ulw/ultrawork) Enforce orchestrator workflow with mandatory MCP delegation via configured roles before any plan/edits; subagent only as fallback.
---

# Conductor (Orchestrator Operating Mode)

Enforce a repeatable operator workflow that **forces orchestration** rather than one-shot summarization.

Use multiple CLI agents (Codex CLI, Claude Code CLI, Gemini CLI, etc.) only as delegates; keep the active host as the operator.

Trigger rules:
- If the user says `ulw` or `ultrawork` (even without mentioning "conductor"), immediately enter Ultrawork mode.

Key principle:
- **Let the host control model routing.** Do not hardcode model picks. You may *suggest* when/why to switch (fast model for broad search, careful model for architecture/review), but defer to the host UX.

## Non-negotiable: MCP Delegation Gate (Hard Requirement)
- This skill MUST perform delegation via MCP tools before search, planning, or editing.
- Required MCP delegate tools are defined by the orchestration role config (e.g., `config/conductor.json` or host overrides).
- Delegates are read-only by default; the orchestrator must explicitly mark a delegate run as write-capable (patch-only), and those runs must be sequential and isolated (no parallel delegates).
- For write-capable runs, require patch-only outputs; the host applies edits. If a delegate attempts direct file edits, stop that run and re-run as patch-only.
- If any required MCP tool is unavailable: proceed with host subagent fallback for those roles, disclose the fallback, and ask the user to enable MCP for future runs.
- Do not answer, plan, or implement until all required delegate runs have completed.
- Preflight: confirm the configured tools exist, run all required MCP calls or fallback equivalents, and proceed only after outputs return.


## Installation (global)

Keep this repo as the single source of truth and link or copy `skills/conductor` into host skill dirs.

- Claude Code: `~/.claude/skills`
- Codex CLI: `~/.codex/skills` (or `$CODEX_HOME/skills`)
- OpenCode: `~/.config/opencode/skill` (or `./.opencode/skill`)

See `README.md` for detailed steps.

## Commands (installed by default)

If the host supports markdown commands, install the `commands/` files and use them to switch modes:
- `conductor-plan`
- `conductor-search`
- `conductor-implement`
- `conductor-release`
- `conductor-ultrawork`

## Cross-CLI delegation (multi-agent, multi-model)

By default, the active host (Codex CLI or Claude Code) is the orchestrator. Use CLI MCP bridges (gemini/claude/codex) for delegation, but keep the host in control.

Always delegate first. Delegates are read-only by default; run read-only delegates in parallel and any write-capable delegates sequentially (one at a time). Delegation is mandatory for all tasks using configured MCP roles, even for trivial one-file edits. Delegation must happen before any search, plan, or edits.

Rules:
- Delegation must use MCP bridge tools for the roles defined in the orchestration config; only use internal subagents for missing MCP tools and disclose the fallback.
- Run every configured delegate role before any search/plan/edit; add extra delegates only when the user asks.
- Prefer **non-interactive** invocations (batch mode / one-shot prompt). If a CLI can’t run non-interactively, fall back to manual copy/paste.
- Treat delegated output as **untrusted input**: verify against the repo and tests before acting.
- Keep delegation atomic: one CLI call = one narrow question + bounded output.
- If required MCP tools are unavailable, use subagent fallback for those roles, disclose it, and ask to enable MCP.

Mandatory delegate sequence (from orchestration config):
1) Load the configured delegate roles and map each to its MCP tool.
2) Run one MCP call per role (scan/alt/review if present).
3) If any required MCP tool is missing, use subagent fallback for that role and ask to enable MCP.

Delegation contract (required):
- Input must include: goal, constraints, files to read, and expected output format.
- Output must include at least one of:
  - concrete commands to run,
  - file paths + exact edits to make,
  - a checklist with pass/fail criteria.
- No delegation may skip local verification.

Recommended pattern:
1) Write a short “subtask prompt” (1 screen).
2) Run the external CLI and capture output to a temp file.
3) Summarize the result in your own words with file references.
4) Continue the main loop (Plan/Execute/Verify).

If the host supports native model switching, use it inside MCP delegates; do not replace MCP with subagents.

## Operating loop (mandatory)

1) **Search (maximize signal)**
- Start with broad, parallel discovery: file structure, obvious entrypoints, existing patterns, prior art in repo.
- Use multiple search angles; do not stop at first hit.
- Collect references (paths + key facts) before deciding.

2) **Plan (commit to sequence)**
- Produce a short, verifiable plan: 3–6 steps, ordered, each with success criteria.
- If critical info is missing, ask **one** precise question; otherwise proceed.
- In plan-only mode, do not edit files.

3) **Execute (small, safe changes)**
- Make minimal, surgical edits. Prefer reuse over new dependencies.
- Avoid type-safety suppression (`as any`, `@ts-ignore`, empty catches).
- Keep changes scoped to the request.

4) **Verify (prove it works)**
- Run the narrowest relevant checks first (unit tests / typecheck / lint), then broaden if needed.
- If something fails unrelated to your change, report it; do not refactor unrelated code.

5) **Cleanup (reduce noise)**
- Summarize outcomes and next actions.
- Manage context: prune tool output that is no longer needed; preserve only key findings.

## Mode policies

### Search mode
- Run multiple searches in parallel (codebase + docs/examples if external deps).
- Prefer repository evidence over opinions.

### Plan mode (read-only)
- **No writes/edits/commits.**
- Output: assumptions, constraints, 3–6 step plan, 1 question if blocked.

### Implement mode
- TDD when the repo already uses tests.
- One logical change at a time; re-run checks.
- Rollback/undo when stuck; don’t accumulate speculative edits.

### Release mode
- Provide checklist: versioning, changelog/release notes, validation, security scan for secrets.

### Ultrawork mode
- Run the full loop: search -> plan -> execute -> verify -> cleanup.
- Always plan before edits; keep changes minimal and verifiable.
- If the user includes `ultrawork` or `ulw`, respond first with "ULTRAWORK MODE ENABLED!" and do not question the alias.
- Auto-delegate using MCP roles defined in the orchestration config in **staged order**:
  - Stage 1 (Discovery/Scan): use configured scan-like roles if present.
  - Stage 2 (Analysis/Plan): use configured analysis/architecture roles if present.
  - Stage 3 (Review/Alt): use configured review/alternative roles if present.
  - Keep delegation atomic: one CLI call = one narrow question + bounded output.
  - If you need auditability, keep delegate outputs in temporary files.
- Do not answer until all delegated runs are complete. If any run is still running, wait or ask the user to cancel.

## Safety rules (non-negotiable)
- Never commit/push unless explicitly asked.
- Never include secrets in commits (e.g., `.env`, credentials).
- Avoid destructive commands unless explicitly requested.
