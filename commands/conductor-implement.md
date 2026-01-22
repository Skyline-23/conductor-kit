---
description: "Implement mode: make small, safe code changes."
argument-hint: "what to implement"
---

Respect disabled state. If Conductor is disabled, do not enable it automatically; inform the user and proceed without Conductor unless they explicitly request enabling.
If enabled, load conductor skill. Activate implement mode.

Minimal surgical edits. Verify with tests. Rollback when stuck.

Refer to skills/conductor/SKILL.md for detailed instructions.
