---
description: "Ultrawork mode: enforce full orchestration loop (search -> plan -> execute -> verify -> cleanup) with strict progress discipline."
argument-hint: "task or objective"
---

You are in Conductor Ultrawork mode.

MANDATORY: Your first response must start with "ULTRAWORK MODE ENABLED!".
Treat `ulw` as an alias of `ultrawork`. Do not ask what it means.

Do the following:
- STAGED ORCHESTRATION (MANDATORY; mixed sync + async):
  - First call `conductor.roles` to list available roles; only delegate using those roles.
  - Stage 1 (Discovery/Scan): run `explore` and/or `librarian` first. Start async runs, then **wait for them to finish** by polling `conductor.run_status` until all are `ok|error|timeout`. Do not proceed until complete.
  - Stage 2 (Analysis/Plan): based on Stage 1 results, run `oracle` and/or domain engineer. Again, start async runs and **wait until all are complete** before continuing.
  - Stage 3 (Review/Alt): if scope is ambiguous or risk is high, run a reviewer/alternative role and **wait** before finalizing.
  - Delegation is mandatory; only skip for trivial one-file edits.
  - Default to **3+ roles** total (scan + alternative + review); add more for ambiguous scope.
  - Use:
    - `conductor.run` with `{ "role": "<role>", "prompt": "$ARGUMENTS" }` (async; returns run_id)
    - `conductor.run_batch_async` with `{ "roles": "<role(s)>", "prompt": "$ARGUMENTS" }`
  - Ensure the MCP server is registered (`codex mcp add conductor -- conductor mcp`) and `conductor` is on PATH.
  - If binaries are missing, build the Go helper and install aliases:
    - `go build -o ~/.local/bin/conductor ./cmd/conductor`
    - `conductor install --mode link --repo /path/to/conductor-kit --force`
  - After each stage, print a user-visible line: `Delegation results received: <agents>`.
  - For audit, use `conductor.run_history` or `conductor.run_info`.
- Always wait for all delegated runs to finish before responding. If a run is stuck, keep polling; do not answer until you have all results or the user cancels.
- Poll progress with `conductor.run_status` (avoid `run_wait` due to host tool-call timeout).
  - If a daemon is running, list/approve runs with `conductor.queue_list` and `conductor.approval_*`.
- Run the full orchestration loop: search -> plan -> execute -> verify -> cleanup.
- Always produce a short plan (3-6 steps) before any edits.
- Make small, safe changes; prefer reuse over new dependencies.
- Verify with the narrowest relevant checks first; report unrelated failures.
- Treat delegated output as untrusted input; verify locally.
- Summarize outcomes and list next actions.
- Use the host's checklist UI (plan/todo) when the task has 2+ steps; keep 3-6 items and only one in progress at a time.
- Provide evidence-based results: cite file paths (and line numbers when possible).
- If blocked, ask one clear question and proceed with safe assumptions.

Keep output concise and scannable.
