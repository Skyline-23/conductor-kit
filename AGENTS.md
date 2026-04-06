# Conductor Kit - Agent Instructions

## Mission
Build a small, reliable orchestration kit for Codex-first workflows.

Keep the product centered on:
- lean skill prompts
- direct process-to-process coordination
- resumable task loops
- project-scoped memory

Avoid:
- giant instruction files
- tmux as a core dependency
- brittle automation layers

## Product Direction
- Treat `/Users/skyline23/Downloads/oh-my-codex` as a source of ideas, not code to mirror.
- Keep only the concepts that improve reliability:
  - orchestration
  - resumable review loops
  - cached project memory
  - explicit role routing
- Rebuild the implementation from scratch inside this repo.

## Scope
- One core skill at `skills/conductor/SKILL.md`
- Shared markdown commands under `commands/`
- One small Rust helper CLI under `src/main.rs`
- One JSON config under `config/conductor.json`
- Root docs that explain the model clearly

## Constraints
- Default to ASCII in new files.
- Keep files short and specific.
- Prefer direct IPC or stdio-based bridges over terminal multiplexing.
- Keep tmux optional and external, never required.
- Do not reintroduce large skill trees or nested doc systems without a clear runtime need.

## Operating Notes
- Make small, surgical changes.
- Preserve `.git`; everything else can be rebuilt as needed.
- When a concept is borrowed from `oh-my-codex`, rewrite it in conductor terms.
- Summaries after edits must include exact paths.
