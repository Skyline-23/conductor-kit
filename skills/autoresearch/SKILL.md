---
name: autoresearch
description: Run the lightweight autoresearch loop on top of conductor.
---

# Autoresearch

Use `$autoresearch` when the task needs repeated measurable experiments instead of a one-shot edit.

## Role & Intent

`$autoresearch` is a lightweight experiment loop, not a second orchestration stack.
Use it when you have:
- a clear optimization goal
- a concrete metric command
- a bounded edit scope

Use `$autoresearch` as a single-entry loop. If the current run is not initialized yet, it should guide setup. If it is already initialized, it should show the current experiment state and continue from there.

## Execution Protocol

1. Start with:
   - `conductor autoresearch`
2. If setup is still needed, establish the experiment contract with:
   - `conductor autoresearch setup --goal "<goal>" --metric-command "<command>" --metric-regex "<regex>" --direction lower|higher --in-scope <path>`
3. For each experiment attempt:
   - make one focused code change inside the allowed scope
   - run `conductor autoresearch continue "<short description>"`
4. Periodically inspect or pause the loop with:
   - `conductor autoresearch status`
   - `conductor autoresearch stop`

## Constraints & Safety

- do not use `$autoresearch` without a measurable metric
- do not modify files outside the declared in-scope paths
- do not keep regressing commits; the loop should discard them
- keep each experiment small enough to explain in one short sentence
- treat this as its own experiment loop, not as a team or Ralph alias

## Outputs

The loop should leave behind:
- a dedicated `feat/autoresearch-YYYYMMDD` branch
- `results.tsv` in the repo root
- `run.log` in the repo root
- one `autoresearch.json` state file under the current conductor run
