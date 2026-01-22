---
description: "Refactor mode: improve code structure without changing behavior."
argument-hint: "what to refactor"
---

Respect disabled state. If Conductor is disabled, do not enable it automatically; inform the user and proceed without Conductor unless they explicitly request enabling.
If enabled, load conductor skill. Activate refactor mode.

Extract patterns, remove duplication, improve naming. Verify behavior unchanged.

Refer to skills/conductor/SKILL.md for detailed instructions.
