---
name: conductor
description: Lean orchestration for multi-step coding work.
---

# Conductor

Use conductor when the task is large enough to benefit from explicit orchestration.

Primary entrypoints:
- `conductor install`
- `conductor uninstall`
- `conductor`
- `conductor init`
- `conductor resume`
- `conductor team`
- `conductor ralph`
- `conductor autoresearch`

Default `conductor` usage should stay narrow. Start the primary surface session
first, then expand with `$team` or `$ralph` only when orchestration is needed.

`$team` should require an explicit team size and configured agent profiles, such
as `conductor team 4 explore build review verify`.

Command shortcuts:
- `$team`
- `$ralph`
- `$autoresearch`

## Intent

Conductor is for:
- decomposition
- worker orchestration
- resumable execution
- verification before trust

Conductor is not for:
- forcing orchestration on trivial edits
- replacing local inspection with ceremony
- hiding decisions behind process

## Default Loop

1. Discover the relevant files, state, and constraints.
2. Spawn or continue only the workers that are justified.
3. Converge findings into one working plan.
4. Execute the smallest correct change.
5. Verify with code, tests, or runtime output.

## Rules

- when `$team` or `$ralph` is invoked, call the matching `conductor` CLI command instead of using model-native sub-agents
- do not use built-in delegation as a substitute for conductor-managed workers
- Use workers because they reduce ambiguity, not because a rule says so.
- Prefer direct evidence from the repo over model speculation.
- If a worker result conflicts with the code, trust the code.
- Keep summaries short and operational.
- If the task is small, skip conductor and just do the work.

## Transport

Prefer direct process communication:
- stdio
- local sockets

Do not assume tmux exists.

## Outputs

When conductor is active, produce:
- a short current phase
- the next concrete action
- the verification result before closing
