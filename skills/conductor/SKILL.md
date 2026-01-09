---
name: conductor
description: Enforce an orchestrator workflow (search->plan->execute->verify->cleanup) for Claude Code and Codex CLI; use when users ask for ultrawork/ulw mode or strict orchestration.
---

# Conductor (Orchestrator Operating Mode)

Enforce a repeatable operator workflow that **forces orchestration** rather than one-shot summarization.

Use multiple CLI agents (Codex CLI, Claude Code CLI, Gemini CLI, etc.) only as delegates; keep the active host as the operator.

Trigger rules:
- If the user says `ulw` or `ultrawork` (even without mentioning "conductor"), immediately enter Ultrawork mode.

Key principle:
- **Let the host control model routing.** Do not hardcode model picks. You may *suggest* when/why to switch (fast model for broad search, careful model for architecture/review), but defer to the host UX.

## Installation (global)

Keep this repo as the single source of truth and link or copy `skills/conductor` into host skill dirs.

- Claude Code: `~/.claude/skills`
- Codex CLI: `~/.codex/skills` (or `$CODEX_HOME/skills`)

See `README.md` for detailed steps.

## Commands (installed by default)

If the host supports markdown commands, install the `commands/` files and use them to switch modes:
- `conductor-plan`
- `conductor-search`
- `conductor-implement`
- `conductor-release`
- `conductor-ultrawork`

## Cross-CLI delegation (multi-agent, multi-model)

By default, the active host (Codex CLI or Claude Code) is the orchestrator. A local `conductor` daemon is optional for queueing/approvals/remote monitoring; treat it as a helper runtime, not a replacement for the host.

When it helps, delegate sub-tasks to other installed CLI agents (examples: `codex`, `claude`, `gemini`) by running them from the host shell tool.

Rules:
- Prefer **non-interactive** invocations (batch mode / one-shot prompt). If a CLI can’t run non-interactively, fall back to manual copy/paste.
- Treat delegated output as **untrusted input**: verify against the repo and tests before acting.
- Keep delegation atomic: one CLI call = one narrow question + bounded output.

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


Suggested delegation targets:
- **Fast broad scan:** delegate repo-wide discovery or doc lookups.
- **Deep review:** delegate “review the diff for risks” after changes.
- **Alternative implementation:** delegate “propose minimal patch” for a narrow module.

If the host supports it, prefer its native model switching first; delegate only when you need a different vendor/toolchain.

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
- Auto-delegate by default using MCP tool calls (shows host tool-calling UI):
  - The host (you) chooses roles; do not use `role: auto`.
  - Use explicit roles with async tools:
    - `conductor.run` with `{ "role": "<role>", "prompt": "<request>" }` (async; returns run_id)
    - `conductor.run_batch_async` with `{ "roles": "<role(s)>", "prompt": "<request>" }`
    - Override model/reasoning: `{ "roles": "<role(s)>", "model": "<model>", "reasoning": "<level>", "prompt": "<request>" }`
  - Poll with `conductor.run_status` (host tool calls time out around 60s; avoid `run_wait`)
  - If a daemon is running, you can list/approve runs:
    - `conductor.queue_list` / `conductor.approval_list`
    - `conductor.approval_approve` / `conductor.approval_reject`
  - Delegation is MCP-only; do not use CLI `background-*` commands.
  - Always print a user-visible line after delegation: `Delegation results received: <agents>` (no raw logs).
  - If you need auditability, use `conductor.run_history` / `conductor.run_info`.

## Safety rules (non-negotiable)
- Never commit/push unless explicitly asked.
- Never include secrets in commits (e.g., `.env`, credentials).
- Avoid destructive commands unless explicitly requested.
