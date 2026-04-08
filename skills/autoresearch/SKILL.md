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

Run the setup first, then keep using `step` until the metric stops improving.

## Execution Protocol

1. Establish the experiment contract with:
   - `conductor autoresearch setup --goal "<goal>" --metric-command "<command>" --metric-regex "<regex>" --direction lower|higher --in-scope <path>`
2. Confirm the baseline with:
   - `conductor autoresearch summary`
3. For each experiment attempt:
   - make one focused code change inside the allowed scope
   - run `conductor autoresearch step "<short description>"`
4. Periodically inspect the loop state with:
   - `conductor autoresearch summary`

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
