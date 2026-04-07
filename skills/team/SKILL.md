---
name: team
description: Expand the current conductor run into an explicit multi-worker team.
---

# Team

Use `$team` when the current task needs parallel orchestration.

## Role & Intent

`$team` expands the current conductor run into an explicit worker team.
The current pane stays in the operator lane.
Workers do the lane work. The operator coordinates, waits for reports, and converges results.

## Operating Principles

- run the matching `conductor team ...` command first
- do not inspect the repo, reason about layout, or explain the command before running it
- after the command returns, stop doing worker-lane work in the current turn
- keep the current surface session as the operator pane
- infer the team width and lane mix from the task when the operator does not provide one
- prefer visible tmux worker lanes and conductor HUD state over silent background delegation

## Execution Protocol

1. Run one of:
   - `conductor team`
   - `conductor team --prompt "<current task>"`
   - `conductor team <count> <profile> [profile...]`
   - `conductor team <count> <profile> [profile...] --prompt "<current task>"`
2. Let conductor expand the current tmux surface into worker lanes.
3. Stay in the operator lane after the team is up.
4. Wait for worker reports and use them to coordinate, reassign, or conclude.
5. If the team shape was inferred, accept it unless there is a clear reason to override it.

## Constraints & Safety

- do not call built-in sub-agent or delegation tools
- do not spawn agents directly from the host model
- do not replace the command with a prose explanation
- do not keep exploring, building, or reviewing in the main pane as if no team exists
- only require explicit profile names when the operator overrides the inferred team shape
- only use configured profile names from conductor settings for explicit overrides

## Verification & Completion

- keep work visible through tmux panes and the conductor HUD
- wait for worker reports instead of duplicating their lane work
- converge findings back into one operator summary before closing
- if completion depends on evidence, ask the worker lanes to report that evidence upward

## Recovery & Lifecycle

- if worker reports stall, stay in the operator lane and nudge or reshape the team
- if the task changes, rerun `conductor team ... --prompt "..."` with the updated objective
- if the team is no longer justified, collapse back to the main surface and continue solo

## Examples

- `conductor team 4 explore build review verify`
- `conductor team 6 explore explore build build review verify`
- `conductor team 4 explore build review verify --prompt "inspect the repository and find the likely bug surface"`
